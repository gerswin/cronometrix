//! The outbound port: what Cronometrix needs a biometric reader to do.
//!
//! Application code depends on this rather than on a manufacturer's client, so
//! adding a brand means writing an adapter instead of editing every caller.
//!
//! `provision` takes an INTENT, not orders. Readers differ in what they can
//! express — schedules, function keys, picture upload — and a caller that
//! issued vendor-specific instructions would be an adapter in disguise. The
//! report exists because this hardware answers `statusCode 1` to writes it does
//! not apply: an adapter must be able to say "I could not do that" without
//! either lying or failing the whole operation.

/// A one-shot instruction with no vendor semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCommand {
    DoorOpen,
    Reboot,
    EnrollmentMode,
}

/// What the installation needs of a reader, in domain terms.
pub struct ProvisioningIntent {
    /// The moment to write to the reader's clock, carrying its own UTC
    /// offset. A single neutral value rather than a vendor-formatted string
    /// pair: Hikvision's wire format — POSIX `TZ`, sign inverted relative to
    /// the ISO offset everyone reads off a clock — is a `client.rs` detail,
    /// not a port concern. It used to leak here as `local_time`/`time_zone`
    /// strings built with `posix_time_zone`; a second adapter receiving
    /// `"CST+4:00:00"` would have had to strip a meaningless prefix and
    /// reverse Hikvision's sign convention to recover its own UTC offset,
    /// and getting the sign wrong stamps every marking 8 hours off with
    /// nothing to catch it.
    pub now: chrono::DateTime<chrono::FixedOffset>,
    /// Whether every marking must carry a direction. A reader that cannot
    /// guarantee it reports `attendance_mode` as unsupported.
    pub require_direction: bool,
    /// Midpoint splitting arrivals from departures when the reader infers them.
    pub day_split: String,
    /// Where the reader should POST events, when it is configured to push.
    ///
    /// `None` means leave the reader's notification targets alone — that is a
    /// pull-mode device, and clearing its slots would be destructive for no
    /// reason. A URL is not a vendor detail: an adapter that cannot honour one
    /// reports `event_webhook` unsupported rather than failing the whole
    /// provisioning.
    pub event_webhook: Option<String>,
}

/// What an adapter managed to apply.
///
/// Returned instead of `()` so a partially-provisioned reader is visible.
#[derive(Debug, Default)]
pub struct ProvisionReport {
    pub applied: Vec<&'static str>,
    /// Capabilities this hardware does not have. Not an error.
    pub unsupported: Vec<&'static str>,
    /// Capabilities it should have honoured and did not.
    pub failed: Vec<String>,
}

/// The outbound port, static-dispatch.
///
/// Not `#[async_trait]`: that macro boxes a future per call, and with one
/// implementor (`isapi::client::DeviceConnection`) plus `Box<dyn
/// BiometricReader>` type erasure on top, LLVM's coverage instrumentation
/// crashed grouping the generated instantiations
/// (`llvm::coverage::CoverageMapping::getInstantiationGroups`, reproduced on
/// both the pinned nightly and current nightly). Bare `async fn` in a trait
/// doesn't guarantee the returned future is `Send`, and these run inside
/// `tokio::spawn`, so each method spells out `impl Future<Output = ..> +
/// Send` instead. An `impl` block may still write `async fn` — that satisfies
/// this signature — only the trait declaration needs the explicit form.
pub trait BiometricReader: Send + Sync {
    fn provision(
        &self,
        intent: &ProvisioningIntent,
    ) -> impl std::future::Future<Output = anyhow::Result<ProvisionReport>> + Send;

    /// `person_id` is the identifier the device will report back on a marking;
    /// `display_name` is what it shows on screen. They are separate because
    /// Hikvision caps `employeeNo` at 32 chars while the name may be 128.
    fn enroll(
        &self,
        person_id: &str,
        display_name: &str,
        face: &[u8],
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    fn revoke(&self, person_id: &str) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    fn capture_face(&self) -> impl std::future::Future<Output = anyhow::Result<Vec<u8>>> + Send;

    fn send_command(
        &self,
        command: DeviceCommand,
    ) -> impl std::future::Future<Output = anyhow::Result<String>> + Send;
}

/// The adapter for one device.
///
/// An enum rather than a trait object: there is one adapter today, type
/// erasure bought nothing, and it made LLVM's coverage instrumentation crash
/// on the generated instantiations. A second brand is a variant plus a match
/// arm — which is also easier to read than a boxed trait object.
pub enum Reader {
    Hikvision(crate::isapi::client::DeviceConnection),
}

impl BiometricReader for Reader {
    async fn provision(&self, intent: &ProvisioningIntent) -> anyhow::Result<ProvisionReport> {
        match self {
            Reader::Hikvision(inner) => inner.provision(intent).await,
        }
    }

    async fn enroll(&self, person_id: &str, display_name: &str, face: &[u8]) -> anyhow::Result<()> {
        match self {
            Reader::Hikvision(inner) => inner.enroll(person_id, display_name, face).await,
        }
    }

    async fn revoke(&self, person_id: &str) -> anyhow::Result<()> {
        match self {
            Reader::Hikvision(inner) => inner.revoke(person_id).await,
        }
    }

    async fn capture_face(&self) -> anyhow::Result<Vec<u8>> {
        match self {
            Reader::Hikvision(inner) => inner.capture_face().await,
        }
    }

    async fn send_command(&self, command: DeviceCommand) -> anyhow::Result<String> {
        match self {
            Reader::Hikvision(inner) => inner.send_command(command).await,
        }
    }
}

/// Build the adapter for a device.
///
/// The only place outside `isapi/` that names a manufacturer. When
/// `devices.vendor` exists this dispatches on it; until then every reader is
/// Hikvision, and saying so in one function beats saying it in seven.
pub fn reader_for(
    base_url: &str,
    username: &str,
    password: &str,
    allow_insecure_tls: bool,
) -> anyhow::Result<Reader> {
    Ok(Reader::Hikvision(crate::isapi::client::DeviceConnection::new(
        base_url,
        username,
        password,
        allow_insecure_tls,
    )?))
}
