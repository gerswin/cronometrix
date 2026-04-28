//! Enrollment fan-out pusher.
//!
//! `spawn_enrollment_pushes` fires a detached tokio task that drives N per-device
//! push tasks concurrently via `JoinSet` (D-06 fire-and-forget pattern).
//! The driver outlives the originating HTTP request (D-09 modal-close-doesn't-cancel).
//!
//! Stub — full implementation in Task 5.
//! Function signatures are final so handlers.rs can compile in Task 4.

#![allow(unused)]

use std::sync::Arc;

use crate::devices::models::DeviceWithPlaintext;
use crate::state::AppState;

/// Fire-and-forget JoinSet fan-out for an enrollment (D-06).
///
/// Returns immediately. A detached tokio task runs N concurrent push tasks,
/// one per active device. When all settle, `finalize_enrollment_status` is called.
pub fn spawn_enrollment_pushes(
    state: AppState,
    enrollment_id: String,
    face_id: String,
    photo_bytes: Arc<Vec<u8>>,
    employee_id: String,
    devices: Vec<DeviceWithPlaintext>,
) {
    // Full implementation in Task 5.
    // Stub: log and skip so handlers compile and tests in Task 4 pass.
    tracing::debug!(
        enrollment_id = %enrollment_id,
        device_count = devices.len(),
        "spawn_enrollment_pushes: stub — Task 5 will implement"
    );
}

/// Push face profile to a single device (reused by retry + backfill worker).
///
/// Full implementation in Task 5.
pub async fn push_one_device(
    state: &AppState,
    enrollment_id: &str,
    face_id: &str,
    photo_bytes: &Arc<Vec<u8>>,
    employee_id: &str,
    full_name: &str,
    device: &DeviceWithPlaintext,
) -> anyhow::Result<()> {
    // Stub — Task 5 implements: upsert_user + upload_face with 30s timeout,
    // push row updates, device_face_mapping upsert, password scrubbing.
    tracing::debug!(
        device_id = %device.id,
        face_id = %face_id,
        "push_one_device: stub — Task 5 will implement"
    );
    Ok(())
}
