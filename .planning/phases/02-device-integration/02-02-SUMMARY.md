---
phase: 02-device-integration
plan: 02
subsystem: api
tags: [libsql, sqlite, dedup, insert-or-ignore, multipart, axum, path-traversal, tempfile, chrono]

requires:
  - phase: 02-device-integration
    provides: devices table (03), device_face_mappings table (03), AppError Timeout/BadGateway variants, PaginatedResponse, epoch_to_iso, require_auth middleware

provides:
  - attendance_events table with composite UNIQUE index enforcing 30-second dedup at the DB level
  - events::service::persist_attendance_event — dedup-safe INSERT OR IGNORE with atomic JPEG write on success only
  - events::service::lookup_employee_for_event — face_id -> employee resolution with employeeNoString fallback
  - events::service::{list, get_by_id, get_photo_path} read-side service helpers
  - events::handlers::{list_events, get_event, get_event_photo} with canonicalize-based path traversal defense (T-2-06)
  - GET /api/v1/events (paginated list with employee_id/device_id/from/to/include_unknown filters)
  - GET /api/v1/events/:id (single event)
  - GET /api/v1/events/:id/photo (streams JPEG bytes, 404 on missing)
  - Wave 0 fixtures: tokio TCP mock Hikvision helper (plain variant) + three canned multipart bodies (k1t341, heartbeat, unknown_face)
  - backend/tests/common/mod.rs helpers: build_multipart_fixture, MINI_JPEG, k1t341_event_xml, heartbeat_event_xml, unknown_face_event_xml, ensure_fixtures_present

affects: [02-03-supervisor-listener, 03-attendance-calculation, 04-dashboard]

tech-stack:
  added: [tempfile 3, bytes 1]
  patterns:
    - "Dedup-as-DB-invariant: composite UNIQUE(employee_id, device_id, direction, bucket_30s) + INSERT OR IGNORE + rows_affected check"
    - "Atomic disk write: tempfile + fsync + rename; invoked ONLY when rows_affected == 1 so dedup never leaves orphans"
    - "Process-wide env-var serialization: EventsRootGuard pairs a static Mutex with a TempDir so tests that mutate CRONOMETRIX_EVENTS_ROOT never race"
    - "Defense-in-depth path handling: reject .. and / in stored path, canonicalize to absolute, verify starts_with(root_canonical)"
    - "raw_xml excluded from API responses (T-2-14) — kept for forensic re-parsing (D-12) but never exposed"

key-files:
  created:
    - backend/src/db/migrations/004_attendance_events.sql (27 lines)
    - backend/src/events/mod.rs
    - backend/src/events/models.rs (57 lines)
    - backend/src/events/service.rs (652 lines incl. 8 unit tests)
    - backend/src/events/handlers.rs (103 lines)
    - backend/tests/common/mock_hikvision.rs (124 lines)
    - backend/tests/event_tests.rs (567 lines, 12 integration tests)
    - backend/tests/fixtures/alertstream_k1t341.bin (1172 bytes)
    - backend/tests/fixtures/alertstream_heartbeat.bin (573 bytes)
    - backend/tests/fixtures/alertstream_unknown_face.bin (1162 bytes)
  modified:
    - backend/Cargo.toml (bytes, tempfile dev-deps)
    - backend/Cargo.lock
    - backend/src/lib.rs (expose events module)
    - backend/src/db/mod.rs (register 004_attendance_events)
    - backend/src/main.rs (wire /events routes into viewer_routes)
    - backend/tests/common/mod.rs (multipart helpers + mock_hikvision submodule)
    - .planning/phases/02-device-integration/02-VALIDATION.md (Per-Task Verification Map)

key-decisions:
  - "Bucket arithmetic uses floor(captured_at / 30) — integer division. Tests chose concrete epochs that are arithmetically correct (bucket 33 = [990..=1019]), not the plan's off-by-one sample value of 1029"
  - "photo_path stored relative under events_root so the root is configurable via CRONOMETRIX_EVENTS_ROOT per environment/test"
  - "mock_hikvision helper lives in backend/tests/common/ (not backend/tests/fixtures/) so the `common` shared test crate module re-exports it as common::mock_hikvision::spawn_mock_hikvision_plain — matches the existing common/ layout"
  - "Fixture .bin files are checked in as committed bytes (produced via a deterministic recipe) so CI never has to regenerate them — ensure_fixtures_present is offered as a safety net but never invoked by runtime code"

patterns-established:
  - "EventsRootGuard: static Mutex + TempDir + env var swap — reusable for any env-var-sensitive test in later phases"
  - "Dedup-safe persist helper signature: Connection + NewEvent -> PersistOutcome { Inserted { photo_path }, Deduplicated }"
  - "Photo handler canonicalization: services the file ONLY if the stored relpath is safe AND canonicalize stays under root — 404 on any mismatch"

