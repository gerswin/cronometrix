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
use chrono::Offset;
use diqwest::WithDigestAuth;
use reqwest::Client;

use crate::state::AppState;
use crate::supervisor::status::{touch_last_seen, update_connection_state};

use super::client::{posix_time_zone, DeviceConnection};
use super::ingest::ingest_alert;

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
    /// `stream` or `push`. Only push-mode devices get a webhook configured.
    pub ingest_mode: String,
    /// Secret for this device's webhook path. Redacted from Debug.
    pub push_token: Option<String>,
}

impl std::fmt::Debug for DeviceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceConfig")
            .field("id", &self.id)
            .field("base_url", &self.base_url)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("push_token", &"[redacted]")
            .field("ingest_mode", &self.ingest_mode)
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
        anyhow::bail!("expected multipart/* content-type, got {}", content_type);
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

    // Bring the reader to the configuration this product needs before consuming
    // any event. Done on every (re)connect rather than once at registration so a
    // replaced or factory-reset unit converges without anyone touching its web
    // UI. Best effort throughout: a device that refuses a write still streams
    // usable events.
    provision_device(cfg, state).await;

    let stream = resp.bytes_stream();
    let constraints = multer::Constraints::new().size_limit(
        multer::SizeLimit::new()
            .per_field(PER_FIELD_LIMIT)
            .whole_stream(STREAM_WHOLE_LIMIT),
    );
    let mut mp = multer::Multipart::with_constraints(stream, boundary, constraints);

    let mut pending_alert: Option<PendingAlert> = None;

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

        let is_alert = ct.starts_with("application/xml")
            || ct.starts_with("application/json")
            || bytes.starts_with(b"<EventNotificationAlert")
            || bytes.first() == Some(&b'{');

        if is_alert {
            // Commit any pending alert with no JPEG (Pitfall 2: some events
            // carry no attachment — and this firmware never sends one).
            if let Some(pending) = pending_alert.take() {
                ingest_pair(state, cfg, pending, None).await?;
            }
            let raw = std::str::from_utf8(&bytes).unwrap_or_default().to_string();
            pending_alert = Some(PendingAlert {
                bytes,
                raw,
                content_type: ct,
            });
        } else if ct.starts_with("image/jpeg") || bytes.starts_with(b"\xFF\xD8\xFF") {
            if let Some(pending) = pending_alert.take() {
                ingest_pair(state, cfg, pending, Some(bytes)).await?;
            }
            // Orphan JPEG (no preceding alert) — drop.
        } else {
            tracing::debug!(
                device_id = %cfg.id,
                content_type = %ct,
                "alertStream part with unknown Content-Type — ignoring"
            );
        }
    }

    // Flush any pending alert on clean end-of-stream.
    if let Some(pending) = pending_alert.take() {
        ingest_pair(state, cfg, pending, None).await?;
    }
    Ok(())
}

/// Attendance mode. `manualAndAuto` labels a marking from the week plan and lets
/// a person override it with a function key on an atypical day. `manual` alone
/// puts the burden on
/// every employee twice a day and fails silently when somebody forgets; `auto`
/// alone offers no escape hatch for early departures or overtime.
const ATTENDANCE_MODE: &str = "manualAndAuto";

/// Midpoint that splits arrivals from departures for unattended markings.
///
/// Deliberately coarse and deliberately NOT the organisation's shift: see
/// [`DeviceConnection::set_attendance_week_plan`]. Sites running night shifts
/// will eventually need this per device rather than as a constant.
const ATTENDANCE_DAY_SPLIT: &str = "13:00:00";

const ATTENDANCE_WEEK_PLAN_NO: u8 = 1;
const ATTENDANCE_TEMPLATE_NO: u8 = 1;

/// Function keys, in the order the reader displays them.
const ATTENDANCE_KEYS: [(u8, &str, &str); 6] = [
    (1, "checkIn", "Check In"),
    (2, "checkOut", "Check Out"),
    (3, "breakOut", "Break Out"),
    (4, "breakIn", "Break In"),
    (5, "overtimeIn", "Overtime In"),
    (6, "overtimeOut", "Overtime Out"),
];

/// Apply every device-side setting this product depends on.
///
/// Each step is independent and non-fatal: losing the attendance config is bad
/// (markings arrive without a direction) but losing the event stream is worse,
/// so nothing here is allowed to abort the connection.
pub(crate) async fn provision_device(cfg: &DeviceConfig, state: &AppState) {
    sync_device_clock(cfg, state).await;

    let conn = match DeviceConnection::new(
        &cfg.base_url,
        &cfg.username,
        &cfg.password,
        cfg.allow_insecure_tls,
    ) {
        Ok(conn) => conn,
        Err(error) => {
            tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: client build failed");
            return;
        }
    };

    let mut failures = 0usize;

    if let Err(error) = conn.set_attendance_mode(ATTENDANCE_MODE).await {
        failures += 1;
        tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: attendance mode");
    }
    for (key, status, label) in ATTENDANCE_KEYS {
        if let Err(error) = conn.set_attendance_key(key, status, label).await {
            failures += 1;
            tracing::warn!(device_id = %cfg.id, key, err = %error, "provisioning: attendance key");
        }
    }
    if let Err(error) = conn
        .set_attendance_week_plan(ATTENDANCE_WEEK_PLAN_NO, ATTENDANCE_DAY_SPLIT)
        .await
    {
        failures += 1;
        tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: week plan");
    }
    if let Err(error) = conn
        .set_attendance_plan_template(ATTENDANCE_TEMPLATE_NO, ATTENDANCE_WEEK_PLAN_NO)
        .await
    {
        failures += 1;
        tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: plan template");
    }
    if let Err(error) = conn.set_capture_upload(true).await {
        failures += 1;
        tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: capture upload");
    }
    if cfg.ingest_mode == "push" {
        if let Err(error) = provision_webhook(&conn, cfg, state).await {
            failures += 1;
            tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: event webhook");
        }
    }

    if failures == 0 {
        tracing::info!(
            device_id = %cfg.id,
            mode = ATTENDANCE_MODE,
            split = ATTENDANCE_DAY_SPLIT,
            "device attendance configuration applied"
        );
    } else {
        tracing::warn!(
            device_id = %cfg.id,
            failures,
            "device attendance configuration incomplete — markings may lack a direction"
        );
    }
}

