//! Enrollment fan-out pusher.
//!
//! `spawn_enrollment_pushes` admits a tracked task that drives N per-device
//! push tasks concurrently via JoinSet. The driver outlives the originating
//! HTTP request but remains owned by application shutdown.
//!
//! `push_one_device` is the reusable single-device push path shared by:
//!   - spawn_enrollment_pushes (fan-out, enrollment context)
//!   - retry_push handler (single retry)
//!   - BackfillWorker (per-employee push to a newly registered device)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::devices::models::DeviceWithPlaintext;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

use super::service;

#[derive(Clone)]
pub struct EnrollmentTaskTracker {
    tasks: Arc<Mutex<JoinSet<()>>>,
    accepting: Arc<AtomicBool>,
    shutdown: CancellationToken,
}

impl EnrollmentTaskTracker {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(JoinSet::new())),
            accepting: Arc::new(AtomicBool::new(true)),
            shutdown: CancellationToken::new(),
        }
    }

    pub async fn spawn<F>(&self, task: F) -> anyhow::Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        if !self.accepting.load(Ordering::Acquire) {
            anyhow::bail!("enrollment task admission is closed");
        }
        while tasks.try_join_next().is_some() {}
        tasks.spawn(task);
        Ok(())
    }

    pub fn cancellation(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Stop new admission without aborting an in-flight device operation.
    /// Every admitted task is awaited through its bounded ISAPI call and DB
    /// terminal transition before the database writer may close.
    pub async fn stop_and_join(&self) {
        self.accepting.store(false, Ordering::Release);
        self.shutdown.cancel();
        let mut tasks = self.tasks.lock().await;
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result {
                tracing::error!(%error, "tracked enrollment task panicked");
            }
        }
    }
}

impl Default for EnrollmentTaskTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Fire-and-forget JoinSet fan-out for an enrollment (D-06).
///
/// Spawns a detached tokio task that:
///   1. Starts a push task per device via JoinSet.
///   2. Awaits all push tasks.
///   3. Calls `finalize_enrollment_status` to set the overall enrollment status.
///
/// Returns immediately — the caller has already sent 202.
pub async fn spawn_enrollment_pushes(
    state: AppState,
    enrollment_id: String,
    face_id: String,
    photo_bytes: Arc<Vec<u8>>,
    employee_id: String,
    employee_name: String,
    devices: Vec<DeviceWithPlaintext>,
) -> anyhow::Result<()> {
    let cancellation = state.enrollment_tasks.cancellation();
    let tracker = state.enrollment_tasks.clone();
    tracker
        .spawn(async move {
        if devices.is_empty() {
            // No devices — immediately finalize as failed.
            let _conn = match state.db.connect() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(err = %e, "push driver: failed to connect for finalize (no devices)");
                    return;
                }
            };
            if let Err(e) = service::finalize_enrollment(&state, &enrollment_id).await {
                tracing::error!(enrollment_id = %enrollment_id, err = %e, "push driver: finalize failed");
            }
            return;
        }

        let mut set: JoinSet<anyhow::Result<()>> = JoinSet::new();

        for device in devices {
            let state = state.clone();
            let enrollment_id = enrollment_id.clone();
            let face_id = face_id.clone();
            let photo_bytes = Arc::clone(&photo_bytes);
            let employee_id = employee_id.clone();
            let full_name = employee_name.clone();
            let cancellation = cancellation.clone();

            set.spawn(async move {
                if cancellation.is_cancelled() {
                    if let Ok(push_id) = service::get_push_id(
                        &state.db.connect().map_err(|error| anyhow::anyhow!(error))?,
                        &enrollment_id,
                        &device.id,
                    )
                    .await
                    {
                        service::complete_push_failure(
                            &state,
                            &push_id,
                            "Enrollment dispatch stopped before device call",
                        )
                        .await
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                    }
                    return Ok(());
                }
                push_one_device(
                    &state,
                    &enrollment_id,
                    &face_id,
                    &photo_bytes,
                    &employee_id,
                    &full_name,
                    &device,
                )
                .await
            });
        }

        // Drain all push task results.
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(err = %e, "push task returned error"),
                Err(e) => tracing::error!(err = %e, "push task panicked"),
            }
        }

        // Finalise enrollment status (success / partial / failed).
        let _conn = match state.db.connect() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(err = %e, "push driver: failed to connect for finalize");
                return;
            }
        };
        if let Err(e) = service::finalize_enrollment(&state, &enrollment_id).await {
            tracing::error!(enrollment_id = %enrollment_id, err = %e, "push driver: finalize failed");
        }
        })
        .await
}

