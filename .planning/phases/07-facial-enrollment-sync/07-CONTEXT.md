# Phase 7: Facial Enrollment & Sync - Context

**Gathered:** 2026-04-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin enrolls an employee's facial profile through the web UI via one of three input modes — Hikvision device camera (kiosk), browser webcam, or JPG upload — and the backend pushes the canonical profile concurrently to every active registered device with per-device sync status visible in real time. Covers ENRL-01..05.

**In scope (07-01 → 07-02):**
- Enrollment backend: `enrollments` + `face_enrollments` tables, ISAPI face profile push, concurrent multi-device tokio JoinSet, polling status endpoint, audit triggers, photo storage on disk, employee `face_id` column, auto-purge on employee deactivation, auto-backfill on new device registration
- Enrollment modal UI: 3-tab capture (Lector Hikvision / Webcam / Subir JPG), face-api.js client-side quality gating, per-device sync progress bars, non-blocking modal close

**Out of scope (Phase 7):**
- Mass admin re-sync screen (deferred — see Deferred Ideas)
- Enrollment audit trail viewer UI (audit_log entries land via triggers; reviewing them is part of any future audit-panel phase)
- Facial recognition outside Hikvision firmware (Cronometrix never participates in real-time access decisions — Phase 2 D-07)
- Mobile / responsive layouts (Phase 4 D-3 desktop-only carries forward)

</domain>

<decisions>
## Implementation Decisions

### Capture Pipeline (Frontend)

- **D-01:** Three capture modes via tabs in the modal (matches mockup `d7atd`): "Lector Hikvision", "Webcam", "Subir JPG". All three converge on the same `POST /api/v1/enrollments` payload (multipart: photo bytes + metadata).
- **D-02:** **Lector Hikvision = physical kiosk model.** Admin selects which registered device to use from a dropdown, clicks "Iniciar Captura", modal shows "Esperando captura en {device.name}…". Backend puts the chosen device into enrollment mode (existing ISAPI command), then pulls the captured JPG synchronously from the device's ISAPI response. On success (or 30s device-side timeout), modal switches to preview + "Aceptar" / "Recapturar". Exact ISAPI capture endpoint locked during research (Phase 2 D-07 noted Cronometrix doesn't drive real-time access — this only fetches the captured image after the device has done its enrollment routine).
- **D-03:** **Webcam = `getUserMedia({ video: { width: 640, height: 480 } })` → `<canvas>` → `canvas.toBlob('image/jpeg', 0.92)`.** Target ~50 KB per frame, fits Hikvision face-profile size envelope. Live preview rendered in modal canvas. Capturar Rostro freezes the latest frame for confirmation.
- **D-04:** **Subir JPG = JPG only, 2 MB cap, no server re-encode.** Frontend `<input type="file" accept="image/jpeg">` rejects non-JPG client-side; backend validates MIME + size + magic bytes (reject HEIC, PNG, mislabeled). Original bytes pushed to devices unchanged. **Risk:** if Hikvision firmware rejects images >200 KB, server-side downscale becomes follow-up work — flag in research.
- **D-05:** **AI Validation panel = client-side `face-api.js`, hard gate.** Lazy-load tinyFaceDetector model (~6 MB) only on the `/enrollment` route; never on dashboard/timesheet. Three checks before "Capturar Rostro" / "Aceptar" enables: (1) face bounding box detected, (2) average frame luminance within 80–200 (canvas pixel sample), (3) face bbox ≥160×160 px. All three must be green. Mockup labels: "Rostro Detectado", "Buena Iluminación", "Resolución Óptima".

### Multi-Device Sync & Status

- **D-06:** **Backend topology = tokio `JoinSet`, fire-and-forget.** `POST /api/v1/enrollments` validates payload, persists `enrollments` + `face_enrollments` + N `enrollment_device_pushes` rows (status=pending, one per active device), spawns N tokio tasks via JoinSet, returns 202 with `enrollment_id` immediately. Each task: decrypt device password (Phase 2 D-01), open ISAPI digest-auth client, push face profile, update its row to `success` or `failed` (with `error_message`). Tasks run independent of the request lifecycle.
- **D-07:** **Per-device status surface = polling.** Frontend `useQuery({ queryKey: ['enrollment', id], refetchInterval: 1500 })` against `GET /api/v1/enrollments/:id` until every push row is terminal (success or failed). Endpoint returns enrollment row + array of push rows. No SSE — keeps Phase 4 SSE channel scoped to dashboard activity.
- **D-08:** **Partial failure = per-device terminal status, manual retry only.** Each push task ends as success or failed (with error message); no automatic retries. Modal shows red bar for each failed device + per-row "Reintentar" button → `POST /api/v1/enrollments/:id/devices/:device_id/retry` which re-fires that single push task. Enrollment-level status is `success` if ≥1 device succeeded, `partial` if some succeeded + some failed, `failed` if 0 succeeded.
- **D-09:** **Modal close mid-sync = sync continues, status persists.** Closing modal does NOT cancel tasks. App-level toast/badge shows "Enrolamiento en curso — X/Y dispositivos" (TanStack Query keeps polling in the background as long as the user stays in the app). Re-opening the employee detail page or enrollment screen surfaces the latest status from `/enrollments/:id`. No DELETE-cancel endpoint.

