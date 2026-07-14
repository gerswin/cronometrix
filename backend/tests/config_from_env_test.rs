//! Unit tests for `config::Config::from_env`. Targets the 0% baseline gap
//! from Plan 03 (08-04A bucket row 5). Covers:
//!   - happy path: every env var set → fields populated
//!   - default fallbacks: each unset var lands the documented default
//!   - error branches: missing JWT_SECRET, JWT_SECRET < 32 chars, missing
//!     DEVICE_CREDS_KEY, malformed DEVICE_CREDS_KEY, wrong-length
//!     DEVICE_CREDS_KEY, malformed SERVER_PORT, malformed
//!     TURSO_SYNC_INTERVAL, malformed TZ
//!   - has_turso() helper
//!   - manual Debug impl redacts secrets
//!
//! These tests serialise via a process-wide Mutex because Config::from_env
//! reads env vars; running tests in parallel without serialisation would
//! produce nondeterministic results.

use cronometrix_api::config::Config;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

const ALL_KEYS: &[&str] = &[
    "CRONOMETRIX_DB_PATH",
    "TURSO_DATABASE_URL",
    "TURSO_AUTH_TOKEN",
    "JWT_SECRET",
    "SERVER_HOST",
    "SERVER_PORT",
    "TURSO_SYNC_INTERVAL",
    "DEVICE_CREDS_KEY",
    "LICENSE_JWT_PATH",
    "DO_FUNCTIONS_ACTIVATE_URL",
    "DO_FUNCTIONS_RENEW_URL",
    "TZ",
];

fn unset_all() {
    for k in ALL_KEYS {
        std::env::remove_var(k);
    }
}

/// Sensible defaults for tests that do not exercise an error branch:
/// JWT_SECRET 32+ chars, DEVICE_CREDS_KEY 32 raw bytes b64-encoded.
fn set_required() {
    std::env::set_var("JWT_SECRET", "this-is-a-32-char-test-jwt-secret!");
    // 32-byte test key (matches common::TEST_DEVICE_CREDS_KEY_B64).
    std::env::set_var(
        "DEVICE_CREDS_KEY",
        "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=",
    );
}

// -----------------------------------------------------------------------------
// Happy path + defaults
// -----------------------------------------------------------------------------

#[test]
fn from_env_with_all_defaults_uses_documented_defaults() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();

    let c = Config::from_env().expect("required vars set, should succeed");
    assert_eq!(c.database_path, "cronometrix.db");
    assert_eq!(c.turso_url, "");
    assert_eq!(c.turso_token, "");
    assert_eq!(c.server_host, "0.0.0.0");
    assert_eq!(c.server_port, 3001);
    assert_eq!(c.turso_sync_interval_secs, 300);
    assert_eq!(c.license_jwt_path, "/opt/cronometrix/data/license.jwt");
    assert_eq!(c.do_functions_activate_url, "");
    assert_eq!(c.do_functions_renew_url, "");
    assert_eq!(c.timezone.name(), "America/Caracas");
    assert_eq!(c.device_creds_key.len(), 32);

    unset_all();
}

#[test]
fn from_env_honours_each_optional_var() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();

    std::env::set_var("CRONOMETRIX_DB_PATH", "/srv/cron.db");
    std::env::set_var("TURSO_DATABASE_URL", "libsql://example.turso.io");
    std::env::set_var("TURSO_AUTH_TOKEN", "tok-123");
    std::env::set_var("SERVER_HOST", "127.0.0.1");
    std::env::set_var("SERVER_PORT", "9001");
    std::env::set_var("TURSO_SYNC_INTERVAL", "60");
    std::env::set_var("LICENSE_JWT_PATH", "/var/lic.jwt");
    std::env::set_var("DO_FUNCTIONS_ACTIVATE_URL", "https://act.example/api");
    std::env::set_var("DO_FUNCTIONS_RENEW_URL", "https://renew.example/api");
    std::env::set_var("TZ", "UTC");

    let c = Config::from_env().expect("env-overridden values must parse");
    assert_eq!(c.database_path, "/srv/cron.db");
    assert_eq!(c.turso_url, "libsql://example.turso.io");
    assert_eq!(c.turso_token, "tok-123");
    assert_eq!(c.server_host, "127.0.0.1");
    assert_eq!(c.server_port, 9001);
    assert_eq!(c.turso_sync_interval_secs, 60);
    assert_eq!(c.license_jwt_path, "/var/lic.jwt");
    assert_eq!(c.do_functions_activate_url, "https://act.example/api");
    assert_eq!(c.do_functions_renew_url, "https://renew.example/api");
    assert_eq!(c.timezone.name(), "UTC");

    unset_all();
}

#[test]
fn has_turso_reflects_url_presence() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    let c = Config::from_env().unwrap();
    assert!(!c.has_turso(), "empty TURSO_DATABASE_URL → has_turso=false");

    std::env::set_var("TURSO_DATABASE_URL", "libsql://x");
    let c = Config::from_env().unwrap();
    assert!(c.has_turso(), "set TURSO_DATABASE_URL → has_turso=true");

    unset_all();
}

