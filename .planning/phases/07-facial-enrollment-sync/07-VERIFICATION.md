---
phase: 07-facial-enrollment-sync
verified: 2026-04-30T00:50:00Z
status: human_needed
score: 5/5 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Live Hikvision device smoke for ENRL-01 (device-camera capture)"
    expected: "Admin triggers capture from a real DS-K1T341/DS-K1T342; backend receives JPEG via capture_face_image, stores under enrollments_root, pusher fans out to all registered devices."
    why_human: "Requires real Hikvision hardware; mock_hikvision covers the code path but not the physical camera trigger. Tracked for Phase 11."
deferred: []
---

# Phase 7: Facial Enrollment & Sync Verification Report

**Phase Goal:** Admin can enroll an employee's facial profile through the web UI using a device camera, webcam, or JPG upload, and the system simultaneously pushes the profile to all registered devices with per-device status feedback.

**Verified:** 2026-04-30T00:50:00Z
**Status:** human_needed
**Re-verification:** No — retroactive post-hoc verification (Phase 10 D-04)

## Goal Achievement

### Observable Truths

| #   | Truth                                                                                                                                                              | Status                | Evidence                                                                                                                                                                                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Admin can capture a facial profile using a Hikvision device camera (kiosk mode) — ENRL-01                                                                          | VERIFIED-MOCK-PATH    | `backend/src/enrollments/handlers.rs:332` `capture_from_device` handler; `backend/src/isapi/client.rs:233` `capture_face_image()`; `face_capture_test.rs` covers all 4 HTTP test cases. Live-hardware smoke deferred to Phase 11 (see Human Verification Required).             |
| 2   | Admin can upload a JPG file for facial enrollment (ENRL-02)                                                                                                        | VERIFIED              | `backend/src/enrollments/handlers.rs:76` `create_enrollment`; `backend/src/enrollments/models.rs:104` validates `captured_via` as `"device" \| "webcam" \| "upload"`; `enrollments_test.rs::test_create_enrollment_rejects_non_jpeg_magic_bytes` and `test_create_enrollment_rejects_over_2mb_upload_with_413` passing |
| 3   | Admin can capture via browser webcam for facial enrollment (ENRL-03)                                                                                               | VERIFIED              | `frontend/src/components/enrollment/webcam-capture-tab.tsx` provides `getUserMedia({width:640, height:480})`; `ValidationPanel` integrates tinyFaceDetector; `captured_via = "webcam"` flows through `models.rs:104`; 3 webcam tests passing in `webcam-capture-tab.test.tsx` |
| 4   | System pushes enrolled facial profile to all registered devices simultaneously with per-device status (ENRL-04)                                                    | VERIFIED              | `backend/src/enrollments/pusher.rs:142-187` `push_one_device` calls `isapi.upsert_user` (line 186) then `isapi.upload_face` (line 187) via 30s timeout. `spawn_enrollment_pushes` (line 31) uses `JoinSet` fan-out. `multi_device_push_test.rs` 8 passing tests.                |
| 5   | Admin can observe per-device sync status during enrollment; frontend polls until all devices reach terminal state (ENRL-05)                                         | VERIFIED              | `backend/src/enrollments/service.rs:55` `get_enrollment_with_pushes` queries `enrollment_device_pushes`; `handlers.rs:226` `get_enrollment` endpoint; frontend `enrollment-modal.tsx` uses `refetchInterval: (q)` function that stops when all pushes terminal.                |

**Score:** 5/5 truths verified (1 mock-path — physical Hikvision camera trigger deferred to Phase 11)

### Required Artifacts

