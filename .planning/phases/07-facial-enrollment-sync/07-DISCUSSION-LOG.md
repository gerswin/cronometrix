# Phase 7: Facial Enrollment & Sync - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-27
**Phase:** 07-facial-enrollment-sync
**Areas discussed:** Capture pipeline & AI validation, Multi-device sync transport & status surface, face_id + photo storage + ISAPI endpoint, Lifecycle (re-enroll / deactivate / new device)

---

## Capture pipeline & AI validation

### Q: For 'Lector Hikvision' mode (capture via device camera) — how does the captured face image get back to Cronometrix?

| Option | Description | Selected |
|--------|-------------|----------|
| ISAPI synchronous pull | Backend triggers device enrollment mode, then immediately pulls the captured JPG from the device's ISAPI response. Treat 'Lector Hikvision' as a kiosk mode — admin walks employee to device, device returns captured JPG in same ISAPI response. | ✓ |
| alertStream side-channel | Backend puts device into enrollment mode, employee positions face, device emits face-capture event over the existing alertStream. Backend correlates by device_id + employee context. | |
| Drop 'Lector Hikvision' for v1 | Ship only Webcam + JPG upload for v1; tab disabled with 'Próximamente'. | |

**User's choice:** ISAPI synchronous pull
**Notes:** Confirmed in follow-up as physical kiosk model — admin selects device, employee walks to it, modal shows "Esperando captura…" spinner, on success switches to preview + Aceptar/Recapturar.

---

### Q: For the Webcam mode, what produces the JPG and at what quality?

| Option | Description | Selected |
|--------|-------------|----------|
| canvas.toBlob jpeg 0.92 @ 640×480 | getUserMedia → canvas → toBlob('image/jpeg', 0.92). ~50 KB per frame. (Recommended) | ✓ |
| Higher resolution 1280×720, q 0.85 | Larger photo for older firmware that downscales. ~120 KB. | |
| User decides per-tenant via global rule | Add enrollment_image_quality to global_rules. | |

**User's choice:** canvas.toBlob jpeg 0.92 @ 640×480
**Notes:** —

---

### Q: For 'Subir JPG' upload mode, what limits and validation?

| Option | Description | Selected |
|--------|-------------|----------|
| JPG/PNG, 5 MB cap, server re-encode | Frontend accept JPG+PNG up to 5 MB; backend re-encodes to ≤200 KB JPEG via `image` crate. | |
| JPG only, 2 MB cap, no re-encode | Stricter input contract; reject PNG/oversized at upload; push original bytes to devices. | ✓ |
| Mirror Phase 3 leave evidence rules | PDF/JPG/PNG, 10 MB cap. | |

**User's choice:** JPG only, 2 MB cap, no re-encode
**Notes:** Risk flagged in CONTEXT.md D-04 — if devices reject >200 KB, server-side downscale becomes follow-up.

---

### Q: AI validation panel from mockup — implement how?

| Option | Description | Selected |
|--------|-------------|----------|
| Client-side face-api.js | Lazy-load tinyFaceDetector (~6 MB) only on enrollment route; detect face bbox + brightness + resolution in browser; real-time feedback. | ✓ |
| Skip for v1, ship UI as-is | Render the panel as static green checkmarks; capture button always enabled. (Recommended for ship velocity) | |
| Server-side validation on submit | Backend uses `image` crate + Rust face-detection crate; blocks UX feedback until submit. | |

**User's choice:** Client-side face-api.js
**Notes:** —

---

### Q: When 'Aceptar' is clicked, should AI validation be a HARD gate or soft warning?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard gate — block submit if any check fails | Capturar Rostro disabled until all 3 checks green. (Recommended) | ✓ |
| Soft warning — allow submit with confirmation | Failed checks show red but admin can submit after a "calidad baja — continuar?" modal. | |
| Display only, no gating | Show panel as informational, never block. | |

**User's choice:** Hard gate
**Notes:** —

---

## Multi-device sync transport & status surface

### Q: Backend topology for concurrent push to N devices?

| Option | Description | Selected |
|--------|-------------|----------|
| tokio JoinSet, fire-and-await | Handler returns 202 immediately with enrollment_id; tasks run in background; per-device rows updated to success/failed. (Recommended) | ✓ |
| Synchronous — wait for all devices | Handler blocks until all tasks finish or hit timeout; final statuses in response body. | |
| Background worker via mpsc queue | Reuse Phase 3 recompute-worker pattern. | |

**User's choice:** tokio JoinSet, fire-and-await
**Notes:** —

---

### Q: How does per-device status reach the modal in real time?

| Option | Description | Selected |
|--------|-------------|----------|
| Polling GET /enrollments/:id every 1.5s | TanStack Query useQuery with refetchInterval until all devices terminal. (Recommended) | ✓ |
| Reuse existing SSE channel | Add enrollment_progress event type to dashboard SSE; frontend filters by enrollment_id. | |
| Dedicated SSE per enrollment | GET /enrollments/:id/stream returns text/event-stream until done. | |

**User's choice:** Polling GET /enrollments/:id every 1.5s
**Notes:** —

---

### Q: What if 1 of 4 devices fails the push?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-device terminal status, no auto-retry | Each row ends success/failed; manual Reintentar per device; enrollment "success" if ≥1 device succeeded. (Recommended) | ✓ |
| Auto-retry 3x with backoff, then failed | Each task retries 3 times (2s/4s/8s) before marking failed. | |
| All-or-nothing transaction | If any fails, rollback successful devices (delete face). | |

**User's choice:** Per-device terminal status, no auto-retry
**Notes:** —

---

### Q: If admin closes the modal mid-sync, what happens?

