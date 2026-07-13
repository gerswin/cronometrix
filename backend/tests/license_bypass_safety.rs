//! Integration tests for D-13 license-bypass safety contract.
//!
//! These tests spawn the actual binary as a child process and assert behavior
//! based on which env flags are set. No in-process mocking — the subprocess
//! approach is intentional: it proves the safety check fires in `main()` before
//! ANY request handling begins.
//!
//! Exit code 2 contract:
//!   CRONOMETRIX_LICENSE_BYPASS=true  (without CRONOMETRIX_E2E=true) → exit 2
//!   CRONOMETRIX_E2E=true + CRONOMETRIX_LICENSE_BYPASS=true           → starts normally
//!
//! Locked by this test file. Do NOT change exit code 2 without updating both
//! this test and the SUMMARY documenting D-13.
//!
//! Implementation notes:
//! - TURSO_DATABASE_URL must NOT be set (leave empty) so the binary uses
//!   local SQLite mode (has_turso() returns false). Setting it to ":memory:"
//!   would make has_turso() true and trigger the Turso builder which panics.
//! - DEVICE_CREDS_KEY must be valid base64 of exactly 32 decoded bytes.
//! - Readiness is detected by polling the TCP port until connect() succeeds
//!   (avoids fragile stderr line parsing that is unreliable under nextest).

use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Valid base64-encoded 32-byte AES-256 key for DEVICE_CREDS_KEY.
/// 32 × 'A' bytes = "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=" in base64.
const TEST_DEVICE_CREDS_KEY: &str = "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=";

/// Minimum 32-char JWT secret for Config::from_env validation.
const TEST_JWT_SECRET: &str = "test-secret-at-least-32-chars-padding!!";

/// Ask the OS for a free port by binding to :0, then immediately release it.
fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind port 0");
    listener.local_addr().expect("local_addr").port()
    // listener drops here, releasing the port
}

/// Poll until a TCP connect to 127.0.0.1:port succeeds or the deadline passes.
/// Returns true if the port accepted a connection before the deadline.
fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if TcpStream::connect(&addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// D-13 negative test (locks the safety in):
/// Spawn binary with CRONOMETRIX_LICENSE_BYPASS=true and NO CRONOMETRIX_E2E set.
/// The binary MUST exit with code 2 immediately (before serving any request).
/// stderr MUST contain "CRONOMETRIX_LICENSE_BYPASS" (the FATAL eprintln! message).
#[test]
fn bypass_without_e2e_aborts_with_code_2() {
    let tmp_dir = tempfile::TempDir::new().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_cronometrix"))
        .env_clear()
        .current_dir(tmp_dir.path())
        // Pass PATH so the binary can locate dynamic libraries.
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        // Required by Config::from_env — JWT_SECRET must be >= 32 chars.
        .env("JWT_SECRET", TEST_JWT_SECRET)
        // Required by Config::from_env — must be valid base64 of exactly 32 bytes.
        .env("DEVICE_CREDS_KEY", TEST_DEVICE_CREDS_KEY)
        .env(
            "LICENSE_JWT_PATH",
            "/tmp/nonexistent-license-bypass-test.jwt",
        )
        // The misconfiguration under test: bypass set WITHOUT e2e flag.
        .env("CRONOMETRIX_LICENSE_BYPASS", "true")
        // Deliberately DO NOT set CRONOMETRIX_E2E — this is the trigger condition.
        .output()
        .expect("failed to spawn cronometrix binary");

    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 (misconfigured bypass); got {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // The FATAL eprintln! goes directly to stderr (not routed through tracing),
    // so it appears even if the tracing subscriber is not yet initialized.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("CRONOMETRIX_LICENSE_BYPASS"),
        "stderr must mention CRONOMETRIX_LICENSE_BYPASS so operators see what went wrong; \
         stderr was: {}",
        stderr,
    );
}

/// D-13 positive test (sanity: both flags set → binary gets past the gate):
/// Spawn binary with BOTH CRONOMETRIX_E2E=true AND CRONOMETRIX_LICENSE_BYPASS=true.
/// Poll the TCP port until the binary starts accepting connections (readiness probe).
/// Kill the child, assert exit code is NOT 2 (gate did not fire).
///
/// TURSO_DATABASE_URL is intentionally NOT set — the binary uses local SQLite mode.
/// The DB path is unique per test run via a fresh tempdir.
#[test]
fn bypass_with_e2e_proceeds_past_license_gate() {
    let port = pick_free_port();

    // Use a tempdir so there are no stale DB or WAL files from previous runs.
    let tmp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp_dir.path().join("test.db");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cronometrix"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        // Required by Config::from_env
        .env("JWT_SECRET", TEST_JWT_SECRET)
        .env("DEVICE_CREDS_KEY", TEST_DEVICE_CREDS_KEY)
        .env(
            "LICENSE_JWT_PATH",
            "/tmp/nonexistent-license-bypass-test.jwt",
        )
        // Both flags set — should pass the gate and proceed to normal startup.
        .env("CRONOMETRIX_E2E", "true")
        .env("CRONOMETRIX_LICENSE_BYPASS", "true")
        // Bind to the OS-assigned free port.
        .env("SERVER_HOST", "127.0.0.1")
        .env("SERVER_PORT", port.to_string())
        // Local SQLite mode: leave TURSO_DATABASE_URL absent (has_turso() = false).
        .env("CRONOMETRIX_DB_PATH", db_path.to_str().unwrap())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn cronometrix binary");

    // Poll the TCP port for up to 20s to detect that the server is accepting
    // connections. This is more reliable than parsing tracing output.
    // 20s budget: fresh-DB migration takes ~150-500ms in debug; 20s is very
    // generous and stays within the 30s VALIDATION.md sampling latency limit.
    let started = wait_for_port(port, Duration::from_secs(20));

    // Terminate the child cleanly.
    let _ = child.kill();
    let status = child.wait().expect("wait for child");

    // tmp_dir drops here, cleaning up the DB files.

    assert_ne!(
        status.code(),
        Some(2),
        "binary must NOT exit with code 2 when both CRONOMETRIX_E2E and \
         CRONOMETRIX_LICENSE_BYPASS are set; got {:?}",
        status,
    );

    assert!(
        started,
        "binary did not start accepting TCP connections on port {} within 20s. \
         The binary may have crashed after the license gate — \
         check for DB init errors.",
        port,
    );
}
