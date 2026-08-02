//! Outbound ISAPI HTTP client (D-09, D-10).
//!
//! One-shot requests to a single device (door open, reboot, enrollment mode).
//! The alertStream consumer in plan 02-02 uses a separate streaming client.
//!
//! Password handling: the `DeviceConnection` owns the plaintext password by value
//! (decrypted on the command dispatch call site). We implement `Debug` manually to
//! scrub it — a `tracing::debug!(device = ?conn)` call MUST NOT leak the password.

use std::time::Duration;

use anyhow::{Context, Result};
use diqwest::WithDigestAuth;
use reqwest::Client;

/// A device completed the HTTP exchange and explicitly rejected the command.
///
/// This is distinct from transport failures and timeouts, whose device-side
/// outcome is ambiguous. Enrollment retries may safely replay this error class
/// because the Hikvision user/face operations are idempotent.
#[derive(Debug, thiserror::Error)]
#[error("device returned non-success status {status}: {body}")]
pub(crate) struct DeviceResponseError {
    status: reqwest::StatusCode,
    body: String,
}

/// Request body shared by `enrollment_mode` and `capture_face_image` — both open
/// the same device-side capture window. `dataType` is required and `binary` is
/// its only accepted value (see `CaptureFaceData/capabilities`).
const CAPTURE_FACE_DATA_BODY: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<CaptureFaceDataCond version="2.0" xmlns="http://www.isapi.org/ver20/XMLSchema">"#,
    r#"<captureInfrared>true</captureInfrared><dataType>binary</dataType>"#,
    r#"</CaptureFaceDataCond>"#
);

/// One capture window is ~6s; a cooperating subject needed 3 windows on hardware.
/// Eight attempts spaced 3s apart bound the wait at roughly 70s.
const CAPTURE_MAX_ATTEMPTS: usize = 8;
/// Spacing between windows. Firing sooner collides with the still-closing window
/// and earns `400 / deviceBusy` — measured: 2s still collides, 3s does not.
const CAPTURE_RETRY_DELAY: Duration = Duration::from_secs(3);

/// Locate `needle` inside `haystack`, returning its start offset.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Pull the `FaceData` JPEG out of a successful `CaptureFaceData` response.
///
/// The device does NOT advertise the boundary in the HTTP `Content-Type` header:
/// the body itself opens with its own `Content-Type: multipart/form-data;
/// boundary=...` line, so the boundary must be read out of the payload. Returns
/// `None` for the plain-XML `captureProgress < 100` response, which carries no
/// image part.
fn extract_face_jpeg(body: &[u8]) -> Option<Vec<u8>> {
    // The boundary declaration lives in the first line; bound the scan so a large
    // JPEG never gets lossily converted just to find it.
    let head = String::from_utf8_lossy(&body[..body.len().min(512)]);
    let boundary = head
        .find("boundary=")
        .map(|idx| &head[idx + "boundary=".len()..])?
        .split(['\r', '\n', ';'])
        .next()?
        .trim()
        .trim_matches('"')
        .to_string();
    if boundary.is_empty() {
        return None;
    }

    let delimiter = format!("--{boundary}");
    let mut cursor = 0usize;
    while let Some(offset) = find_subslice(&body[cursor..], delimiter.as_bytes()) {
        let part_start = cursor + offset + delimiter.len();
        cursor = part_start;
        let rest = &body[part_start..];
        // Part headers end at the first blank line.
        let Some(header_end) = find_subslice(rest, b"\r\n\r\n") else {
            break;
        };
        let headers = String::from_utf8_lossy(&rest[..header_end]);
        let content_start = header_end + 4;
        let Some(next) = find_subslice(&rest[content_start..], delimiter.as_bytes()) else {
            continue;
        };
        if !headers.contains("FaceData") && !headers.contains("image/jpeg") {
            continue;
        }
        // The delimiter is preceded by the CRLF that terminates the part body.
        let mut content_end = content_start + next;
        while content_end > content_start && matches!(body[part_start + content_end - 1], b'\r' | b'\n')
        {
            content_end -= 1;
        }
        return Some(rest[content_start..content_end].to_vec());
    }
    None
}

