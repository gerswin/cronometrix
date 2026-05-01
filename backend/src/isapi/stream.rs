//! Long-lived alertStream consumer (Plan 02-03 Task 1).
//!
//! Opens ONE persistent `reqwest` GET against a device's
//! `/ISAPI/Event/notification/alertStream` endpoint with digest auth, parses
//! the multipart/mixed body as it streams in, and dispatches each
//! `(xml, optional jpeg)` pair into `events::service::persist_attendance_event`.
//!
//! Heartbeats never persist — they only refresh `devices.last_seen_at`.
//! Any byte successfully read from the device sets `connection_state=online`
//! and touches `last_seen_at`; the watchdog is responsible for marking stale
//! devices offline.
//!
//! Errors propagate to `supervisor::task::device_task`, which handles the
//! reconnect + backoff loop.

use std::time::Duration;

use bytes::Bytes;
use diqwest::WithDigestAuth;
use reqwest::Client;

use crate::events::models::{NewAttendanceEvent, PersistOutcome};
use crate::events::service as events_service;
use crate::state::AppState;
use crate::supervisor::status::{touch_last_seen, update_connection_state};

use super::events::{direction_for_attendance_status, strip_xmlns, EventNotificationAlert};

/// Minimal plaintext-carrying handle for the stream loop. Deliberately NOT
/// `Debug`/`Serialize` — the password must stay on the task stack and must
/// never appear in tracing output.
pub struct DeviceConfig {
    pub id: String,
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub direction_default: String,
    pub allow_insecure_tls: bool,
}

impl std::fmt::Debug for DeviceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceConfig")
            .field("id", &self.id)
            .field("base_url", &self.base_url)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("direction_default", &self.direction_default)
            .field("allow_insecure_tls", &self.allow_insecure_tls)
            .finish()
    }
}

/// T-2-19 / T-2-08 mitigations:
/// - per-field 10 MB cap — the largest realistic JPEG from K1T3xx firmware is
///   ~150 KB; 10 MB gives 60× headroom without opening the door to OOM.
/// - whole-stream 1 GiB — the alertStream is long-lived so we don't want a
///   low cap to terminate a healthy connection; 1 GiB is far above anything
///   a device emits in weeks.
const PER_FIELD_LIMIT: u64 = 10 * 1024 * 1024;
const STREAM_WHOLE_LIMIT: u64 = 1024 * 1024 * 1024;

/// Extract the `boundary=...` parameter from a Content-Type header value.
///
/// `multer::parse_boundary` only accepts `multipart/form-data`, but Hikvision
/// devices emit `multipart/mixed`. We implement a permissive parser that
/// accepts any `multipart/*` subtype and returns the boundary string.
fn extract_boundary(content_type: &str) -> anyhow::Result<String> {
    // Fast path: find "boundary=" and take the value until `;` or end-of-string.
    let lower = content_type.to_ascii_lowercase();
    if !lower.starts_with("multipart/") {
        anyhow::bail!(
            "expected multipart/* content-type, got {}",
            content_type
        );
    }
    let idx = lower
        .find("boundary=")
        .ok_or_else(|| anyhow::anyhow!("no boundary parameter in content-type"))?;
    let after = &content_type[idx + "boundary=".len()..];
    let value = after.split(';').next().unwrap_or(after).trim();
    // Boundaries may be quoted.
    let unquoted = value
        .trim_start_matches('"')
        .trim_end_matches('"')
        .to_string();
    if unquoted.is_empty() {
        anyhow::bail!("empty boundary parameter");
    }
    Ok(unquoted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_boundary_multipart_mixed() {
        assert_eq!(
            extract_boundary("multipart/mixed; boundary=MIME_boundary").unwrap(),
            "MIME_boundary"
        );
    }

    #[test]
    fn extract_boundary_quoted() {
        assert_eq!(
            extract_boundary("multipart/mixed; boundary=\"xyz\"").unwrap(),
            "xyz"
        );
    }

    #[test]
    fn extract_boundary_form_data() {
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=abc").unwrap(),
            "abc"
        );
    }

    #[test]
    fn extract_boundary_rejects_non_multipart() {
        assert!(extract_boundary("application/json").is_err());
    }
}