/// Push a face profile to a single device.
///
/// Steps:
///   1. UPDATE enrollment_device_pushes SET status='in_progress', started_at=now
///   2. Build DeviceConnection (password already plaintext — decrypted by caller).
///   3. timeout(30s) wraps both ISAPI calls:
///      a. upsert_user(face_id, full_name)
///      b. upload_face(face_id, jpeg_bytes)
///      4a. On success: UPDATE push row to status='success', upsert device_face_mappings.
///      4b. On failure: UPDATE push row to status='failed', error_message (scrubbed).
///
/// Password scrubbing (T-7-06): any occurrence of device.password in the error
/// string is replaced with "[redacted]" before being persisted.
pub async fn push_one_device(
    state: &AppState,
    enrollment_id: &str,
    face_id: &str,
    photo_bytes: &Arc<Vec<u8>>,
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
) -> anyhow::Result<()> {
    // Find the push row id for this (enrollment_id, device_id) pair.
    let conn = state
        .db
        .connect()
        .map_err(|e| anyhow::anyhow!("connect: {e}"))?;
    let push_id = match service::get_push_id(&conn, enrollment_id, &device.id).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(
                enrollment_id = %enrollment_id,
                device_id = %device.id,
                err = %e,
                "push_one_device: no push row found — skipping"
            );
            return Err(anyhow::anyhow!("push row lookup failed: {e}"));
        }
    };

    let checkpoint_key = service::enrollment_checkpoint_key(&push_id);
    let checkpoint =
        match service::admit_device_operation(state, &checkpoint_key, "enrollment_push").await {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                service::complete_push_failure(
                    state,
                    &push_id,
                    "Device push checkpoint admission failed",
                )
                .await
                .map_err(|persist| {
                    anyhow::anyhow!("persist checkpoint admission failure: {persist}")
                })?;
                return Err(anyhow::anyhow!("admit durable device checkpoint: {error}"));
            }
        };

    if !checkpoint.fresh {
        match checkpoint.state {
            service::DeviceOperationState::DeviceApplied => {
                return service::complete_push_success(
                    state,
                    &push_id,
                    &device.id,
                    face_id,
                    employee_id,
                )
                .await
                .map_err(|error| anyhow::anyhow!("recover successful device push: {error}"));
            }
            service::DeviceOperationState::Prepared | service::DeviceOperationState::Manual => {
                service::record_push_recovery_failure(
                    state,
                    &push_id,
                    "Device operation outcome is ambiguous; manual reconciliation required",
                    true,
                )
                .await
                .map_err(|error| anyhow::anyhow!("persist manual reconciliation: {error}"))?;
                return Err(anyhow::anyhow!(
                    "device operation requires manual reconciliation"
                ));
            }
        }
    }

    // Mark in_progress.
    if let Err(error) = service::mark_push_in_progress_queued(state, &push_id).await {
        service::record_push_recovery_failure(
            state,
            &push_id,
            "Device push state could not be admitted; manual reconciliation required",
            true,
        )
        .await
        .map_err(|persist| anyhow::anyhow!("persist device push admission failure: {persist}"))?;
        return Err(anyhow::anyhow!("admit device push state: {error}"));
    }

    // Build ISAPI client.
    let isapi = match DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    ) {
        Ok(isapi) => isapi,
        Err(error) => {
            let scrubbed = scrub_password(error.to_string(), &device.password);
            service::complete_push_failure(state, &push_id, &scrubbed)
                .await
                .map_err(|persist| {
                    anyhow::anyhow!("persist invalid connection failure: {persist}")
                })?;
            return Err(anyhow::anyhow!("build ISAPI connection: {scrubbed}"));
        }
    };

    let jpeg_bytes = (**photo_bytes).clone();
    let fid = face_id.to_string();
    let fname = full_name.to_string();

    // Both ISAPI calls wrapped in a 30-second timeout.
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        isapi.upsert_user(&fid, &fname).await?;
        isapi.upload_face(&fid, jpeg_bytes).await
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            if let Err(error) = service::mark_device_operation(
                state,
                &checkpoint_key,
                service::DeviceOperationState::DeviceApplied,
            )
            .await
            {
                service::record_push_recovery_failure(
                    state,
                    &push_id,
                    "Device accepted face but durable recovery checkpoint failed",
                    true,
                )
                .await
                .map_err(|persist| {
                    anyhow::anyhow!("persist manual checkpoint recovery: {persist}")
                })?;
                return Err(anyhow::anyhow!(
                    "checkpoint successful device push: {error}"
                ));
            }
            // Success path: update push row + upsert device_face_mapping.
            if let Err(error) =
                service::complete_push_success(state, &push_id, &device.id, face_id, employee_id)
                    .await
            {
                let recovery = "Device push succeeded but mapping persistence failed";
                if let Err(recovery_error) =
                    service::record_push_recovery_failure(state, &push_id, recovery, false).await
                {
                    tracing::error!(
                        push_id,
                        err = %recovery_error,
                        "failed to persist device-push recovery state"
                    );
                }
                return Err(anyhow::anyhow!("commit successful device push: {error}"));
            }
            Ok(())
        }
        Ok(Err(e)) => {
            let scrubbed = scrub_password(e.to_string(), &device.password);
            if let Err(ue) = service::complete_push_failure(state, &push_id, &scrubbed).await {
                tracing::warn!(err = %ue, "failed to update push row to failed");
            }
            Err(anyhow::anyhow!("ISAPI push failed: {scrubbed}"))
        }
        Err(_timeout) => {
            let msg = "Device did not respond within 30 seconds";
            if let Err(ue) = service::complete_push_failure(state, &push_id, msg).await {
                tracing::warn!(err = %ue, "failed to update push row to timeout");
            }
            Err(anyhow::anyhow!("{msg}"))
        }
    }
}

