//! Lifecycle-owned post-commit enrollment dispatcher.
//!
//! Database transactions synchronously enqueue an authorized command from
//! `after_commit`. The receiver remains alive through the writer's first
//! shutdown flush, then drains every command and tracked device task.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use libsql::params;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::devices::models::DeviceWithPlaintext;
use crate::state::AppState;

use super::pusher;
use super::service::{self, DeviceOperationState};

/// Reconcile every durable device-operation checkpoint before any producer or
/// HTTP route can issue a new Hikvision call. `prepared` is always ambiguous
/// after restart and is therefore made manual; only `device_applied` permits a
/// DB-only completion.
pub async fn recover_startup_checkpoints(state: &AppState) -> anyhow::Result<()> {
    let conn = state.db.connect()?;
    let mut rows = conn
        .query(
            "SELECT operation_key, operation, state \
             FROM device_operation_checkpoints ORDER BY operation_key",
            (),
        )
        .await?;
    let mut checkpoints = Vec::new();
    while let Some(row) = rows.next().await? {
        checkpoints.push((
            row.get::<String>(0)?,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
        ));
    }
    drop(rows);
    drop(conn);

    for (operation_key, operation, checkpoint_state) in checkpoints {
        let state_value = match checkpoint_state.as_str() {
            "prepared" => DeviceOperationState::Prepared,
            "device_applied" => DeviceOperationState::DeviceApplied,
            "manual" => DeviceOperationState::Manual,
            _ => anyhow::bail!("invalid checkpoint state for {operation_key}"),
        };
        match operation.as_str() {
            "enrollment_push" => {
                let push_id = operation_key
                    .strip_prefix("enrollment:")
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("invalid enrollment checkpoint key"))?;
                let conn = state.db.connect()?;
                let mut rows = conn
                    .query(
                        "SELECT p.device_id, enr.employee_id, emp.face_id, p.enrollment_id \
                         FROM enrollment_device_pushes p \
                         JOIN enrollments enr ON enr.id=p.enrollment_id \
                         JOIN employees emp ON emp.id=enr.employee_id WHERE p.id=?1",
                        params![push_id],
                    )
                    .await?;
                let row = rows.next().await?.ok_or_else(|| {
                    anyhow::anyhow!("checkpoint references missing enrollment push")
                })?;
                let device_id: String = row.get(0)?;
                let employee_id: String = row.get(1)?;
                let face_id: Option<String> = row.get(2)?;
                let enrollment_id: String = row.get(3)?;
                let face_id = face_id.ok_or_else(|| {
                    anyhow::anyhow!("checkpoint enrollment employee has no face id")
                })?;
                drop(rows);
                drop(conn);
                if state_value == DeviceOperationState::DeviceApplied {
                    service::complete_recovered_push_success(
                        state,
                        &enrollment_id,
                        push_id,
                        &device_id,
                        &face_id,
                        &employee_id,
                    )
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                } else {
                    service::record_push_recovery_failure(
                        state,
                        push_id,
                        "Restart found an ambiguous device operation; manual reconciliation required",
                        true,
                    )
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                    let conn = state.db.connect()?;
                    let mut rows = conn
                        .query(
                            "SELECT enrollment_id FROM enrollment_device_pushes WHERE id=?1",
                            params![push_id],
                        )
                        .await?;
                    let enrollment_id: String = rows
                        .next()
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("push disappeared during recovery"))?
                        .get(0)?;
                    service::finalize_enrollment(state, &enrollment_id)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                }
            }
            "backfill_push" => {
                let suffix = operation_key
                    .strip_prefix("backfill:")
                    .ok_or_else(|| anyhow::anyhow!("invalid backfill checkpoint key"))?;
                let (device_id, face_id) = suffix
                    .split_once(':')
                    .filter(|(device, face)| !device.is_empty() && !face.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("invalid backfill checkpoint key"))?;
                let conn = state.db.connect()?;
                let mut rows = conn
                    .query(
                        "SELECT id FROM employees WHERE face_id=?1 AND status='active'",
                        params![face_id],
                    )
                    .await?;
                let employee_id: String = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("backfill checkpoint has no active employee"))?
                    .get(0)?;
                drop(rows);
                let mut device_rows = conn
                    .query("SELECT id FROM devices WHERE id=?1", params![device_id])
                    .await?;
                if device_rows.next().await?.is_none() {
                    anyhow::bail!("backfill checkpoint references missing device");
                }
                drop(device_rows);
                drop(conn);
                if state_value == DeviceOperationState::DeviceApplied {
                    service::complete_backfill_mapping(
                        state,
                        &operation_key,
                        device_id,
                        face_id,
                        &employee_id,
                    )
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                } else {
                    service::mark_device_operation(
                        state,
                        &operation_key,
                        DeviceOperationState::Manual,
                    )
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                }
            }
            "purge_delete" => {
                let mapping_id = operation_key
                    .strip_prefix("purge:")
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("invalid purge checkpoint key"))?;
                let conn = state.db.connect()?;
                let mut rows = conn
                    .query(
                        "SELECT id FROM device_face_mappings WHERE id=?1",
                        params![mapping_id],
                    )
                    .await?;
                if rows.next().await?.is_none() {
                    anyhow::bail!("purge checkpoint references missing mapping");
                }
                drop(rows);
                drop(conn);
                if state_value == DeviceOperationState::DeviceApplied {
                    service::complete_purge_mapping(state, &operation_key, mapping_id)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                } else {
                    service::mark_device_operation(
                        state,
                        &operation_key,
                        DeviceOperationState::Manual,
                    )
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                    service::mark_mapping_pending_delete_queued(state, mapping_id)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                }
            }
            _ => anyhow::bail!("unsupported checkpoint operation {operation}"),
        }
    }
    Ok(())
}

