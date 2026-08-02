//! Inbound webhook for readers configured to push events at us.
//!
//! The counterpart to `isapi::stream`, which pulls. Both hand their payload to
//! `isapi::ingest::ingest_alert`, so a marking is recorded identically whichever
//! transport carried it.
//!
//! Authentication is a per-device secret in the URL path. That is not a stylistic
//! choice: the reader cannot present a JWT, and DS-K1T341CMFW leaves
//! `httpAuthenticationMethod` empty, so nothing else is available. This route is
//! therefore mounted OUTSIDE the authenticated router and must treat every field
//! as hostile — a caller who guesses the URL writes straight into payroll.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use bytes::Bytes;
use libsql::params;

use crate::errors::AppError;
use crate::isapi::ingest::{ingest_alert, IngestOutcome};
use crate::state::AppState;
use crate::supervisor::status::{touch_last_seen, update_connection_state};

/// Device identity resolved from a webhook path.
struct PushDevice {
    direction_default: String,
}

/// Look up an active push-mode device whose token matches.
///
/// Returns `None` for every failure mode — unknown device, wrong token, device
/// not in push mode — so the caller answers identically in each case and cannot
/// be used to enumerate device ids.
async fn authorize(
    state: &AppState,
    device_id: &str,
    token: &str,
) -> Result<Option<PushDevice>, AppError> {
    if token.is_empty() {
        return Ok(None);
    }
    let conn = state
        .db
        .connect()
        .map_err(|error| AppError::Internal(error.into()))?;
    let mut rows = conn
        .query(
            "SELECT direction, push_token FROM devices \
             WHERE id = ?1 AND status = 'active' AND ingest_mode = 'push'",
            params![device_id.to_string()],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?;

    let Some(row) = rows
        .next()
        .await
        .map_err(|error| AppError::Internal(error.into()))?
    else {
        return Ok(None);
    };

    let direction_default: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let stored: Option<String> = row.get(1).map_err(|e| AppError::Internal(e.into()))?;

    match stored {
        // Constant-time compare: a byte-by-byte `==` on a secret leaks its prefix
        // through timing to anyone who can measure the response.
        Some(expected) if constant_time_eq(expected.as_bytes(), token.as_bytes()) => {
            Ok(Some(PushDevice { direction_default }))
        }
        _ => Ok(None),
    }
}

/// Length-independent equality that does not short-circuit on the first
/// difference.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `POST /api/v1/devices/{device_id}/push/{token}`
///
/// Answers 204 for anything it accepts or deliberately ignores, because a reader
/// that receives an error re-queues the event and retries forever. Only an
/// unauthorized caller gets a distinct status.
pub async fn receive_push(
    State(state): State<AppState>,
    Path((device_id, token)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let Some(device) = authorize(&state, &device_id, &token).await? else {
        tracing::warn!(device_id = %device_id, "rejected push: unknown device or bad token");
        return Ok(StatusCode::UNAUTHORIZED);
    };

    // Reaching this point proves the reader is alive and reachable, which is the
    // only liveness signal a push-mode device produces — it never opens a stream
    // for the watchdog to observe.
    let _ = update_connection_state(&state, &device_id, "online").await;
    let _ = touch_last_seen(&state, &device_id).await;

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    let parts = split_payload(&content_type, &body);
    for (part_content_type, alert_bytes, jpeg) in parts {
        let raw = String::from_utf8_lossy(&alert_bytes).to_string();
        match ingest_alert(
            &state,
            &device_id,
            &device.direction_default,
            &alert_bytes,
            &part_content_type,
            raw,
            jpeg,
        )
        .await
        {
            Ok(IngestOutcome::Persisted) => {}
            Ok(_) => {}
            Err(error) => {
                // Swallow rather than 500: a persistent failure would make the
                // device retry the same event forever, and the log is where an
                // operator can actually see it.
                tracing::error!(device_id = %device_id, err = %error, "push ingest failed");
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Split a webhook body into `(content_type, alert, jpeg)` tuples.
///
/// Readers post either a bare JSON/XML alert or a multipart body pairing the
/// alert with the captured face. The multipart boundary arrives in the HTTP
/// header here, unlike `CaptureFaceData` where it is buried in the payload.
fn split_payload(content_type: &str, body: &Bytes) -> Vec<(String, Bytes, Option<Bytes>)> {
    let Some(boundary) = boundary_of(content_type) else {
        return vec![(content_type.to_string(), body.clone(), None)];
    };

    let delimiter = format!("--{boundary}");
    let mut alert: Option<(String, Bytes)> = None;
    let mut jpeg: Option<Bytes> = None;

    for segment in split_on(body, delimiter.as_bytes()) {
        let Some(header_end) = find(&segment, b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&segment[..header_end]).to_lowercase();
        let mut content = segment.slice(header_end + 4..);
        while content
            .last()
            .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b'-'))
        {
            content = content.slice(..content.len() - 1);
        }
        if content.is_empty() {
            continue;
        }

        if headers.contains("image/jpeg") || content.starts_with(&[0xFF, 0xD8, 0xFF]) {
            jpeg = Some(content);
        } else if alert.is_none() {
            let part_type = if headers.contains("json") {
                "application/json"
            } else {
                "application/xml"
            };
            alert = Some((part_type.to_string(), content));
        }
    }

    match alert {
        Some((part_type, bytes)) => vec![(part_type, bytes, jpeg)],
        None => Vec::new(),
    }
}

fn boundary_of(content_type: &str) -> Option<String> {
    let lower = content_type.to_ascii_lowercase();
    if !lower.starts_with("multipart/") {
        return None;
    }
    let index = lower.find("boundary=")?;
    let value = content_type[index + "boundary=".len()..]
        .split(';')
        .next()?
        .trim()
        .trim_matches('"')
        .to_string();
    (!value.is_empty()).then_some(value)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn split_on(body: &Bytes, delimiter: &[u8]) -> Vec<Bytes> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = find(&body[cursor..], delimiter) {
        let start = cursor + offset + delimiter.len();
        let next = find(&body[start..], delimiter).map(|n| start + n);
        out.push(body.slice(start..next.unwrap_or(body.len())));
        match next {
            Some(position) => cursor = position,
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_only_identical_secrets() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124"));
        // A shared prefix must not pass, and a length difference must not panic.
        assert!(!constant_time_eq(b"abc", b"abc123"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn split_payload_passes_through_a_bare_json_body() {
        let body = Bytes::from_static(br#"{"eventType":"AccessControllerEvent"}"#);
        let parts = split_payload("application/json", &body);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].2.is_none(), "no JPEG in a bare body");
    }

    #[test]
    fn split_payload_separates_the_alert_from_the_captured_face() {
        let body = Bytes::from(
            [
                b"--B\r\nContent-Type: application/json\r\n\r\n".as_slice(),
                br#"{"eventType":"AccessControllerEvent"}"#,
                b"\r\n--B\r\nContent-Type: image/jpeg\r\n\r\n",
                &[0xFF, 0xD8, 0xFF, 0x00, 0xFF, 0xD9],
                b"\r\n--B--\r\n",
            ]
            .concat(),
        );
        let parts = split_payload("multipart/form-data; boundary=B", &body);
        assert_eq!(parts.len(), 1);
        let (part_type, alert, jpeg) = &parts[0];
        assert_eq!(part_type, "application/json");
        assert!(alert.starts_with(b"{"));
        let jpeg = jpeg.as_ref().expect("JPEG part must be extracted");
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert!(jpeg.ends_with(&[0xFF, 0xD9]), "trailing CRLF must be trimmed");
    }
}