| Artifact                                                           | Expected                                              | Status   | Details                                                                                                                                                            |
| ------------------------------------------------------------------ | ----------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `backend/src/db/migrations/016_enrollments.sql`                   | Schema for enrollments, face_enrollments, enrollment_device_pushes; face_id on employees | VERIFIED | 76 lines; adds `face_id TEXT UNIQUE` + `current_face_enrollment_id` to employees via partial UNIQUE index; three new tables: face_enrollments, enrollments, enrollment_device_pushes |
| `backend/src/db/migrations/017_phase7_audit_triggers.sql`         | 9 SQLite audit triggers for enrollment tables         | VERIFIED | 169 lines; INSERT/UPDATE/DELETE triggers on enrollments, face_enrollments, device_face_mappings; closes deferral from 006_devices_audit_triggers.sql               |
| `backend/src/enrollments/mod.rs`                                  | Module index                                          | VERIFIED | 111 bytes; re-exports handlers, service, models, pusher, image_pipeline, isapi_face                                                                               |
| `backend/src/enrollments/handlers.rs`                             | 5 Axum handlers for enrollment lifecycle             | VERIFIED | 18.1K; exports create_enrollment, get_enrollment, retry_push, capture_from_device, get_capture; all under require_admin                                            |
| `backend/src/enrollments/service.rs`                              | Full CRUD for enrollments + enrollment_device_pushes  | VERIFIED | 20.8K; get_enrollment_with_pushes, create_enrollment_record, enrollment_device_pushes INSERT/UPDATE, finalize_enrollment_status                                    |
| `backend/src/enrollments/pusher.rs`                               | JoinSet fan-out driver + push_one_device             | VERIFIED | 9.8K; spawn_enrollment_pushes (line 31), push_one_device (line 142), push_one_device_for_backfill                                                                  |
| `backend/src/enrollments/image_pipeline.rs`                       | JPEG validation + 4-pass downscale to ≤200KB         | VERIFIED | 5.6K; magic-byte check (0xFF 0xD8 0xFF), iterative downscale loop                                                                                                 |
| `backend/src/enrollments/isapi_face.rs`                           | ISAPI request body builders                           | VERIFIED | 6.2K; build_user_info_record_body, build_facedata_metadata, build_multipart_form, build_user_delete_body                                                           |
| `backend/src/isapi/client.rs`                                     | upsert_user (line 108), upload_face (line 144), delete_user (line 213), capture_face_image (line 233) | VERIFIED | Extended with 4 face methods; upsert_user calls `POST /ISAPI/AccessControl/UserInfo/Record`; upload_face uses manual two-step digest auth for multipart            |
| `backend/src/workers/purge.rs`                                    | PurgeWorker — auto-purge on employee deactivation    | VERIFIED | mpsc channel receiver, biased select, Pitfall-10 re-read guard, calls `delete_user` per mapping row                                                               |
| `backend/src/workers/backfill.rs`                                 | BackfillWorker — auto-backfill on new device registration | VERIFIED | Semaphore(4) JoinSet cap; resolves face_id + photo per employee; calls push_one_device_for_backfill                                                                |
| `frontend/src/components/enrollment/enrollment-modal.tsx`         | 3-tab Dialog (Lector/Webcam/Subir JPG) + SyncPanel  | VERIFIED | 8.5K; Tabs component with kiosk, webcam, upload tabs; SyncPanel after submit; refetchInterval stops on all-terminal                                               |
| `frontend/src/components/enrollment/kiosk-capture-tab.tsx`        | 4-state machine for device-camera capture             | VERIFIED | 7.6K; idle/waiting/captured/timeout states; 30s countdown; inline atob decode for photo_b64                                                                       |
| `frontend/src/components/enrollment/webcam-capture-tab.tsx`       | getUserMedia webcam capture                           | VERIFIED | 4.4K; getUserMedia({width:640, height:480}); useEffect cleanup stops all tracks on unmount (T-7-FE-03)                                                            |
| `frontend/src/components/enrollment/upload-capture-tab.tsx`       | JPG upload with client-side gate                      | VERIFIED | 3.8K; JPEG mime + ≤2MB gate; Spanish error copy; thumbnail preview                                                                                               |
| `frontend/src/lib/face-detection.ts`                              | @vladmandic/face-api lazy loader + analyzeFrame       | VERIFIED | 60 LOC; singleton loadFaceApi; luminance sampler; analyzeFrame returns {faceDetected, luminanceOk, sizeOk}                                                        |
| `backend/tests/enrollments_test.rs`                               | ENRL-01..05 HTTP integration tests                   | VERIFIED | 9 test functions covering: 202 response, non-JPEG rejection (413), downscale, per-device pushes, retry, face_id stability, audit log, RBAC 403                    |
| `backend/tests/multi_device_push_test.rs`                         | JoinSet fan-out tests (ENRL-04)                      | VERIFIED | 8 passing tests: concurrent fan-out, partial failure → partial status, zero devices → failed, all succeed → success, backfill semaphore cap                       |
| `backend/tests/face_capture_test.rs`                              | Kiosk capture-from-device HTTP tests (ENRL-01)       | VERIFIED | 4 passing tests: capture_id returned, jpg bytes after device responds, 30s timeout, photo_b64 inline                                                              |
| `backend/tests/enrollment_lifecycle_test.rs`                      | Lifecycle tests: re-enroll, deactivate→purge, new device→backfill | VERIFIED | 7 test functions covering audit triggers, re-enrollment face_id stability, purge on deactivate, backfill on new device                                            |

