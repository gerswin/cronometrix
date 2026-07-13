//! Enrollment fan-out pusher.
//!
//! `spawn_enrollment_pushes` fires a detached tokio task that drives N per-device
//! push tasks concurrently via JoinSet (D-06 fire-and-forget pattern).
//! The driver outlives the originating HTTP request (D-09 modal-close-doesn't-cancel).
//!
//! `push_one_device` is the reusable single-device push path shared by:
//!   - spawn_enrollment_pushes (fan-out, enrollment context)
//!   - retry_push handler (single retry)
//!   - BackfillWorker (per-employee push to a newly registered device)

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;

use crate::devices::models::DeviceWithPlaintext;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

use super::service;

/// Fire-and-forget JoinSet fan-out for an enrollment (D-06).
///
/// Spawns a detached tokio task that:
///   1. Starts a push task per device via JoinSet.
///   2. Awaits all push tasks.
///   3. Calls `finalize_enrollment_status` to set the overall enrollment status.
///
/// Returns immediately — the caller has already sent 202.
pub fn spawn_enrollment_pushes(
    state: AppState,
    enrollment_id: String,
    face_id: String,
    photo_bytes: Arc<Vec<u8>>,
    employee_id: String,
    devices: Vec<DeviceWithPlaintext>,
) {
    tokio::spawn(async move {
        if devices.is_empty() {
            // No devices — immediately finalize as failed.
            let _conn = match state.db.connect() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(err = %e, "push driver: failed to connect for finalize (no devices)");
                    return;
                }
            };
            if let Err(e) = service::finalize_enrollment_status_queued(&state, &enrollment_id).await
            {
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

            // Fetch employee name for ISAPI UserInfo/Record.
            let full_name = {
                let conn = match state.db.connect() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!(err = %e, "push driver: failed to connect for name lookup");
                        continue;
                    }
                };
                let mut rows = match conn
                    .query(
                        "SELECT name FROM employees WHERE id = ?1",
                        libsql::params![employee_id.clone()],
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(err = %e, "push driver: failed to query employee name");
                        continue;
                    }
                };
                match rows.next().await {
                    Ok(Some(row)) => row.get::<String>(0).unwrap_or_default(),
                    _ => employee_id.clone(), // fallback
                }
            };

            set.spawn(async move {
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
        if let Err(e) = service::finalize_enrollment_status_queued(&state, &enrollment_id).await {
            tracing::error!(enrollment_id = %enrollment_id, err = %e, "push driver: finalize failed");
        }
    });
}

/// Push a face profile to a single device.
///
/// Steps:
///   1. UPDATE enrollment_device_pushes SET status='in_progress', started_at=now
///   2. Build DeviceConnection (password already plaintext — decrypted by caller).
///   3. timeout(30s) wraps both ISAPI calls:
///      a. upsert_user(face_id, full_name)
///      b. upload_face(face_id, jpeg_bytes)
///   4a. On success: UPDATE push row to status='success', upsert device_face_mappings.
///   4b. On failure: UPDATE push row to status='failed', error_message (scrubbed).
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
            return Ok(());
        }
    };

    // Mark in_progress.
    if let Err(e) = service::mark_push_in_progress_queued(state, &push_id).await {
        tracing::warn!(err = %e, "failed to mark push in_progress");
    }

    // Build ISAPI client.
    let isapi = DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )?;

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
            // Success path: update push row + upsert device_face_mapping.
            if let Err(e) =
                service::update_push_status_queued(state, &push_id, "success", None).await
            {
                tracing::warn!(err = %e, "failed to update push row to success");
            }
            if let Err(e) =
                service::upsert_device_face_mapping_queued(state, &device.id, face_id, employee_id)
                    .await
            {
                tracing::warn!(err = %e, "failed to upsert device_face_mapping");
            }
            Ok(())
        }
        Ok(Err(e)) => {
            let scrubbed = scrub_password(e.to_string(), &device.password);
            if let Err(ue) =
                service::update_push_status_queued(state, &push_id, "failed", Some(&scrubbed)).await
            {
                tracing::warn!(err = %ue, "failed to update push row to failed");
            }
            Err(anyhow::anyhow!("ISAPI push failed: {scrubbed}"))
        }
        Err(_timeout) => {
            let msg = "Device did not respond within 30 seconds";
            if let Err(ue) =
                service::update_push_status_queued(state, &push_id, "failed", Some(msg)).await
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
    let isapi = DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )?;

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
            service::upsert_device_face_mapping_queued(state, &device.id, face_id, employee_id)
                .await
                .map_err(|e| anyhow::anyhow!("upsert mapping: {e}"))?;
            Ok(())
        }
        Ok(Err(e)) => {
            let scrubbed = scrub_password(e.to_string(), &device.password);
            Err(anyhow::anyhow!("backfill ISAPI push failed: {scrubbed}"))
        }
        Err(_) => Err(anyhow::anyhow!("backfill: device timeout after 30s")),
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