/// Push a face profile to a single device in the backfill context.
///
/// Unlike `push_one_device`, this does NOT touch enrollment_device_pushes rows
/// (there is no enrollment context for backfill — just upsert the face on the
/// device and update device_face_mappings on success).
pub async fn push_one_device_for_backfill(
    state: &AppState,
    face_id: &str,
    photo_bytes: &[u8],
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
) -> anyhow::Result<()> {
    let checkpoint_key = service::backfill_checkpoint_key(&device.id, face_id);
    let checkpoint = service::admit_device_operation(state, &checkpoint_key, "backfill_push")
        .await
        .map_err(|error| anyhow::anyhow!("admit backfill checkpoint: {error}"))?;
    if !checkpoint.fresh {
        match checkpoint.state {
            service::DeviceOperationState::DeviceApplied => {
                return service::complete_backfill_mapping(
                    state,
                    &checkpoint_key,
                    &device.id,
                    face_id,
                    employee_id,
                )
                .await
                .map_err(|error| anyhow::anyhow!("recover backfill mapping: {error}"));
            }
            service::DeviceOperationState::Prepared | service::DeviceOperationState::Manual => {
                service::mark_device_operation(
                    state,
                    &checkpoint_key,
                    service::DeviceOperationState::Manual,
                )
                .await
                .map_err(|error| anyhow::anyhow!("mark ambiguous backfill: {error}"))?;
                return Err(anyhow::anyhow!(
                    "backfill device result is ambiguous; manual reconciliation required"
                ));
            }
        }
    }

    let isapi = match DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    ) {
        Ok(isapi) => isapi,
        Err(error) => {
            service::clear_device_operation(state, &checkpoint_key).await?;
            return Err(error);
        }
    };

    let jpeg_bytes = photo_bytes.to_vec();
    let fid = face_id.to_string();
    let fname = full_name.to_string();

    let result = tokio::time::timeout(Duration::from_secs(30), async {
        isapi.upsert_user(&fid, &fname).await?;
        isapi.upload_face(&fid, jpeg_bytes).await
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            service::mark_device_operation(
                state,
                &checkpoint_key,
                service::DeviceOperationState::DeviceApplied,
            )
            .await
            .map_err(|error| anyhow::anyhow!("checkpoint successful backfill: {error}"))?;
            service::complete_backfill_mapping(
                state,
                &checkpoint_key,
                &device.id,
                face_id,
                employee_id,
            )
            .await
            .map_err(|e| anyhow::anyhow!("complete backfill mapping: {e}"))?;
            Ok(())
        }
        Ok(Err(e)) => {
            let scrubbed = scrub_password(e.to_string(), &device.password);
            service::clear_device_operation(state, &checkpoint_key).await?;
            Err(anyhow::anyhow!("backfill ISAPI push failed: {scrubbed}"))
        }
        Err(_) => {
            service::clear_device_operation(state, &checkpoint_key).await?;
            Err(anyhow::anyhow!("backfill: device timeout after 30s"))
        }
    }
}

/// Scrub all occurrences of `password` from `text` to prevent credential leakage
/// in persisted error messages (T-7-06 threat mitigation).
fn scrub_password(text: String, password: &str) -> String {
    if password.is_empty() {
        return text;
    }
    text.replace(password, "[redacted]")
}