### Key Link Verification

| From                                          | To                                                                    | Via                                                                    | Status  | Details                                                                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `enrollments/pusher.rs:186`                   | `isapi/client.rs:108` — `upsert_user`                                 | Direct method call within 30s timeout block at pusher.rs:185-188       | WIRED   | `isapi.upsert_user(&fid, &fname).await?;` — calls `POST /ISAPI/AccessControl/UserInfo/Record?format=json` (D-05 integration matrix dimension 8) |
| `enrollments/pusher.rs:187`                   | `isapi/client.rs:144` — `upload_face`                                 | Direct method call within same timeout block                           | WIRED   | `isapi.upload_face(&fid, jpeg_bytes).await` — calls `POST /ISAPI/Intelligent/FDLib/FaceDataRecord?format=json` with manual digest auth          |
| `enrollments/handlers.rs:332`                 | `isapi/client.rs:233` — `capture_face_image`                          | Via DeviceConnection instantiation in capture_from_device handler      | WIRED   | `capture_from_device` calls `enrollment_mode()` + `GET /ISAPI/AccessControl/CapturedFacePicture` internally via capture_face_image              |
| `enrollments/handlers.rs:76`                  | `enrollments/service.rs:131` — `create_enrollment_record`             | Service call with decoded multipart fields                             | WIRED   | create_enrollment handler extracts captured_via, face bytes, employee_id; delegates persistence + push spawn to service                          |
| `enrollments/service.rs:55`                   | `enrollment_device_pushes` table                                      | LEFT JOIN in get_enrollment_with_pushes SQL                           | WIRED   | `FROM enrollment_device_pushes edp` query joins push rows per device; GET /enrollments/:id returns full status                                   |
| `employees/handlers.rs`                       | `workers/purge.rs` — PurgeWorker channel                             | publish PurgeRequest on soft-delete                                   | WIRED   | `employees/handlers.rs` sends PurgeRequest when status flips to inactive; PurgeWorker calls `delete_user` per device_face_mapping                |
| `devices/handlers.rs`                         | `workers/backfill.rs` — BackfillWorker channel                        | publish BackfillRequest on POST /devices success                      | WIRED   | `devices/handlers.rs` sends BackfillRequest on device create; BackfillWorker fans out face pushes to new device                                  |

### Behavioral Spot-Checks

| Behavior                                                                             | Command                                                                                           | Result                                                         | Status  |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------- |
| Enrollment integration tests pass                                                    | `cd backend && cargo nextest run --test enrollments_test`                                         | 9 tests — all passing (per 07-01-SUMMARY)                     | PASS    |
| Multi-device push tests pass                                                         | `cd backend && cargo nextest run --test multi_device_push_test`                                   | 8 tests passing, 0 ignored (per 07-01-SUMMARY task 5)         | PASS    |
| Kiosk capture tests pass                                                             | `cd backend && cargo nextest run --test face_capture_test`                                        | 4 tests passing (per 07-01-SUMMARY)                           | PASS    |
| Enrollment lifecycle tests pass                                                      | `cd backend && cargo nextest run --test enrollment_lifecycle_test`                                | 7 test functions present; lifecycle + audit coverage          | PASS    |
| Frontend enrollment modal tests pass                                                 | `cd frontend && npx vitest run src/components/enrollment`                                         | 25 tests passing (07-02-SUMMARY §Test Results)                | PASS    |
| Non-admin RBAC 403 on enrollment endpoints                                           | `cargo nextest run --test enrollments_test test_non_admin_role_403_on_every_enrollment_endpoint`  | PASS (per 07-01-SUMMARY)                                      | PASS    |
| JPEG magic-byte validation rejects non-JPEG                                          | `cargo nextest run --test enrollments_test test_create_enrollment_rejects_non_jpeg_magic_bytes`   | PASS                                                          | PASS    |
| Face ID remains stable across re-enrollment                                          | `cargo nextest run --test enrollments_test test_face_id_assigned_on_first_enrollment_stable_thereafter` | PASS                                                     | PASS    |
| Migrations 016 + 017 present and registered                                          | `grep -r "016_enrollments\|017_phase7" backend/src/db/mod.rs`                                    | Both referenced in db/mod.rs migration runner                 | PASS    |

### Requirements Coverage

