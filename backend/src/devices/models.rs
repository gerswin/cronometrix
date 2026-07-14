use serde::{Deserialize, Serialize};
use validator::Validate;

/// DeviceResponse is the ONLY device struct ever serialised to an API response.
///
/// Per D-03 there is INTENTIONALLY NO `password` / `encrypted_password` field —
/// the password, plain or encrypted, never leaves the backend. Integration tests
/// assert that the substring "password" does not appear in response JSON bodies.
#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: i64,
    pub scheme: String, // "http" | "https"
    pub username: String,
    // NO password field here — see D-03 / threat model T-2-01.
    pub direction: String, // "entry" | "exit"
    pub allow_insecure_tls: bool,
    pub connection_state: String,     // "online" | "offline" | "unknown"
    pub last_seen_at: Option<String>, // ISO 8601
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Body for `POST /api/v1/devices`. All fields required.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateDeviceRequest {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 chars"))]
    pub name: String,
    #[validate(length(min = 1, max = 100, message = "ip must be a valid address"))]
    pub ip: String,
    #[validate(range(min = 1, max = 65535, message = "port must be 1..=65535"))]
    pub port: i64,
    #[validate(length(min = 1, max = 10))]
    pub scheme: String,
    #[validate(length(min = 1, max = 100))]
    pub username: String,
    #[validate(length(min = 1, max = 200, message = "password must be 1-200 chars"))]
    pub password: String,
    #[validate(length(min = 1, max = 10))]
    pub direction: String,
    #[serde(default)]
    pub allow_insecure_tls: bool,
}

/// Body for `PATCH /api/v1/devices/:id`. All fields optional except `version`.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDeviceRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub ip: Option<String>,
    #[validate(range(min = 1, max = 65535))]
    pub port: Option<i64>,
    pub scheme: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub username: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub password: Option<String>,
    pub direction: Option<String>,
    pub allow_insecure_tls: Option<bool>,
    /// "active" | "inactive" — mutation parity with employees for admin-driven lifecycle.
    pub status: Option<String>,
    pub version: i64,
}

/// Body for `POST /api/v1/devices/:id/commands`.
/// Per D-10, a single command value carries the verb so new commands become
/// enum variants (additive) rather than new routes.
#[derive(Debug, Deserialize, Validate)]
pub struct CommandRequest {
    #[validate(length(min = 1, max = 50))]
    pub command: String,
}

/// Response for a successful command dispatch (the device replied 2xx within 10s).
/// Timeout / error cases short-circuit via `AppError` — they are NEVER shaped as
/// a `CommandResult` so the client has a single decision point ("did the request succeed").
#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub outcome: String,         // "ok"
    pub device_response: String, // raw text/XML from the device
    pub dispatched_at: String,   // ISO 8601
    pub completed_at: String,
}

/// Query string for `GET /api/v1/devices`.
#[derive(Debug, Deserialize, Default)]
pub struct DeviceListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
    pub direction: Option<String>,
}

/// Supported commands. Decoding takes a `String` because `validator::Validate`
/// runs against the raw request body; the enum is derived by `Command::from_request_str`
/// at the handler boundary.
#[derive(Debug, Clone, Copy)]
pub enum Command {
    DoorOpen,
    Reboot,
    EnrollmentMode,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::DoorOpen => "door_open",
            Command::Reboot => "reboot",
            Command::EnrollmentMode => "enrollment_mode",
        }
    }

    /// Parse the request body `command` field. Rejects unknown values with 422.
    pub fn from_request_str(s: &str) -> Option<Self> {
        match s {
            "door_open" => Some(Command::DoorOpen),
            "reboot" => Some(Command::Reboot),
            "enrollment_mode" => Some(Command::EnrollmentMode),
            _ => None,
        }
    }
}

/// Internal-only struct carrying a plaintext password alongside the minimum
/// metadata command dispatch + supervisor tasks need. Constructed by
/// `service::get_decrypted` or `service::list_active`.
///
/// Security (RESEARCH § Security Domain rule #2):
/// - does NOT derive `Serialize` (never leaves the process)
/// - does NOT derive `Debug` — `Debug` is implemented manually below to redact the password
///
/// Plan 02-03 extension: added `name`, `direction`, `status`, `version` so
/// the supervisor can spawn tasks from a single `list_active` call without a
/// second lookup per device.
pub struct DeviceWithPlaintext {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub username: String,
    pub password: String, // plaintext — short-lived on the stack
    pub direction: String,
    pub allow_insecure_tls: bool,
    pub status: String,
    pub version: i64,
}

impl std::fmt::Debug for DeviceWithPlaintext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceWithPlaintext")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field("direction", &self.direction)
            .field("allow_insecure_tls", &self.allow_insecure_tls)
            .field("status", &self.status)
            .field("version", &self.version)
            .finish()
    }
}

/// Validate scheme is `http` or `https`.
pub fn validate_scheme(s: &str) -> Result<(), &'static str> {
    match s {
        "http" | "https" => Ok(()),
        _ => Err("scheme must be 'http' or 'https'"),
    }
}

/// Validate direction is `entry` or `exit`.
pub fn validate_direction(s: &str) -> Result<(), &'static str> {
    match s {
        "entry" | "exit" => Ok(()),
        _ => Err("direction must be 'entry' or 'exit'"),
    }
}

/// Validate IP is parseable as IPv4/IPv6. Used by service-layer checks.
pub fn validate_ip(s: &str) -> Result<(), &'static str> {
    use std::net::IpAddr;
    use std::str::FromStr;
    IpAddr::from_str(s)
        .map(|_| ())
        .map_err(|_| "ip must be a valid IPv4 or IPv6 address")
}

/// Validate status is `active` or `inactive` (for PATCH).
pub fn validate_status(s: &str) -> Result<(), &'static str> {
    match s {
        "active" | "inactive" => Ok(()),
        _ => Err("status must be 'active' or 'inactive'"),
    }
}
