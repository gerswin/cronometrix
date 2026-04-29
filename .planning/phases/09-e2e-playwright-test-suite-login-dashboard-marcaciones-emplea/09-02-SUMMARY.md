---
phase: 09
plan: "02"
subsystem: backend/license
tags: [license, security, d13, bypass-safety, tdd, integration-test]
dependency_graph:
  requires: []
  provides:
    - evaluate_bypass pure function in backend/src/license/service.rs
    - BypassDecision enum exported from license::service
    - D-13 bypass-flag safety wired in main.rs (exit code 2 contract)
    - integration test backend/tests/license_bypass_safety.rs locking exit code 2
  affects:
    - backend/src/main.rs (license gate startup flow)
    - backend/src/license/service.rs (new exports)
    - backend/tests/license_bypass_safety.rs (new integration test file)
tech_stack:
  added: []
  patterns:
    - TDD RED/GREEN cycle for pure function (no refactor needed)
    - Subprocess-spawn integration test pattern (spawn binary, assert exit code)
    - TCP port polling for readiness detection (avoids fragile stderr parsing)
    - tempfile::TempDir for DB isolation in subprocess tests
key_files:
  created:
    - backend/tests/license_bypass_safety.rs
  modified:
    - backend/src/license/service.rs
    - backend/src/main.rs
decisions:
  - "exit code 2 for AbortMisconfigured — matches plan contract; locked by integration test; do NOT change"
  - "TCP port polling for readiness detection — stderr parsing via BufReader::read_line() blocks indefinitely when child is alive; TCP connect is reliable and simpler"
  - "tempfile::TempDir for subprocess DB — avoids stale 0-byte DB files and WAL conflicts between test runs; same pattern as other integration tests"
  - "DEVICE_CREDS_KEY must be valid base64 of exactly 32 decoded bytes — test uses QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE= (32 'A' bytes)"
  - "TURSO_DATABASE_URL must NOT be set in subprocess tests — setting it to ':memory:' makes has_turso()=true and panics on invalid URI"
metrics:
  duration: "51 minutes"
  completed: "2026-04-29T02:29:54Z"
  tasks_completed: 3
  files_modified: 3
---

# Phase 09 Plan 02: License Bypass Safety (D-13) Summary

Lock the D-13 license-bypass safety contract: `evaluate_bypass(e2e, bypass) -> BypassDecision` pure function in `license::service`, wiring in `main.rs` that aborts with exit code 2 when `CRONOMETRIX_LICENSE_BYPASS=true` appears without `CRONOMETRIX_E2E=true`, and integration test that spawns the real binary and asserts exit code 2.

## What Was Built

### `BypassDecision` enum + `evaluate_bypass` function (backend/src/license/service.rs)

Copy-paste ready for Plan 13 docs:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BypassDecision {
    AllowBypass,
    AbortMisconfigured,
    NormalPath,
}

pub fn evaluate_bypass(e2e: bool, bypass: bool) -> BypassDecision {
    match (e2e, bypass) {
        (true, true)  => BypassDecision::AllowBypass,
        (false, true) => BypassDecision::AbortMisconfigured,
        _             => BypassDecision::NormalPath,
    }
}
```

Truth table (locked by 4 unit tests in `evaluate_bypass_tests` module):

| e2e   | bypass | result              |
|-------|--------|---------------------|
| true  | true   | AllowBypass         |
| false | true   | AbortMisconfigured  |
| true  | false  | NormalPath          |
| false | false  | NormalPath          |

### Exit code contract (backend/src/main.rs)

**Exit code 2 = CRONOMETRIX_LICENSE_BYPASS set without CRONOMETRIX_E2E. DO NOT CHANGE — locked by integration test.**

The wiring in `main.rs` calls `evaluate_bypass(e2e, bypass)` BEFORE `load_and_validate_license`. On `AbortMisconfigured`:
- `tracing::error!` emits (visible in log aggregators)
- `eprintln!` emits (visible even without tracing subscriber configured)
- `std::process::exit(2)` halts the process immediately

No request handler is ever registered when exit code 2 fires.

### Integration test pattern (backend/tests/license_bypass_safety.rs)

Template for future "spawn binary and assert exit code" tests:

```rust
let out = Command::new(env!("CARGO_BIN_EXE_cronometrix-api"))
    .env_clear()
    .env("PATH", std::env::var("PATH").unwrap_or_default())
    .env("JWT_SECRET", "test-secret-at-least-32-chars-padding!!")
    .env("DEVICE_CREDS_KEY", "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=")
    // ... other required env vars ...
    .output()
    .expect("spawn");