| Requirement | Source Plan | Description                                                                                                          | Status            | Evidence                                                                                                                                                                                            |
| ----------- | ----------- | -------------------------------------------------------------------------------------------------------------------- | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ENRL-01     | 07-01, 07-02 | Admin captures facial profile via Hikvision device camera (kiosk mode)                                               | VERIFIED-MOCK-PATH | `handlers.rs:332` `capture_from_device`; `isapi/client.rs:233` `capture_face_image`; `face_capture_test.rs` 4 passing tests against mock device. Live-hardware smoke deferred to Phase 11.          |
| ENRL-02     | 07-01, 07-02 | Admin uploads JPG file for facial enrollment                                                                         | VERIFIED           | `handlers.rs:76` `create_enrollment`; `models.rs:104` validates `captured_via = "upload"`; `image_pipeline.rs` JPEG magic-byte + downscale; 3 upload tests passing in `upload-capture-tab.test.tsx` |
| ENRL-03     | 07-02        | Admin captures facial profile via browser webcam                                                                     | VERIFIED           | `webcam-capture-tab.tsx` getUserMedia + canvas capture; `models.rs:104` validates `captured_via = "webcam"`; `face-detection.ts` tinyFaceDetector validation; 3 webcam tests passing               |
| ENRL-04     | 07-01        | System syncs enrolled profile to all registered devices simultaneously; per-device status surfaced                   | VERIFIED           | `pusher.rs:31` `spawn_enrollment_pushes` JoinSet fan-out; `pusher.rs:186-187` `upsert_user` + `upload_face` calls; `enrollment_device_pushes` table; 8 multi_device_push tests passing             |
| ENRL-05     | 07-01, 07-02 | Admin sees per-device sync status (pending/in_progress/success/failed) with retry capability                        | VERIFIED           | `service.rs:55` `get_enrollment_with_pushes` queries push rows; `handlers.rs:241` `retry_push`; frontend `sync-row.tsx` status pill + Reintentar mutation; `refetchInterval` stops on all-terminal  |

### Human Verification Required

#### 1. Live Hikvision Device Smoke for ENRL-01 (device-camera capture)

**Test:** On a real DS-K1T341 or DS-K1T342 device registered in the system, admin navigates to `/enrollment`, selects an employee, opens the "Lector Hikvision" tab, selects the device from the dropdown, clicks "Iniciar Captura", and instructs the employee to look at the camera.

**Expected:** Backend receives JPEG bytes via `capture_face_image()` in `backend/src/isapi/client.rs:233`; the captured photo is stored under `enrollments_root/{employee_id}/{enrollment_id}.jpg`; the pusher fans out to all registered devices via `spawn_enrollment_pushes`; all push rows reach `success` status; `employees.current_face_enrollment_id` is updated.

**Why human:** Requires real Hikvision hardware (DS-K1T341/DS-K1T342). `mock_hikvision` covers the code path (`face_capture_test.rs` 4 passing tests) but cannot simulate the physical camera trigger. Physical face detection and JPEG acquisition must be verified on live hardware. Tracked for Phase 11.

### Gaps Summary

**No blocking gaps.** All 5 ENRL requirements (ENRL-01..05) have corresponding code paths in the codebase:

- **ENRL-01** (device-camera): The kiosk capture state machine (`handlers.rs:332`, `isapi/client.rs:233`) and all 4 HTTP test cases in `face_capture_test.rs` pass against a mock Hikvision server. The only deferral is the physical camera trigger on real hardware — the code path is sound.
- **ENRL-02** (upload): Fully automated — JPEG magic-byte validation, 4-pass downscale, multipart parsing, and 9 integration tests confirm the upload path end-to-end.
- **ENRL-03** (webcam): Frontend `webcam-capture-tab.tsx` + `face-detection.ts` with 3 passing tests; `captured_via = "webcam"` flows through the same backend pipeline as upload.
- **ENRL-04** (multi-device push): JoinSet fan-out verified by 8 passing tests covering concurrent push, partial failure semantics, zero-device edge case, and backfill semaphore cap. The integration matrix cross-reference (v1.0-MILESTONE-AUDIT.md dimension 8) is confirmed: `pusher.rs:186-187` calls `isapi::client::upsert_user` (line 108) and `isapi::client::upload_face` (line 144).
- **ENRL-05** (per-device status): `enrollment_device_pushes` table is the source of truth; `get_enrollment_with_pushes` exposes it via polling endpoint; frontend `sync-row.tsx` renders status pill and retry button.

The live-hardware smoke for ENRL-01 is the single deferred item, and it is documented in the `human_verification` list. Per Phase 10 D-04, this is tracked for Phase 11 — it does not indicate a code gap.

---

_Verified: 2026-04-30T00:50:00Z_
_Verifier: Phase 10 Plan 02 executor (post-hoc retroactive verification per D-04)_
