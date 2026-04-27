use std::fmt;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};

/// Configuration loaded from environment variables at startup.
/// Panics if required variables (JWT_SECRET, DEVICE_CREDS_KEY) are missing or invalid.
#[derive(Clone)]
pub struct Config {
    pub database_path: String,
    pub turso_url: String,
    pub turso_token: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub turso_sync_interval_secs: u64,
    /// AES-256-GCM key for device credential encryption (D-01, D-02).
    /// 32 raw bytes, decoded from a base64 env var. Never cloned into logs.
    pub device_creds_key: [u8; 32],
    /// IANA timezone for local-date arithmetic (Phase 3 attendance engine).
    /// Parsed from `TZ` env var; defaults to `America/Caracas` (UTC-4, no DST).
    pub timezone: chrono_tz::Tz,
    /// Path to the cached license JWT file. Loaded at startup; written by
    /// activate_license. Defaults to /opt/cronometrix/data/license.jwt to
    /// match the installer's INSTALL_DIR convention.
    pub license_jwt_path: String,
    /// DO Functions URL for license activation (POST). Empty string means
    /// activation is not configured — handler returns 502 ACTIVATION_UNREACHABLE.
    pub do_functions_activate_url: String,
    /// DO Functions URL for license renewal (POST). Empty string means
    /// renewal is disabled (offline-only deployment).
    pub do_functions_renew_url: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Manually redact secrets. Per D-02 + RESEARCH § Security Domain rule #1,
        // DEVICE_CREDS_KEY must NEVER be printable and JWT_SECRET should not leak either.
        f.debug_struct("Config")
            .field("database_path", &self.database_path)
            .field("turso_url", &self.turso_url)
            .field("turso_token", &"[redacted]")
            .field("jwt_secret", &"[redacted]")
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("turso_sync_interval_secs", &self.turso_sync_interval_secs)
            .field("device_creds_key", &"[redacted 32 bytes]")
            .field("timezone", &self.timezone.name())
            .field("license_jwt_path", &self.license_jwt_path)
            .field("do_functions_activate_url", &self.do_functions_activate_url)
            .field("do_functions_renew_url", &self.do_functions_renew_url)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_path = std::env::var("CRONOMETRIX_DB_PATH")
            .unwrap_or_else(|_| "cronometrix.db".to_string());

        let turso_url = std::env::var("TURSO_DATABASE_URL").unwrap_or_default();
        let turso_token = std::env::var("TURSO_AUTH_TOKEN").unwrap_or_default();

        let jwt_secret = std::env::var("JWT_SECRET")
            .context("JWT_SECRET environment variable is required")?;

        if jwt_secret.len() < 32 {
            anyhow::bail!(
                "JWT_SECRET must be at least 32 characters long (got {})",
                jwt_secret.len()
            );
        }

        let server_host = std::env::var("SERVER_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse::<u16>()
            .context("SERVER_PORT must be a valid port number")?;

        let turso_sync_interval_secs = std::env::var("TURSO_SYNC_INTERVAL")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .context("TURSO_SYNC_INTERVAL must be a valid number of seconds")?;

        let device_creds_key = load_device_creds_key()?;

        let license_jwt_path = std::env::var("LICENSE_JWT_PATH")
            .unwrap_or_else(|_| "/opt/cronometrix/data/license.jwt".to_string());
        let do_functions_activate_url = std::env::var("DO_FUNCTIONS_ACTIVATE_URL")
            .unwrap_or_default();
        let do_functions_renew_url = std::env::var("DO_FUNCTIONS_RENEW_URL")
            .unwrap_or_default();

        // Phase 3: timezone used by the attendance engine + nightly reconcile
        // cron. Target market Venezuela does not observe DST since May 2016, so
        // `America/Caracas` gives unambiguous local-date arithmetic. Any future
        // DST-observing market plugs in by setting TZ.
        let tz_str = std::env::var("TZ").unwrap_or_else(|_| "America/Caracas".to_string());
        let timezone = tz_str.parse::<chrono_tz::Tz>().map_err(|_| {
            anyhow::anyhow!("Unknown IANA timezone in TZ env var: {}", tz_str)
        })?;

        Ok(Config {
            database_path,
            turso_url,
            turso_token,
            jwt_secret,
            server_host,
            server_port,
            turso_sync_interval_secs,
            device_creds_key,
            timezone,
            license_jwt_path,
            do_functions_activate_url,
            do_functions_renew_url,
        })
    }

    /// Returns true if Turso remote sync is configured (URL is non-empty).
    pub fn has_turso(&self) -> bool {
        !self.turso_url.is_empty()
    }
}

/// Load and validate DEVICE_CREDS_KEY from the environment.
///
/// Requirements (D-02):
/// - variable set
/// - decodes from base64
/// - decoded length is EXACTLY 32 bytes (AES-256 key size)
///
/// NOT reused from JWT_SECRET — must be its own variable so compromise of one
/// does not auto-compromise the other.
fn load_device_creds_key() -> Result<[u8; 32]> {
    let raw = std::env::var("DEVICE_CREDS_KEY")
        .context("DEVICE_CREDS_KEY environment variable is required")?;

    let decoded = STANDARD
        .decode(raw.as_bytes())
        .context("DEVICE_CREDS_KEY must be valid base64")?;

    decoded
        .as_slice()
        .try_into()
        .map_err(|_| {
            anyhow::anyhow!(
                "DEVICE_CREDS_KEY must decode to exactly 32 bytes (got {} bytes)",
                decoded.len()
            )
        })
}
