//! Inbound webhook for readers configured to push events at us.
//!
//! The counterpart to `isapi::stream`, which pulls. Both eventually hand their
//! payload to `isapi::ingest::ingest_alert`, so a marking is recorded
//! identically whichever transport carried it — but this handler no longer
//! calls it directly. Per C-10 part 2, `receive_push` only authorizes the
//! caller and stores the raw body in `device_push_inbox`; parsing and ingestion
//! happen out of the response path, in `workers::push_drain`, which retries
//! transient failures and dead-letters permanent ones.
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
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::AppError;
use crate::isapi::parser;
use crate::state::AppState;
use crate::supervisor::status::{touch_last_seen, update_connection_state};

/// Look up an active push-mode device whose token matches.
///
/// Returns `false` for every failure mode — unknown device, wrong token, device
/// not in push mode — so the caller answers identically in each case and cannot
/// be used to enumerate device ids.
///
/// Only proves the caller may write; `direction_default` is no longer resolved
/// here. Processing (and the device row it needs) happens later, in
/// `workers::push_drain`, which re-reads the device row from the inbox row's
/// `device_id` at drain time.
async fn authorize(state: &AppState, device_id: &str, token: &str) -> Result<bool, AppError> {
    if token.is_empty() {
        return Ok(false);
    }
    let conn = state
        .db
        .connect()
        .map_err(|error| AppError::Internal(error.into()))?;
    let mut rows = conn
        .query(
            "SELECT push_token FROM devices \
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
        return Ok(false);
    };

    let stored: Option<String> = row.get(0).map_err(|e| AppError::Internal(e.into()))?;

    Ok(match stored {
        // Constant-time compare: a byte-by-byte `==` on a secret leaks its prefix
        // through timing to anyone who can measure the response.
        Some(expected) => constant_time_eq(expected.as_bytes(), token.as_bytes()),
        None => false,
    })
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
/// Answers 200 — NOT 204 — for anything it accepts or deliberately ignores.
///
/// DS-K1T341CMFW treats only a 200 as delivery confirmation. Given a 204 it
/// re-sends the identical event forever: the queue head never advances, so
/// every later event, including the face recognitions attendance actually needs,
/// sits behind it and is never delivered. Measured on hardware — under 204 the
/// same `serialNo` arrived on four consecutive pushes; under 200 the counter
/// moved on each one.
///
/// 200 means "received and safe" — NOT "processed". Per C-10 part 2, parsing
/// and ingestion no longer happen on this response path at all: once the raw
/// body lands in `device_push_inbox`, `workers::push_drain` owns turning it
/// into an attendance event, on its own cadence, with its own retries. A
/// *processing* failure (bad XML, unknown event, business rejection) is
/// therefore invisible here by construction — it can only be observed later,
/// as a `failed` row in the inbox — because the event was already safe the
/// moment this handler stored it, and re-sending it would not help. A
/// *durability* failure (the inbox INSERT itself did not land) is the one
/// case that answers non-200 on purpose: nothing was stored, so the device
/// keeping the event and retrying is correct backpressure, not the swallowed
/// error this comment used to justify.
const ACK: StatusCode = StatusCode::OK;

pub async fn receive_push(
    State(state): State<AppState>,
    Path((device_id, token)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let authorized = authorize(&state, &device_id, &token).await?;
    if !authorized {
        tracing::warn!(device_id = %device_id, "rejected push: unknown device or bad token");
        return Ok(StatusCode::UNAUTHORIZED);
    }

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

    // C-10: guardar primero, interpretar despues. Todo lo que ocurra a partir
    // de aqui puede fallar sin perder el evento.
    let body_sha256 = format!("{:x}", Sha256::digest(body.as_ref()));
    let inbox_id = Uuid::new_v4().to_string();
    let stored = state
        .db_write
        .statement(
            "push.inbox-store",
            "INSERT INTO device_push_inbox \
               (id, device_id, content_type, body, body_sha256, received_at, status, attempts) \
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch(), 'pending', 0)",
            vec![
                libsql::Value::Text(inbox_id.clone()),
                libsql::Value::Text(device_id.clone()),
                libsql::Value::Text(content_type.clone()),
                libsql::Value::Blob(body.to_vec()),
                libsql::Value::Text(body_sha256),
            ],
        )
        .await;

    if let Err(error) = stored {
        // Fallo de DURABILIDAD, no de procesamiento. Aqui SI respondemos
        // no-200 a proposito: el device conserva el evento y reintenta, que es
        // contrapresion correcta cuando la base no puede aceptarlo. Responder
        // 200 sin haber guardado es la mentira que causo C-10.
        tracing::error!(device_id = %device_id, err = %error, "push inbox store failed");
        return Ok(StatusCode::SERVICE_UNAVAILABLE);
    }

    // C-10 part 2: no parsing, no `ingest_alert`, no device row lookup beyond
    // `authorize` above. The body is durable; `workers::push_drain` does the
    // rest off this response path.
    Ok(ACK)
}

/// Split a webhook body into `(content_type, alert, jpeg)` tuples.
///
/// The tolerance this needs — parts with no `Content-Type`, and no blank line
/// before the body — lives in `isapi::parser`, shared with the other two places
/// the firmware sends multipart. A bare (non-multipart) body is passed straight
/// through: some readers post the event JSON on its own.
///
/// `pub(crate)` — `workers::push_drain` is the only other caller. It invokes
/// this exact function rather than re-implementing the split, per the
/// hexagonal boundary: parsing stays owned here, the worker only drains.
pub(crate) fn split_payload(
    content_type: &str,
    body: &Bytes,
) -> Vec<(String, Bytes, Option<Bytes>)> {
    let Some(boundary) = parser::boundary_of(content_type) else {
        return vec![(content_type.to_string(), body.clone(), None)];
    };

    let parts = parser::split_multipart(body, &boundary);
    let alert_type = parts
        .iter()
        .find(|part| part.is_alert())
        .map(|part| part.alert_content_type().to_string());

    parser::pair_events(parts)
        .into_iter()
        .map(|pair| {
            (
                alert_type
                    .clone()
                    .unwrap_or_else(|| "application/xml".to_string()),
                pair.xml,
                pair.jpeg,
            )
        })
        .collect()
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

    /// Verbatim framing from DS-K1T341CMFW: `Content-Disposition` with no
    /// `Content-Type`, and NO blank line before the body. Requiring the RFC 7578
    /// blank line silently dropped every pushed marking while still answering
    /// 204, so the device saw success and the events simply never existed.
    #[test]
    fn split_payload_accepts_a_part_with_no_blank_line_before_the_body() {
        let body = Bytes::from(
            [
                b"--MIME_boundary\r\n".as_slice(),
                b"Content-Disposition: form-data; name=\"event_log\"\r\n",
                br#"{"eventType":"AccessControllerEvent","AccessControllerEvent":{"employeeNoString":"123"}}"#,
                b"\r\n--MIME_boundary--\r\n",
            ]
            .concat(),
        );
        let parts = split_payload("multipart/form-data; boundary=MIME_boundary", &body);
        assert_eq!(parts.len(), 1, "the part must not be discarded");
        let (part_type, alert, _) = &parts[0];
        assert!(
            alert.starts_with(b"{") && alert.ends_with(b"}"),
            "body must be extracted whole, got {:?}",
            String::from_utf8_lossy(alert)
        );
        assert!(
            !alert.windows(19).any(|w| w == b"Content-Disposition"),
            "the header must not leak into the payload"
        );
        // The part declares no Content-Type, so the format is sniffed from the
        // leading brace rather than defaulted.
        assert_eq!(part_type, "application/json");
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
        assert!(
            jpeg.ends_with(&[0xFF, 0xD9]),
            "trailing CRLF must be trimmed"
        );
    }
}
