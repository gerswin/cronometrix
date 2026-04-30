---
phase: 02-device-integration
verified: 2026-04-20T04:08:18Z
status: human_needed
score: 21/21 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Hardware smoke — real DS-K1T341 emits an event via alertStream"
    expected: "After registering a real Hikvision device via POST /api/v1/devices with valid credentials, a physical face scan produces exactly one row in attendance_events with employee_id resolved (or is_unknown=1), direction set from attendanceStatus, raw_xml non-empty, and photo_path pointing at a saved JPEG under ./data/events/YYYY-MM-DD/."
    why_human: "Requires real hardware on LAN, cannot be exercised in CI; documented as the single MANUAL item in 02-VALIDATION.md § Manual-Only Verifications."
  - test: "Reconnect under real network drop"
    expected: "Power-cycling a registered device produces WARN 'stream ended with error' followed by 'reconnect backoff' debug lines doubling 1s→2s→4s→…→60s; after device returns, connection_state flips to 'online' and last_seen_at refreshes within the next heartbeat."
    why_human: "Requires physical device power-cycling; mock fixtures cover the logic path but not the real TCP RST/reset timing on production hardware."
  - test: "Dashboard-style real-time feel"
    expected: "GET /api/v1/devices shows connection_state transitions (offline -> online -> offline) within seconds of network events from the device."
    why_human: "End-to-end perceived latency between physical action and API read is a UX property; automated tests assert individual writes but not the human-perceived roundtrip."
---

# Phase 2: Device Integration Verification Report

**Phase Goal:** The system maintains live alertStream connections to all registered Hikvision devices, captures attendance events in real time, and operators can manage device configuration from the backend.

**Verified:** 2026-04-20T04:08:18Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

Must-haves merged from (a) ROADMAP.md Phase 2 Success Criteria and (b) PLAN frontmatter truths across 02-01, 02-02, 02-03. Roadmap SCs take precedence in naming; plan truths are additional technical behaviors that support the SC.