/// Open and consume one alertStream connection. Returns when the upstream
/// closes (gracefully or with an error). The caller (`device_task`) is
/// responsible for the reconnect loop.
pub async fn connect_and_stream(cfg: &DeviceConfig, state: &AppState) -> anyhow::Result<()> {
    let url = format!("{}/ISAPI/Event/notification/alertStream", cfg.base_url);

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(cfg.allow_insecure_tls)
        .build()?;

    let resp = client
        .get(&url)
        .send_digest_auth((cfg.username.as_str(), cfg.password.as_str()))
        .await?;

    let status = resp.status();
    anyhow::ensure!(
        status.is_success(),
        "alertStream returned status {} for device {}",
        status,
        cfg.id
    );

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("alertStream response missing Content-Type header"))?
        .to_string();
    // Hikvision sends `multipart/mixed`; `multer::parse_boundary` only
    // accepts `multipart/form-data`. Extract the boundary directly.
    let boundary = extract_boundary(&content_type)?;

    // Mark the device as online BEFORE we start pulling bytes. If the first
    // `next_field` call fails we still want the transition recorded (so the
    // watchdog sees a sensible `last_seen_at` timeline).
    update_connection_state(state, &cfg.id, "online").await?;
    touch_last_seen(state, &cfg.id).await?;

    let stream = resp.bytes_stream();
    let constraints = multer::Constraints::new().size_limit(
        multer::SizeLimit::new()
            .per_field(PER_FIELD_LIMIT)
            .whole_stream(STREAM_WHOLE_LIMIT),
    );
    let mut mp = multer::Multipart::with_constraints(stream, boundary, constraints);

    let mut pending_xml: Option<(Bytes, String)> = None;

    loop {
        let field_res = mp.next_field().await;
        let field_opt = match field_res {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow::anyhow!("multipart parse error: {}", e));
            }
        };
        let Some(field) = field_opt else {
            break;
        };

        let ct = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_default();
        let bytes = field.bytes().await?;

        // Any successful read means the device is alive — refresh last_seen.
        touch_last_seen(state, &cfg.id).await?;

        if ct.starts_with("application/xml") || bytes.starts_with(b"<EventNotificationAlert") {
            // Commit any pending XML with no JPEG (Pitfall 2: some events
            // carry no attachment).
            if let Some((prev_bytes, prev_raw)) = pending_xml.take() {
                ingest_pair(state, cfg, prev_bytes, None, prev_raw).await?;
            }
            let raw = std::str::from_utf8(&bytes).unwrap_or_default().to_string();
            pending_xml = Some((bytes, raw));
        } else if ct.starts_with("image/jpeg") || bytes.starts_with(b"\xFF\xD8\xFF") {
            if let Some((prev_bytes, prev_raw)) = pending_xml.take() {
                ingest_pair(state, cfg, prev_bytes, Some(bytes), prev_raw).await?;
            }
            // Orphan JPEG (no preceding XML) — drop.
        } else {
            tracing::debug!(
                device_id = %cfg.id,
                content_type = %ct,
                "alertStream part with unknown Content-Type — ignoring"
            );
        }
    }

    // Flush any pending XML on clean end-of-stream.
    if let Some((prev_bytes, prev_raw)) = pending_xml.take() {
        ingest_pair(state, cfg, prev_bytes, None, prev_raw).await?;
    }
    Ok(())
}

