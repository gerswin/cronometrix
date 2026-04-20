---
phase: 02-device-integration
reviewed: 2026-04-19T00:00:00Z
depth: standard
files_reviewed: 36
files_reviewed_list:
  - backend/.env.example
  - backend/Cargo.toml
  - backend/src/config.rs
  - backend/src/db/migrations/003_devices.sql
  - backend/src/db/migrations/004_attendance_events.sql
  - backend/src/db/migrations/005_command_audit_log.sql
  - backend/src/db/migrations/006_devices_audit_triggers.sql
  - backend/src/db/mod.rs
  - backend/src/devices/crypto.rs
  - backend/src/devices/handlers.rs
  - backend/src/devices/mod.rs
  - backend/src/devices/models.rs
  - backend/src/devices/service.rs
  - backend/src/errors.rs
  - backend/src/events/handlers.rs
  - backend/src/events/mod.rs
  - backend/src/events/models.rs
  - backend/src/events/service.rs
  - backend/src/isapi/client.rs
  - backend/src/isapi/events.rs
  - backend/src/isapi/mod.rs
  - backend/src/isapi/parser.rs
  - backend/src/isapi/stream.rs
  - backend/src/lib.rs
  - backend/src/main.rs
  - backend/src/state.rs
  - backend/src/supervisor/mod.rs
  - backend/src/supervisor/status.rs
  - backend/src/supervisor/task.rs
  - backend/src/supervisor/watchdog.rs
  - backend/tests/common/mock_hikvision.rs
  - backend/tests/common/mod.rs
  - backend/tests/device_tests.rs
  - backend/tests/event_tests.rs
  - backend/tests/listener_tests.rs
  - backend/tests/supervisor_tests.rs
findings:
  critical: 0
  warning: 5
  info: 7
  total: 12
status: issues_found
---

# Phase 2: Code Review Report

**Reviewed:** 2026-04-19
**Depth:** standard
**Files Reviewed:** 36
**Status:** issues_found

## Summary

Phase 2 delivers a well-architected device integration surface: AES-256-GCM credential encryption with strong redaction discipline, a digest-authenticated ISAPI client, a multipart alertStream consumer with a line-scan fallback, a per-device supervisor with exponential backoff + jitter, and a single-writer-per-transition watchdog. The security posture is deliberate (plaintext passwords never derive `Serialize`/`Debug`; `DeviceResponse` has no password field by construction; audit triggers deliberately omit the ciphertext column; traversal defenses on photo serving; `DEVICE_CREDS_KEY` separated from `JWT_SECRET`). Tests cover the happy paths and most negative paths (401 / timeout / traversal / dedup / heartbeat / missing file).

No critical defects were found. Twelve findings below are mostly hardening and correctness edge-cases; the most notable is that `touch_last_seen` triggers the `audit_devices_update` trigger on every captured event, causing audit-log write amplification proportional to stream traffic (WR-01), and the `overtimeOut` attendance-status mapping has a casing asymmetry that silently mis-routes a subset of OT punches to `entry` (WR-02).

## Warnings

### WR-01: `touch_last_seen` writes audit_log row on every event (write amplification)