| #   | Truth                                                                                                                           | Status     | Evidence |
| --- | ------------------------------------------------------------------------------------------------------------------------------- | ---------- | -------- |
| 1   | Admin can register a Hikvision device with IP, credentials, direction; device appears immediately (ROADMAP SC-1)                | VERIFIED   | `tests/device_tests.rs::create_device_encrypts_password` + `create_device_stores_encrypted`; POST returns 201 and GET shows device without password |
| 2   | Admin can edit, disable, or send ISAPI commands to any registered device (ROADMAP SC-2)                                         | VERIFIED   | `dispatch_door_open_writes_audit`, `patch_updates_password_and_reencrypts`, `deactivate_sets_status_inactive_and_deleted_at` — all passing |
| 3   | System maintains persistent alertStream connections and auto-reconnects when device drops (ROADMAP SC-3, EVT-01, EVT-02)        | VERIFIED   | `connect_and_stream_persists_one_event`, `reconnect backoff` contract test (`doubling_from_initial_caps_at_60s_in_nine_steps`); `src/supervisor/task.rs` reconnect loop with 1s→60s ± 25% jitter |
| 4   | Every event stored with UTC epoch; duplicate events within 30s from same employee silently discarded (ROADMAP SC-4, EVT-03/04)  | VERIFIED   | `src/db/migrations/004_attendance_events.sql` composite UNIQUE; `persist_dedup_within_30s` + `persist_cross_device_within_30s` + `persist_epoch_is_utc_integer` passing |
| 5   | Device connection status (online/offline) readable from API (ROADMAP SC-5, DEV-02)                                              | VERIFIED   | `DeviceResponse` exposes `connection_state` and `last_seen_at`; watchdog flips stale devices (`watchdog_flips_device_offline_after_90s`) |
| 6   | POST /api/v1/devices returns 201 with body containing NO password field                                                         | VERIFIED   | `DeviceResponse` struct (lines 21-38 of `models.rs`) has no password field by construction; `create_device_encrypts_password` asserts JSON lacks password |
| 7   | Device password round-trips through AES-256-GCM with tamper detection                                                           | VERIFIED   | `src/devices/crypto.rs` unit tests: `encrypt_then_decrypt`, `tampered_ciphertext_fails`, `wrong_key_fails`, `nonce_is_random` (all passing) |
| 8   | Admin can PATCH device fields (ip/port/username/password/direction/status/allow_insecure_tls)                                   | VERIFIED   | `patch_updates_password_and_reencrypts` + `patch_requires_correct_version` + `patch_ip_emits_restart_event` |
| 9   | DELETE soft-deletes (status=inactive, deleted_at set)                                                                           | VERIFIED   | `deactivate_sets_status_inactive_and_deleted_at`, `deactivate_soft_delete_idempotent` |
| 10  | Command dispatch returns 200 / 504 / 502 with audit row written on every branch                                                 | VERIFIED   | `dispatch_door_open_writes_audit`, `dispatch_timeout_returns_504`, `dispatch_bad_gateway_on_500`; all three outcomes insert into `command_audit_log` |
| 11  | Viewer and Supervisor receive 403 on mutation/command endpoints                                                                 | VERIFIED   | `dispatch_viewer_forbidden`, `dispatch_supervisor_forbidden` |
| 12  | DEVICE_CREDS_KEY validated as 32 bytes (base64) at Config::from_env                                                             | VERIFIED   | `src/config.rs:100-113` decode + length check; integration test invokes this via `test_device_creds_key()` helper |
| 13  | INSERT OR IGNORE enforces dedup as DB invariant; photo written only when rows_affected=1                                         | VERIFIED   | `persist_photo_written_on_insert` + `persist_photo_skipped_on_dedup` passing; `src/events/service.rs:64` + line 88-92 gates write_photo_atomic on inserted path |
| 14  | Unknown face events persist with employee_id=NULL, is_unknown=1, face_id set                                                    | VERIFIED   | `persist_unknown_face_sets_is_unknown` + `unknown_face_persists_with_is_unknown` |
| 15  | GET /api/v1/events returns PaginatedResponse with limit/offset/employee_id/device_id/from/to filters                            | VERIFIED   | `list_events_filters_by_employee_id`, `list_events_filters_by_device_id`, `list_events_filters_by_time_range`, `list_events_pagination_clamps_limit` |
| 16  | GET /api/v1/events/:id/photo streams JPEG with Content-Type image/jpeg; rejects path traversal                                  | VERIFIED   | `get_event_photo_returns_jpeg_bytes`, `get_event_photo_404_if_file_missing`, `get_event_photo_rejects_path_traversal` |
| 17  | Viewer role can read /events and photo endpoints                                                                                | VERIFIED   | `list_events_viewer_can_read`; `/events` routes are in viewer_routes group (`main.rs:92-94`) |
| 18  | Wave 0 fixtures exist (mock Hikvision TCP server + 3 canned multipart samples)                                                  | VERIFIED   | `tests/common/mock_hikvision.rs` with `spawn_mock_hikvision_plain/digest/401`; three .bin fixtures present |
| 19  | Heartbeat frames update last_seen_at but do NOT persist as events                                                               | VERIFIED   | `heartbeat_updates_last_seen_at_and_does_not_persist`; `is_heartbeat_detects_videoloss_inactive`, `is_heartbeat_detects_explicit_heartbeat` |
| 20  | Reconnect uses exponential backoff 1s→60s with ±25% jitter, cancellation-aware                                                  | VERIFIED   | `src/supervisor/task.rs:27-40` constants INITIAL=1000ms, MAX=60_000ms; `sleep_ms_with_jitter` adds `backoff_ms/4` jitter; `tokio::select!` with CancellationToken short-circuits sleep; `doubling_from_initial_caps_at_60s_in_nine_steps` test pins 1s→2s→…→60s cap |
| 21  | CRUD emits lifecycle signals: POST→Start, PATCH(connection fields)→Restart, PATCH(name only)→no event, DELETE→Stop              | VERIFIED   | `post_device_emits_start_event`, `patch_ip_emits_restart_event`, `patch_password_emits_restart_event`, `patch_name_only_does_not_emit_restart`, `delete_device_emits_stop_event` |

