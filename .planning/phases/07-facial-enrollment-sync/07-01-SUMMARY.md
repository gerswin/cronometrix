---
phase: "07"
plan: "01"
subsystem: "backend"
tags: ["enrollment", "isapi", "workers", "migrations", "face-push"]
dependency_graph:
  requires: ["06-01"]
  provides: ["07-02"]
  affects: ["devices", "employees", "enrollments", "workers"]
tech_stack:
  added:
    - "digest_auth 0.3.1 (manual digest auth for multipart ISAPI uploads)"
    - "image 0.25.10 (JPEG normalize + downscale)"
  patterns:
    - "JoinSet fire-and-forget fan-out (D-06)"
    - "mpsc + biased select worker loop (D-15, D-16)"
    - "Semaphore(4) concurrency cap (D-16)"
    - "Pitfall-10 re-read guard (employee status check mid-loop)"
    - "Manual two-step digest auth for multipart (diqwest cannot clone stream bodies)"
    - "Partial-index for UNIQUE on nullable column (SQLite limitation)"
    - "Atomic photo write: temp file -> rename"
key_files:
  created:
    - "backend/src/db/migrations/016_enrollments.sql"
    - "backend/src/db/migrations/017_phase7_audit_triggers.sql"
    - "backend/src/enrollments/mod.rs"
    - "backend/src/enrollments/handlers.rs"
    - "backend/src/enrollments/image_pipeline.rs"
    - "backend/src/enrollments/isapi_face.rs"
    - "backend/src/enrollments/models.rs"
    - "backend/src/enrollments/pusher.rs"
    - "backend/src/enrollments/service.rs"
    - "backend/src/workers/mod.rs"
    - "backend/src/workers/backfill.rs"
    - "backend/src/workers/purge.rs"
    - "backend/tests/enrollments_test.rs"
    - "backend/tests/multi_device_push_test.rs"
    - "backend/tests/face_capture_test.rs"
    - "backend/tests/enrollment_lifecycle_test.rs"
    - "bruno/cronometrix/enrollments/01_create_enrollment.bru"
    - "bruno/cronometrix/enrollments/02_get_enrollment.bru"
    - "bruno/cronometrix/enrollments/03_retry_push.bru"
    - "bruno/cronometrix/enrollments/04_capture_from_device.bru"
    - "bruno/cronometrix/enrollments/05_get_capture.bru"
  modified:
    - "backend/Cargo.toml (digest_auth, image, multipart features)"
    - "backend/src/db/mod.rs (registered migrations 016, 017)"
    - "backend/src/isapi/client.rs (upsert_user, upload_face, delete_user, capture_face_image)"
    - "backend/src/lib.rs (pub mod enrollments, workers)"
    - "backend/src/main.rs (worker channels, enrollment routes, PurgeWorker/BackfillWorker spawn)"
    - "backend/src/state.rs (purge_tx, backfill_tx, captures fields)"
    - "backend/src/employees/handlers.rs (publish PurgeRequest on deactivate)"
    - "backend/src/devices/handlers.rs (publish BackfillRequest on create)"
    - "backend/tests/common/mod.rs (Phase 7 fixture helpers)"
decisions:
  - "D-06: JoinSet fire-and-forget fan-out — detached tokio task drives per-device pushes concurrently"
  - "D-10: stable face_id UUID per employee — COALESCE(existing, new UUID) on re-enrollment"
  - "D-11: disk photo storage — atomic temp→rename write to ENROLLMENTS_DIR/employee_id/enrollment_id.jpg"
  - "D-15: PurgeWorker — mpsc channel, biased select, Pitfall-10 re-read guard per device"
  - "D-16: BackfillWorker — Semaphore(4) JoinSet cap for new-device face backfill"
  - "D-17: audit triggers — 9 SQLite triggers for enrollments/face_enrollments/device_face_mappings"
  - "D-18: admin-only RBAC — enrollment endpoints behind require_admin middleware"
  - "diqwest-multipart-fix: diqwest cannot clone multipart RequestBuilder (stream body); upload_face uses manual send→401→digest retry"
