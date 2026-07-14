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

use futures::FutureExt;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::devices::models::DeviceWithPlaintext;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

use super::dispatcher::{AuthorizedAttempt, AuthorizedDispatchCommand};
use super::service;

#[derive(Clone)]
pub struct EnrollmentTaskTracker {
    tasks: Arc<Mutex<JoinSet<anyhow::Result<()>>>>,
    accepting: Arc<AtomicBool>,
    shutdown: CancellationToken,
    errors: Arc<std::sync::Mutex<Vec<String>>>,
}

impl EnrollmentTaskTracker {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(JoinSet::new())),
            accepting: Arc::new(AtomicBool::new(true)),
            shutdown: CancellationToken::new(),
            errors: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub async fn spawn<F>(&self, task: F) -> anyhow::Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.spawn_result(async move {
            task.await;
            Ok(())
        })
        .await
    }

    async fn spawn_result<F>(&self, task: F) -> anyhow::Result<()>
    where
        F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        if !self.accepting.load(Ordering::Acquire) {
            anyhow::bail!("enrollment task admission is closed");
        }
        while let Some(result) = tasks.try_join_next() {
            self.record_join_result(result);
        }
        tasks.spawn(task);
        Ok(())
    }

    pub fn cancellation(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Stop new admission without aborting an in-flight device operation.
    /// Every admitted task is awaited through its bounded ISAPI call and DB
    /// terminal transition before the database writer may close.
    pub async fn stop_and_join(&self) -> anyhow::Result<()> {
        self.accepting.store(false, Ordering::Release);
        self.shutdown.cancel();
        let mut tasks = self.tasks.lock().await;
        while let Some(result) = tasks.join_next().await {
            self.record_join_result(result);
        }
        let errors = std::mem::take(
            &mut *self
                .errors
                .lock()
                .expect("enrollment tracker error registry poisoned"),
        );
        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(
                "{} tracked enrollment task(s) failed; first: {}",
                errors.len(),
                errors[0]
            )
        }
    }

    fn record_join_result(&self, result: Result<anyhow::Result<()>, tokio::task::JoinError>) {
        let error = match result {
            Ok(Ok(())) => return,
            Ok(Err(error)) => error.to_string(),
            Err(error) => format!("task join failure: {error}"),
        };
        tracing::error!(%error, "tracked enrollment task failed");
        self.errors
            .lock()
            .expect("enrollment tracker error registry poisoned")
            .push(error);
    }
}

impl Default for EnrollmentTaskTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Fire-and-forget JoinSet fan-out for an enrollment (D-06).
///
/// Admits a lifecycle-owned tokio task that:
///   1. Starts a push task per device via JoinSet.
///   2. Awaits all push tasks.
///   3. Calls `finalize_enrollment_status` to set the overall enrollment status.
///
/// Returns after admission; shutdown drains the tracked task before the writer closes.
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