**Score:** 21/21 truths verified

### Required Artifacts

| Artifact                                                        | Expected                                                     | Status     | Details |
| --------------------------------------------------------------- | ------------------------------------------------------------ | ---------- | ------- |
| `backend/src/db/migrations/003_devices.sql`                     | devices + device_face_mappings tables                        | VERIFIED   | 43 lines; contains `CREATE TABLE IF NOT EXISTS devices`, `CREATE UNIQUE INDEX idx_devices_ip_port_active`, `CREATE TABLE IF NOT EXISTS device_face_mappings` |
| `backend/src/db/migrations/004_attendance_events.sql`           | attendance_events + composite UNIQUE dedup index             | VERIFIED   | 27 lines; `idx_attendance_dedup` on (employee_id, device_id, direction, bucket_30s); `raw_xml TEXT NOT NULL` |
| `backend/src/db/migrations/005_command_audit_log.sql`           | append-only command audit table                              | VERIFIED   | 20 lines; `CREATE TABLE IF NOT EXISTS command_audit_log` with dispatched_at/completed_at/outcome |
| `backend/src/db/migrations/006_devices_audit_triggers.sql`      | INSERT/UPDATE/DELETE audit triggers; ciphertext scrubbed      | VERIFIED   | 58 lines; all three triggers present; `encrypted_password` column string NOT in file (grep count = 0) |
| `backend/src/devices/crypto.rs`                                 | encrypt_password/decrypt_password/load_key_from_env          | VERIFIED   | 174 lines; Aes256Gcm + OsRng 96-bit nonce; 4 inline unit tests |
| `backend/src/devices/models.rs`                                 | DeviceResponse (no password), CreateDeviceRequest, etc.       | VERIFIED   | 198 lines; `DeviceResponse` intentionally omits password field; DeviceWithPlaintext has manual redacting Debug |
| `backend/src/devices/service.rs`                                | create/list/get/update/deactivate/get_decrypted/write_command_audit/list_active | VERIFIED | 545 lines; all functions exported; `encrypt_password` called on create/update paths |
| `backend/src/devices/handlers.rs`                               | 6 handlers + dispatch_command with 10s timeout                | VERIFIED   | 299 lines; `timeout(Duration::from_secs(10), ...)` on all 3 commands; `write_command_audit` called on every exit branch |
| `backend/src/isapi/client.rs`                                   | DeviceConnection with diqwest digest auth                     | VERIFIED   | 142 lines; door_open/reboot/enrollment_mode; `send_digest_auth((user, pass))` |
| `backend/src/isapi/events.rs`                                   | EventNotificationAlert + AccessControllerEvent serde + helpers | VERIFIED  | 246 lines; `strip_xmlns`, `is_heartbeat`, `direction_for_attendance_status`; 8 unit tests |
| `backend/src/isapi/parser.rs`                                   | multer parser + line-scan fallback                            | VERIFIED   | 227 lines; `parse_buffer` with `SizeLimit::new().per_field(10MB).whole_stream(64MB)`; `parse_line_scan_fallback`; 7 unit tests |
| `backend/src/isapi/stream.rs`                                   | connect_and_stream: digest auth → multer → persist pipeline   | VERIFIED   | 323 lines; `quick_xml::de::from_str`, `WithDigestAuth`, `SizeLimit` 10MB/1GiB; calls persist_attendance_event + lookup_employee_for_event + touch_last_seen + update_connection_state |
| `backend/src/supervisor/mod.rs`                                 | Supervisor with bootstrap + lifecycle mpsc loop               | VERIFIED   | 192 lines; `DeviceLifecycleEvent`, `Supervisor::new`, `run`, `handle_event`; graceful drain |
| `backend/src/supervisor/task.rs`                                | device_task with 1s→60s backoff + ±25% jitter + CancellationToken | VERIFIED | 137 lines; `INITIAL_BACKOFF_MS=1_000`, `MAX_BACKOFF_MS=60_000`; `sleep_ms_with_jitter`; `tokio::select!` |
| `backend/src/supervisor/watchdog.rs`                            | watchdog_task + run_once; flips stale devices offline         | VERIFIED   | 59 lines; SQL `WHERE status='active' AND connection_state != 'offline' AND (last_seen_at IS NULL OR last_seen_at < unixepoch() - 90)` |
| `backend/src/supervisor/status.rs`                              | update_connection_state + touch_last_seen                     | VERIFIED   | 43 lines; both functions present |
| `backend/src/events/service.rs`                                 | persist_attendance_event (dedup-safe) + list + get + photo   | VERIFIED   | 652 lines; `INSERT OR IGNORE INTO attendance_events`; `write_photo_atomic` gated on rows_affected==1; 8 unit tests |
| `backend/src/events/models.rs`                                  | AttendanceEventResponse + NewAttendanceEvent + PersistOutcome | VERIFIED   | 57 lines; PersistOutcome enum Inserted/Deduplicated |
| `backend/src/events/handlers.rs`                                | list_events + get_event + get_event_photo                    | VERIFIED   | 103 lines; `canonicalize()` + `starts_with(&root_canonical)` path-traversal defense |
| `backend/tests/common/mock_hikvision.rs` (plan said `fixtures/`)| mock Hikvision TCP server                                     | VERIFIED (path deviation documented) | 9.6K; spawn_mock_hikvision_plain/digest/401; placement in `tests/common/` is a documented SUMMARY 02-02 decision to match the existing shared-test-module layout |
| `backend/tests/fixtures/alertstream_k1t341.bin`                 | canned DS-K1T341 multipart body                               | VERIFIED   | 1172 B |
| `backend/tests/fixtures/alertstream_heartbeat.bin`              | canned heartbeat multipart body                               | VERIFIED   | 573 B |
| `backend/tests/fixtures/alertstream_unknown_face.bin`           | canned unknown-face multipart body                            | VERIFIED   | 1162 B |
| `backend/tests/device_tests.rs`                                 | DEV-01..04 integration tests                                  | VERIFIED   | 924 lines; all required test names present and passing |
| `backend/tests/event_tests.rs`                                  | EVT-03/04 read-API tests                                       | VERIFIED   | 568 lines; 12 integration tests |
| `backend/tests/listener_tests.rs`                               | stream consumer integration                                   | VERIFIED   | 392 lines; 10 integration tests |
| `backend/tests/supervisor_tests.rs`                             | supervisor lifecycle/reconnect/watchdog                       | VERIFIED   | 870 lines; 14 integration + 1 backoff unit |
| `backend/src/errors.rs`                                         | Timeout (504) + BadGateway (502) variants                    | VERIFIED   | Both variants present; IntoResponse mapping verified |