requirements-completed: [EVT-03, EVT-04, DEV-02]

duration: 60min
completed: 2026-04-19
---

# Phase 2 Plan 02: Attendance Events Store Summary

**Dedup-as-DB-invariant attendance_events schema + INSERT-OR-IGNORE persist helper with atomic JPEG writes + viewer-readable GET /events read API with canonicalize-based path traversal defense**

## Performance

- **Duration:** ~60 min
- **Tasks:** 2 (both TDD)
- **Files created:** 10
- **Files modified:** 7
- **Tests added:** 20 (8 unit + 12 integration)
- **Regression:** 88 total tests pass (all 68 pre-existing Phase 1/02-01 tests still green)

## Accomplishments

- attendance_events table with composite UNIQUE index on (employee_id, device_id, direction, bucket_30s) — dedup is now a DB invariant, not an application-layer rule (D-05/D-06)
- persist_attendance_event returns PersistOutcome::{Inserted{photo_path}, Deduplicated} via INSERT OR IGNORE + rows_affected; callers never need to reason about race conditions
- Photo JPEG is written via a tempfile + fsync + rename sequence, AND ONLY when rows_affected == 1 — dedup hits never leave orphan files on disk (D-13)
- Unknown-face events persist with employee_id=NULL, is_unknown=1, face_id set (D-07) — SQLite's UNIQUE(NULL, ...) semantics intentionally let every unknown event through for forensic review
- raw_xml stored verbatim on every event (D-12) but EXCLUDED from API responses (T-2-14) — preserves forensic re-parsing capability without exposing attacker-controlled bytes on the read path
- GET /api/v1/events returns PaginatedResponse<AttendanceEventResponse> with employee_id/device_id/from/to/include_unknown filters; limit is clamped to [1, 100] matching the existing employees/departments idiom
- GET /api/v1/events/:id/photo streams image/jpeg bytes; rejects `..`/absolute paths, then canonicalizes and verifies containment under events_root before reading — 404 on any mismatch, never 500
- Wave 0 fixtures: mock_hikvision::spawn_mock_hikvision_plain serves canned multipart bodies over an ephemeral TCP port; three canned multipart samples on disk for Plan 02-03's listener/parser work
- lookup_employee_for_event codifies the A3 fallback: device_face_mappings first, then employees.employee_code == employeeNoString

## Task Commits

1. **Task 1: migration + persist helper + Wave 0 fixtures** — `dc79fb6` (feat)
   - Migration 004_attendance_events; events module (mod/models/service/handlers stub); persist_attendance_event + lookup_employee_for_event + list/get_by_id/get_photo_path; 8 inline unit tests; mock_hikvision helper + 4 fixture tests; three canned .bin fixtures
2. **Task 2: events read API + integration tests** — `64f734d` (feat)
   - events::handlers implemented; /events routes wired into viewer_routes; 12 integration tests covering list/get/photo behaviors including the path-traversal defense

## Files Created/Modified

### Created

- `backend/src/db/migrations/004_attendance_events.sql` (27 lines) — table + composite UNIQUE + secondary indexes
- `backend/src/events/mod.rs` (3 lines) — module declarations
- `backend/src/events/models.rs` (57 lines) — AttendanceEventResponse, NewAttendanceEvent, PersistOutcome, EventListQuery
- `backend/src/events/service.rs` (652 lines) — persist helper, lookup, list/get/get_photo_path, atomic write, 8 unit tests
- `backend/src/events/handlers.rs` (103 lines) — list_events, get_event, get_event_photo
- `backend/tests/common/mock_hikvision.rs` (124 lines) — tokio TCP mock + fixture-presence tests + smoke test
- `backend/tests/event_tests.rs` (567 lines) — 12 integration tests
- `backend/tests/fixtures/alertstream_k1t341.bin` (1172 B)
- `backend/tests/fixtures/alertstream_heartbeat.bin` (573 B)
- `backend/tests/fixtures/alertstream_unknown_face.bin` (1162 B)

### Modified

- `backend/Cargo.toml` — added `bytes` and `tempfile` to [dev-dependencies]
- `backend/Cargo.lock`
- `backend/src/lib.rs` — `pub mod events;`
- `backend/src/db/mod.rs` — register `004_attendance_events`
- `backend/src/main.rs` — import `events` and wire `/events`, `/events/{id}`, `/events/{id}/photo` into `viewer_routes`
- `backend/tests/common/mod.rs` — added `pub mod mock_hikvision` + multipart/XML/JPEG helpers + `ensure_fixtures_present`
- `.planning/phases/02-device-integration/02-VALIDATION.md` — Per-Task Verification Map populated for Plan 02-02