assert_eq!(out.status.code(), Some(2));
```

Key implementation notes captured in file header:
- `DEVICE_CREDS_KEY` must be base64 of exactly 32 decoded bytes
- `TURSO_DATABASE_URL` must NOT be set (leave absent for local SQLite mode)
- TCP port polling is more reliable than stderr line parsing for readiness detection
- Use `tempfile::TempDir` for DB isolation to prevent stale file conflicts

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Positive test readiness detection via stderr parsing failed**
- **Found during:** Task 3
- **Issue:** Plan specified waiting for "listening on" in stdout via `BufReader::read_line()`. This blocks indefinitely because the BufReader blocks on subsequent reads while the child process is still running. Thread-based approach also timed out because nextest's subprocess environment behaved differently than the manual invocation.
- **Fix:** Replaced stderr line scanning with TCP port polling (`TcpStream::connect()` loop). The binary starts in ~200ms; polling every 100ms with 20s budget is robust and independent of tracing output format.
- **Files modified:** `backend/tests/license_bypass_safety.rs`

**2. [Rule 1 - Bug] TURSO_DATABASE_URL=":memory:" panics in subprocess**
- **Found during:** Task 3
- **Issue:** Plan template set `TURSO_DATABASE_URL=":memory:"` which makes `has_turso()` return true and calls `Builder::new_remote_replica(":memory:", ...)` — this panics with `InvalidUri(InvalidAuthority)`.
- **Fix:** Leave `TURSO_DATABASE_URL` absent (empty string default); set `CRONOMETRIX_DB_PATH` to a temp path instead.
- **Files modified:** `backend/tests/license_bypass_safety.rs`

**3. [Rule 1 - Bug] CARGO_BIN_EXE_ macro name correction**
- **Found during:** Task 3
- **Issue:** Plan used `env!("CARGO_BIN_EXE_cronometrix")` but the binary is named `cronometrix-api` (package name in Cargo.toml). The correct macro is `env!("CARGO_BIN_EXE_cronometrix-api")`.
- **Fix:** Used the correct macro name.
- **Files modified:** `backend/tests/license_bypass_safety.rs`

**4. [Rule 1 - Bug] DEVICE_CREDS_KEY must be valid base64**
- **Found during:** Task 3
- **Issue:** Plan template used `"test-device-creds-key-32-bytes-pad"` which is NOT valid base64 and causes `Config::from_env()` to fail before reaching the license gate.
- **Fix:** Used `"QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE="` (32 'A' bytes base64-encoded).
- **Files modified:** `backend/tests/license_bypass_safety.rs`

## Test Results

```
evaluate_bypass unit tests:   4/4 PASSED (0.011s)
license_bypass_safety:        2/2 PASSED (1.469s total)
license_tests (regression):  24/24 PASSED (no regressions)
```

All tests pass. Debug build and release build both succeed with no errors.

## Known Stubs

None. All three artifacts (pure function, main.rs wiring, integration test) are fully implemented with real behavior.

## Threat Surface Scan

No new network endpoints, auth paths, or schema changes introduced. The changes are:
- Pure function addition (no I/O)
- Main.rs startup logic modification (early exit path only)
- Test file (no production surface)

T-09-01 (Elevation of Privilege via CRONOMETRIX_LICENSE_BYPASS) is now **mitigated** per threat model:
- `evaluate_bypass(false, true) → AbortMisconfigured → process::exit(2)` fires BEFORE any handler is registered
- Locked by `bypass_without_e2e_aborts_with_code_2` integration test

## Self-Check: PASSED

| Item | Status |
|------|--------|
| `backend/src/license/service.rs` exists | FOUND |
| `backend/src/main.rs` exists | FOUND |
| `backend/tests/license_bypass_safety.rs` exists | FOUND |
| `test(09-02): RED` commit (c846064) | FOUND |
| `feat(09-02): GREEN` commit (13a6687) | FOUND |
| `feat(09-02): wire evaluate_bypass` commit (2222137) | FOUND |
| `feat(09-02): integration test` commit (c827ff7) | FOUND |