### Key Link Verification

Links manually verified because gsd-tools parses `path::function` strings literally and reports source file not found (false negative). Each link confirmed by grep in the intended source file.

| From                                                      | To                                                   | Via                                                        | Status  | Details |
| --------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------------------------------- | ------- | ------- |
| `backend/src/main.rs`                                     | `/api/v1/devices` router                             | viewer + admin groups via require_admin/require_auth       | WIRED   | `main.rs:90-91` viewer; `main.rs:115-118` admin |
| `backend/src/main.rs`                                     | `/api/v1/events` router group                        | viewer_routes with require_auth                            | WIRED   | `main.rs:92-94` |
| `backend/src/main.rs`                                     | `Supervisor::run` + `CancellationToken`              | spawned before axum::serve; ctrl_c cancels                 | WIRED   | `main.rs:47,57-60,142-150` |
| `backend/src/main.rs`                                     | `watchdog::watchdog_task`                            | spawned with shutdown child token                          | WIRED   | `main.rs:63-69` |
| `backend/src/devices/service.rs::create`                  | `crypto::encrypt_password`                           | encrypt before INSERT                                      | WIRED   | `service.rs:85` and `service.rs:274` (update path) |
| `backend/src/devices/handlers.rs::dispatch_command`       | `DeviceConnection`                                   | `timeout(Duration::from_secs(10), ...)`                    | WIRED   | `handlers.rs:253-256` all 3 commands wrapped |
| `backend/src/devices/handlers.rs::dispatch_command`       | `command_audit_log`                                  | `service::write_command_audit` on every branch             | WIRED   | `handlers.rs:272` |
| `backend/src/config.rs::Config::from_env`                 | `DEVICE_CREDS_KEY` env var                           | base64 decode + 32-byte length check                       | WIRED   | `config.rs:100-113` |
| `backend/src/devices/handlers.rs` (create/update/deactivate) | `DeviceLifecycleEvent`                            | `emit_lifecycle` helper sending via `lifecycle_tx`          | WIRED   | `handlers.rs:88,185,203` |
| `backend/src/events/service.rs::persist_attendance_event` | `attendance_events` UNIQUE index                     | `INSERT OR IGNORE` + rows_affected                          | WIRED   | `service.rs:64` |
| `backend/src/events/service.rs::persist_attendance_event` | `./data/events/YYYY-MM-DD/{id}.jpg`                  | `write_photo_atomic` only if rows_affected==1              | WIRED   | `service.rs:88-93,103` |
| `backend/src/events/service.rs::lookup_employee_for_event`| `device_face_mappings`                               | SELECT with device_id + face_id                            | WIRED   | `service.rs:134` |
| `backend/src/supervisor/task.rs::device_task`             | `connect_and_stream`                                 | inner tokio::select!; errors trigger backoff               | WIRED   | `task.rs:22,66` |
| `backend/src/isapi/stream.rs::connect_and_stream`         | `persist_attendance_event`                           | per parsed non-heartbeat event                             | WIRED   | `stream.rs:306` |
| `backend/src/supervisor/watchdog.rs`                      | `devices.connection_state`                           | UPDATE … WHERE last_seen_at < unixepoch()-90               | WIRED   | `watchdog.rs` run_once |