/// Parse a (xml, jpeg?) pair and route it through the persist pipeline.
async fn ingest_pair(
    state: &AppState,
    cfg: &DeviceConfig,
    xml_bytes: Bytes,
    jpeg_bytes: Option<Bytes>,
    raw_xml: String,
) -> anyhow::Result<()> {
    let xml_str = std::str::from_utf8(&xml_bytes)
        .map_err(|e| anyhow::anyhow!("XML part is not valid UTF-8: {}", e))?;
    let stripped = strip_xmlns(xml_str);
    let alert: EventNotificationAlert = match quick_xml::de::from_str(&stripped) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(
                device_id = %cfg.id,
                err = %e,
                "failed to parse EventNotificationAlert XML — skipping part"
            );
            return Ok(());
        }
    };

    // Heartbeats: last_seen_at is already refreshed; skip persistence.
    if alert.is_heartbeat() {
        tracing::debug!(device_id = %cfg.id, "heartbeat received");
        return Ok(());
    }

    let Some(ace) = alert.access_controller_event.as_ref() else {
        tracing::debug!(device_id = %cfg.id, "alert without AccessControllerEvent — skipped");
        return Ok(());
    };

    let captured_at = alert
        .captured_at_epoch()
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    let direction = if !ace.attendance_status.is_empty() {
        direction_for_attendance_status(&ace.attendance_status).to_string()
    } else {
        cfg.direction_default.clone()
    };

    let face_id = if ace.face_id.is_empty() {
        None
    } else {
        Some(ace.face_id.clone())
    };
    let employee_no_string = if ace.employee_no_string.is_empty() {
        None
    } else {
        Some(ace.employee_no_string.clone())
    };

    let conn = state.db.connect().map_err(anyhow::Error::from)?;
    let employee_id = events_service::lookup_employee_for_event(
        &conn,
        &cfg.id,
        face_id.as_deref(),
        employee_no_string.as_deref(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("lookup_employee_for_event failed: {}", e))?;
    let is_unknown = employee_id.is_none();

    let new_event = NewAttendanceEvent {
        id: uuid::Uuid::new_v4().to_string(),
        employee_id,
        device_id: cfg.id.clone(),
        direction,
        captured_at,
        is_unknown,
        face_id,
        employee_no_string,
        raw_xml,
        photo_bytes: jpeg_bytes.map(|b| b.to_vec()),
    };

    // Retain fields we need for the Phase 3 recompute publish BEFORE consuming
    // `new_event` into `persist_attendance_event` (which takes it by value).
    let recompute_snapshot = crate::events::models::NewAttendanceEvent {
        id: new_event.id.clone(),
        employee_id: new_event.employee_id.clone(),
        device_id: new_event.device_id.clone(),
        direction: new_event.direction.clone(),
        captured_at: new_event.captured_at,
        is_unknown: new_event.is_unknown,
        face_id: new_event.face_id.clone(),
        employee_no_string: new_event.employee_no_string.clone(),
        raw_xml: String::new(), // not needed for recompute publish
        photo_bytes: None,
    };

    match events_service::persist_attendance_event_queued(&state, &state.paths.events_root, new_event).await {
        Ok(PersistOutcome::Inserted { photo_path }) => {
            tracing::info!(
                device_id = %cfg.id,
                photo_path = ?photo_path,
                "event persisted"
            );
            // Phase 3 D-02: publish recompute AFTER successful insert.
            // publish_recompute_if_employee guards on employee_id.is_some()
            // and on state.recompute_tx.is_some() so unknown-face events and
            // test setups without a worker are silently skipped.
            events_service::publish_recompute_if_employee(state, &recompute_snapshot);
            // Phase 4: broadcast to SSE stream clients (non-fatal if no subscribers).
            events_service::publish_sse_event(state, &recompute_snapshot, &photo_path);
        }
        Ok(PersistOutcome::Deduplicated) => {
            tracing::debug!(device_id = %cfg.id, "event deduplicated");
        }
        Err(e) => {
            return Err(anyhow::anyhow!("persist_attendance_event failed: {}", e));
        }
    }

    Ok(())
}