/// Unforgeable outside the enrollment module. Its presence means the
/// `prepared` checkpoint and push row were committed by the transaction that
/// produced this exact command; it must not pass through ambiguous-checkpoint
/// admission again.
pub(super) struct AuthorizedAttempt {
    _private: (),
}

impl AuthorizedAttempt {
    pub(super) fn committed() -> Self {
        Self { _private: () }
    }
}

pub(super) struct AuthorizedDispatchTarget {
    pub push_id: String,
    pub device: DeviceWithPlaintext,
    pub attempt: AuthorizedAttempt,
}

pub(super) struct AuthorizedDispatchCommand {
    pub enrollment_id: String,
    pub face_id: String,
    pub photo_bytes: Arc<Vec<u8>>,
    pub employee_id: String,
    pub employee_name: String,
    pub targets: Vec<AuthorizedDispatchTarget>,
}

enum Message {
    Dispatch(AuthorizedDispatchCommand),
    Close,
}

struct Inner {
    sender: mpsc::UnboundedSender<Message>,
    receiver: Mutex<Option<mpsc::UnboundedReceiver<Message>>>,
    accepting: AtomicBool,
}

#[derive(Clone)]
pub struct EnrollmentDispatcher {
    inner: Arc<Inner>,
}

impl EnrollmentDispatcher {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Inner {
                sender,
                receiver: Mutex::new(Some(receiver)),
                accepting: AtomicBool::new(true),
            }),
        }
    }

    pub async fn start(&self, state: AppState) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        let mut receiver = self.inner.receiver.lock().await;
        let mut receiver = receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("enrollment dispatcher already started"))?;
        Ok(tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                match message {
                    Message::Dispatch(command) => {
                        if let Err(error) =
                            pusher::spawn_authorized_enrollment_pushes(state.clone(), command).await
                        {
                            tracing::error!(%error, "authorized enrollment dispatch admission failed");
                        }
                    }
                    Message::Close => break,
                }
            }
            state.enrollment_tasks.stop_and_join().await?;
            Ok(())
        }))
    }

    pub(super) fn enqueue(&self, command: AuthorizedDispatchCommand) -> anyhow::Result<()> {
        if !self.inner.accepting.load(Ordering::Acquire) {
            anyhow::bail!("enrollment dispatcher admission is closed");
        }
        self.inner
            .sender
            .send(Message::Dispatch(command))
            .map_err(|_| anyhow::anyhow!("enrollment dispatcher receiver stopped"))
    }

    pub fn close(&self) -> anyhow::Result<()> {
        if self.inner.accepting.swap(false, Ordering::AcqRel) {
            self.inner
                .sender
                .send(Message::Close)
                .map_err(|_| anyhow::anyhow!("enrollment dispatcher receiver stopped"))?;
        }
        Ok(())
    }
}

impl Default for EnrollmentDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
