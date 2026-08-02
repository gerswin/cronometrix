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

use async_trait::async_trait;

/// A one-shot instruction with no vendor semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCommand {
    DoorOpen,
    Reboot,
    EnrollmentMode,
}

/// What the installation needs of a reader, in domain terms.
pub struct ProvisioningIntent {
    /// Local wall-clock time, `%Y-%m-%dT%H:%M:%S`.
    pub local_time: String,
    /// POSIX `TZ` string. The sign is inverted relative to the ISO offset.
    pub time_zone: String,
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

#[async_trait]
pub trait BiometricReader: Send + Sync {
    async fn provision(&self, intent: &ProvisioningIntent) -> anyhow::Result<ProvisionReport>;
    /// `person_id` is the identifier the device will report back on a marking;
    /// `display_name` is what it shows on screen. They are separate because
    /// Hikvision caps `employeeNo` at 32 chars while the name may be 128.
    async fn enroll(
        &self,
        person_id: &str,
        display_name: &str,
        face: &[u8],
    ) -> anyhow::Result<()>;
    async fn revoke(&self, person_id: &str) -> anyhow::Result<()>;
    async fn capture_face(&self) -> anyhow::Result<Vec<u8>>;
    async fn execute(&self, command: DeviceCommand) -> anyhow::Result<String>;
}
