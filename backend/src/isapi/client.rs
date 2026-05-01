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
        let url = format!(
            "{}/ISAPI/AccessControl/RemoteControl/door/1",
            self.base_url
        );
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
    /// JSON body per Hikvision docs; the device replies with `<ResponseStatus>`.
    pub async fn enrollment_mode(&self) -> Result<String> {
        let url = format!("{}/ISAPI/AccessControl/CaptureFaceData", self.base_url);
        let body = r#"{"CaptureInfo":{"captureInfrared":true}}"#;
        self.send_json(&url, reqwest::Method::POST, body).await
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
            let text = resp.text().await.context("read ISAPI FaceDataRecord response")?;
            anyhow::ensure!(
                status.is_success(),
                "device returned non-success status {status}: {text}"
            );
            return Ok(text);
        }

        // 401 path: compute digest auth and retry with fresh form.
        let www_auth = resp
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let path = format!("/ISAPI/Intelligent/FDLib/FaceDataRecord?format=json");
        let context = digest_auth::AuthContext::new_with_method(
            &self.username,
            &self.password,
            &path,
            None::<&[u8]>, // body bytes not used for multipart digest
            digest_auth::HttpMethod::POST,
        );
        let mut prompt = digest_auth::parse(&www_auth).context("parse WWW-Authenticate")?;
        let auth_header = prompt.respond(&context).context("compute digest auth")?;

        let resp2 = self
            .client
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, auth_header.to_header_string())
            .multipart(build_multipart_form(face_id, jpeg_bytes)?)
            .send()
            .await
            .context("ISAPI FaceDataRecord digest-auth request failed")?;

        let status = resp2.status();
        let text = resp2.text().await.context("read ISAPI FaceDataRecord response")?;
        anyhow::ensure!(
            status.is_success(),
            "device returned non-success status {status}: {text}"
        );
        Ok(text)
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
    /// Step 1: `POST /ISAPI/AccessControl/CaptureFaceData` (existing `enrollment_mode`)
    ///          puts the device into live-capture mode.
    /// Step 2: `GET /ISAPI/AccessControl/CapturedFacePicture` retrieves the JPEG.
    ///
    /// NOTE: RESEARCH assumption A1 — the GET path is the most-cited convention
    /// for this device family. Adjust if hardware smoke tests reveal a different path.
    pub async fn capture_face_image(&self) -> Result<Vec<u8>> {
        // Step 1: enter enrollment (capture) mode.
        self.enrollment_mode().await?;

        // Step 2: retrieve the captured picture bytes.
        let url = format!(
            "{}/ISAPI/AccessControl/CapturedFacePicture",
            self.base_url
        );
        use diqwest::WithDigestAuth;
        let resp = self
            .client
            .get(&url)
            .send_digest_auth((self.username.as_str(), self.password.as_str()))
            .await
            .context("ISAPI CapturedFacePicture request failed")?;

        let status = resp.status();
        anyhow::ensure!(
            status.is_success(),
            "device returned non-success status {status} on CapturedFacePicture"
        );
        let bytes = resp.bytes().await.context("read CapturedFacePicture body")?;
        Ok(bytes.to_vec())
    }

    async fn send_xml(
        &self,
        url: &str,
        method: reqwest::Method,
        body: &str,
    ) -> Result<String> {
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
        anyhow::ensure!(
            status.is_success(),
            "device returned non-success status {status}: {text}"
        );
        Ok(text)
    }

    async fn send_json(
        &self,
        url: &str,
        method: reqwest::Method,
        body: &str,
    ) -> Result<String> {
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
        anyhow::ensure!(
            status.is_success(),
            "device returned non-success status {status}: {text}"
        );
        Ok(text)
    }
}