**File:** `backend/src/supervisor/status.rs:37-42` + `backend/src/db/migrations/006_devices_audit_triggers.sql:28-42`
**Issue:** `touch_last_seen` issues an `UPDATE devices SET last_seen_at = unixepoch() WHERE id = ?1` for every successfully-read multipart part in `isapi::stream::connect_and_stream` (and again per-heartbeat). The `audit_devices_update` trigger fires `AFTER UPDATE` unconditionally, so it inserts an `audit_log` row for every single `last_seen_at` bump. With 4 devices and even one event/second during peak hours, that is 345k rows/day of semantically-empty audit entries (OLD and NEW columns in the json payload are identical because `last_seen_at` is not included). This violates the intent of RESEARCH § Security Domain rule that audit captures *changes*, and long-term it will swamp the `audit_log` table, making real security/RBAC audit forensics harder to read and increasing sync bandwidth to Turso.
**Fix:** Either (a) exclude `last_seen_at`-only updates from the audit trigger by gating on a changed column set, or (b) split the trigger into an explicit WHEN clause that compares the json payloads. Concrete option (a):
```sql
CREATE TRIGGER IF NOT EXISTS audit_devices_update
    AFTER UPDATE ON devices
    WHEN OLD.name       IS NOT NEW.name
      OR OLD.ip         IS NOT NEW.ip
      OR OLD.port       IS NOT NEW.port
      OR OLD.scheme     IS NOT NEW.scheme
      OR OLD.username   IS NOT NEW.username
      OR OLD.encrypted_password IS NOT NEW.encrypted_password
      OR OLD.direction  IS NOT NEW.direction
      OR OLD.allow_insecure_tls IS NOT NEW.allow_insecure_tls
      OR OLD.connection_state   IS NOT NEW.connection_state
      OR OLD.status     IS NOT NEW.status
      OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
  ...
END;
```
This keeps `last_seen_at` / `updated_at`-only bumps out of the audit log while still capturing every real mutation.

### WR-02: `direction_for_attendance_status` casing asymmetry drops `overtimeOut` → `entry`

**File:** `backend/src/isapi/events.rs:84-89`
**Issue:** The match arms pair `"overtimeIn"` (lowercase `t`) with `"overTimeOut"` (uppercase `T`). Hikvision firmware emits these values consistently per device line, but none of the documented enumerations from K1T3xx mix the two casings; the ISAPI schema lists them as `overtimeIn` / `overtimeOut` OR `overTimeIn` / `overTimeOut`. Whichever casing a given firmware emits, this match will only catch one of the two, so a subset of real overtime-exit punches will fall through to the `_ => "entry"` default silently — mis-classifying them as entries. Unit test `direction_mapping_check_in_is_entry` enshrines the asymmetry rather than catching it.
**Fix:**
```rust
pub fn direction_for_attendance_status(s: &str) -> &'static str {
    match s {
        "checkIn" | "breakIn" | "overtimeIn" | "overTimeIn" => "entry",
        "checkOut" | "breakOut" | "overtimeOut" | "overTimeOut" => "exit",
        _ => "entry",
    }
}
```
Also extend the test to assert BOTH casings map correctly.

### WR-03: `reboot` command can race audit-log `completed_at` with actual reboot

**File:** `backend/src/devices/handlers.rs:252-258` + `backend/src/isapi/client.rs:81-87`
**Issue:** `DeviceConnection::reboot()` calls `PUT /ISAPI/System/reboot` with an empty body. Hikvision devices typically 200-OK immediately and then reset, so the handler marks `outcome = ok` in `command_audit_log`. But devices that reboot *before* sending the 200 produce a dropped connection that surfaces as `Err(reqwest::...)`, and the audit row is written with `outcome = error` even though the reboot succeeded. There is no corroborating health-check loop that flips the audit status after the device comes back. For a command whose whole purpose is to power-cycle, this creates false-positive "error" audit entries that operators will chase.
**Fix:** Classify `reqwest` transport errors that occur on `reboot` specifically (e.g., `error.is_connect()` || connection reset mid-body) as `outcome = ok` with a diagnostic note in `result`. Minimal patch in `dispatch_command`:
```rust
let audit_outcome = match (&command, &result) {
    (Command::Reboot, Ok(Err(e))) if is_transport_drop(e) =>
        CommandAuditOutcome::Ok("device likely rebooted (connection dropped)".into()),
    (_, Ok(Ok(body))) => CommandAuditOutcome::Ok(body.clone()),
    (_, Ok(Err(e))) => CommandAuditOutcome::Error { code: "DEVICE_ERROR", message: e.to_string() },
    (_, Err(_)) => CommandAuditOutcome::Timeout,
};
```