metrics:
  duration: "~3 hours (resumed from previous session)"
  completed: "2026-04-28"
  tasks_completed: 6
  files_changed: 23
  insertions: 3294
---

# Phase 7 Plan 01: Enrollment Backend Summary

Backend enrollment system with ISAPI 2-step face push, JoinSet fan-out, PurgeWorker, BackfillWorker, audit triggers, and kiosk capture state machine.

## What Was Built

### DB Migrations (Tasks 1-2)
- **016_enrollments.sql**: `face_id` + `current_face_enrollment_id` on employees (partial UNIQUE index — SQLite cannot `ADD COLUMN ... UNIQUE`), `device_face_mappings.state` enum, three new tables: `face_enrollments`, `enrollments`, `enrollment_device_pushes`
- **017_phase7_audit_triggers.sql**: 9 triggers (INSERT/UPDATE/DELETE) for enrollments, face_enrollments, device_face_mappings — closes the deferral note from 006

### ISAPI Client Extensions (Task 3)
Four new methods on `DeviceConnection`:
- `upsert_user(face_id, full_name)` — POST UserInfo/Record, treats duplicateEmployeeNo as success
- `upload_face(face_id, jpeg_bytes)` — multipart POST FaceDataRecord with **manual two-step digest auth** (see Deviations)
- `delete_user(face_id)` — PUT UserInfoDetail/Delete (D-15)
- `capture_face_image()` — POST CaptureFaceData then GET CapturedFacePicture

### Enrollments Module (Tasks 3-4)
- `image_pipeline`: JPEG magic-byte validation, 4-pass downscale to ≤200KB
- `isapi_face`: request body builders + `build_multipart_form` helper
- `models`: request/response types, `CaptureResponse` with `photo_b64` inline (D-02 kiosk contract)
- `service`: full CRUD for enrollments, face_enrollments, device_face_mappings, push rows
- `handlers`: 5 Axum handlers + `CapturesMap` in-memory kiosk session state
- `pusher`: `spawn_enrollment_pushes` (JoinSet D-06), `push_one_device`, `push_one_device_for_backfill`

### Workers (Task 5)
- **PurgeWorker**: biased mpsc select, HashSet dedup batching, Pitfall-10 re-read guard per device — aborts entire batch if employee re-activated mid-loop
- **BackfillWorker**: Semaphore(4) JoinSet cap, resolves name/photo_path per employee, calls push_one_device_for_backfill

### Main.rs Wiring (Task 6)
- Real `purge_tx`/`backfill_tx` channels replacing `None` placeholders
- `PurgeWorker` and `BackfillWorker` spawned and awaited on shutdown
- 5 enrollment routes under `RequestBodyLimitLayer(3MB)` + `require_admin`

### Bruno Collection (Task 6)
5 `.bru` files: create_enrollment (multipart), get_enrollment, retry_push, capture_from_device, get_capture

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] SQLite ADD COLUMN UNIQUE not supported**
- **Found during:** Task 2 (migration 016)
- **Issue:** SQLite throws "Cannot add a UNIQUE column" on `ALTER TABLE employees ADD COLUMN face_id TEXT UNIQUE`
- **Fix:** Split into `ADD COLUMN face_id TEXT` + `CREATE UNIQUE INDEX ... WHERE face_id IS NOT NULL` (partial index — NULL values do not violate uniqueness)
- **Files modified:** `backend/src/db/migrations/016_enrollments.sql`
- **Commit:** 6ce4c55