### Data-Flow Trace (Level 4)

| Artifact                            | Data Variable                      | Source                                                                                   | Produces Real Data | Status   |
| ----------------------------------- | ---------------------------------- | ---------------------------------------------------------------------------------------- | ------------------ | -------- |
| devices API (list/get)              | DeviceResponse rows                | `SELECT … FROM devices` populated by CRUD handlers                                        | Yes                | FLOWING  |
| events API (list/get)               | AttendanceEventResponse rows       | `INSERT OR IGNORE INTO attendance_events` written by `connect_and_stream` ingest loop    | Yes                | FLOWING  |
| supervisor task map                 | HashMap<id, (JoinHandle, Cancel)>  | `list_active` at bootstrap + mpsc Start/Stop/Restart from CRUD handlers                  | Yes                | FLOWING  |
| connection_state column             | text online/offline                | writers are `update_connection_state` (stream task) + watchdog `run_once`                 | Yes                | FLOWING  |
| event photo bytes                   | JPEG on disk                       | written by `write_photo_atomic` only when INSERT succeeds; served via canonicalize guard  | Yes                | FLOWING  |
| command_audit_log rows              | audit outcome rows                 | `dispatch_command` handler writes on every branch (Ok/Err/Timeout)                        | Yes                | FLOWING  |

No HOLLOW or DISCONNECTED data flows found. Every visible artifact has a demonstrated upstream writer exercised by tests.

### Behavioral Spot-Checks

| Behavior                                                | Command                                                | Result      | Status |
| ------------------------------------------------------- | ------------------------------------------------------ | ----------- | ------ |
| Full suite compiles cleanly                             | `cargo check --all-targets`                            | exit 0      | PASS   |
| Full test suite passes                                  | `cargo test --all-targets`                             | 139 passed, 1 ignored (Phase 1 marker), 0 failing across 11 suites in 13.72s | PASS |
| Crypto unit tests (round-trip/tamper/nonce/wrong key)   | via `cargo test`                                       | all 4 pass  | PASS   |
| Listener integration (stream → persist)                 | via `cargo test --test listener_tests`                 | 10/10 pass  | PASS   |
| Supervisor integration (lifecycle/reconnect/watchdog)   | via `cargo test --test supervisor_tests`               | 14 int + 1 backoff unit pass | PASS |
| No regressions in Phase 1 suites                        | `cargo test` includes auth/department/employee/rules/db/event | all green | PASS |