| Option | Description | Selected |
|--------|-------------|----------|
| Sync continues, status visible elsewhere | Tasks keep running; persistent toast/badge; re-opening shows latest status. (Recommended) | ✓ |
| Cancel — abort in-flight pushes | Closing sends DELETE /enrollments/:id; CancellationToken aborts tasks. | |
| Block close until all devices terminal | Modal X disabled until last device terminal. | |

**User's choice:** Sync continues, status visible elsewhere
**Notes:** —

---

## face_id + photo storage + ISAPI endpoint

### Q: face_id generation strategy?

| Option | Description | Selected |
|--------|-------------|----------|
| Cronometrix-generated UUID v4 per employee | Single canonical face_id per employee; same ID pushed to every device. (Recommended) | ✓ |
| Cronometrix-generated short numeric (1–9999999) | Sequential or hash-derived numeric to mirror older firmware expectations. | |
| Per-device face_id assigned by Hikvision | Push photo, device responds with assigned face_id; one row per device per employee. | |

**User's choice:** Cronometrix-generated UUID v4 per employee
**Notes:** —

---

### Q: Where does the canonical enrolled JPG persist?

| Option | Description | Selected |
|--------|-------------|----------|
| ./data/enrollments/{employee_id}.jpg | One file per employee, latest replaces previous. (Recommended) | |
| ./data/enrollments/{employee_id}/{enrollment_id}.jpg + history | Keep every enrollment attempt; current_face_enrollment_id points to active. | ✓ |
| Inside enrollments table as BLOB | Store JPG bytes in SQLite. | |

**User's choice:** ./data/enrollments/{employee_id}/{enrollment_id}.jpg + history
**Notes:** Audit-history retention for legal traceability.

---

### Q: Which Hikvision ISAPI endpoint family for the face profile push?

| Option | Description | Selected |
|--------|-------------|----------|
| Research will pick — lock during planning | Defer to research; same pattern as Phase 2 alertStream. (Recommended) | ✓ |
| Lock to UserInfo/Record + FDLib/FaceDataRecord now | 2-step modern firmware (2022+) standard. | |
| Try both, fallback chain | Push tries 2-step first, falls back to single-step on 404/501. | |

**User's choice:** Research will pick — lock during planning
**Notes:** —

---

### Q: When does a `device_face_mappings` row get written?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-device, on push success only | INSERT OR REPLACE on success; failed pushes write nothing. (Recommended) | ✓ |
| Speculative on enrollment, cleanup on failure | Pre-write N rows; failed-device tasks delete. | |
| Single row per (employee_id), pivot table for devices | Rewrites Phase 2 D-08 schema. | |

**User's choice:** Per-device, on push success only
**Notes:** —

---

## Lifecycle: re-enroll, deactivate, new device

### Q: Re-enrollment behavior — what happens to face_id and device_face_mappings?

| Option | Description | Selected |
|--------|-------------|----------|
| face_id stable, photo replaced on devices | employees.face_id stays same forever; new photo pushed via PUT same ID; mappings untouched. (Recommended) | ✓ |
| New face_id per enrollment | Fresh UUID per re-enroll; DELETE old face_id from each device; race-window risk. | |
| Stop — let me describe | Free-text. | |

**User's choice:** face_id stable, photo replaced on devices
**Notes:** —

---

### Q: When admin soft-deletes / deactivates an employee, what happens to their face profile on the registered devices?

| Option | Description | Selected |
|--------|-------------|----------|
| Delete from all devices automatically | Background job purges via ISAPI; pending_delete state on failure for retry. (Recommended) | ✓ |
| Leave on devices, ignore events at processing time | Phase 2 event processor checks employees.status. | |
| Manual purge action on employee page | Admin clicks "Eliminar de dispositivos" explicitly. | |

**User's choice:** Delete from all devices automatically
**Notes:** —

---

### Q: When a NEW device is registered after enrollments already exist, what happens?

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-backfill on device activation | Background job pushes every active employee's face profile to the new device. (Recommended) | ✓ |
| Manual "Sincronizar todo" button on device page | Device page shows badge with manual trigger. | |
| Defer to a future phase | v1 only handles fresh enrollments. | |

**User's choice:** Auto-backfill on device activation
**Notes:** —

---

### Q: Audit triggers — which new tables get audit_log entries?

| Option | Description | Selected |
|--------|-------------|----------|
| enrollments + device_face_mappings | INSERT/UPDATE/DELETE triggers on both, mirroring Phase 2 D-08 deferral note. (Recommended) | ✓ |
| enrollments only | Skip device_face_mappings (derived state). | |
| Application-level audit, no SQLite triggers | Inconsistent with Phase 1–6 — not recommended. | |

**User's choice:** enrollments + device_face_mappings
**Notes:** Also covers face_enrollments (history table) per CONTEXT.md D-17.

---

## Claude's Discretion

- Exact column types, indexes, FK ON DELETE behavior on new tables (follow Phase 1/2 conventions)
- Migration numbering (planner sequences against current `015` tip)
- Background purge worker (D-15) and backfill worker (D-16) topology
- face-api.js model loader UX (loading skeleton on first /enrollment route hit)
- Per-device push timeout (likely 30s mirroring Phase 2 ISAPI REQUEST_TIMEOUT)
- Whether kiosk capture flow uses dedicated state machine or composes ISAPI client primitives

## Deferred Ideas

- Mass admin re-sync screen
- Server-side image downscale fallback
- Face quality scoring v2 (per-enrollment metrics over time)
- Device firmware compatibility matrix (auto-detect on registration)
- Enrollment audit panel UI
- Batch enrollment via CSV + photos
- Face detection on the alertStream side (recognition drift)