## Decisions Made

1. **Bucket arithmetic uses exact SQL integer division** — the plan's example `(captured_at=1000, captured_at=1029, same bucket=33)` was arithmetically wrong (1029/30 = 34). The correct boundary is bucket 33 = [990..=1019]. Tests use 1000 and 1019 as the two same-bucket values and 1000/1030 as the adjacent-bucket pair.
2. **CRONOMETRIX_EVENTS_ROOT env var for photo root** — makes tests hermetic (each test gets a TempDir) without polluting the real `./data/events/` path. Production falls back to `./data/events`.
3. **mock_hikvision helper lives in `backend/tests/common/`, not `backend/tests/fixtures/`** — the `common` crate module already re-exports everything the integration tests need; adding a `fixtures/` submodule would require either a third integration test crate or a shared-package dance. Plan 02-03 extends this same file with digest-auth and delay variants.
4. **EventsRootGuard uses a process-wide static Mutex** — Rust `std::env::set_var` is process-global. Integration tests run in the same process (cargo test spawns threads, not processes per test), so tests that swap CRONOMETRIX_EVENTS_ROOT must serialize. The Mutex is held for the lifetime of the guard.
5. **Fixture .bin files are committed bytes, not regenerated per test** — CI reproducibility + git diff visibility for any future fixture change. `ensure_fixtures_present` exists as a dev-time safety net but is never called from production or test code.
6. **Admin token helper kept even though viewer covers the read path** — `_keep_admin_token_alive` placeholder keeps the helper available for Plan 02-03 supervisor tests without triggering dead-code lints.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan's bucket-arithmetic example was arithmetically wrong**
- **Found during:** Task 1 — initial test run of `persist_dedup_within_30s`
- **Issue:** Plan specified "second insert at captured_at=1029 (same bucket=33)" for the dedup test, but floor(1029/30) = 34, not 33. Second insert landed in a different bucket and correctly did NOT dedup, which caused the test to fail.
- **Fix:** Changed test to use `captured_at=1019` for the second event (floor(1019/30) = 33, same bucket as 1000). Also fixed `persist_photo_skipped_on_dedup` (second captured_at 1020 -> 1015, both in bucket 33).
- **Files modified:** `backend/src/events/service.rs` (inline unit tests)
- **Verification:** All 8 persist unit tests pass; composite UNIQUE + INSERT OR IGNORE are exercised correctly.
- **Committed in:** `dc79fb6`

**2. [Rule 3 - Blocking] departments schema mismatch in seed helper**
- **Found during:** Task 1 verify (`cargo test events::service::tests`)
- **Issue:** Initial seed_employee helper inserted into `departments (id, name, description, status, version, created_at, updated_at)` but the Phase 1 schema requires `base_salary_cents, shift_start_time, shift_end_time, lunch_mode, lunch_duration_min` and has no `description` column. Tests panicked at seed time.
- **Fix:** Rewrote seed_employee to INSERT all required columns with sensible defaults ('09:00', '17:00', 'fixed', 60).
- **Files modified:** `backend/src/events/service.rs` (test seed helpers), `backend/tests/event_tests.rs` (same pattern)
- **Verification:** `cargo test` green.
- **Committed in:** `dc79fb6` / `64f734d`

**3. [Rule 3 - Blocking] duplicate (ip, port) in cross-device test**
- **Found during:** Task 1 verify — `persist_cross_device_within_30s` failed
- **Issue:** seed_device hardcoded `ip='10.0.0.1', port=8443`, so seeding two devices in the same test tripped the partial UNIQUE index from Plan 02-01 (idx_devices_ip_port_active).
- **Fix:** Derive a stable port and IP from a cheap hash of the device id so each seeded device gets a unique (ip, port).
- **Files modified:** `backend/src/events/service.rs` (seed_device helper)
- **Verification:** Cross-device test passes; composite UNIQUE on attendance_events still fires for same-device dedup.
- **Committed in:** `dc79fb6`

**4. [Rule 1 - Bug] Marker-length off-by-one in fixture presence check**
- **Found during:** Task 1 verify — `fixture_k1t341_exists_and_contains_event_xml`
- **Issue:** Used `windows(22)` with marker `<EventNotificationAlert` (23 bytes including the `<`).
- **Fix:** Replaced with `const MARKER: &[u8] = b"<EventNotificationAlert"; windows(MARKER.len())`.
- **Files modified:** `backend/tests/common/mock_hikvision.rs`
- **Verification:** All 4 fixture tests pass.
- **Committed in:** `dc79fb6`

---

**Total deviations:** 4 auto-fixed (2× Rule 1 bug, 2× Rule 3 blocking)
**Impact on plan:** All four were arithmetic/schema mismatches between the plan's example code and the actual Phase 1/02-01 runtime. Security model, threat mitigations, and architectural decisions are unchanged.