### WR-04: `connect_and_stream` has no response-headers timeout — hostile/slow device can block indefinitely

**File:** `backend/src/isapi/stream.rs:133-141`
**Issue:** The reqwest client built inside `connect_and_stream` sets `connect_timeout(5s)` but deliberately omits `.timeout(...)` because the alertStream is a long-lived body. That's correct for the body, BUT it also means the *initial response* (TCP handshake succeeded, TLS completed, GET sent, device never writes the status line) has no upper bound. A compromised or misbehaving device can park a supervisor task forever before any `CancellationToken` check runs at the outer `select!` in `device_task`, because `connect_and_stream` is not itself cancellation-aware until it is `await`-ing a specific multipart chunk.
**Fix:** Apply a distinct read timeout to the response-header phase using `reqwest::Client::read_timeout` (reqwest 0.13) or wrap the `client.get(&url).send_digest_auth(...)` call in a `tokio::time::timeout(Duration::from_secs(15), ...)`:
```rust
let resp = tokio::time::timeout(
    Duration::from_secs(15),
    client.get(&url).send_digest_auth((cfg.username.as_str(), cfg.password.as_str())),
).await.map_err(|_| anyhow::anyhow!("alertStream headers timed out"))??;
```
Then drop the timeout for the streaming body loop (as today).

### WR-05: `Supervisor::spawn_device` has a TOCTOU window on the handles map

**File:** `backend/src/supervisor/mod.rs:116-137`
**Issue:** `spawn_device` checks `h.contains_key(&dev_id)` under the lock, **drops the lock**, spawns the task, and **re-acquires the lock** to insert. Today `run()` serialises `handle_event` calls from a single `select!`, so in production there is no actual concurrent spawn. But any future change that fans out lifecycle handling (e.g., multiple Start events buffered and drained in parallel) would produce duplicate tasks for the same device — both would connect to the same alertStream, each multiplying `persist_attendance_event` workload and audit rows. The test-only `active_count`/`active_ids` accessors also race against the map between insertions.
**Fix:** Hold the lock across the spawn, or use `entry(dev_id).or_insert_with(...)` to make the check-and-insert atomic:
```rust
let mut map = self.handles.lock().await;
if map.contains_key(&dev_id) {
    return;
}
let child_tok = self.shutdown.child_token();
let state = self.state.clone();
let tok_clone = child_tok.clone();
let handle = tokio::spawn(async move {
    task::device_task(dev, state, tok_clone).await;
});
map.insert(dev_id, (handle, child_tok));
```
Holding the lock across `tokio::spawn` is fine — spawn does not `await`.

## Info

### IN-01: `crypto::decrypt_password` length precondition is too loose

**File:** `backend/src/devices/crypto.rs:63-66`
**Issue:** The check is `combined.len() > 12`, which accepts any input of 13+ bytes. A valid AES-GCM payload is always ≥ 12 (nonce) + 16 (tag) = 28 bytes; shorter inputs cannot possibly authenticate. `aes-gcm` rejects them anyway, but an explicit pre-check gives a clearer error and eliminates a class of malformed inputs before hitting the cipher.
**Fix:** `anyhow::ensure!(combined.len() >= 12 + 16, "...");`

### IN-02: `config.rs` duplicates `crypto::load_key_from_env` logic

**File:** `backend/src/config.rs:99-116` vs `backend/src/devices/crypto.rs:85-98`
**Issue:** Both files independently decode and validate `DEVICE_CREDS_KEY` with the same shape (env lookup → base64 decode → try_into `[u8; 32]`). `Config::from_env` calls `load_device_creds_key` from the same file instead of the `crypto::load_key_from_env` helper that already exists.
**Fix:** Replace the local `load_device_creds_key` with `crate::devices::crypto::load_key_from_env("DEVICE_CREDS_KEY")` so there is exactly one key-loader. Reduces the surface area for divergence (e.g., someone adding length logging in one copy but not the other).

