//! Shared attendance-event ingestion.
//!
//! Two transports deliver the same payload: the long-lived `alertStream` we
//! pull, and the `httpHosts` webhook the device pushes to. They differ only in
//! how bytes arrive — parsing, identity resolution, filtering and persistence
//! must stay identical, or a marking would be recorded differently depending on
//! which transport a given reader happens to use.
//!
//! This module only decodes the Hikvision wire format into a vendor-neutral
//! `RawMarking` and hands it to `attendance::ingest`; resolution and
//! persistence live there so a second reader brand can reuse them.

use bytes::Bytes;

use crate::attendance;
use crate::attendance::marking::RawMarking;

use super::events::{parse_alert, EventNotificationAlert};

/// Re-exported so `use crate::isapi::ingest::{ingest_alert, IngestOutcome}`
/// call sites (e.g. `devices/push.rs`) keep resolving after the outcome type
/// moved to `attendance::ingest`.
pub use crate::attendance::ingest::IngestOutcome;

/// Parse one alert payload and persist it if it is a real marking.
///
/// `direction_default` comes from the device row and is used only when the
/// reader did not report an attendance status of its own.
pub async fn ingest_alert(
    state: &crate::state::AppState,
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
            // Distinct from the `Skipped` returns below: this body was never a
            // valid marking, heartbeat, or notification — it just didn't parse.
            // `workers::push_drain` treats this as a permanent failure (dead
            // letter), never as something worth retrying.
            return Ok(IngestOutcome::Unparseable);
        }
    };

    // Heartbeats: liveness is already recorded by the caller; nothing to store.
    if alert.is_heartbeat() {
        tracing::debug!(device_id = %device_id, "heartbeat received");
        return Ok(IngestOutcome::Skipped);
    }

    let ace = match alert.access_controller_event.as_ref() {
        Some(ace) => ace,
        None => {
            tracing::debug!(device_id = %device_id, "alert without AccessControllerEvent — skipped");
            return Ok(IngestOutcome::Skipped);
        }
    };

    // Door, tamper and status events share the AccessControllerEvent envelope
    // with real markings but name nobody. Persisting them would invent an
    // unknown-face row every time the door moved. Logged here, with the sub/
    // major codes, because only the adapter knows what those codes mean.
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

    let marking = RawMarking {
        external_person_id: (!ace.employee_no_string.is_empty())
            .then(|| ace.employee_no_string.clone()),
        face_id: (!ace.face_id.is_empty()).then(|| ace.face_id.clone()),
        occurred_at: alert
            .captured_at_epoch()
            .unwrap_or_else(|| chrono::Utc::now().timestamp()),
        direction: ace.reported_direction().map(str::to_string),
        photo: jpeg_bytes.map(|bytes| bytes.to_vec()),
        raw_payload,
    };

    attendance::ingest::ingest(state, device_id, direction_default, marking).await
}
