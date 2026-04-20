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