### Requirements Coverage

All 8 requirement IDs declared in plan frontmatter cross-referenced against REQUIREMENTS.md:

| Requirement | Source Plan(s) | Description                                                                                        | Status    | Evidence |
| ----------- | -------------- | -------------------------------------------------------------------------------------------------- | --------- | -------- |
| DEV-01      | 02-01          | Admin can register a Hikvision device with IP, ISAPI credentials, direction                        | SATISFIED | `create_device_encrypts_password` + POST /api/v1/devices returns 201 |
| DEV-02      | 02-01, 02-02, 02-03 | Admin can view real-time connection status of all registered devices                          | SATISFIED | `DeviceResponse.connection_state` + `last_seen_at`; supervisor touches; watchdog flips to offline |
| DEV-03      | 02-01          | Admin can send ISAPI commands (door open, reboot, enrollment mode)                                 | SATISFIED | `dispatch_door_open_writes_audit`, `dispatch_timeout_returns_504`, `dispatch_bad_gateway_on_500` |
| DEV-04      | 02-01, 02-03   | Admin can edit or disable a registered device                                                      | SATISFIED | `patch_updates_password_and_reencrypts`, `patch_ip_emits_restart_event`, `delete_device_emits_stop_event` |
| EVT-01      | 02-03          | System maintains persistent alertStream connections                                                | SATISFIED | `Supervisor` boots tasks per active device; `connect_and_stream_persists_one_event` |
| EVT-02      | 02-03          | System auto-reconnects when alertStream drops                                                      | SATISFIED | `sleep_ms_with_jitter` + `tokio::select!` reconnect loop; backoff contract test |
| EVT-03      | 02-02          | System deduplicates events within a 30-second window from the same employee                        | SATISFIED | Composite UNIQUE index + `INSERT OR IGNORE`; `persist_dedup_within_30s`, `persist_cross_device_within_30s` |
| EVT-04      | 02-02          | System stores raw events with UTC epoch timestamps                                                 | SATISFIED | `captured_at INTEGER` + `raw_xml TEXT NOT NULL`; `persist_epoch_is_utc_integer`, `persist_raw_xml_round_trip` |

**Cross-check against REQUIREMENTS.md traceability table:**
- Phase 2 rows: DEV-01, DEV-02, DEV-03, DEV-04, EVT-01, EVT-02, EVT-03, EVT-04 — all 8 present.
- No ORPHANED requirements: every REQUIREMENTS.md Phase-2 entry is claimed by at least one plan.

### Anti-Patterns Found

Surface scan on all Phase 2 files modified per SUMMARY key-files lists.

| File                                         | Line | Pattern                                                              | Severity | Impact |
| -------------------------------------------- | ---- | -------------------------------------------------------------------- | -------- | ------ |
| `backend/src/db/migrations/006_devices_audit_triggers.sql` | 28-42 | `audit_devices_update` has no WHEN clause — fires on every UPDATE including `last_seen_at` bumps | Warning (REVIEW WR-01) | Write amplification in audit_log proportional to live-event volume; does NOT block Phase 2 goal; flagged in 02-REVIEW.md for follow-up. |
| `backend/src/isapi/events.rs`                | 83-89 | `direction_for_attendance_status` pairs `overtimeIn` with `overTimeOut` (casing asymmetry) | Warning (REVIEW WR-02) | A subset of real overtime-exit punches may fall through to the `_ => "entry"` default; unit test enshrines one casing only. Non-blocking — fallback is still "entry" which is conservative; flagged for follow-up. |
| `backend/src/devices/handlers.rs` + `src/isapi/client.rs` | — | Reboot dropped connection surfaces as Err; audit row can be written with outcome=error even on successful reboot | Warning (REVIEW WR-03) | Operator confusion on reviewing command_audit_log; non-blocking — reboot still succeeds physically. Flagged for follow-up. |
| `backend/src/isapi/stream.rs`                | 133-141 | `connect_and_stream` has no response-headers timeout (body is intentionally unbounded) | Warning (REVIEW WR-04) | A hostile device that opens TCP/TLS but never writes status line can stall a supervisor task indefinitely; cancellation only short-circuits body awaits. Low real-world probability; flagged for follow-up. |
| `backend/src/supervisor/mod.rs`              | 116-137 | `spawn_device` drops lock between contains_key check and insert (TOCTOU) | Info (REVIEW WR-05) | Benign today because `run()` serialises events; could duplicate tasks if future fan-out is introduced. Flagged for follow-up. |
| `backend/tests/common/mod.rs`                | 26-42 | `test_db()` creates `/tmp/cronometrix_test_*.db` files and never deletes them | Info (REVIEW IN-04) | CI cruft accumulation on long-running runners; non-blocking. |
| `backend/src/devices/crypto.rs`              | 22   | Unused `_Cipher` type alias retained as a grep anchor for security audits | Info (IN-03) | Dead code; intentional per SUMMARY decision #5. |
| `backend/src/devices/crypto.rs`              | 63   | `decrypt_password` length precondition is `> 12` (valid AES-GCM is >= 28) | Info (IN-01) | `aes-gcm` rejects shorter input anyway; cosmetic. |

