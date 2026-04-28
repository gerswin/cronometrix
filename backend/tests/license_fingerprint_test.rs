//! Coverage gap-fill for `backend/src/license/fingerprint.rs` (08-04B Task 2).
//!
//! Baseline 13.33% line. Target ≥70%.
//!
//! This module reads OS pseudo-files (`/proc/cpuinfo`, `/sys/class/net`,
//! `/sys/block`) to compute a SHA256 hardware fingerprint. The OS reads are
//! NOT abstracted behind a trait, so the production code path is platform-
//! specific:
//!   * Linux: all three readers succeed → returns Ok(64-char hex digest).
//!   * macOS: `/proc/cpuinfo` does not exist → first read errors → returns Err.
//!
//! Strategy:
//!   * Determinism check on Linux only (cfg-gated).
//!   * Hex-format check on Linux: 64 lowercase chars 0-9a-f.
//!   * macOS branch is exercised by the existing `license_module_is_reachable`
//!     test in `license_tests.rs` (Wave 0) which calls the function on
//!     whatever host runs the suite.
//!   * Cross-platform branch: assert collect_fingerprint() returns either Ok
//!     (Linux container, CI) or a contextual Err (macOS dev box) — Plan 04B
//!     surfaces the OS-read tested-only-where-runnable limitation as the
//!     candidate exclusion at the 04C checkpoint.

use cronometrix_api::license::fingerprint::collect_fingerprint;

#[cfg(target_os = "linux")]
#[test]
fn fingerprint_is_deterministic_on_linux() {
    let a = collect_fingerprint().expect("Linux fingerprint must succeed");
    let b = collect_fingerprint().expect("Linux fingerprint must succeed");
    assert_eq!(a, b, "fingerprint must be deterministic across calls");
}

#[cfg(target_os = "linux")]
#[test]
fn fingerprint_is_64_char_hex_on_linux() {
    let fp = collect_fingerprint().expect("Linux fingerprint must succeed");
    assert_eq!(fp.len(), 64, "SHA256 hex must be 64 chars");
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint must be lowercase hex: {fp}"
    );
    // The format!("{:x}", ...) call always emits lowercase hex.
    assert_eq!(fp, fp.to_lowercase(), "must be lowercase hex");
}

#[cfg(target_os = "linux")]
#[test]
fn fingerprint_is_stable_across_many_calls_on_linux() {
    let baseline = collect_fingerprint().expect("Linux fingerprint must succeed");
    for _ in 0..10 {
        let fp = collect_fingerprint().unwrap();
        assert_eq!(fp, baseline);
    }
}

/// Cross-platform smoke: the function must always return without panicking,
/// regardless of host. On macOS dev hosts /proc/cpuinfo does not exist and
/// the function returns Err — that is acceptable per RESEARCH.
#[test]
fn fingerprint_does_not_panic_on_any_host() {
    let _ = collect_fingerprint();
}

#[cfg(not(target_os = "linux"))]
#[test]
fn fingerprint_errors_on_non_linux_with_context() {
    let result = collect_fingerprint();
    // macOS dev: /proc/cpuinfo does not exist → first read fails.
    assert!(
        result.is_err(),
        "expected Err on non-linux; got Ok({:?})",
        result.ok()
    );
    let err = result.unwrap_err();
    let msg = err.to_string();
    // anyhow::anyhow!("read /proc/cpuinfo: {}", e) is the contextual error.
    assert!(
        msg.contains("/proc/cpuinfo") || msg.contains("read"),
        "err must mention the OS file: {msg}"
    );
}
