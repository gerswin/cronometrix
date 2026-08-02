//! Vendor-neutral attendance ingestion.
//!
//! Everything from identity resolution onward: which employee a marking belongs
//! to, whether it is worth storing, and how it is persisted. Adapters supply a
//! `RawMarking`; nothing in here knows which protocol produced it.

use crate::events::models::{NewAttendanceEvent, PersistOutcome};
use crate::events::service as events_service;
use crate::state::AppState;

use super::marking::RawMarking;

/// What ingesting one marking did, so callers can log or count without
/// re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    /// A new attendance row was written.
    Persisted,
    /// Already recorded — the device replayed it, or both transports saw it.
    Deduplicated,
    /// A heartbeat, a door/tamper notification, or anything else that names
    /// nobody. Deliberately not persisted.
    Skipped,
}

/// Resolve and persist one marking. `direction_default` comes from the device
/// row and is used only when the reader did not report a direction of its own.
pub async fn ingest(
    state: &AppState,
    device_id: &str,
    direction_default: &str,
    marking: RawMarking,
) -> anyhow::Result<IngestOutcome> {
    // Door, tamper and status events share the marking shape with real
    // attendance but name nobody. Persisting them would invent an
    // unknown-face row every time the door moved.
    if !marking.has_identity() {
        tracing::debug!(device_id = %device_id, "marking without an identity — skipped");
        return Ok(IngestOutcome::Skipped);
    }

    let captured_at = marking.occurred_at;

    let direction = marking
        .direction
        .unwrap_or_else(|| direction_default.to_string());

    let face_id = marking.face_id;
    let employee_no_string = marking.external_person_id;

    let conn = state.db.connect().map_err(anyhow::Error::from)?;
    let employee_id = events_service::lookup_employee_for_event(
        &conn,
        device_id,
        face_id.as_deref(),
        employee_no_string.as_deref(),
    )
    .await
    .map_err(|error| anyhow::anyhow!("lookup_employee_for_event failed: {error}"))?;
    let is_unknown = employee_id.is_none();

    let new_event = NewAttendanceEvent {
        id: uuid::Uuid::new_v4().to_string(),
        employee_id,
        device_id: device_id.to_string(),
        direction,
        captured_at,
        is_unknown,
        face_id,
        employee_no_string,
        raw_payload: marking.raw_payload,
        photo_bytes: marking.photo,
    };

    // Snapshot the fields the SSE publish needs BEFORE `new_event` is consumed
    // by value.
    let sse_snapshot = NewAttendanceEvent {
        id: new_event.id.clone(),
        employee_id: new_event.employee_id.clone(),
        device_id: new_event.device_id.clone(),
        direction: new_event.direction.clone(),
        captured_at: new_event.captured_at,
        is_unknown: new_event.is_unknown,
        face_id: new_event.face_id.clone(),
        employee_no_string: new_event.employee_no_string.clone(),
        raw_payload: String::new(),
        photo_bytes: None,
    };

    match events_service::persist_attendance_event_queued(
        state,
        &state.paths.events_root,
        new_event,
    )
    .await
    {
        Ok(PersistOutcome::Inserted { photo_path }) => {
            tracing::info!(device_id = %device_id, photo_path = ?photo_path, "event persisted");
            events_service::publish_recompute_if_employee(state, &sse_snapshot);
            events_service::publish_sse_event(state, &sse_snapshot, &photo_path).await;
            Ok(IngestOutcome::Persisted)
        }
        Ok(PersistOutcome::Deduplicated) => {
            tracing::debug!(device_id = %device_id, "event deduplicated");
            Ok(IngestOutcome::Deduplicated)
        }
        Err(error) => Err(anyhow::anyhow!("persist_attendance_event failed: {error}")),
    }
}