## Issues Encountered

- cargo-nextest is not installed on this build machine; `cargo test` was used instead. Same assertions, slightly slower output parsing. The acceptance criteria referenced `cargo nextest run ...` but the underlying test framework is identical.
- The plan example in RESEARCH.md uses 10 positional params for the persist helper; the final implementation has 11 (added `employee_no_string` to keep the device-emitted fallback available to Plan 02-03's supervisor without a schema change).

## Handoff Notes for Plan 02-03 (Supervisor / Listener / Parser)

- `cronometrix_api::events::service::persist_attendance_event` is the stable entry point for the listener — call it with a `NewAttendanceEvent` populated from the parsed alertStream XML. The composite UNIQUE index will dedup automatically; callers just inspect the `PersistOutcome` and decide whether to update `last_seen_at` / connection_state.
- `cronometrix_api::events::service::lookup_employee_for_event` resolves (device_id, face_id, employee_no_string) -> Option<employee_id>. When None, build the event with `employee_id: None, is_unknown: true`.
- `backend/tests/common/mock_hikvision::spawn_mock_hikvision_plain` is the MEDIUM-fidelity test double. Plan 02-03 should extend this module with:
  - `spawn_mock_hikvision_digest(...)` that responds 401 + `WWW-Authenticate: Digest ...` on the first request and 200 + multipart body on the retried request
  - `spawn_mock_hikvision_with_delay(...)` that paces part emission to exercise reconnect/backoff
  - `spawn_mock_hikvision_multi_connect(...)` that closes and reopens to verify reconnect paths
- Fixture bytes in `backend/tests/fixtures/*.bin` are consumable directly via `std::fs::read`. The k1t341 sample uses `faceID=42` and `employeeNoString=EMP001`; the unknown_face sample uses `faceID=9999` and an empty employeeNoString.
- `CRONOMETRIX_EVENTS_ROOT` env var overrides the JPEG root. Supervisor code should NOT hardcode paths — always go through `events::service::events_root()`.

## User Setup Required

None — no new environment variables, credentials, or external service wiring. `CRONOMETRIX_EVENTS_ROOT` is optional; prod defaults to `./data/events`.

## Threat Surface Scan

All mitigations from the plan's `<threat_model>` are implemented:

- **T-2-02 (dedup duplicate injection):** composite UNIQUE + INSERT OR IGNORE, pinned by `persist_dedup_within_30s` and `persist_cross_device_within_30s`
- **T-2-06 (photo path traversal):** `photo_path` server-generated from UUID + ISO date; handler rejects `..`/absolute, canonicalizes, verifies root containment — pinned by `get_event_photo_rejects_path_traversal`
- **T-2-13 (SQL injection):** all parameters via `libsql::params!` / `params_from_iter`; no string interpolation of user input
- **T-2-14 (raw_xml leakage):** `AttendanceEventResponse` intentionally excludes `raw_xml`; SELECT column list for the response mapper is a private `const EVENT_SELECT_COLS` that omits it
- **T-2-16 (RBAC on reads):** `require_auth` middleware on viewer_routes; unauthenticated returns 401

No new threat surface beyond the plan. No threat_flags.

## Self-Check: PASSED

- [x] `backend/src/db/migrations/004_attendance_events.sql` exists and contains `CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_dedup`
- [x] `backend/src/db/migrations/004_attendance_events.sql` contains `raw_xml TEXT NOT NULL`
- [x] `grep -q "INSERT OR IGNORE INTO attendance_events" backend/src/events/service.rs` matches
- [x] `grep -q "events::handlers::list_events" backend/src/main.rs` matches
- [x] `grep -q "events::handlers::get_event_photo" backend/src/main.rs` matches
- [x] `grep -q "canonicalize" backend/src/events/handlers.rs` matches
- [x] `grep -q "starts_with(&root_canonical)" backend/src/events/handlers.rs` matches
- [x] `backend/tests/fixtures/alertstream_k1t341.bin` exists
- [x] `backend/tests/fixtures/alertstream_heartbeat.bin` exists
- [x] `backend/tests/fixtures/alertstream_unknown_face.bin` exists
- [x] `grep -q "spawn_mock_hikvision_plain" backend/tests/common/` (recursive) matches in `backend/tests/common/mock_hikvision.rs`
- [x] Task 1 commit `dc79fb6` present in `git log` on this worktree branch
- [x] Task 2 commit `64f734d` present in `git log`
- [x] `cargo check --all-targets` exits 0
- [x] `cargo test` runs 88 tests green (all 20 new + 68 pre-existing — zero regressions)

---
*Phase: 02-device-integration*
*Completed: 2026-04-19*
