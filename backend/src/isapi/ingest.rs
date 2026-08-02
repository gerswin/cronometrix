//! Shared attendance-event ingestion.
//!
//! Two transports deliver the same payload: the long-lived `alertStream` we
//! pull, and the `httpHosts` webhook the device pushes to. They differ only in
//! how bytes arrive — parsing, identity resolution, filtering and persistence
//! must stay identical, or a marking would be recorded differently depending on
//! which transport a given reader happens to use.

use bytes::Bytes;

use crate::events::models::{NewAttendanceEvent, PersistOutcome};
use crate::events::service as events_service;
use crate::state::AppState;

use super::events::{parse_alert, EventNotificationAlert};

/// What ingesting one payload did, so callers can log or count without
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

/// Parse one alert payload and persist it if it is a real marking.
///
/// `direction_default` comes from the device row and is used only when the
/// reader did not report an attendance status of its own.
pub async fn ingest_alert(
    state: &AppState,
    device_id: &str,
    direction_default: &str,
    bytes: &[u8],
    content_type: &str,
    raw_payload: String,
    jpeg_bytes: Option<Bytes>,
) -> anyhow::Result<IngestOutcome> {
    // The verbatim payload, at TRACE. Firmware in this family disagrees with its
    // own documentation often enough that "what did the reader actually send"
    // is the first question in every investigation, and reconstructing it from
    // a packet capture means taking the device offline. Truncated because a
    // pushed event carries an ~80 KB JPEG that would bury the log.
    tracing::trace!(
        device_id = %device_id,
        content_type = %content_type,
        payload = %raw_payload.chars().take(2000).collect::<String>(),
        "alert payload received"
    );

    let alert: EventNotificationAlert = match parse_alert(bytes, content_type) {
        Ok(alert) => alert,
        Err(error) => {
            // Include the payload here regardless of level: a parse failure is
            // useless without the bytes that caused it.
            tracing::warn!(
                device_id = %device_id,
                content_type = %content_type,
                err = %error,
                payload = %raw_payload.chars().take(2000).collect::<String>(),
                "failed to parse alert payload — skipping"
            );
            return Ok(IngestOutcome::Skipped);
        }
    };

    // Heartbeats: liveness is already recorded by the caller; nothing to store.
    if alert.is_heartbeat() {
        tracing::debug!(device_id = %device_id, "heartbeat received");
        return Ok(IngestOutcome::Skipped);
    }

    let Some(ace) = alert.access_controller_event.as_ref() else {
        tracing::debug!(device_id = %device_id, "alert without AccessControllerEvent — skipped");
        return Ok(IngestOutcome::Skipped);
    };

    // Door, tamper and status events share the AccessControllerEvent envelope
    // with real markings but name nobody. Persisting them would invent an
    // unknown-face row every time the door moved.
    if !ace.has_identity() {
        // The decoded fields go in alongside the codes: knowing an event was
        // `sub=76` says nothing without knowing it carried a picture, a verify
        // mode and no name — which is exactly what distinguishes "somebody
        // unrecognised walked past" from "the reader is misconfigured".
        tracing::debug!(
            device_id = %device_id,
            major = ?ace.major_event_type,
            sub = ?ace.sub_event_type,
            device_time = %alert.date_time,
            verify_mode = %ace.current_verify_mode,
            attendance_status = %ace.attendance_status,
            has_photo = jpeg_bytes.is_some(),
            "access-control event without an identity — skipped"
        );
        return Ok(IngestOutcome::Skipped);
    }

    let captured_at = alert
        .captured_at_epoch()
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    let direction = ace
        .reported_direction()
        .map(str::to_string)
        .unwrap_or_else(|| direction_default.to_string());

    let face_id = (!ace.face_id.is_empty()).then(|| ace.face_id.clone());
    let employee_no_string =
        (!ace.employee_no_string.is_empty()).then(|| ace.employee_no_string.clone());

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
        raw_payload,
        photo_bytes: jpeg_bytes.map(|b| b.to_vec()),
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