### face_id, Photo Storage & ISAPI

- **D-10:** **face_id = Cronometrix-generated UUID v4, stable per employee.** New nullable column `employees.face_id TEXT UNIQUE`. Assigned on first successful enrollment (any device), never changes for the lifetime of that employee row. Same face_id pushed to every Hikvision device — `device_face_mappings` rows all share the same face_id for one employee. UUID fits Hikvision string ID limits.
- **D-11:** **Canonical photo storage = `./data/enrollments/{employee_id}/{enrollment_id}.jpg` with full history.** Each enrollment attempt persists its source JPG. New `face_enrollments` table tracks per-attempt metadata (id, employee_id, captured_via ∈ {device,webcam,upload}, source_device_id NULL unless device mode, photo_path, face_quality_score JSON, created_at, created_by). `employees.current_face_enrollment_id TEXT` points to the active one (nullable until first enroll). Old files kept on disk for legal/audit traceability.
- **D-12:** **Hikvision ISAPI face-profile endpoint family = lock during research.** Phase 7 backend has no existing face-upload code path. Research must fetch current Hikvision ISAPI docs and pick between (a) modern 2-step `PUT /ISAPI/AccessControl/UserInfo/Record` + `POST /ISAPI/Intelligent/FDLib/FaceDataRecord` multipart, or (b) legacy single-step `PUT /ISAPI/AccessControl/UserInfo/SetUp`. Researcher must validate against target firmware (DS-K1T341, DS-K1T342 — same model set as Phase 2 alertStream). Decision lands in 07-RESEARCH.md and is reflected in 07-01-PLAN.md.
- **D-13:** **`device_face_mappings` write timing = per-device, on push success only.** Each successful task does `INSERT OR REPLACE INTO device_face_mappings (id, device_id, face_id, employee_id, version, created_at, updated_at)`. Failed pushes write nothing. Re-push on retry is `INSERT OR REPLACE`. Implication: events from a freshly added device that hasn't been backfilled yet correctly land as `is_unknown=1` (Phase 2 D-07) until D-16 backfill completes.

### Lifecycle: Re-enroll, Deactivate, New Device

- **D-14:** **Re-enrollment = stable face_id, photo replaced on devices, history retained.** When admin re-enrolls an existing employee: new `face_enrollments` row, photo file written to `./data/enrollments/{employee_id}/{new_enrollment_id}.jpg`, `employees.current_face_enrollment_id` updated, then push to all active devices using the **same `employees.face_id`** (Hikvision PUT replaces on existing user record). `device_face_mappings` rows untouched (face_id unchanged). Old photo files stay on disk indefinitely. No risk of orphaned mappings or in-flight event mismatch.
- **D-15:** **Employee deactivation = auto-purge from all devices.** When `employees.status` flips to `inactive` (or `deleted_at` set — soft delete per Phase 1), enqueue a background purge job: for each row in `device_face_mappings` for this employee, call ISAPI `DELETE /ISAPI/AccessControl/UserInfo/Delete` (exact endpoint to be confirmed in research alongside D-12), then `DELETE` the mapping row. On per-device failure: mark mapping row state as `pending_delete` (new column on `device_face_mappings`) and a periodic worker retries on the next purge tick. Prevents inactive employees from logging attendance via device firmware.
- **D-16:** **New device registration = auto-backfill all active employee profiles.** When `POST /api/v1/devices` succeeds (device row created, status=active), spawn a backfill job: for every employee where `face_id IS NOT NULL` and `status = 'active'`, push the current photo (`./data/enrollments/{employee_id}/{current_face_enrollment_id}.jpg`) to the new device. Use the same per-device push code path as D-06 — one device, N employees fan-out. Status visible on the device detail page (separate panel: "Sincronización Inicial — X/Y empleados"). Mirrors the product's "zero-manual" promise.
- **D-17:** **Audit triggers on `enrollments`, `face_enrollments`, AND `device_face_mappings`.** SQLite triggers on INSERT/UPDATE/DELETE writing to existing `audit_log`, mirroring Phase 1/2 trigger pattern. Closes Phase 2's deferred note on `device_face_mappings` audit ("deferred to Phase 7"). Captures who enrolled whom, which photo was active when, and which devices got which face. Application-level audit (e.g., enrollment_event source) handled by triggers reading session context columns where present, or by handlers writing supplementary `command_audit_log` entries for ISAPI calls (consistent with Phase 2 D-11 dispatch audit).