// -----------------------------------------------------------------------------
// Error branches
// -----------------------------------------------------------------------------

#[test]
fn from_env_errors_when_jwt_secret_missing() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    // DEVICE_CREDS_KEY set but JWT_SECRET unset.
    std::env::set_var(
        "DEVICE_CREDS_KEY",
        "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=",
    );
    let err = Config::from_env().expect_err("must error without JWT_SECRET");
    let s = format!("{:#}", err);
    assert!(s.contains("JWT_SECRET"), "err mentions JWT_SECRET: {s}");
    unset_all();
}

#[test]
fn from_env_errors_when_jwt_secret_too_short() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    std::env::set_var("JWT_SECRET", "short"); // < 32 chars
    let err = Config::from_env().expect_err("short JWT_SECRET must error");
    let s = format!("{:#}", err);
    assert!(
        s.contains("32 characters") || s.contains("32 ") || s.contains("at least 32"),
        "err mentions length requirement: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_device_creds_key_missing() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    std::env::set_var("JWT_SECRET", "this-is-a-32-char-test-jwt-secret!");
    let err = Config::from_env().expect_err("must error without DEVICE_CREDS_KEY");
    let s = format!("{:#}", err);
    assert!(
        s.contains("DEVICE_CREDS_KEY"),
        "err mentions DEVICE_CREDS_KEY: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_device_creds_key_not_base64() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    std::env::set_var("JWT_SECRET", "this-is-a-32-char-test-jwt-secret!");
    std::env::set_var("DEVICE_CREDS_KEY", "not!!base64!!at!!all");
    let err = Config::from_env().expect_err("must error on bad base64");
    let s = format!("{:#}", err);
    assert!(
        s.contains("base64") || s.contains("DEVICE_CREDS_KEY"),
        "err identifies base64 problem: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_device_creds_key_wrong_length() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    std::env::set_var("JWT_SECRET", "this-is-a-32-char-test-jwt-secret!");
    // Valid base64 but 16 bytes (too short for AES-256).
    std::env::set_var("DEVICE_CREDS_KEY", "MTIzNDU2Nzg5MDEyMzQ1Ng==");
    let err = Config::from_env().expect_err("must error on wrong byte length");
    let s = format!("{:#}", err);
    assert!(
        s.contains("32 bytes") || s.contains("32 byte") || s.contains("DEVICE_CREDS_KEY"),
        "err mentions size requirement: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_server_port_not_numeric() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    std::env::set_var("SERVER_PORT", "not-a-port");
    let err = Config::from_env().expect_err("must error on bad port");
    let s = format!("{:#}", err);
    assert!(
        s.contains("SERVER_PORT") || s.contains("port"),
        "err identifies port: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_sync_interval_not_numeric() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    std::env::set_var("TURSO_SYNC_INTERVAL", "five-hundred");
    let err = Config::from_env().expect_err("must error on bad interval");
    let s = format!("{:#}", err);
    assert!(
        s.contains("TURSO_SYNC_INTERVAL") || s.contains("seconds"),
        "err identifies interval: {s}"
    );
    unset_all();
}

#[test]
fn from_env_errors_when_tz_unknown() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    std::env::set_var("TZ", "Mars/Olympus_Mons");
    let err = Config::from_env().expect_err("unknown IANA tz must error");
    let s = format!("{:#}", err);
    assert!(
        s.contains("timezone") || s.contains("TZ") || s.contains("Mars"),
        "err mentions tz: {s}"
    );
    unset_all();
}

// -----------------------------------------------------------------------------
// Debug-impl redaction (Security Domain rule #1)
// -----------------------------------------------------------------------------

#[test]
fn debug_redacts_secrets_in_output() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    std::env::set_var("JWT_SECRET", "very-secret-jwt-key-for-debug-redact!!");
    std::env::set_var(
        "DEVICE_CREDS_KEY",
        "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=",
    );
    std::env::set_var("TURSO_AUTH_TOKEN", "very-secret-turso-token");
    let c = Config::from_env().unwrap();
    let dbg = format!("{:?}", c);

    assert!(
        !dbg.contains("very-secret-jwt-key-for-debug-redact"),
        "JWT_SECRET must NOT appear in Debug, got: {dbg}"
    );
    assert!(
        !dbg.contains("very-secret-turso-token"),
        "TURSO_AUTH_TOKEN must NOT appear in Debug, got: {dbg}"
    );
    // device_creds_key as bytes must not be reconstructible from Debug.
    assert!(
        dbg.contains("[redacted"),
        "Debug must label redacted fields: {dbg}"
    );
    // Non-secret fields do appear.
    assert!(dbg.contains("server_port"));
    assert!(dbg.contains("America/Caracas"));

    unset_all();
}

#[test]
fn config_clone_preserves_all_fields() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unset_all();
    set_required();
    std::env::set_var("SERVER_PORT", "4242");
    let c = Config::from_env().unwrap();
    let c2 = c.clone();
    assert_eq!(c.server_port, c2.server_port);
    assert_eq!(c.jwt_secret, c2.jwt_secret);
    assert_eq!(c.device_creds_key, c2.device_creds_key);
    unset_all();
}