/// Point the reader's event webhook at this backend.
///
/// Replaces the device's whole host list, which is how a stale third-party
/// endpoint gets removed: a reader was found in the field posting every
/// marking, name and face to a public Pipedream URL somebody had left behind.
///
/// Requires `CRONOMETRIX_DEVICE_PUSH_BASE_URL`, because a bind address is not a
/// destination — `SERVER_HOST` is usually `0.0.0.0`, which the device cannot
/// dial. Without it the webhook is left untouched rather than pointed at a
/// guess.
async fn provision_webhook(
    conn: &DeviceConnection,
    cfg: &DeviceConfig,
    state: &AppState,
) -> anyhow::Result<()> {
    let base = state.config.device_push_base_url.trim();
    anyhow::ensure!(
        !base.is_empty(),
        "CRONOMETRIX_DEVICE_PUSH_BASE_URL is unset; refusing to guess a webhook address"
    );
    let token = cfg
        .push_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow::anyhow!("device is in push mode but has no push_token"))?;

    let parsed = reqwest::Url::parse(base)
        .map_err(|error| anyhow::anyhow!("invalid device push base URL '{base}': {error}"))?;
    let use_https = parsed.scheme().eq_ignore_ascii_case("https");
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("device push base URL has no host"))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("device push base URL has no port"))?;

    let path = format!("/api/v1/devices/{}/push/{}", cfg.id, token);
    anyhow::ensure!(
        path.len() <= 128,
        "webhook path is {} chars; firmware caps `url` at 128",
        path.len()
    );

    // ORDER IS LOAD-BEARING: clear the spare slot BEFORE writing ours. Writing
    // slot 2 makes this firmware recompact the list — it empties slot 1 and
    // shifts our entry down, dropping `ipAddress` and `portNo` on the way, while
    // still answering `statusCode 1`. Doing it first leaves nothing to shift.
    //
    // Clearing at all is what evicts a stale third-party endpoint: a reader was
    // found in the field posting every marking, name and face to a public
    // Pipedream URL somebody had left behind.
    if let Err(error) = conn.clear_event_http_host(2).await {
        tracing::warn!(device_id = %cfg.id, err = %error, "could not clear the spare webhook slot");
    }
    conn.set_event_http_host(&host, port, &path, use_https)
        .await?;
    tracing::info!(
        device_id = %cfg.id,
        host = %host,
        port,
        "event webhook pointed at this backend"
    );
    Ok(())
}

/// Push the server's wall clock onto the device, in the operator's timezone.
///
/// Non-fatal by design: a failure here must not cost us the event stream, which
/// is the device's primary job. It runs on every reconnect rather than once at
/// startup so a reader that was power-cycled (and reset its clock) is corrected
/// without operator action.
async fn sync_device_clock(cfg: &DeviceConfig, state: &AppState) {
    let now = chrono::Utc::now().with_timezone(&state.config.timezone);
    let local_time = now.format("%Y-%m-%dT%H:%M:%S").to_string();
    let time_zone = posix_time_zone(now.offset().fix().local_minus_utc());

    let conn = match DeviceConnection::new(
        &cfg.base_url,
        &cfg.username,
        &cfg.password,
        cfg.allow_insecure_tls,
    ) {
        Ok(conn) => conn,
        Err(error) => {
            tracing::warn!(device_id = %cfg.id, err = %error, "clock sync: client build failed");
            return;
        }
    };

    match conn.set_time(&local_time, &time_zone).await {
        Ok(_) => tracing::info!(
            device_id = %cfg.id,
            local_time = %local_time,
            time_zone = %time_zone,
            "device clock synchronised"
        ),
        Err(error) => tracing::warn!(
            device_id = %cfg.id,
            err = %error,
            "device clock sync failed — events may carry a skewed captured_at"
        ),
    }
}

/// An alert part held back until we know whether a JPEG part follows it.
struct PendingAlert {
    bytes: Bytes,
    /// Verbatim payload, persisted for audit. Named `raw_xml` in the schema for
    /// historical reasons; it may now hold JSON.
    raw: String,
    content_type: String,
}

/// Parse an (alert, jpeg?) pair and route it through the persist pipeline.
async fn ingest_pair(
    state: &AppState,
    cfg: &DeviceConfig,
    pending: PendingAlert,
    jpeg_bytes: Option<Bytes>,
) -> anyhow::Result<()> {
    let PendingAlert {
        bytes,
        raw,
        content_type,
    } = pending;

    ingest_alert(
        state,
        &cfg.id,
        &cfg.direction_default,
        &bytes,
        &content_type,
        raw,
        jpeg_bytes,
    )
    .await
    .map(|_| ())
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