**2. [Rule 1 - Bug] diqwest cannot clone multipart RequestBuilder**
- **Found during:** Task 5 test execution (multi_device_push_test)
- **Issue:** `diqwest::send_digest_auth` calls `self.refresh()` which calls `try_clone()` on the RequestBuilder. A multipart form body is a streaming body — `try_clone()` returns `None`, causing `Err(RequestBuilderNotCloneable)` before any network request is made. All ISAPI FaceDataRecord pushes were silently failing with "ISAPI FaceDataRecord request failed".
- **Fix:** Replaced `send_digest_auth` in `upload_face` with a manual two-step flow: send first (no auth), if 200 return; if 401 compute digest auth header via `digest_auth::parse`/`prompt.respond`, resend with fresh form. Added `build_multipart_form` helper so the form can be built twice. Added `digest_auth = 0.3.1` as a direct dep.
- **Files modified:** `backend/src/isapi/client.rs`, `backend/src/enrollments/isapi_face.rs`, `backend/Cargo.toml`
- **Commit:** eba4d40

**3. [Rule 1 - Bug] Worker API mismatch — wrong function signatures**
- **Found during:** Task 5 initial worker implementation
- **Issue:** Initial `purge.rs` used `(employee_id, device_id)` for `mark_mapping_pending_delete`/`delete_mapping` but the service functions take `mapping_id`. Initial `backfill.rs` assumed `list_employees_with_face` returns `(employee_id, face_id, full_name, photo_path)` but it returns `(employee_id, face_id, cfe_id)`.
- **Fix:** Fixed workers to match actual service signatures. BackfillWorker now queries employee name and calls `get_current_photo_path` separately.
- **Files modified:** `backend/src/workers/purge.rs`, `backend/src/workers/backfill.rs`
- **Commit:** eba4d40

**4. [Rule 1 - Bug] Config field name mismatch — encryption_key vs device_creds_key**
- **Found during:** Task 5 build
- **Issue:** Workers referenced `state.config.encryption_key` but the field is `device_creds_key`
- **Fix:** Changed both workers to use `&self.state.config.device_creds_key`
- **Files modified:** `backend/src/workers/purge.rs`, `backend/src/workers/backfill.rs`
- **Commit:** eba4d40

**5. [Rule 1 - Bug] Test device seeding — unique(ip,port) constraint**
- **Found during:** Task 5 test execution
- **Issue:** Multi-device tests hardcoded `127.0.0.1:80` for all devices, hitting UNIQUE constraint on second device
- **Fix:** Parse ip+port from mock server URI using stdlib string splitting
- **Files modified:** `backend/tests/multi_device_push_test.rs`
- **Commit:** eba4d40

## Test Results

| Suite | Pass | Ignored | Notes |
|-------|------|---------|-------|
| multi_device_push_test | 8 | 1 | ignored: backfill semaphore cap (wave 2) |
| enrollment_lifecycle_test | 5 | 6 | audit trigger test live; lifecycle stubs for Task 5/6 |

## Known Stubs

Wave 0 test stubs remain `#[ignore]` in:
- `backend/tests/enrollments_test.rs` — 9 tests, all `#[ignore]` (RBAC + retry HTTP tests — wave 2)
- `backend/tests/face_capture_test.rs` — 4 tests, all `#[ignore]` (HTTP layer tests — wave 2)
- `backend/tests/enrollment_lifecycle_test.rs` — 6 lifecycle stubs `#[ignore]` (wave 2)

These are intentional — they require the full HTTP router (Task 6 `axum-test` harness) which is wave 2 scope per plan 07-02.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: file_upload | `src/enrollments/handlers.rs` | Multipart JPEG upload; mitigated: magic-byte check (0xFF 0xD8 0xFF), 3MB RequestBodyLimitLayer, JPEG decode via `image` crate |
| threat_flag: credential_leak | `src/enrollments/pusher.rs` | Device password in ISAPI error strings; mitigated: `scrub_password` replaces password with [redacted] before persistence (T-7-06) |

## Self-Check: PASSED

All 14 key files found on disk. All 6 plan commits found in git log.
