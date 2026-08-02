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

use crate::devices::reader::{reader_for, ProvisioningIntent};
use crate::state::AppState;
use crate::supervisor::status::{touch_last_seen, update_connection_state};

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

/// Attendance mode.
///
/// `manual` makes the person select arrival or departure on the reader before
/// authenticating, so the direction stored is the one they declared rather than
/// one inferred from a clock. Chosen deliberately over `manualAndAuto`: an
/// inferred direction is silently wrong whenever reality departs from the
/// schedule — a shift swap, an early exit, a night worker — and a wrong
/// direction is worse than a missing one, because it produces a plausible day
/// that nobody flags.
///
/// There is no silent-default failure mode: verified on hardware that the
/// reader records NOTHING when someone authenticates without selecting first.
/// A forgotten selection therefore leaves a visible gap an operator can correct,
/// never a plausible-looking row with the wrong direction — which is the
/// property that makes this mode safe for payroll.
///
/// The trade is friction: every employee selects twice a day. `manualAndAuto`
/// removes that by inferring from the week plan, at the cost of reintroducing
/// silently wrong directions whenever reality departs from the schedule.
// `pub(crate)`: also read by the `BiometricReader` impl in `isapi::client` so
// the port's provision path uses the same constant instead of a duplicate
// that could drift from this one.
pub(crate) const ATTENDANCE_MODE: &str = "manual";

/// Midpoint that splits arrivals from departures when the reader infers them.
///
/// Unused while [`ATTENDANCE_MODE`] is `manual` — the person selects instead —
/// but still provisioned so switching modes needs no visit to the device. Kept
/// deliberately coarse and deliberately NOT the organisation's shift: see
/// `isapi::client::DeviceConnection::set_attendance_week_plan`.
const ATTENDANCE_DAY_SPLIT: &str = "13:00:00";

pub(crate) const ATTENDANCE_WEEK_PLAN_NO: u8 = 1;
pub(crate) const ATTENDANCE_TEMPLATE_NO: u8 = 1;

/// Function keys, in the order the reader displays them.
pub(crate) const ATTENDANCE_KEYS: [(u8, &str, &str); 6] = [
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
/// so nothing here is allowed to abort the connection. Delegates to
/// `BiometricReader::provision`; this function's job is only to assemble the
/// vendor-neutral `ProvisioningIntent`, log the returned `ProvisionReport`, and
/// attribute each failure to the right symptom — a clock miss and a webhook
/// miss are different operator problems even though the port files both under
/// the same generic `report.failed`.
pub(crate) async fn provision_device(cfg: &DeviceConfig, state: &AppState) {
    // `.fixed_offset()` collapses the IANA zone down to what the port needs:
    // the instant plus its UTC offset. Hikvision's wire format for this
    // (POSIX `TZ`, sign inverted) is assembled inside `client.rs`, not here —
    // that is a vendor detail, not a domain one.
    let now = chrono::Utc::now()
        .with_timezone(&state.config.timezone)
        .fixed_offset();

    let reader = match reader_for(
        &cfg.base_url,
        &cfg.username,
        &cfg.password,
        cfg.allow_insecure_tls,
    ) {
        Ok(reader) => reader,
        Err(error) => {
            tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: client build failed");
            return;
        }
    };

    // `event_webhook` must stay `None` for a stream-mode device: the port
    // treats `Some` as "point the reader at this URL", and doing that to a
    // device nobody asked to push would clear its notification slots for no
    // reason. Push-mode devices get the same URL `provision_webhook` used to
    // assemble; if that isn't possible (no base URL configured, no push
    // token, or an oversized path) it's recorded as a failure here, same as
    // when this validation lived inline before the port existed — the port
    // itself never sees the field, so its report has no way to.
    let mut webhook_build_failure = false;
    let mut webhook_target: Option<WebhookTarget> = None;
    let event_webhook = if cfg.ingest_mode == "push" {
        match build_webhook_url(cfg, state) {
            Ok(target) => {
                let url = target.url.clone();
                webhook_target = Some(target);
                Some(url)
            }
            Err(error) => {
                tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: event webhook");
                webhook_build_failure = true;
                None
            }
        }
    } else {
        None
    };

    let intent = ProvisioningIntent {
        now,
        // Always true: `ATTENDANCE_MODE` is the fixed constant `"manual"`,
        // which requires the person to declare a direction on every marking.
        require_direction: true,
        day_split: ATTENDANCE_DAY_SPLIT.to_string(),
        event_webhook,
    };

    let report = match reader.provision(&intent).await {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(device_id = %cfg.id, err = %error, "provisioning: request failed");
            return;
        }
    };

    for failure in &report.failed {
        // The clock and the webhook are distinct operator-facing failure
        // modes, not interchangeable "configuration incomplete" noise: a
        // skewed clock puts wrong timestamps on payroll rows, independent of
        // direction. Restored from the pre-port `sync_device_clock` warning
        // (see docs/ARQUITECTURA-HEXAGONAL.md) — the port's `report.failed`
        // is a flat `Vec<String>` with no room for this distinction, so it is
        // reconstructed here from the `"clock: ..."` prefix `client.rs`
        // pushes.
        if let Some(detail) = failure.strip_prefix("clock: ") {
            tracing::warn!(
                device_id = %cfg.id,
                err = %detail,
                "device clock sync failed — events may carry a skewed captured_at"
            );
        } else {
            tracing::warn!(device_id = %cfg.id, %failure, "provisioning: step failed");
        }
    }
    if !report.unsupported.is_empty() {
        tracing::info!(
            device_id = %cfg.id,
            unsupported = ?report.unsupported,
            "provisioning: unsupported by this device"
        );
    }

    // Same audit-trail line `provision_webhook` used to emit directly. After
    // the incident recorded on `clear_event_http_host` — a reader found
    // posting every marking to a third-party endpoint someone had left
    // configured — knowing where a reader was actually pointed has
    // independent investigative value, separate from whether the write
    // succeeded.
    if report.applied.contains(&"event_webhook") {
        if let Some(target) = &webhook_target {
            tracing::info!(
                device_id = %cfg.id,
                host = %target.host,
                port = target.port,
                "event webhook pointed at this backend"
            );
        }
    }

    let failures = report.failed.len() + usize::from(webhook_build_failure);
    if failures == 0 {
        tracing::info!(
            device_id = %cfg.id,
            mode = ATTENDANCE_MODE,
            split = ATTENDANCE_DAY_SPLIT,
            applied = ?report.applied,
            "device attendance configuration applied"
        );
    } else {
        // Cause-neutral on purpose: the specific reason (clock, webhook, or a
        // plain config write) was already warned above with its own message,
        // so this summary must not imply a single cause like "no direction".
        tracing::warn!(
            device_id = %cfg.id,
            failures,
            "device attendance configuration incomplete — see prior warnings for cause"
        );
    }
}