### IN-03: Unused `_Cipher` type alias in crypto.rs

**File:** `backend/src/devices/crypto.rs:22`
**Issue:** `type _Cipher = aes_gcm::Aes256Gcm;` is declared and never referenced. The comment above justifies the `use` wildcards but the alias itself is dead code and will trigger `unused_type_alias` lints if the crate ever elevates warnings.
**Fix:** Remove the alias, or elevate it to a used `pub(crate) type Cipher = Aes256Gcm;` if a canonical name is wanted.

### IN-04: `test_db()` leaks `/tmp/cronometrix_test_*.db` files across runs

**File:** `backend/tests/common/mod.rs:26-42` and `backend/src/events/service.rs:353-365`
**Issue:** Tests build a uniquely-named libSQL database at `/tmp/cronometrix_test_{uuid}.db` and never delete it. Over many CI runs this accumulates thousands of small files in `/tmp`, and the hard-coded `/tmp` path is Linux/macOS-only (Windows developers or any sandbox without `/tmp` will fail to run tests).
**Fix:** Use `tempfile::NamedTempFile` (already a dev-dep) or `TempDir` so files are dropped automatically when the handle goes out of scope:
```rust
let dir = tempfile::TempDir::new().expect("tempdir");
let tmp_path = dir.path().join("cronometrix.db");
```
Return the `TempDir` alongside the `Database` so the caller keeps it alive for the test duration.

### IN-05: `EventListQuery` has no `Validate` derive — limit/offset coercion is silent

**File:** `backend/src/events/models.rs:47-57` and `backend/src/events/service.rs:171-172`
**Issue:** `EventListQuery` derives `Deserialize` only; the service clamps limit to `[1, 100]` and offset to `>= 0` silently. A caller passing `limit=-50` gets `limit=1`, not `422 VALIDATION_ERROR` — the behaviour differs from the validator-driven error shape elsewhere. Not a bug, but inconsistent with how `DeviceListQuery` and the create/update bodies behave in the same codebase.
**Fix:** Add `validator::Validate` with `#[validate(range(min = 1, max = 100))]` on `limit` and `#[validate(range(min = 0))]` on `offset`, and call `q.validate()` at the top of `list`.

### IN-06: `ingest_pair` opens a fresh libSQL connection per event

**File:** `backend/src/isapi/stream.rs:282`
**Issue:** For every parsed multipart pair the code does `state.db.connect()?`. libSQL connections are cheap, but under burst traffic (a fleet of 4 devices all punching in at shift-change) this allocates and tears down a connection per event, which is avoidable. The outer `connect_and_stream` holds one long-lived response stream per device; nothing prevents it from holding one long-lived DB connection per device task too.
**Fix:** Acquire the connection once at the top of `connect_and_stream` (or `device_task`) and pass `&Connection` into `ingest_pair`. This also collapses three connection calls in `ingest_pair` (lookup_employee_for_event, persist_attendance_event, touch_last_seen reached indirectly through `update_connection_state`) into a single handle.

### IN-07: `lib.rs` declares `pub mod setup;` — not in review scope but exposed globally

**File:** `backend/src/lib.rs:12`
**Issue:** The library re-exports `setup`, `rules`, `departments`, `employees` — Phase 1 modules. These are outside the Phase 2 diff but sharing `AppState` with Phase 2 means a Phase 2 change that accidentally added a field the Phase 1 modules depended on would only be caught at link time. Not a defect — flagged as a project-wide observation so future phases (e.g., Phase 3 calc) keep the `AppState` surface tight.
**Fix:** None required for this phase. When `AppState` grows, prefer wrapping new long-lived handles (e.g., a metrics sink, a background job queue) in dedicated sub-structs so modules opt into what they actually need.

---

_Reviewed: 2026-04-19_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