None are blockers for Phase 2 goal achievement. All appear in 02-REVIEW.md as `warning` or `info` severity, not `critical`.

### Human Verification Required

Three items need human testing — all related to real hardware behavior that cannot be exercised in CI.

#### 1. Hardware smoke — real DS-K1T341 emits an event via alertStream

**Test:** Register a real Hikvision device via POST /api/v1/devices with valid credentials. Physically perform a face scan on the device.
**Expected:** Exactly one row appears in `attendance_events` within a few seconds, with `employee_id` resolved from `device_face_mappings` (or `is_unknown=1` if face_id is not enrolled yet), `direction` set from `attendanceStatus`, `raw_xml` non-empty, and `photo_path` pointing at a saved JPEG under `./data/events/YYYY-MM-DD/`.
**Why human:** Requires real hardware on the LAN. Mock fixtures cover the logic path but not real firmware emission quirks.

#### 2. Reconnect under real network drop

**Test:** Power-cycle a registered device while the supervisor is running; observe supervisor + stream logs.
**Expected:** `WARN stream ended with error …` appears within the TCP timeout, followed by `DEBUG reconnect backoff backoff_ms=1000 → 2000 → … → 60000` lines capped at 60s. After the device recovers, `connection_state` flips to `online` and `last_seen_at` refreshes.
**Why human:** Requires physically power-cycling a device. Mock fixtures cover the logic path but not real TCP RST/reset timing on production hardware.

#### 3. Dashboard-style real-time feel

**Test:** `GET /api/v1/devices` repeatedly during a real connect/disconnect cycle.
**Expected:** `connection_state` transitions (offline → online → offline) within seconds of the physical event on the device.
**Why human:** End-to-end perceived latency between physical action and API read is a UX property; automated tests assert individual writes but not the human-perceived roundtrip.

### Gaps Summary

**No implementation gaps found.** All 21 observable truths are VERIFIED, all artifacts exist and are substantive and wired, all key links are connected with data actively flowing through them, and all 8 requirement IDs are SATISFIED. Zero test failures across 139 tests; Phase 1 regressions are absent.

Five anti-pattern warnings from 02-REVIEW.md (WR-01..05) persist in the codebase but are non-blocking for Phase 2 goal achievement (write-amplification, casing asymmetry, reboot audit false-negative, missing response-headers timeout, spawn TOCTOU). They are documented and tracked; they do not prevent the system from delivering the Phase 2 goal.

The overall status is **human_needed** because Phase 2's goal includes live hardware integration behavior that can only be validated against a real DS-K1T341 device. All software paths are green; the manual verification items above close the loop with physical hardware per 02-VALIDATION.md § Manual-Only Verifications.

---

_Verified: 2026-04-20T04:08:18Z_
_Verifier: Claude (gsd-verifier)_