### Schema Additions (informational — planner finalizes)

- New columns on `employees`: `face_id TEXT UNIQUE` (nullable), `current_face_enrollment_id TEXT` (nullable, FK to `face_enrollments.id`)
- New columns on `device_face_mappings`: `state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active','pending_delete'))`
- New table `face_enrollments`: id, employee_id, captured_via, source_device_id (nullable), photo_path, face_quality_score (JSON: {face_detected, luminance, width, height}), created_at, created_by
- New table `enrollments`: id, employee_id, face_enrollment_id, status (in_progress|success|partial|failed), started_at, completed_at, started_by
- New table `enrollment_device_pushes`: id, enrollment_id, device_id, status (pending|success|failed), error_message, started_at, completed_at
- New endpoints: `POST /api/v1/enrollments` (multipart), `GET /api/v1/enrollments/:id`, `POST /api/v1/enrollments/:id/devices/:device_id/retry`, `POST /api/v1/enrollments/capture-from-device` (kicks off Lector Hikvision flow, returns captured JPG for preview before commit)

### RBAC

- **D-18:** Admin only. All enrollment endpoints + UI gated by `require_admin` middleware (Phase 1 D-09 + Phase 4 D-14 pattern). Supervisor and Viewer see the sidebar nav item but the screen renders a "Acceso restringido" placeholder if they navigate to it. (Carry-forward — no new policy.)

### Research Lock-Ins (2026-04-27, post-RESEARCH.md)

- **D-12 LOCKED:** Use **modern 2-step ISAPI flow**: `POST /ISAPI/AccessControl/UserInfo/Record?format=json` (create person, JSON body) followed by `POST /ISAPI/Intelligent/FDLib/FaceDataRecord?format=json` (multipart: JSON metadata part + JPEG bytes part). Default `FDID="1"`, `faceLibType="blackFD"` (research A2 — verify on first hardware smoke test). Legacy `UserInfo/SetUp` path discarded.
- **D-15 LOCKED:** Face delete = `PUT /ISAPI/AccessControl/UserInfoDetail/Delete?format=json` with `UserInfoDetail` body listing employeeNo. Confirmed in research.
- **D-04 SUPERSEDED — server-side downscale moved into Phase 7 scope.** Phone uploads (1–4 MB) exceed Hikvision's 200 KB face-image cap; without downscale the upload tab fails on real devices. Add `image = "0.25.10"` to `backend/Cargo.toml`. Multipart receive path runs an iterative downscale loop (resize → JPEG encode at quality 90 → if still >200 KB, drop quality by 10, repeat down to quality 50; if still >200 KB, halve dimensions and reset quality) before persistence and ISAPI push. Original frontend MIME/magic-bytes validation stays. Removes the `Server-side image downscale` entry from Deferred Ideas.
- **D-05 SUPERSEDED — use `@vladmandic/face-api@1.7.15` (maintained fork)** instead of literal `face-api.js@0.22.2` (unmaintained, TFJS 1.7 incompatible with React 19). Same API surface; bundles TFJS 4. Lazy-load `tinyFaceDetector` (~190 KB quantized weights, NOT 6 MB — the 6 MB number in Specifics referred to the full model set). Loaded only inside the modal `useEffect`.
- **D-02 LOCKED — kiosk capture is a 2-step state machine handler-side.** Endpoint `POST /api/v1/enrollments/capture-from-device` returns immediately with `capture_id`; frontend polls `GET /api/v1/enrollments/captures/:capture_id` (or reuses the same enrollment-status polling shape) until the device-side capture completes (success → preview JPG returned; timeout/error → terminal error). Modal renders preview + Aceptar/Recapturar; Aceptar submits the JPG to `POST /api/v1/enrollments` exactly like the other two tabs. Keeps the 30s device-side timeout cleanly outside the request lifecycle.
- **Concurrency cap:** D-06 fan-out runs unbounded JoinSet (one task per active device — small fleets). D-16 backfill uses `tokio::sync::Semaphore::new(4)` to cap concurrent ISAPI calls per device under load. Confirmed in research as Hikvision Pitfall 5 mitigation.
- **`GET /enrollments/:id/photo` deferred** to future audit-panel phase. Phase 7 endpoints stay scoped to write/push/status only.