/// The host/port a reader was pointed at, alongside the full URL handed to
/// the port. Kept apart from `url` so the "event webhook pointed at this
/// backend" audit line never has to parse `push_token` back out of a URL —
/// it logs only what the device dials, not the credential riding in the path.
struct WebhookTarget {
    url: String,
    host: String,
    port: u16,
}

/// Assemble the URL `ProvisioningIntent::event_webhook` needs for a push-mode
/// device — same host/port/scheme extraction and the same 128-char firmware
/// path cap `provision_webhook` used to enforce directly against the device
/// connection. The port has no notion of "device id" or "push token", only
/// "where to POST events", so this is where those get folded into the path.
///
/// Requires `CRONOMETRIX_DEVICE_PUSH_BASE_URL`, because a bind address is not a
/// destination — `SERVER_HOST` is usually `0.0.0.0`, which the device cannot
/// dial. Without it the webhook is left untouched rather than pointed at a
/// guess.
fn build_webhook_url(cfg: &DeviceConfig, state: &AppState) -> anyhow::Result<WebhookTarget> {
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
    let scheme = if parsed.scheme().eq_ignore_ascii_case("https") {
        "https"
    } else {
        "http"
    };
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

    // `push_token` is operator-supplied — nothing in this codebase mints it —
    // and unvalidated. `client.rs::parse_webhook_target` round-trips this URL
    // through `reqwest::Url::parse`, which treats an unescaped `?` as the
    // start of a query string and `#` as the start of a fragment: either one
    // silently truncates everything after it out of the path, including the
    // rest of the token. The device would then be pointed at a token-less
    // URL — every push gets rejected, the reader goes silent, and this
    // function still returns `Ok`, so provisioning logs success. Reject those
    // characters here, before the token ever reaches a URL.
    anyhow::ensure!(
        !token.contains(['?', '#']),
        "push_token contains a character ('?' or '#') a URL cannot carry through its path; rotate the token to remove it"
    );

    Ok(WebhookTarget {
        url: format!("{scheme}://{host}:{port}{path}"),
        host,
        port,
    })
}

/// An alert part held back until we know whether a JPEG part follows it.
struct PendingAlert {
    bytes: Bytes,
    /// Verbatim payload, persisted for audit.
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