fn accepted_response(status: reqwest::StatusCode, body: String) -> Result<String> {
    if status.is_success() {
        Ok(body)
    } else {
        Err(DeviceResponseError { status, body }.into())
    }
}

/// Timeouts:
/// - `connect_timeout(5s)` — TCP establish; devices unreachable should fail fast.
/// - `timeout(30s)` — full request ceiling; plan 02-01 handlers wrap this in a
///   further `tokio::time::timeout(10s)` per D-09 so the hard user-visible deadline
///   is 10s and this 30s only exists as a safety net if tokio's timer is starved.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct DeviceConnection {
    pub client: Client,
    pub base_url: String,
    pub username: String,
    /// Plaintext — decrypted on the stack by the caller. Never logged.
    password: String,
}

impl std::fmt::Debug for DeviceConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceConnection")
            .field("base_url", &self.base_url)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .finish()
    }
}

impl DeviceConnection {
    pub fn new(
        base_url: &str,
        username: &str,
        password: &str,
        allow_insecure_tls: bool,
    ) -> Result<Self> {
        // Pitfall 3 (RESEARCH): Hikvision devices ship self-signed certs. We default
        // to strict TLS; the per-device `allow_insecure_tls` flag opts in to
        // `danger_accept_invalid_certs(true)`.
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .danger_accept_invalid_certs(allow_insecure_tls)
            .build()
            .context("build reqwest Client for ISAPI")?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    /// `PUT /ISAPI/AccessControl/RemoteControl/door/1` — open the door for N seconds
    /// (the XML body `<cmd>open</cmd>` instructs the device to unlock briefly).
    pub async fn door_open(&self) -> Result<String> {
        let url = format!("{}/ISAPI/AccessControl/RemoteControl/door/1", self.base_url);
        let body = r#"<RemoteControlDoor><cmd>open</cmd></RemoteControlDoor>"#;
        self.send_xml(&url, reqwest::Method::PUT, body).await
    }

    /// `PUT /ISAPI/System/reboot` — request a device reboot. Device typically
    /// 200s immediately then drops; the caller's 10s `tokio::time::timeout`
    /// absorbs any lag.
    pub async fn reboot(&self) -> Result<String> {
        let url = format!("{}/ISAPI/System/reboot", self.base_url);
        // Some firmware accepts empty body on PUT; others want an empty XML root.
        // Empty string is the lowest-common-denominator and matches the ISAPI spec
        // for simple control commands.
        self.send_xml(&url, reqwest::Method::PUT, "").await
    }

    /// `POST /ISAPI/AccessControl/CaptureFaceData` — enter enrollment mode.
    ///
    /// XML body, verified against DS-K1T341CMFW firmware V3.3.8. The device's
    /// `CaptureFaceData/capabilities` advertises a `<CaptureFaceDataCond>` root
    /// with `captureInfrared` (`true,false`) and a REQUIRED `dataType` whose
    /// only accepted value is `binary`. A JSON body is rejected with
    /// `statusCode 6 / Invalid Content / badParameters` — with or without
    /// `?format=json`, which this endpoint ignores. On success the device
    /// replies `<CaptureFaceData><captureProgress>0</captureProgress></CaptureFaceData>`.
    pub async fn enrollment_mode(&self) -> Result<String> {
        let url = format!("{}/ISAPI/AccessControl/CaptureFaceData", self.base_url);
        self.send_xml(&url, reqwest::Method::POST, CAPTURE_FACE_DATA_BODY)
            .await
    }

    // =========================================================================
    // Phase 7 — facial enrollment methods (D-12 LOCKED, D-15 LOCKED)
    // =========================================================================

    /// Step 1 of the 2-step face profile push (D-12 LOCKED).
    ///
    /// `POST /ISAPI/AccessControl/UserInfo/Record?format=json`
    ///
    /// Creates or replaces the person record on the device.  Hikvision may
    /// return `subStatusCode: "duplicateEmployeeNo"` if the employee is already
    /// registered — that is treated as success (idempotent upsert behaviour).
    pub async fn upsert_user(&self, face_id: &str, full_name: &str) -> Result<String> {
        use crate::enrollments::isapi_face::build_user_info_record_body;

        let url = format!(
            "{}/ISAPI/AccessControl/UserInfo/Record?format=json",
            self.base_url
        );
        let body = build_user_info_record_body(face_id, full_name);

        match self.send_json(&url, reqwest::Method::POST, &body).await {
            Ok(text) => {
                // Treat duplicate as success — device already has this person.
                if text.contains("duplicateEmployeeNo") {
                    tracing::warn!(
                        face_id = %face_id,
                        "device reports duplicateEmployeeNo — treating as success"
                    );
                }
                Ok(text)
            }
            Err(e) => Err(e),
        }
    }

    /// Step 2 of the 2-step face profile push (D-12 LOCKED).
    ///
    /// `POST /ISAPI/Intelligent/FDLib/FaceDataRecord?format=json`
    ///
    /// Multipart form with two parts:
    ///   - "FaceDataRecord" (application/json): metadata JSON (faceLibType, FDID, FPID)
    ///   - "FaceImage"      (image/jpeg):       raw JPEG bytes ≤200KB after normalisation
    ///
    /// NOTE: diqwest cannot clone a multipart RequestBuilder (the body is a stream),
    /// so we implement a manual two-step digest auth flow here:
    ///   1. Send without auth.  If the device returns 200, return it directly.
    ///   2. If 401: parse WWW-Authenticate, compute digest auth header, resend with it.
    pub async fn upload_face(&self, face_id: &str, jpeg_bytes: Vec<u8>) -> Result<String> {
        use crate::enrollments::isapi_face::build_multipart_form;

        let url = format!(
            "{}/ISAPI/Intelligent/FDLib/FaceDataRecord?format=json",
            self.base_url
        );

        // First attempt — no auth header.
        let resp = self
            .client
            .post(&url)
            .multipart(build_multipart_form(face_id, jpeg_bytes.clone())?)
            .send()
            .await
            .context("ISAPI FaceDataRecord first request failed")?;

        if resp.status() != reqwest::StatusCode::UNAUTHORIZED {
            let status = resp.status();
            let text = resp
                .text()
                .await
                .context("read ISAPI FaceDataRecord response")?;
            return accepted_response(status, text);
        }

        // 401 path: compute digest auth and retry with fresh form.
        let www_auth = resp
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let rejected_auth = |detail: &str| DeviceResponseError {
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: format!("device rejected digest authentication: {detail}"),
        };
        if www_auth.is_empty() {
            return Err(rejected_auth("missing WWW-Authenticate challenge").into());
        }

        let path = "/ISAPI/Intelligent/FDLib/FaceDataRecord?format=json".to_string();
        let context = digest_auth::AuthContext::new_with_method(
            &self.username,
            &self.password,
            &path,
            None::<&[u8]>, // body bytes not used for multipart digest
            digest_auth::HttpMethod::POST,
        );
        let mut prompt = digest_auth::parse(&www_auth)
            .map_err(|error| rejected_auth(&format!("invalid challenge: {error}")))?;
        let auth_header = prompt
            .respond(&context)
            .map_err(|error| rejected_auth(&format!("challenge response failed: {error}")))?;

        let resp2 = self
            .client
            .post(&url)
            .header(
                reqwest::header::AUTHORIZATION,
                auth_header.to_header_string(),
            )
            .multipart(build_multipart_form(face_id, jpeg_bytes)?)
            .send()
            .await
            .context("ISAPI FaceDataRecord digest-auth request failed")?;

        let status = resp2.status();
        let text = resp2
            .text()
            .await
            .context("read ISAPI FaceDataRecord response")?;
        accepted_response(status, text)
    }

    /// Delete a person record from the device (D-15 LOCKED).
    ///
    /// `PUT /ISAPI/AccessControl/UserInfoDetail/Delete?format=json`
    ///
    /// Uses `mode: byEmployeeNo` with the face_id as the Hikvision employeeNo.
    pub async fn delete_user(&self, face_id: &str) -> Result<String> {
        use crate::enrollments::isapi_face::build_user_delete_body;

        let url = format!(
            "{}/ISAPI/AccessControl/UserInfoDetail/Delete?format=json",
            self.base_url
        );
        let body = build_user_delete_body(face_id);
        self.send_json(&url, reqwest::Method::PUT, &body).await
    }

    /// Trigger a device-side face capture and retrieve the captured JPEG bytes
    /// (D-02 LOCKED — kiosk mode step).
    ///
    /// There is NO second endpoint. `GET /ISAPI/AccessControl/CapturedFacePicture`
    /// — the path this used to call — answers `404 / statusCode 4 / notSupport` on
    /// DS-K1T341CMFW firmware V3.3.8. With `dataType=binary` the image comes back
    /// inline on the same `CaptureFaceData` POST that opens the capture window.
    ///
    /// Each POST opens a capture window of roughly 6 seconds and then answers:
    ///
    /// - `200` + a 131-byte `<captureProgress>0</captureProgress>` — nobody was in
    ///   front of the reader; retry.
    /// - `200` + a multipart payload whose XML part reads `captureProgress=100` and
    ///   whose `FaceData` part is the JPEG — done.
    /// - `400` + `subStatusCode deviceBusy` — a previous window is still open;
    ///   back off and retry, do NOT treat as fatal.
    ///
    /// Observed on hardware: a cooperating subject took 3 attempts (~27s).
    pub async fn capture_face_image(&self) -> Result<Vec<u8>> {
        use diqwest::WithDigestAuth;

        let url = format!("{}/ISAPI/AccessControl/CaptureFaceData", self.base_url);
        let mut last_progress: Option<String> = None;

        for attempt in 1..=CAPTURE_MAX_ATTEMPTS {
            let resp = self
                .client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/xml")
                .body(CAPTURE_FACE_DATA_BODY)
                .send_digest_auth((self.username.as_str(), self.password.as_str()))
                .await
                .context("ISAPI CaptureFaceData request failed")?;

            let status = resp.status();
            let bytes = resp.bytes().await.context("read CaptureFaceData body")?;

            if status.is_success() {
                if let Some(jpeg) = extract_face_jpeg(&bytes) {
                    return Ok(jpeg);
                }
                // 200 without an image part means captureProgress < 100.
                last_progress = Some(String::from_utf8_lossy(&bytes).trim().to_string());
            } else {
                let body = String::from_utf8_lossy(&bytes).to_string();
                // `deviceBusy` is our own previous window still closing — transient.
                if !body.contains("deviceBusy") {
                    return Err(DeviceResponseError { status, body }.into());
                }
            }

            if attempt < CAPTURE_MAX_ATTEMPTS {
                tokio::time::sleep(CAPTURE_RETRY_DELAY).await;
            }
        }

        anyhow::bail!(
            "no face captured after {} attempts; last device response: {}",
            CAPTURE_MAX_ATTEMPTS,
            last_progress.as_deref().unwrap_or("device busy on every attempt")
        )
    }

    async fn send_xml(&self, url: &str, method: reqwest::Method, body: &str) -> Result<String> {
        let resp = self
            .client
            .request(method, url)
            .header(reqwest::header::CONTENT_TYPE, "application/xml")
            .body(body.to_string())
            .send_digest_auth((self.username.as_str(), self.password.as_str()))
            .await
            .context("ISAPI request failed")?;
        let status = resp.status();
        let text = resp.text().await.context("read ISAPI response body")?;
        accepted_response(status, text)
    }

    async fn send_json(&self, url: &str, method: reqwest::Method, body: &str) -> Result<String> {
        let resp = self
            .client
            .request(method, url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send_digest_auth((self.username.as_str(), self.password.as_str()))
            .await
            .context("ISAPI request failed")?;
        let status = resp.status();
        let text = resp.text().await.context("read ISAPI response body")?;
        accepted_response(status, text)
    }
}