### Claude's Discretion

- Exact column types, indexes, FK ON DELETE behavior on the new tables — follow Phase 1/2 conventions (UUID PKs, INTEGER UTC epoch timestamps, version column on mutable rows).
- Migration numbering (`016_enrollments.sql`, `017_face_enrollments.sql`, `018_phase7_audit_triggers.sql`, etc.) — planner sequences.
- Background purge worker (D-15) and backfill worker (D-16) topology — likely a single tokio task per worker type using the Phase 3 recompute-worker mpsc + debounce pattern, or one-shot tokio::spawn per trigger. Planner picks based on expected volume.
- face-api.js model loader UX (loading skeleton during ~6 MB download on first use of `/enrollment` route).
- Per-device push timeout — likely 30s to mirror Phase 2 ISAPI `REQUEST_TIMEOUT`, but plan can adjust.
- Whether the kiosk-mode capture flow (D-02) uses a dedicated state machine in the handler or is composed from existing ISAPI client primitives.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/REQUIREMENTS.md` — ENRL-01..05 are Phase 7 scope; all other IDs out of scope.
- `.planning/PROJECT.md` — constraints (on-prem, Hikvision-only, audit-everything), Key Decisions table.
- `.planning/STATE.md` — accumulated decisions; Phase 7 is the final phase of v1.0 milestone.
- `CLAUDE.md` (root) — locked stack: `reqwest` 0.13, `diqwest`, `quick-xml` 0.39, `tokio` 1.51, `libsql`. Also Auth & RBAC section (admin-only enforcement), ISAPI Integration Patterns section.
- `frontend/CLAUDE.md` + `frontend/AGENTS.md` — Next.js variant warning: read `node_modules/next/dist/docs/` before writing route/handler code.

### Prior-phase contexts (carry-forward conventions)
- `.planning/phases/01-foundation/01-CONTEXT.md` — UUID PKs, UTC epoch INTEGER, audit triggers, version column, `/api/v1` prefix, error envelope, RBAC roles (Admin/Supervisor/Viewer), 3-role middleware, audit_log schema.
- `.planning/phases/02-device-integration/02-CONTEXT.md` — D-01/D-02 (AES-256-GCM device password encryption — Phase 7 must decrypt to call ISAPI), D-07 (Cronometrix never owns access decisions), D-08 (`device_face_mappings` schema definition — Phase 7 populates), D-13 (filesystem JPEG storage convention), Deferred: "Async/queued command dispatch for multi-device enrollment batch — revisit when Phase 7 enrollment scope lands" → that's THIS phase.
- `.planning/phases/03-time-calculation-engine/03-01-PLAN.md` — recompute-worker tokio mpsc + 500ms debounce pattern (reference for D-15 purge worker / D-16 backfill worker if planner chooses queue topology).
- `.planning/phases/04-frontend-ui/04-CONTEXT.md` — D-3 (desktop-only ≥1280px), D-12 (`/enrollment` sidebar route + placeholder page already exist), D-13 (TanStack Query 401 handler), D-14 (RBAC UI gating pattern with `useAuth()` context).

### Existing code (read before extending)
- `backend/src/isapi/client.rs` — `DeviceConnection` with `send_xml` / `send_json` digest-auth helpers; `enrollment_mode()` already wired (puts device into face-capture mode). Phase 7 adds face-profile push + delete methods on this struct.
- `backend/src/devices/handlers.rs` + `devices/models.rs` — `Command` enum and ISAPI dispatch handler — pattern to mirror for the new enrollment endpoints (decrypt → tokio::time::timeout → ISAPI call → audit).
- `backend/src/devices/service.rs` — device password decryption call site (reuse the same helper for enrollment push).
- `backend/src/events/service.rs` — face_id → employee lookup logic (reads `device_face_mappings`); confirms the read side is already wired so Phase 7 only needs to populate.
- `backend/src/db/migrations/003_devices.sql` — `device_face_mappings` schema definition (Phase 7 adds the audit triggers + the new `state` column for D-15).
- `backend/src/db/migrations/006_devices_audit_triggers.sql` — line: "device_face_mappings triggers are deferred to Phase 7 (enrollment)" — that promise is paid here.
- `backend/src/common.rs`, `backend/src/errors.rs` — `PaginatedResponse<T>`, `AppError` variants (reuse Timeout, Validation, NotFound, Conflict, Internal); add no new variants unless strictly needed.
- `backend/src/auth/middleware.rs` + `auth/rbac.rs` — `require_admin` (Phase 7 endpoints all use this — D-18).
- `backend/src/state.rs` — `AppState` shape; planner decides whether to add a JoinSet handle here or spawn detached tasks.
- `frontend/src/app/(dashboard)/enrollment/page.tsx` — current placeholder (Phase 4 D-12); Phase 7 replaces with the real screen.
- `frontend/src/components/devices/command-modal.tsx` — modal + ISAPI command pattern reference (similar shape to enrollment modal).
- `frontend/src/components/employees/employee-table.tsx` (or directory) — employee selection entry point; Phase 7 adds an "Enrolar Rostro" action per row.
- `frontend/src/proxy.ts` — backend API proxy config; new enrollment routes register here.

### External — research must capture
- Hikvision ISAPI face profile endpoints: `UserInfo/Record` + `Intelligent/FDLib/FaceDataRecord` (modern 2-step) vs `UserInfo/SetUp` (legacy single-step) — D-12 lock-in.
- Hikvision ISAPI face delete endpoint: likely `PUT /ISAPI/AccessControl/UserInfo/Delete` — D-15 confirms exact path.
- Hikvision face-profile size and resolution constraints (typically ≤200 KB JPEG; verify against target firmware) — informs D-04 risk validation.
- `face-api.js` v0.22+ docs: tinyFaceDetector model size, browser memory footprint, lazy-load pattern in Next.js App Router.
- Real ISAPI multipart payload sample for face upload from target device model — same blocker pattern as Phase 2 alertStream traffic capture (STATE.md noted device-model variance).

### Mockup
- `untitled.pen` node `d7atd` ("Enrolamiento Facial" modal) — open via `mcp__pencil__open_document` then `get_screenshot`. Confirms 3-tab layout, AI Validation panel (right), per-device sync bars (right, lower), "Capturar Rostro" CTA (bottom). UI-SPEC for 07-02 generated from this.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `DeviceConnection` (backend/src/isapi/client.rs): digest-auth client with `send_xml` / `send_json` helpers, password redaction in Debug, configurable TLS-skip flag. Phase 7 adds `enroll_face(face_id, jpg_bytes, employee_meta)` and `delete_face(face_id)` methods on this struct.
- `enrollment_mode()` ISAPI command + `EnrollmentMode` enum variant already exist — Phase 7 reuses to enter the device into capture mode for the kiosk flow (D-02).
- `device_face_mappings` table already exists with the right shape `(device_id, face_id) → employee_id`, version column, audit deferral note. Phase 7 only adds the `state` column + triggers.
- AES-256-GCM device-password decryption helper (Phase 2 D-01/D-02) — reuse on every per-device push task.
- `command_audit_log` table — model for `enrollment_device_pushes` row (per-invocation audit row with started_at/completed_at/result/error).
- `AppError`, `PaginatedResponse<T>`, `epoch_to_iso()` in `common.rs` — directly usable.
- `require_admin` middleware — wraps all new enrollment routes.
- TanStack Query 401 handler + `useAuth()` context — frontend RBAC gating reuses Phase 4 D-13/D-14 plumbing.
- shadcn `Dialog`, `Tabs`, `Progress`, `Button`, `Sonner` (toast) primitives — modal composes from existing UI library.

### Established Patterns
- Module layout `backend/src/{domain}/{mod.rs, models.rs, service.rs, handlers.rs}` — new module `enrollments/` follows.
- Migration naming `00X_*.sql` with idempotent `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS`. Phase 7 appends `016_enrollments.sql`, `017_face_enrollments.sql`, `018_phase7_audit_triggers.sql` (planner finalizes numbering against current migration tip — currently `015`).
- Audit via SQLite triggers (`002_audit_triggers.sql` + `006_devices_audit_triggers.sql` + `011_phase3_audit_triggers.sql` + `014_phase5_audit_triggers.sql` pattern) — D-17 follows.
- Soft delete via `status` + `deleted_at` on `employees` and `devices` — D-15 (purge job) hooks into employees soft-delete; D-16 (backfill) hooks into devices INSERT.
- `/api/v1` router composition in `main.rs` — add `enrollments_routes` (admin-only).
- TanStack Query `useQuery` + `refetchInterval` for polling (D-07).
- Validator-derive on request DTOs (multipart variant where needed).

### Integration Points
- `backend/src/main.rs` — register `enrollments_routes` under `/api/v1` with admin middleware; spawn purge worker + backfill worker tasks during bootstrap (after `init_db`, alongside Phase 2 alertStream supervisor and Phase 3 recompute worker).
- `backend/src/employees/service.rs` (or handlers) — soft-delete code path triggers the D-15 purge job (publish to purge worker mpsc or spawn detached task).
- `backend/src/devices/handlers.rs` — `POST /devices` success triggers the D-16 backfill job.
- `frontend/src/app/(dashboard)/enrollment/page.tsx` — replaces the current placeholder body with the full screen (employee picker → modal trigger or "in-progress enrollments" list).
- `frontend/src/app/(dashboard)/employees/page.tsx` — add "Enrolar Rostro" action per row in the employee table (Admin only via `useAuth()` gate).
- `frontend/src/components/layout/sidebar.tsx` — already has the "Enrolamiento" item; no nav change needed.
- `frontend/src/proxy.ts` — register `/api/v1/enrollments/*` proxy entries.

</code_context>

<specifics>
## Specific Ideas

- **Mockup `d7atd` is the visual contract.** Three tabs (Lector Hikvision / Webcam / Subir JPG), AI Validation panel right-side with three labeled checks, Sincronización a Dispositivos panel below it with one progress bar per device showing percentage completion (treat as binary 0% / 100% per device — no real granular progress is available from ISAPI; intermediate values are decorative).
- **Hikvision face-profile size envelope is the load-bearing unknown.** D-04 lets large JPGs through unchanged on the assumption devices accept up to ~200 KB. Research must confirm — if devices reject, server-side downscale via `image` crate is the fallback (cheap to add later, but planner should leave a clean seam for it).
- **Polling at 1.5s × N devices = ~3–4 polls per enrollment in steady state** (typical ISAPI face push completes in 1–2s). Keep `GET /enrollments/:id` cheap: single LEFT JOIN, no N+1.
- **Auto-backfill (D-16) on a 50-employee fleet adding a new device = 50 sequential or concurrent ISAPI pushes.** Concurrency-cap the backfill (e.g., 4 in flight at a time) to avoid hammering a fresh device — planner picks the limit.
- **Kiosk capture mode (D-02) requires admin to coordinate physical presence with web UI.** Modal copy must make this obvious: "Pídale al empleado que se acerque al dispositivo {device.name} y mire la cámara. Esperando captura…" with a 30s countdown. If timeout: "No se detectó captura. ¿Reintentar?".
- **face-api.js is a 6 MB cold load.** Lazy-load it only inside the modal's `useEffect`, not at the route level — admins who navigate to `/enrollment` without opening the modal don't pay the cost.

</specifics>

<deferred>
## Deferred Ideas

- **Mass admin re-sync screen** — UI to push all employees to all devices on demand (e.g., after a mass device firmware swap). Logic exists via D-16; just no admin UI in v1.
- **Face quality scoring v2** — track per-enrollment quality metrics over time (face-api.js outputs detection confidence + face descriptor); surface as a "calidad histórica" chart on the employee detail page.
- **Device firmware compatibility matrix** — document which Hikvision models support which ISAPI face endpoints; auto-detect on device registration. v1 assumes all registered devices speak the chosen endpoint family.
- **Enrollment audit panel UI** — read-only view of `audit_log` filtered to enrollment events with photo previews. Backend audit data lands in v1; UI is a future phase (likely a generic audit-panel phase). Includes the deferred `GET /api/v1/enrollments/:id/photo` read-back endpoint.
- **Batch enrollment via CSV+photos** — admin uploads ZIP of photos named by cédula; backend matches and bulk enrolls. Out of scope for v1.
- **Face detection on the alertStream side** — currently Cronometrix relies on device firmware to recognize. A future enhancement could verify recognition quality against the canonical photo and flag drift.

</deferred>

---

*Phase: 07-facial-enrollment-sync*
*Context gathered: 2026-04-27*