/// Admit an exact post-commit snapshot. Each target carries the private token
/// created by the transaction that inserted its push row and `prepared`
/// checkpoint, so this path never reclassifies that checkpoint as ambiguous.
pub(super) async fn spawn_authorized_enrollment_pushes(
    state: AppState,
    command: AuthorizedDispatchCommand,
) -> anyhow::Result<()> {
    let recovery_enrollment_id = command.enrollment_id.clone();
    let recovery_push_ids: Vec<String> = command
        .targets
        .iter()
        .map(|target| target.push_id.clone())
        .collect();
    let tracker = state.enrollment_tasks.clone();
    let task_state = state.clone();
    let admitted = tracker
        .spawn_result(async move {
            let AuthorizedDispatchCommand {
                enrollment_id,
                face_id,
                photo_bytes,
                employee_id,
                employee_name,
                targets,
            } = command;
            let mut tasks = JoinSet::new();
            let mut first_error = None;
            for target in targets {
                let state = state.clone();
                let enrollment_id = enrollment_id.clone();
                let face_id = face_id.clone();
                let photo_bytes = photo_bytes.clone();
                let employee_id = employee_id.clone();
                let employee_name = employee_name.clone();
                tasks.spawn(async move {
                    let push_id = target.push_id.clone();
                    let result = std::panic::AssertUnwindSafe(push_committed_device(
                        &state,
                        &enrollment_id,
                        &face_id,
                        &photo_bytes,
                        &employee_id,
                        &employee_name,
                        target.push_id,
                        target.device,
                        target.attempt,
                    ))
                    .catch_unwind()
                    .await;
                    match result {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(error)) => {
                            service::terminalize_authorized_failure(
                                &state,
                                &push_id,
                                "Enrollment device operation failed; manual reconciliation required",
                            )
                            .await
                            .map_err(|persist| anyhow::anyhow!(
                                "{error}; terminalize authorized push {push_id}: {persist}"
                            ))?;
                            Err(error)
                        }
                        Err(_) => {
                            service::terminalize_authorized_failure(
                                &state,
                                &push_id,
                                "Enrollment device operation panicked; manual reconciliation required",
                            )
                            .await
                            .map_err(|persist| anyhow::anyhow!(
                                "terminalize panicked authorized push {push_id}: {persist}"
                            ))?;
                            Err(anyhow::anyhow!("authorized enrollment push panicked"))
                        }
                    }
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "authorized push failed");
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                    Err(error) => {
                        tracing::error!(%error, "authorized push panicked");
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                }
            }
            if let Err(error) = service::finalize_enrollment(&state, &enrollment_id).await {
                tracing::error!(%error, %enrollment_id, "authorized enrollment finalize failed");
                first_error.get_or_insert_with(|| error.to_string());
            }
            if let Some(error) = first_error {
                anyhow::bail!("authorized enrollment dispatch failed: {error}");
            }
            Ok(())
        })
        .await;
    if let Err(admission_error) = admitted {
        for push_id in recovery_push_ids {
            service::record_push_recovery_failure(
                &task_state,
                &push_id,
                "Enrollment dispatch could not be admitted; manual reconciliation required",
                true,
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        service::finalize_enrollment(&task_state, &recovery_enrollment_id)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        return Err(admission_error);
    }
    Ok(())
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
    push_enrollment_device(
        state,
        enrollment_id,
        face_id,
        photo_bytes,
        employee_id,
        full_name,
        device,
        None,
        Duration::from_secs(30),
    )
    .await
}

/// Test seam for the otherwise fixed 30-second device deadline.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub async fn push_one_device_with_timeout(
    state: &AppState,
    enrollment_id: &str,
    face_id: &str,
    photo_bytes: &Arc<Vec<u8>>,
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
    call_timeout: Duration,
) -> anyhow::Result<()> {
    push_enrollment_device(
        state,
        enrollment_id,
        face_id,
        photo_bytes,
        employee_id,
        full_name,
        device,
        None,
        call_timeout,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn push_committed_device(
    state: &AppState,
    enrollment_id: &str,
    face_id: &str,
    photo_bytes: &Arc<Vec<u8>>,
    employee_id: &str,
    full_name: &str,
    push_id: String,
    device: DeviceWithPlaintext,
    attempt: AuthorizedAttempt,
) -> anyhow::Result<()> {
    push_enrollment_device(
        state,
        enrollment_id,
        face_id,
        photo_bytes,
        employee_id,
        full_name,
        &device,
        Some((push_id, attempt)),
        Duration::from_secs(30),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn push_enrollment_device(
    state: &AppState,
    enrollment_id: &str,
    face_id: &str,
    photo_bytes: &Arc<Vec<u8>>,
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
    committed: Option<(String, AuthorizedAttempt)>,
    call_timeout: Duration,
) -> anyhow::Result<()> {
    // Find the push row id for this (enrollment_id, device_id) pair.
    let (push_id, authorized) = if let Some((push_id, _)) = committed {
        (push_id, true)
    } else {
        let conn = state
            .db
            .connect()
            .map_err(|e| anyhow::anyhow!("connect: {e}"))?;
        let push_id = service::get_push_id(&conn, enrollment_id, &device.id)
            .await
            .map_err(|error| anyhow::anyhow!("push row lookup failed: {error}"))?;
        (push_id, false)
    };

    let checkpoint_key = service::enrollment_checkpoint_key(&push_id);
    if !authorized {
        let checkpoint = match service::admit_device_operation(
            state,
            &checkpoint_key,
            "enrollment_push",
        )
        .await
        {
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
    let result = tokio::time::timeout(call_timeout, async {
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
            let terminal = e
                .downcast_ref::<crate::isapi::client::DeviceResponseError>()
                .is_some();
            let persisted = if terminal {
                service::complete_push_failure(state, &push_id, &scrubbed).await
            } else {
                service::record_push_recovery_failure(
                    state,
                    &push_id,
                    "Device operation outcome is ambiguous; manual reconciliation required",
                    true,
                )
                .await
            };
            if let Err(update_error) = persisted {
                tracing::warn!(err = %update_error, "failed to update push row to failed");
            }
            Err(anyhow::anyhow!("ISAPI push failed: {scrubbed}"))
        }
        Err(_timeout) => {
            let msg = "Device did not respond within 30 seconds";
            if let Err(ue) = service::record_push_recovery_failure(
                state,
                &push_id,
                "Device operation timed out; manual reconciliation required",
                true,
            )
            .await
            {
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
    push_one_device_for_backfill_with_timeout(
        state,
        face_id,
        photo_bytes,
        employee_id,
        full_name,
        device,
        Duration::from_secs(30),
    )
    .await
}

#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub async fn push_one_device_for_backfill_with_timeout(
    state: &AppState,
    face_id: &str,
    photo_bytes: &[u8],
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
    call_timeout: Duration,
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

    let result = tokio::time::timeout(call_timeout, async {
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
            service::mark_device_operation(
                state,
                &checkpoint_key,
                service::DeviceOperationState::Manual,
            )
            .await?;
            Err(anyhow::anyhow!("backfill ISAPI push failed: {scrubbed}"))
        }
        Err(_) => {
            service::mark_device_operation(
                state,
                &checkpoint_key,
                service::DeviceOperationState::Manual,
            )
            .await?;
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
