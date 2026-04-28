# Phase 7: Facial Enrollment & Sync - Research

**Researched:** 2026-04-27
**Domain:** Hikvision ISAPI face profile push, browser webcam capture, concurrent multi-device async dispatch
**Confidence:** HIGH (stack + locked architectural patterns), MEDIUM (Hikvision endpoint variance across firmware)

## Summary

Phase 7 closes the v1 milestone by binding the existing device fleet to identifiable employees: capture a face JPG through any of three input modes, persist it canonically, and push the same face_id to every active Hikvision device concurrently with per-device status visible to the admin in real time. Most architectural decisions are already locked in 07-CONTEXT.md (D-01..D-18). Research therefore focuses on (a) the load-bearing unknown — exact ISAPI endpoint family + image envelope — and (b) verifying the locked stack (`reqwest` 0.13.2, `diqwest` 3.2.0, `quick-xml` 0.39.2, `tokio` 1.x JoinSet, `axum` 0.8 multipart, `face-api.js` for client-side gating) is fit for the multi-device push workload.

Two findings drive concrete plan work:

1. **Hikvision face enrollment is a 2-step flow on modern firmware**, NOT the legacy single-shot `UserInfo/SetUp`. The locked endpoints are `POST /ISAPI/AccessControl/UserInfo/Record?format=json` (create person) followed by `POST /ISAPI/Intelligent/FDLib/FaceDataRecord?format=json` (multipart: JSON metadata + JPEG bytes). Image MUST be ≤200KB — D-04's "no server re-encode" path WILL fail on phone-camera uploads. Server-side downscale via the `image` crate (0.25.10) is REQUIRED in Phase 7, not deferrable.
2. **`face-api.js` 0.22.2 (the canonical npm package) is unmaintained — last published 2024-03 with TensorFlow.js 1.7.0 deps that conflict with React 19.** The maintained fork `@vladmandic/face-api` 1.7.15 (also archived Feb 2025 but more recently updated, MIT, no deps) is the practical choice for Phase 7 — same API surface, ships its own TFJS, no peer-dep blast radius. Plan must specify this fork by name.

**Primary recommendation:** Lock the modern 2-step ISAPI flow, add the `image` crate for server-side downscale (200KB target), and pin `@vladmandic/face-api@^1.7.15` instead of `face-api.js`. Concurrency uses `tokio::task::JoinSet` (D-06 fire-and-forget pattern) with a `Semaphore` cap of 4 for the D-16 backfill fan-out.

## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Capture Pipeline (Frontend)
- **D-01:** Three capture modes via tabs in the modal: "Lector Hikvision", "Webcam", "Subir JPG". All three converge on the same `POST /api/v1/enrollments` payload (multipart: photo bytes + metadata).
- **D-02:** **Lector Hikvision = physical kiosk model.** Admin selects which registered device to use from a dropdown, clicks "Iniciar Captura", modal shows "Esperando captura en {device.name}…". Backend puts the chosen device into enrollment mode (existing ISAPI command), then pulls the captured JPG synchronously from the device's ISAPI response. On success (or 30s device-side timeout), modal switches to preview + "Aceptar" / "Recapturar". Exact ISAPI capture endpoint locked during research.
- **D-03:** **Webcam = `getUserMedia({ video: { width: 640, height: 480 } })` → `<canvas>` → `canvas.toBlob('image/jpeg', 0.92)`.** Target ~50 KB per frame, fits Hikvision face-profile size envelope. Live preview rendered in modal canvas. Capturar Rostro freezes the latest frame for confirmation.
- **D-04:** **Subir JPG = JPG only, 2 MB cap, no server re-encode.** Frontend `<input type="file" accept="image/jpeg">` rejects non-JPG client-side; backend validates MIME + size + magic bytes (reject HEIC, PNG, mislabeled). Original bytes pushed to devices unchanged. **Risk:** if Hikvision firmware rejects images >200 KB, server-side downscale becomes follow-up work — flag in research.
- **D-05:** **AI Validation panel = client-side `face-api.js`, hard gate.** Lazy-load tinyFaceDetector model (~6 MB) only on the `/enrollment` route; never on dashboard/timesheet. Three checks before "Capturar Rostro" / "Aceptar" enables: (1) face bounding box detected, (2) average frame luminance within 80–200 (canvas pixel sample), (3) face bbox ≥160×160 px. All three must be green.

#### Multi-Device Sync & Status
- **D-06:** **Backend topology = tokio `JoinSet`, fire-and-forget.** `POST /api/v1/enrollments` validates payload, persists `enrollments` + `face_enrollments` + N `enrollment_device_pushes` rows (status=pending, one per active device), spawns N tokio tasks via JoinSet, returns 202 with `enrollment_id` immediately. Each task: decrypt device password (Phase 2 D-01), open ISAPI digest-auth client, push face profile, update its row to `success` or `failed` (with `error_message`). Tasks run independent of the request lifecycle.
- **D-07:** **Per-device status surface = polling.** Frontend `useQuery({ queryKey: ['enrollment', id], refetchInterval: 1500 })` against `GET /api/v1/enrollments/:id` until every push row is terminal. No SSE.
- **D-08:** **Partial failure = per-device terminal status, manual retry only.** Each push task ends as success or failed (with error message); no automatic retries. Modal shows red bar for each failed device + per-row "Reintentar" button → `POST /api/v1/enrollments/:id/devices/:device_id/retry`. Enrollment-level status is `success` if ≥1 device succeeded, `partial` if some succeeded + some failed, `failed` if 0 succeeded.
- **D-09:** **Modal close mid-sync = sync continues, status persists.** Closing modal does NOT cancel tasks. App-level toast/badge shows "Enrolamiento en curso — X/Y dispositivos".

#### face_id, Photo Storage & ISAPI
- **D-10:** **face_id = Cronometrix-generated UUID v4, stable per employee.** New nullable column `employees.face_id TEXT UNIQUE`. Assigned on first successful enrollment (any device), never changes for the lifetime of that employee row. Same face_id pushed to every Hikvision device.
- **D-11:** **Canonical photo storage = `./data/enrollments/{employee_id}/{enrollment_id}.jpg` with full history.** New `face_enrollments` table tracks per-attempt metadata. `employees.current_face_enrollment_id TEXT` points to the active one.
- **D-12:** **Hikvision ISAPI face-profile endpoint family = lock during research.** Phase 7 backend has no existing face-upload code path. Research must fetch current Hikvision ISAPI docs and pick between (a) modern 2-step `PUT /ISAPI/AccessControl/UserInfo/Record` + `POST /ISAPI/Intelligent/FDLib/FaceDataRecord` multipart, or (b) legacy single-step `PUT /ISAPI/AccessControl/UserInfo/SetUp`. Researcher must validate against target firmware (DS-K1T341, DS-K1T342).
- **D-13:** **`device_face_mappings` write timing = per-device, on push success only.**

#### Lifecycle: Re-enroll, Deactivate, New Device
- **D-14:** **Re-enrollment = stable face_id, photo replaced on devices, history retained.**
- **D-15:** **Employee deactivation = auto-purge from all devices.**
- **D-16:** **New device registration = auto-backfill all active employee profiles.**
- **D-17:** **Audit triggers on `enrollments`, `face_enrollments`, AND `device_face_mappings`.**

#### RBAC
- **D-18:** Admin only. All enrollment endpoints + UI gated by `require_admin` middleware.

### Claude's Discretion
- Exact column types, indexes, FK ON DELETE behavior on the new tables — follow Phase 1/2 conventions (UUID PKs, INTEGER UTC epoch timestamps, version column on mutable rows).
- Migration numbering (`016_enrollments.sql`, `017_face_enrollments.sql`, `018_phase7_audit_triggers.sql`, etc.) — planner sequences.
- Background purge worker (D-15) and backfill worker (D-16) topology — likely a single tokio task per worker type using the Phase 3 recompute-worker mpsc + debounce pattern, or one-shot tokio::spawn per trigger. Planner picks based on expected volume.
- face-api.js model loader UX (loading skeleton during ~6 MB download on first use of `/enrollment` route).
- Per-device push timeout — likely 30s to mirror Phase 2 ISAPI `REQUEST_TIMEOUT`, but plan can adjust.
- Whether the kiosk-mode capture flow (D-02) uses a dedicated state machine in the handler or is composed from existing ISAPI client primitives.

### Deferred Ideas (OUT OF SCOPE)
- Mass admin re-sync screen
- Server-side image downscale (RECONSIDERED — see § Open Questions Q1; this research RECOMMENDS pulling forward into Phase 7)
- Face quality scoring v2
- Device firmware compatibility matrix
- Enrollment audit panel UI
- Batch enrollment via CSV+photos
- Face detection on the alertStream side

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENRL-01 | Admin can capture a facial profile via Hikvision device camera | Kiosk mode flow uses existing `enrollment_mode()` ISAPI primitive (already wired in `isapi/client.rs`) + a new `capture_face_image()` method that polls `/ISAPI/AccessControl/CaptureFaceData` for the JPG result. See § ISAPI Integration → Kiosk Capture. |
| ENRL-02 | Admin can upload a JPG photo for facial enrollment | Axum 0.8 `Multipart` extractor + magic-byte validation + `image` crate downscale to ≤200KB before push. See § Don't Hand-Roll → Image validation. |
| ENRL-03 | Admin can capture a facial profile via webcam | `getUserMedia` + `<canvas>` + `toBlob('image/jpeg', 0.92)` produces ~50KB frame; fits envelope without server downscale. See § Code Examples → Webcam capture. |
| ENRL-04 | System syncs enrolled facial profile to all registered devices simultaneously | `tokio::task::JoinSet` spawns N concurrent push tasks per enrollment; same pattern with `Semaphore::new(4)` cap for D-16 backfill fan-out. See § Architecture → Multi-Device Push. |
| ENRL-05 | Admin can see per-device sync status during enrollment (progress/success/failure) | `enrollment_device_pushes` rows updated by each task; `GET /api/v1/enrollments/:id` returns the array; TanStack Query `refetchInterval` with dynamic stop function (return `false` when all rows terminal). See § Architecture → Status Surface. |

## Project Constraints (from CLAUDE.md)

These directives MUST be honored in plans — same authority as locked CONTEXT.md decisions:

- **Backend stack:** Rust 1.77+ / Axum 0.8.8 / Tokio 1.x — non-negotiable.
- **HTTP client to devices:** `reqwest` 0.13.2 + `diqwest` 3.2.0 (digest auth) — no alternatives.
- **XML parsing:** `quick-xml` 0.39.x with `serialize` feature — already in Cargo.toml.
- **Multipart on backend:** Axum 0.8 `multipart` feature (already enabled). `multer` 3.1.0 transitively used by Axum.
- **Database:** Raw libSQL queries only — no SeaORM, no Diesel.
- **Audit:** Every mutation to enrollment-related tables MUST trigger an `audit_log` row via SQLite triggers.
- **Frontend stack:** Next.js 16.2 (Proxy renamed from Middleware), React 19.2, shadcn/ui base-nova, TanStack Query v5.99, react-hook-form 7.72, Zod 4.3 — versions already pinned in `frontend/package.json`.
- **No CSS-in-JS:** Tailwind 4 only.
- **Spanish (Venezuela) UI copy** — project memory `project_jurisdiction.md`.
- **TZ = America/Caracas, no DST** — `chrono-tz` already at 0.10.4 in Cargo.toml.
- **Argon2id for passwords** — not relevant to Phase 7 (no new auth surface).
- **GSD Workflow Enforcement:** All file changes must go through a GSD command — Phase 7 plans execute via `/gsd-execute-phase`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Webcam stream + canvas capture | Browser | — | `getUserMedia` is a browser-only API; secure context (HTTPS) required. The Cloudflare tunnel terminates HTTPS at the Cronometrix domain so this is satisfied in production. |
| face-api.js validation gating | Browser | — | TFJS runs in browser; ~6 MB model bundle loads via dynamic import on the `/enrollment` route only. Backend never sees the validation outcome — it's a UX gate, not a server-side authority. |
| JPG file selection + client-side MIME guard | Browser | API (re-validation) | First gate is in the browser for UX; backend re-validates magic bytes + size because client-side checks are bypassable. |
| Multipart receive + magic-byte + size validation | API / Backend | — | Authoritative — never trust the client-side gate. |
| Server-side image downscale (≤200KB JPEG) | API / Backend | — | Hikvision firmware enforces 200KB limit — server is the only place that can guarantee compliance regardless of upload source (device kiosk JPG, webcam JPG, user upload). |
| ISAPI digest-auth push to each device | API / Backend | — | Devices are on-prem LAN, only the backend has network reach + decryption key for credentials. |
| Concurrent multi-device fan-out | API / Backend | — | `tokio::task::JoinSet` orchestration — pure backend concern. |
| Enrollment + push state persistence | Database / SQLite | Cloud (Turso replica) | Local-first per DATA-03; Turso replicates async. |
| Photo file storage | Local filesystem (`./data/enrollments/`) | — | Per Phase 2 D-13 convention; mirrors `./data/events/` for attendance JPEGs. NOT in libSQL BLOB (see § Architecture → Photo Storage). |
| Per-device status polling | Browser ↔ API | — | TanStack Query polls `GET /api/v1/enrollments/:id`; backend reads from `enrollment_device_pushes` table. |
| Background purge worker (D-15) | API / Backend | — | Triggered by employee deactivation; runs detached from request lifecycle. |
| Background backfill worker (D-16) | API / Backend | — | Triggered by `POST /devices` success; semaphore-capped fan-out. |
| Audit log writes | Database (SQLite triggers) | — | Per Phase 1 Decision; legal traceability for biometric data — must be enforced at DB level, not application level. |
| RBAC gating (admin only) | Browser (UI gate) ↔ API (auth) | — | `useAuth()` hides UI; `require_admin` middleware is authoritative. |

## Standard Stack

### Core (already in repo — verify versions match plan)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | 0.8.8 | HTTP server, multipart extractor (`features = ["multipart"]` already on) | Stack lock per CLAUDE.md `[VERIFIED: Cargo.toml]` |
| `tokio` | 1.x (full features) | Runtime + `task::JoinSet` for D-06 fan-out | Stack lock `[VERIFIED: Cargo.toml]` |
| `reqwest` | 0.13.2 | Outbound HTTP to ISAPI, multipart body builder via `reqwest::multipart` | Stack lock `[VERIFIED: Cargo.toml]` |
| `diqwest` | 3.2.0 | Digest auth (RFC 2617) for Hikvision | Stack lock `[VERIFIED: Cargo.toml]`; already used by `DeviceConnection::send_xml`/`send_json` |
| `quick-xml` | 0.39.2 | XML parsing if device returns `<ResponseStatus>` shape | Stack lock `[VERIFIED: Cargo.toml]` |
| `multer` | 3.1.0 | Underlying multipart parser (transitively used by Axum) | Already in Cargo.toml `[VERIFIED]` |
| `serde_json` | 1 | JSON encode for `UserInfo/Record` body | Stack lock `[VERIFIED: Cargo.toml]` |
| `libsql` | 0.9.30 | SQLite + Turso sync | Stack lock `[VERIFIED: Cargo.toml]` |
| `uuid` | 1 (v4) | face_id and enrollment_id generation | Stack lock `[VERIFIED: Cargo.toml]` |
| `chrono` | 0.4 (serde) | Timestamps in audit + status rows | Stack lock `[VERIFIED: Cargo.toml]` |
| `validator` | 0.20 | Multipart DTO validation | Stack lock `[VERIFIED: Cargo.toml]` |
| `tracing` | 0.1 | Per-task structured logging during fan-out | Stack lock `[VERIFIED: Cargo.toml]` |
| `bytes` | 1 | Holding decoded JPG bytes between downscale + push | Stack lock `[VERIFIED: Cargo.toml]` |

### NEW dependencies (Phase 7 adds)

| Library | Version | Purpose | Why Now |
|---------|---------|---------|---------|
| `image` | 0.25.10 | Server-side JPEG decode + downscale to ≤200KB envelope | Hikvision FaceDataRecord enforces 200KB hard limit `[CITED: Hikvision TPP wiki]`. Without this, D-04 uploads from phone cameras (typically 1–4 MB) will reject device-side. The crate is mainstream, MIT licensed, depends on `zune-jpeg` 0.5+ for fast pure-Rust decode. `[VERIFIED: cargo search image @ 0.25.10]` |

**Installation (Rust):**
```bash
# Add to backend/Cargo.toml [dependencies]
image = { version = "0.25.10", default-features = false, features = ["jpeg"] }
```
Disable default features to avoid pulling PNG/AVIF/WebP/etc — only JPEG is needed for decode-resize-encode.

### Frontend NEW dependencies

| Library | Version | Purpose | Why This One |
|---------|---------|---------|--------------|
| `@vladmandic/face-api` | ^1.7.15 | Browser-side face detection for D-05 hard gate | The original `face-api.js@0.22.2` was last published 2024 with a hard dependency on `@tensorflow/tfjs-core@1.7.0` — incompatible with React 19 / TFJS 4+. The vladmandic fork ships its own bundled TFJS (zero peer deps), MIT licensed, 23.7 MB unpacked. Repo archived Feb 2025 but the published 1.7.15 build is the current de-facto choice and works in static deployments. `[VERIFIED: npm view face-api.js & npm view @vladmandic/face-api]` |
| `tabs`, `progress`, `select`, `sonner`, `badge`, `skeleton` (shadcn) | latest via `npx shadcn add` | UI primitives for the modal | Per UI-SPEC § Component Inventory. shadcn/ui copy-paste; no version pin (you own the code). |

**Installation (frontend):**
```bash
npm install @vladmandic/face-api
npx shadcn@latest add tabs progress select sonner badge skeleton
```

**Models bundle (separate from npm install):**
- Download `tiny_face_detector_model-weights_manifest.json` and `tiny_face_detector_model-shard1` from `@vladmandic/face-api` GitHub `model/` folder.
- Vendor into `frontend/public/models/` so the browser fetches them from the same origin (no CDN dependency, no cross-origin model load).
- Total weight: ~190KB (quantized) — much smaller than the originally-cited "6 MB" in CONTEXT.md Specifics. The 6 MB figure likely refers to the full model set (SSD MobileNet + landmarks + recognition). For Phase 7's "is there a face?" gate, ONLY tinyFaceDetector is needed. `[CITED: justadudewhohacks/face-api.js README]`

> **Update to CONTEXT.md Specifics (informational):** The "face-api.js is a 6 MB cold load" warning overstates the actual cost. tinyFaceDetector quantized model is ~190KB; the npm package itself is ~24MB unpacked but most of that is the bundled TFJS WebAssembly backend, which is loaded once and cached. Real first-load cost ≈ 1.5–2 MB gzipped (TFJS + tinyFaceDetector). Lazy-load discipline still matters — D-05's mandate to load only on `/enrollment` route + only when modal opens still applies.

### Alternatives Considered

| Instead of | Could Use | Tradeoff — Why Rejected |
|------------|-----------|--------|
| `image` crate | `turbojpeg` (libjpeg-turbo wrapper) | turbojpeg is 2-4x faster for decode but requires libjpeg-turbo system lib; Docker image becomes harder. `image` is pure Rust + the workload (5-10 enrollments/day per site) doesn't justify the ops overhead. |
| `image` crate | `fast_image_resize` | Excellent SIMD perf but only does resize — still need a JPEG decoder/encoder. `image` is one dependency vs two. |
| `@vladmandic/face-api` | `face-api.js` (original) | TFJS 1.7.0 dep blocks; React 19 incompatibility. |
| `@vladmandic/face-api` | `@vladmandic/human` | Larger model bundle, more features (age/gender/emotion) — overkill for a 3-check binary gate. |
| `@vladmandic/face-api` | MediaPipe Face Detection | Google library, modern, but has WebAssembly + Web Worker plumbing complexity. face-api.js is a single import. |
| `@vladmandic/face-api` | Server-side detection (Rust crate `dlib-face-recognition` or similar) | Round-trips on every webcam frame; ruins UX; Cronometrix product promise is local-first low-latency. |
| `tokio::task::JoinSet` | `futures::stream::FuturesUnordered` | JoinSet integrates with tokio's task aborter on drop; FuturesUnordered does not. JoinSet is the documented pattern. `[CITED: tokio docs.rs JoinSet]` |
| `tokio::task::JoinSet` | Manual `tokio::spawn` + `Vec<JoinHandle>` | Loses the structured-concurrency guarantee + harder to drain. |
| Multipart JSON+JPG via Axum | Send JSON + photo upload as separate API calls | Two-call flow adds a transaction window where photo is uploaded but enrollment never created (or vice versa). One multipart call is atomic. |
| Polling 1500ms (D-07) | SSE | Keeps Phase 4's SSE channel scoped to dashboard activity feed — adding enrollment events would balloon broadcast volume. Polling is simple and a 5-device fleet completes in ~3-4 polls. |
| Polling 1500ms | WebSockets | Overkill for one-directional status. Polling matches Phase 2 lifecycle pattern. |

## Architecture Patterns

### System Architecture Diagram

```
┌────────────────────────────── BROWSER (React 19, Next.js 16) ──────────────────────────────┐
│                                                                                              │
│  /enrollment route                                                                           │
│       │                                                                                      │
│       └─→ EnrollmentModal (lazy-loaded)                                                      │
│              │                                                                               │
│              ├─→ KioskCaptureTab ──→ POST /api/v1/enrollments/capture-from-device (1)        │
│              │                       (returns { capture_id, photo_blob_path })               │
│              │                                                                               │
│              ├─→ WebcamCaptureTab → getUserMedia → <canvas> → toBlob('image/jpeg', 0.92)     │
│              │                       │                                                       │
│              │                       └─→ @vladmandic/face-api (tinyFaceDetector) ──┐         │
│              │                            evaluates ~10 fps:                       │         │
│              │                            - face bbox detected                     ▼         │
│              │                            - luminance 80-200                  CTA enabled    │
│              │                            - bbox ≥160×160 px                                 │
│              │                                                                               │
│              ├─→ UploadCaptureTab ──→ <input type="file" accept="image/jpeg"> (2 MB cap)     │
│              │                                                                               │
│              └─→ DialogFooter "Aceptar"                                                      │
│                       │                                                                      │
│                       └─→ POST /api/v1/enrollments (multipart) ────────────────────┐         │
│                            FormData: { employee_id, captured_via, photo: Blob }    │         │
│                                                                                    │         │
│              ┌────────────── useQuery refetchInterval=1500ms ──────────┐           │         │
│              │   GET /api/v1/enrollments/:id                            │           │         │
│              │     while (!allTerminal) repeat                          │           │         │
│              └──────────────────────────────────────────────────────────┘           │         │
└──────────────────────────────────────────────────────────────────────────────────────┼───────┘
                                                                                      │
                                                                                      ▼
┌─────────────────────────────── BACKEND (Rust + Axum 0.8) ───────────────────────────────────┐
│                                                                                              │
│  POST /api/v1/enrollments  [require_admin middleware]                                        │
│       │                                                                                      │
│       1. Multipart::extract → magic-byte + size validation (2 MB)                            │
│       2. Decode JPEG with `image` crate                                                      │
│       3. If decoded size > 200KB OR JPEG bytes > 200KB: downscale + re-encode at q=85        │
│          loop until output ≤ 200KB (typically 1-2 iterations)                                │
│       4. Persist:                                                                            │
│          - face_enrollments row (photo_path = ./data/enrollments/{emp}/{enr}.jpg)            │
│          - enrollments row (status=in_progress)                                              │
│          - N enrollment_device_pushes rows (status=pending, one per active device)           │
│          - employees.face_id ← UUID v4 if NULL (D-10)                                        │
│          - employees.current_face_enrollment_id ← face_enrollment.id                         │
│       5. Write JPEG bytes to disk (atomic temp+rename, mirror events::write_photo_atomic)    │
│       6. Spawn N tokio tasks via JoinSet (detached, returns 202)                             │
│                                                                                              │
│       │                                                                                      │
│       ▼                                                                                      │
│  ┌──────────── tokio::task::JoinSet (per-enrollment) ───────────────┐                        │
│  │                                                                   │                        │
│  │  for each active device:                                         │                        │
│  │    spawn(async {                                                 │                        │
│  │       1. devices::service::get_decrypted (AES-256-GCM unwrap)    │                        │
│  │       2. DeviceConnection::new(...)                              │                        │
│  │       3. POST /ISAPI/AccessControl/UserInfo/Record (JSON)        │                        │
│  │          (idempotent — adds OR errors duplicate; treat dup as ok)│                        │
│  │       4. POST /ISAPI/Intelligent/FDLib/FaceDataRecord (multipart)│                        │
│  │          ├ part: FaceDataRecord (Content-Type: application/json) │                        │
│  │          │   { faceLibType: "blackFD", FDID: "1", FPID: face_id }│                        │
│  │          └ part: FaceImage (Content-Type: image/jpeg, JPEG bytes)│                        │
│  │       5. tokio::time::timeout(30s, ...)                          │                        │
│  │       6. UPDATE enrollment_device_pushes SET status,error_message│                        │
│  │       7. INSERT OR REPLACE INTO device_face_mappings on success  │                        │
│  │       8. Append command_audit_log row                            │                        │
│  │    });                                                           │                        │
│  │                                                                   │                        │
│  │  All tasks independent; JoinSet drained in detached driver task. │                        │
│  └─────────────────────────────────────────────────────────────────┘                        │
│                                                                                              │
│  GET /api/v1/enrollments/:id  [require_admin]                                                │
│       │                                                                                      │
│       └─→ SELECT enrollments + LEFT JOIN enrollment_device_pushes + devices.name             │
│            return { id, status, device_pushes: [...] }                                       │
│                                                                                              │
│  POST /api/v1/enrollments/:id/devices/:dev/retry  [require_admin]                            │
│       │                                                                                      │
│       └─→ Spawn ONE more push task (same shape as inner spawn above) → 202                   │
│                                                                                              │
│  ┌──────────── Background workers (started in main.rs after init_db) ─────────┐              │
│  │  PurgeWorker (D-15):                                                       │              │
│  │     mpsc::Receiver<EmployeeId> ← employees::service::deactivate            │              │
│  │     for each row in device_face_mappings: ISAPI delete + DELETE row        │              │
│  │     on per-device failure: mark mapping state='pending_delete', retry next │              │
│  │                                                                            │              │
│  │  BackfillWorker (D-16):                                                    │              │
│  │     mpsc::Receiver<DeviceId> ← devices::handlers::create_device            │              │
│  │     SELECT employee.id WHERE face_id IS NOT NULL AND status='active'       │              │
│  │     fan-out via JoinSet capped at Semaphore::new(4)                        │              │
│  └────────────────────────────────────────────────────────────────────────────┘              │
│                                                                                              │
│  ┌──────────── SQLite (libSQL local + Turso async replica) ────────────────────┐             │
│  │   New tables: enrollments, face_enrollments, enrollment_device_pushes      │             │
│  │   Modified tables: employees (+face_id, +current_face_enrollment_id),      │             │
│  │                    device_face_mappings (+state)                           │             │
│  │   New triggers: audit_enrollments_*, audit_face_enrollments_*,             │             │
│  │                 audit_device_face_mappings_* (triple set per Phase 1 conv) │             │
│  │   New rows in audit_log on every INSERT/UPDATE/DELETE                      │             │
│  │   NO photo BLOB column — bytes live on disk per Phase 2 D-13               │             │
│  └─────────────────────────────────────────────────────────────────────────────┘             │
│                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
                                                          │
                                                          │ digest auth (reqwest + diqwest)
                                                          ▼
┌───────────────────────── Hikvision DS-K1T341 / DS-K1T342 (LAN) ──────────────────────────────┐
│                                                                                              │
│   /ISAPI/AccessControl/UserInfo/Record           POST  JSON   create person                  │
│   /ISAPI/AccessControl/UserInfo/Modify           PUT   JSON   modify person                  │
│   /ISAPI/AccessControl/UserInfoDetail/Delete     PUT   JSON   delete person (async, paired)  │
│   /ISAPI/AccessControl/UserInfoDetail/DeleteProcess GET JSON  poll delete completion         │
│   /ISAPI/Intelligent/FDLib/FaceDataRecord        POST  multipart   add face photo            │
│   /ISAPI/AccessControl/CaptureFaceData           POST  JSON   trigger device-camera capture  │
│                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
backend/src/
├── enrollments/                       # NEW module — mirrors Phase 1 / 2 layout
│   ├── mod.rs                         # pub use
│   ├── models.rs                      # CreateEnrollmentForm (multipart), EnrollmentResponse, DevicePushResponse, FaceQualityScore
│   ├── service.rs                     # persist_enrollment, list_devices_for_push, update_push_status, write_face_id_if_missing
│   ├── handlers.rs                    # POST /enrollments (multipart), GET /enrollments/:id, POST .../retry, POST /capture-from-device
│   ├── pusher.rs                      # spawn_pushes(JoinSet), single push_task fn (decrypt → connection → 2-step ISAPI → persist)
│   ├── image_pipeline.rs              # decode + downscale-to-200KB loop using `image` crate
│   └── isapi_face.rs                  # FaceProfilePayload struct, build_user_info_record_json, build_facedata_multipart
├── isapi/
│   ├── client.rs                      # EXTEND: add upsert_user(...), upload_face(...), delete_user(...), capture_face_image(...)
│   └── ...                            # (existing)
├── workers/                           # NEW or extend existing pattern
│   ├── purge.rs                       # D-15 purge worker (mpsc + per-device fan-out)
│   └── backfill.rs                    # D-16 backfill worker (mpsc + Semaphore-capped JoinSet)
├── employees/service.rs               # EXTEND: deactivate publishes to PurgeWorker mpsc
├── devices/handlers.rs                # EXTEND: create_device publishes to BackfillWorker mpsc
├── state.rs                           # EXTEND: AppState gets purge_tx, backfill_tx (Option<UnboundedSender<...>>)
├── main.rs                            # EXTEND: spawn purge/backfill workers after init_db, before alertStream supervisor
└── db/migrations/
    ├── 016_enrollments.sql                    # enrollments + face_enrollments + enrollment_device_pushes + employees columns + device_face_mappings.state
    ├── 017_phase7_audit_triggers.sql          # audit triggers for all 4 mutated tables
    └── (planner finalizes numbering against current 015)

frontend/src/
├── app/(dashboard)/enrollment/page.tsx        # REPLACE placeholder
├── components/enrollment/
│   ├── enrollment-modal.tsx
│   ├── kiosk-capture-tab.tsx
│   ├── webcam-capture-tab.tsx
│   ├── upload-capture-tab.tsx
│   ├── validation-panel.tsx                   # @vladmandic/face-api logic lives here
│   ├── sync-panel.tsx
│   ├── sync-row.tsx
│   ├── employee-enrollment-picker.tsx
│   └── in-progress-list.tsx
├── components/common/access-restricted.tsx    # NEW shared RBAC placeholder
├── lib/face-detection.ts                      # tinyFaceDetector loader + frame analyzer (extracted for testability)
├── lib/validations.ts                         # extend with enrollmentSubmitSchema
├── types/api.ts                               # extend with Enrollment types
├── public/models/                             # NEW directory — vendored tinyFaceDetector model files
│   ├── tiny_face_detector_model-weights_manifest.json
│   └── tiny_face_detector_model-shard1
└── proxy.ts                                   # register /api/v1/enrollments/*
```

### Pattern 1: tokio::task::JoinSet for fire-and-forget multi-device fan-out

**What:** Spawn N independent push tasks per enrollment; let them run detached; each task updates its row in `enrollment_device_pushes`.

**When to use:** D-06 (per-enrollment fan-out), D-16 (per-employee backfill on new device with concurrency cap).

**Example:**

```rust
// Source: https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html (verified)
use tokio::task::JoinSet;
use std::sync::Arc;

/// Fan out one push task per active device. Detached driver task drains the
/// JoinSet so the originating HTTP handler can return 202 immediately.
pub fn spawn_enrollment_pushes(
    state: AppState,
    enrollment_id: String,
    face_id: String,
    photo_bytes: Arc<Vec<u8>>,
    employee_id: String,
    devices: Vec<DeviceWithPlaintext>,
) {
    tokio::spawn(async move {
        let mut set = JoinSet::new();
        for device in devices {
            let state = state.clone();
            let enrollment_id = enrollment_id.clone();
            let face_id = face_id.clone();
            let photo_bytes = Arc::clone(&photo_bytes);
            let employee_id = employee_id.clone();
            set.spawn(async move {
                push_one_device(state, enrollment_id, face_id, photo_bytes, employee_id, device).await
            });
        }
        // Drain — every result is logged; final enrollment-level status
        // (success/partial/failed) is computed in a single SQL UPDATE
        // after the loop closes.
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(_)) => {}                                  // task wrote success row
                Ok(Err(e)) => tracing::warn!(err = %e, "push task returned error"),
                Err(e) => tracing::error!(err = %e, "push task panicked"),
            }
        }
        // Recompute enrollment.status from device_pushes
        if let Err(e) = finalize_enrollment_status(&state, &enrollment_id).await {
            tracing::error!(err = %e, "failed to finalize enrollment status");
        }
    });
}
```

### Pattern 2: Hikvision 2-step face profile push with reqwest::multipart

**What:** Modern Hikvision firmware splits person creation from face binding. Step 1 is a JSON POST that creates/updates the user metadata; step 2 is a multipart POST that binds a face image to that user via FDID/FPID.

**When to use:** Every per-device push task (the main D-06 path). Idempotent — re-pushing for re-enrollment overwrites the existing face record on the device.

**Example:**

```rust
// Source: composed from
//   https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-EEC135B9-F974-4C17-8188-E74F05F8B536.html
//   https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-91A7BD78-D7A9-4014-A171-3A549DE81694.html
// CITED — both are official Hikvision TPP ISAPI wiki pages.

impl DeviceConnection {
    /// Step 1: Create or update the person record on the device.
    /// employee_no = our face_id (UUID v4 — Hikvision treats employeeNo as a string).
    /// name = our employee.name (Hikvision UTF-8, ≤32 chars per most firmware).
    pub async fn upsert_user(
        &self,
        face_id: &str,
        full_name: &str,
    ) -> Result<String> {
        let url = format!(
            "{}/ISAPI/AccessControl/UserInfo/Record?format=json",
            self.base_url
        );
        let body = serde_json::json!({
            "UserInfo": {
                "employeeNo": face_id,
                "name": truncate_utf8(full_name, 32),
                "userType": "normal",
                "Valid": {
                    "enable": true,
                    "beginTime": "2020-01-01T00:00:00",
                    "endTime":   "2099-12-31T23:59:59",
                    "timeType":  "local"
                },
                "doorRight": "1",
                "RightPlan": [{ "doorNo": 1, "planTemplateNo": "1" }]
            }
        });
        self.send_json(&url, reqwest::Method::POST, &body.to_string()).await
    }

    /// Step 2: Bind the JPG to the user record via FDLib multipart.
    /// faceLibType="blackFD" is the access-control face library on these devices.
    /// FPID must equal employeeNo from step 1.
    pub async fn upload_face(
        &self,
        face_id: &str,
        jpeg_bytes: Vec<u8>,
    ) -> Result<String> {
        let url = format!(
            "{}/ISAPI/Intelligent/FDLib/FaceDataRecord?format=json",
            self.base_url
        );
        let metadata = serde_json::json!({
            "faceLibType": "blackFD",
            "FDID":        "1",       // default access-control library on K1T34x
            "FPID":        face_id,
        }).to_string();

        let form = reqwest::multipart::Form::new()
            // Part 1: JSON metadata, name MUST be "FaceDataRecord"
            .part(
                "FaceDataRecord",
                reqwest::multipart::Part::text(metadata)
                    .mime_str("application/json")?,
            )
            // Part 2: image bytes, name MUST be "FaceImage"
            .part(
                "FaceImage",
                reqwest::multipart::Part::bytes(jpeg_bytes)
                    .file_name("face.jpg")
                    .mime_str("image/jpeg")?,
            );

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send_digest_auth((self.username.as_str(), self.password.as_str()))
            .await
            .context("ISAPI face upload request failed")?;
        let status = resp.status();
        let text = resp.text().await.context("read ISAPI response body")?;
        anyhow::ensure!(
            status.is_success(),
            "device returned non-success status {status}: {text}"
        );
        Ok(text)
    }

    /// Delete the person (and implicitly, their face record) from the device.
    /// Hikvision delete is asynchronous — it returns 200 on submit then completes
    /// in the background. We treat the 200 as success and let any cleanup error
    /// surface on the next sync attempt rather than poll DeleteProcess.
    pub async fn delete_user(&self, face_id: &str) -> Result<String> {
        let url = format!(
            "{}/ISAPI/AccessControl/UserInfoDetail/Delete?format=json",
            self.base_url
        );
        let body = serde_json::json!({
            "UserInfoDetail": {
                "mode": "byEmployeeNo",
                "EmployeeNoList": [{ "employeeNo": face_id }]
            }
        });
        self.send_json(&url, reqwest::Method::PUT, &body.to_string()).await
    }
}
```

### Pattern 3: Server-side image downscale loop

**What:** Iteratively reduce JPEG quality + dimensions until output ≤ 200KB. Hikvision FaceDataRecord rejects anything above this threshold; webcam captures at 0.92 quality JPEG are typically 50KB so they pass through; phone-camera uploads (1-4 MB) need this stage.

**When to use:** Always, on every photo received via `POST /api/v1/enrollments` or `/capture-from-device`. Cheap when no shrinking is needed (just a re-encode probe), bounded to ≤3 iterations.

**Example:**

```rust
// Source: https://docs.rs/image/latest/image/ — verified API surface
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use std::io::Cursor;

pub const MAX_FACE_BYTES: usize = 200 * 1024;
pub const TARGET_DIM_PX: u32 = 480;   // typical face library sweet spot

/// Decodes the input as JPEG (rejecting any other format for safety),
/// then downscales/re-encodes until the output is ≤ MAX_FACE_BYTES.
/// Returns the bytes ready to push to a Hikvision device.
pub fn normalize_face_jpeg(input: &[u8]) -> anyhow::Result<Vec<u8>> {
    // Reject inputs that aren't valid JPEG by magic bytes.
    anyhow::ensure!(
        input.len() >= 3 && &input[0..3] == &[0xFF, 0xD8, 0xFF],
        "input is not a valid JPEG (magic bytes mismatch)"
    );

    if input.len() <= MAX_FACE_BYTES {
        // Decode + re-encode to canonicalize EXIF orientation and strip metadata.
        let img = image::load_from_memory_with_format(input, ImageFormat::Jpeg)?;
        return reencode_jpeg(&img, 90);
    }

    let img = image::load_from_memory_with_format(input, ImageFormat::Jpeg)?;
    // First pass: shrink longest side to TARGET_DIM_PX, encode at q=85.
    let scaled = img.resize(TARGET_DIM_PX, TARGET_DIM_PX, FilterType::Lanczos3);
    let bytes = reencode_jpeg(&scaled, 85)?;
    if bytes.len() <= MAX_FACE_BYTES { return Ok(bytes); }

    // Second pass: q=70.
    let bytes = reencode_jpeg(&scaled, 70)?;
    if bytes.len() <= MAX_FACE_BYTES { return Ok(bytes); }

    // Third pass: 320 px @ q=70.
    let scaled = img.resize(320, 320, FilterType::Lanczos3);
    let bytes = reencode_jpeg(&scaled, 70)?;
    anyhow::ensure!(
        bytes.len() <= MAX_FACE_BYTES,
        "could not compress face image to {} bytes after 3 passes (final {} bytes)",
        MAX_FACE_BYTES, bytes.len()
    );
    Ok(bytes)
}

fn reencode_jpeg(img: &DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("encode failed: {e}"))?;
    // Note: image 0.25 JPEG encoder takes quality via JpegEncoder::new_with_quality
    // — wire that through if quality < default. For brevity in this snippet, the
    // caller can call JpegEncoder directly:
    //   JpegEncoder::new_with_quality(&mut buf, quality).encode_image(img)?;
    let _ = quality;
    Ok(buf.into_inner())
}
```

> **Note:** The `image` crate's high-level `write_to` doesn't expose a quality knob directly; use `image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality).encode_image(&img)` for explicit control. Plan should encode this in `image_pipeline.rs`.

### Pattern 4: TanStack Query polling with dynamic stop

**What:** Frontend polls `GET /api/v1/enrollments/:id` every 1500ms; stops automatically when every push row has reached a terminal state.

**When to use:** Modal `syncing` state through `terminal` (D-07). Same pattern works for the in-app background toast when modal is closed (D-09) — query continues running.

**Example:**

```typescript
// Source: https://tanstack.com/query/latest/docs/framework/react/guides/polling — verified
import { useQuery } from '@tanstack/react-query';

export function useEnrollmentStatus(id: string | null) {
  return useQuery({
    queryKey: ['enrollment', id],
    queryFn: () => fetch(`/api/v1/enrollments/${id}`).then(r => r.json()),
    enabled: !!id,
    refetchInterval: (query) => {
      const data = query.state.data as Enrollment | undefined;
      if (!data) return 1500;
      const allDone = data.device_pushes.every(
        p => p.status === 'success' || p.status === 'failed'
      );
      return allDone ? false : 1500;
    },
    refetchIntervalInBackground: false, // OK: admin must keep tab visible
  });
}
```

### Pattern 5: Webcam capture → JPEG Blob

**What:** Acquire a 640×480 video stream, draw it to a `<canvas>`, export as JPEG Blob at quality 0.92.

**When to use:** WebcamCaptureTab, only after the three face-api.js validation rows are green.

**Example:**

```typescript
// Source: https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia — verified
async function startWebcam(videoEl: HTMLVideoElement): Promise<MediaStream> {
  // getUserMedia REQUIRES a secure context (HTTPS). The Cloudflare tunnel
  // satisfies this for production; local dev needs `next dev --experimental-https`
  // or an in-browser localhost exemption.
  const stream = await navigator.mediaDevices.getUserMedia({
    video: { width: { ideal: 640 }, height: { ideal: 480 }, facingMode: 'user' },
    audio: false,
  });
  videoEl.srcObject = stream;
  await videoEl.play();
  return stream;
}

function captureFrameAsBlob(
  videoEl: HTMLVideoElement,
  canvasEl: HTMLCanvasElement
): Promise<Blob> {
  canvasEl.width = 640;
  canvasEl.height = 480;
  const ctx = canvasEl.getContext('2d')!;
  ctx.drawImage(videoEl, 0, 0, 640, 480);
  return new Promise((resolve, reject) => {
    canvasEl.toBlob(
      (b) => (b ? resolve(b) : reject(new Error('toBlob returned null'))),
      'image/jpeg',
      0.92
    );
  });
}

function stopWebcam(stream: MediaStream) {
  stream.getTracks().forEach(t => t.stop());
}
```

### Anti-Patterns to Avoid

- **Storing JPEG bytes in libSQL BLOB.** Turso async replication has bandwidth cost; replicating biometric photos to cloud doubles the data plane and creates a GDPR-style data minimization concern. Project decision (`./data/events/` for attendance JPEGs in Phase 2 D-13) already establishes filesystem storage as the convention. Phase 7 follows. `[CITED: Phase 2 D-13]`
- **Sharing a single `reqwest::Client` across all push tasks.** Each `DeviceConnection::new` builds a fresh client, which is the existing pattern in `isapi/client.rs`. A shared client would cache connections to one device and cross-contaminate digest auth state on the others. Per-task new client is cheap (microseconds).
- **Cancelling tokio tasks when the client disconnects.** D-09 explicitly says modal close ≠ cancel. Use `tokio::spawn` (detached) for the JoinSet driver so it survives the originating handler's drop.
- **Synchronous decode-resize on the request thread.** `image::load_from_memory` is CPU-bound and can take 100ms+ for multi-MB inputs. Wrap in `tokio::task::spawn_blocking` to avoid stalling other handlers.
- **Building one JoinSet per `POST /enrollments` AND awaiting it inline.** That blocks the response. Instead: build the JoinSet inside a `tokio::spawn(async move { ... })` so the handler returns 202 immediately.
- **Returning the photo bytes in `GET /enrollments/:id`.** That balloons response size on every poll. Photos are accessible via a separate `GET /enrollments/:id/photo` endpoint if needed (UI-SPEC doesn't require this; modal already has the bytes from preview).
- **Treating Hikvision delete as synchronous.** It uses an async pattern (`Delete` then `DeleteProcess` poll). For Phase 7 D-15 purge, treat the initial PUT 200 as "submitted"; if the next sync window finds the user still present, retry. `[CITED: Hikvision TPP wiki — UserInfoDetail/Delete]`
- **Putting face-api.js dependency on the dashboard route.** UI-SPEC mandates lazy load only on `/enrollment` modal open. Use Next.js `dynamic()` import or a plain `await import('@vladmandic/face-api')` inside `useEffect`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-device concurrent push | Manual `Vec<JoinHandle>` + `for h in handles { h.await }` | `tokio::task::JoinSet` | Built-in drop-aborts, drain ordering, panic propagation. `[CITED: tokio docs.rs]` |
| Multipart parsing on backend | Hand-rolled boundary scanner | Axum `Multipart` extractor (already enabled) | Multer-backed; handles boundary edge cases, Content-Disposition parsing, streaming. |
| Face detection in browser | Hand-coded Canvas pixel sampler for "is there a face" | `@vladmandic/face-api` tinyFaceDetector | Quantized 190KB model, ~10 fps on commodity laptops, MIT. |
| HTTP digest auth | Manual MD5 challenge-response | `diqwest` (already used) | RFC 2617 done right; `reqwest` integration. |
| JPEG decode + resize + encode | Pure Rust hand-rolled | `image` 0.25.10 | Pure Rust (no system libjpeg), supports `JpegEncoder::new_with_quality`. |
| HTTP polling lifecycle | `setInterval` + `clearInterval` + custom React state | TanStack Query `refetchInterval` (function form) | Auto-stop, suspends on tab background, cache integration. `[CITED: TanStack Query v5 docs]` |
| File magic-byte sniffing | Custom byte inspector | Plain check `bytes[0..3] == [0xFF, 0xD8, 0xFF]` for JPEG SOI | This IS the rule of thumb — but DON'T use the `infer` crate for what's effectively 3 bytes. |
| AES-256-GCM credential decrypt | New crypto module | Existing `devices::crypto::decrypt_password` (Phase 2 D-01) | Already vetted; same `device_creds_key` from `AppState.config`. |
| Audit log for enrollment events | Application-side INSERT in handlers | SQLite triggers (Phase 1 / 2 / 3 / 5 pattern) | Trigger-based audit cannot be skipped. Legal traceability. |
| Background worker dispatch | Cron crate | Tokio `mpsc::UnboundedSender` + receiver loop (Phase 3 RecomputeWorker pattern) | Already proven; deterministic; no extra dep. |

**Key insight:** Every "don't hand-roll" entry above maps to existing project assets or stack-locked crates. Phase 7's only NEW dependency at the crate level is `image` (backend) and `@vladmandic/face-api` (frontend) — everything else extends what's already proven in earlier phases.

## Common Pitfalls

### Pitfall 1: face-api.js requires a secure context AND a model fetch path

**What goes wrong:** Webcam never streams in dev; tinyFaceDetector model never loads.

**Why it happens:** `getUserMedia` requires HTTPS. In local dev (`next dev` over plain HTTP), the API silently returns `undefined` for `navigator.mediaDevices`. Separately, `loadFromUri('/models')` fetches the model files from the same origin — if you forget to vendor them into `public/models/`, the request 404s.

**How to avoid:**
1. Run dev with `next dev --experimental-https` (Next 13+) or accept-insecure-cert via Chrome's `chrome://flags` `Allow invalid certificates for localhost`.
2. Vendor `tiny_face_detector_model-weights_manifest.json` + `tiny_face_detector_model-shard1` into `frontend/public/models/` at plan time.
3. Add a smoke test: open `/enrollment` in a Playwright/manual run, expect the canvas to mount within 2s.

**Warning signs:** "navigator.mediaDevices is undefined", DevTools 404 on `/models/tiny_face_detector_model-weights_manifest.json`.

### Pitfall 2: Hikvision FaceDataRecord 400 on >200KB JPEG

**What goes wrong:** Push task succeeds for webcam captures (50KB) but fails for upload-tab phone photos (2 MB), making the upload mode effectively broken.

**Why it happens:** D-04 says "no server re-encode". The Hikvision wiki explicitly states `faceURL` (and binary `FaceImage`) "must be smaller than 200KB". `[CITED: tpp.hikvision.com/Wiki/.../GUID-EEC135B9...]`

**How to avoid:** Always run `normalize_face_jpeg()` on the request path BEFORE persisting/pushing. This contradicts D-04 — the deferred-ideas list flags downscale as a follow-up, but this research recommends pulling it into Phase 7 because the failure mode is a cleanly-broken happy path. See § Open Questions Q1.

**Warning signs:** ISAPI 400 / 415 with body `<ResponseStatus><statusCode>4</statusCode><statusString>Bad Request</statusString></ResponseStatus>`.

### Pitfall 3: Hikvision name field UTF-8 truncation surprise

**What goes wrong:** Spanish names with diacritics (`José`, `Andrés`, `Núñez`) get rejected or truncated mid-character, producing garbled employee labels on the device's local UI.

**Why it happens:** Most Hikvision firmware caps `name` at 32 BYTES (not characters). A multibyte UTF-8 string can blow past 32 bytes well under 32 chars; some firmware rejects with 400, others silently truncate mid-codepoint.

**How to avoid:** Use a UTF-8-safe truncator that walks codepoints and drops the last full one when the byte count would exceed 32. Don't use `&name[..32]` — that panics on multibyte boundaries.

```rust
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}
```

**Warning signs:** Devices return "Bad Request" for specific employees only (those with diacritics in the name).

### Pitfall 4: Self-signed TLS on Hikvision devices breaks reqwest by default

**What goes wrong:** Every push task fails with "tls handshake error: self-signed certificate" before even hitting the digest-auth challenge.

**Why it happens:** Hikvision ships self-signed certs by default. `DeviceConnection::new` already accepts `allow_insecure_tls` per device — but if the operator doesn't toggle it on during device registration, every push fails identically.

**How to avoid:** Existing `devices` table already has `allow_insecure_tls INTEGER`. Plan must surface this in the `error_message` field clearly — distinguish "TLS handshake failed" from "credentials rejected" — so the admin gets a UI hint to toggle the device's insecure-TLS flag.

**Warning signs:** Many devices fail with the same TLS error; one device with `allow_insecure_tls=1` succeeds.

### Pitfall 5: Concurrent JoinSet pushes on a single overloaded device

**What goes wrong:** D-16 backfill (50 employees × 1 new device) opens 50 simultaneous digest-auth handshakes against the same device; firmware connection table overflows; some pushes 503 / connection-reset.

**Why it happens:** Hikvision DS-K1T34x firmware has a small concurrent-connection ceiling (typically 5-10 simultaneous ISAPI sessions). 50 parallel pushes saturate this.

**How to avoid:** Cap the backfill JoinSet with `tokio::sync::Semaphore::new(4)` — each task acquires a permit before opening its connection. Per-enrollment fan-out (D-06) is bounded by the device count (typically 2-4), so no semaphore needed there.

```rust
let sem = Arc::new(Semaphore::new(4));
let mut set = JoinSet::new();
for emp in employees {
    let sem = Arc::clone(&sem);
    let dev = device.clone();
    set.spawn(async move {
        let _permit = sem.acquire_owned().await.unwrap();
        push_one_employee_to_device(emp, dev).await
    });
}
```

**Warning signs:** Backfill rate is dramatically slower than expected, devices show 503/connection-reset in `error_message`.

### Pitfall 6: getUserMedia stream not stopped on modal close → red light persists

**What goes wrong:** User closes the modal mid-capture, the laptop's camera light stays on indefinitely. Looks like spyware.

**Why it happens:** `MediaStream.getTracks()` continues to hold the camera until each track is `.stop()`ed. React's `useEffect` cleanup must call `stopWebcam(stream)` on unmount. Forgetting this is a common bug.

**How to avoid:** Always pair `startWebcam` with a cleanup in the same `useEffect`:

```typescript
useEffect(() => {
  let stream: MediaStream | null = null;
  startWebcam(videoRef.current!).then(s => { stream = s; });
  return () => { if (stream) stopWebcam(stream); };
}, [tab]);  // re-run when user switches tabs
```

**Warning signs:** Camera indicator stays on after dialog closes; second open of modal fails with "device in use".

### Pitfall 7: device_face_mappings unique key conflict on re-enrollment of moved-employee

**What goes wrong:** Employee A is re-enrolled. The `INSERT OR REPLACE` writes (device_id, face_id) → A. But the previous `face_id` for A was different (D-10 says face_id is stable, so this CAN'T happen — but if the data ever drifts, it does). Result: orphan mapping rows pointing to a face_id no device knows about.

**Why it happens:** Phase 7 banks heavily on D-10's "face_id never changes". A bug in `write_face_id_if_missing` (e.g., regenerating instead of preserving) silently corrupts the invariant.

**How to avoid:**
- Make `employees.face_id` `NOT NULL` once first set, by applying it via an UPDATE that only writes when current value IS NULL.
- Add a unit test that re-enrolling an employee twice keeps the face_id constant.
- Consider an `audit_log` query in tests: a face_id change row should never appear in normal operation.

**Warning signs:** `device_face_mappings` row count grows unbounded across re-enrollments; old rows for dropped face_ids never cleaned up.

### Pitfall 8: Multipart body limit silently truncates large uploads

**What goes wrong:** A 4 MB phone photo upload arrives at the backend with `Multipart` returning an empty stream — request appears as "no photo provided".

**Why it happens:** Axum's default body limit is 2 MB. CONTEXT.md D-04 caps uploads at 2 MB precisely for this reason — but without an explicit `RequestBodyLimitLayer` set to (e.g.) 3 MB on the enrollment route, the 2 MB Axum default can clip BEFORE the validator sees the size to reject it cleanly.

**How to avoid:** Apply `tower_http::limit::RequestBodyLimitLayer::new(3 * 1024 * 1024)` to the `POST /enrollments` route specifically. This ensures the user gets a clear `413 Payload Too Large` for >3 MB and the backend has 1 MB of slack to catch the >2 MB rejection at the validator level with a friendlier message.

**Warning signs:** Some uploads silently fail; client never sees a 413; backend logs "field reached size limit".

### Pitfall 9: Polling 1500ms × 50 admins × 50 enrollments = 1500 RPS on a single endpoint

**What goes wrong:** Stress test or production with multiple concurrent enrollments balloons the polling rate to thousands of requests per second against `GET /enrollments/:id`.

**Why it happens:** 1500ms polling is fine per-modal, but each modal counts. Real-world max is maybe 5 concurrent enrollments, so this is theoretical — but in pathological cases (e.g., re-render loop on the frontend), polling can fire much faster than 1500ms.

**How to avoid:**
- Keep the SELECT query a single LEFT JOIN with explicit indexes on `enrollment_device_pushes(enrollment_id)`.
- Frontend: ensure `useQuery` is keyed by `enrollment_id` only — re-renders shouldn't spawn new queries.
- Cache GET responses for 1s in a tower middleware if profiling shows hotspot.

**Warning signs:** APM dashboard shows `/enrollments/:id` as the single hottest endpoint.

### Pitfall 10: Operator deletes employee while D-15 purge is mid-flight

**What goes wrong:** Admin deactivates Employee X. PurgeWorker starts deleting from device 1. Admin re-activates X (UI doesn't gate this). PurgeWorker continues, deletes from device 2. Now X is "active" with face_id but absent from device 2.

**Why it happens:** The purge loop iterates `device_face_mappings` rows; status changes during iteration aren't observed.

**How to avoid:** Each iteration of the purge loop re-reads `employees.status` for the row's employee. If `status='active'` again, abort the purge and clear `pending_delete`. Plan should specify this guard.

**Warning signs:** "Employee enrolled but missing from device N" reports without an obvious failure log.

## Code Examples

### Webcam capture + face validation (frontend, lazy-loaded)

```typescript
// Source: composed from
//   https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia
//   https://github.com/vladmandic/face-api (README, verified 1.7.15)

import { useEffect, useRef, useState } from 'react';

export function useWebcamWithValidation(active: boolean) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [validation, setValidation] = useState({
    faceDetected: false,
    luminanceOk: false,
    sizeOk: false,
  });
  const [stream, setStream] = useState<MediaStream | null>(null);
  const faceapiRef = useRef<typeof import('@vladmandic/face-api') | null>(null);

  // Lazy-load face-api only when active.
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    (async () => {
      const faceapi = await import('@vladmandic/face-api');
      await faceapi.nets.tinyFaceDetector.loadFromUri('/models');
      if (cancelled) return;
      faceapiRef.current = faceapi;
    })();
    return () => { cancelled = true; };
  }, [active]);

  // Start/stop webcam.
  useEffect(() => {
    if (!active) return;
    let s: MediaStream | null = null;
    (async () => {
      s = await navigator.mediaDevices.getUserMedia({
        video: { width: 640, height: 480, facingMode: 'user' },
      });
      if (videoRef.current) {
        videoRef.current.srcObject = s;
        await videoRef.current.play();
      }
      setStream(s);
    })();
    return () => { s?.getTracks().forEach(t => t.stop()); setStream(null); };
  }, [active]);

  // Per-frame validation loop (~10 fps).
  useEffect(() => {
    if (!active || !stream) return;
    const interval = setInterval(async () => {
      const faceapi = faceapiRef.current;
      const v = videoRef.current;
      const c = canvasRef.current;
      if (!faceapi || !v || !c) return;
      // 1. face bbox
      const det = await faceapi.detectSingleFace(
        v,
        new faceapi.TinyFaceDetectorOptions({ inputSize: 224, scoreThreshold: 0.5 })
      );
      const faceDetected = !!det;
      const sizeOk = !!det && det.box.width >= 160 && det.box.height >= 160;
      // 2. luminance — sample canvas pixels
      c.width = 64; c.height = 48;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(v, 0, 0, 64, 48);
      const px = ctx.getImageData(0, 0, 64, 48).data;
      let total = 0;
      for (let i = 0; i < px.length; i += 4) {
        total += 0.299 * px[i] + 0.587 * px[i + 1] + 0.114 * px[i + 2];
      }
      const avg = total / (px.length / 4);
      const luminanceOk = avg >= 80 && avg <= 200;
      setValidation({ faceDetected, luminanceOk, sizeOk });
    }, 100);
    return () => clearInterval(interval);
  }, [active, stream]);

  return { videoRef, canvasRef, validation, stream };
}
```

### Multipart enrollment receive (backend)

```rust
// Source: https://docs.rs/axum/latest/axum/extract/multipart/index.html — verified

use axum::extract::{Multipart, State};
use axum::Json;
use axum::http::StatusCode;
use crate::auth::rbac::AuthUser;
use crate::errors::AppError;
use crate::state::AppState;
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct EnrollmentSubmitResponse {
    pub enrollment_id: String,
    pub device_pushes: Vec<DevicePushSummary>,
}

pub async fn create_enrollment(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    mut mp: Multipart,
) -> Result<(StatusCode, Json<EnrollmentSubmitResponse>), AppError> {
    let mut employee_id: Option<String> = None;
    let mut captured_via: Option<String> = None;
    let mut source_device_id: Option<String> = None;
    let mut photo_bytes: Option<Vec<u8>> = None;

    while let Some(field) = mp.next_field().await
        .map_err(|e| AppError::Validation { code: "VALIDATION_ERROR", message: e.to_string() })?
    {
        match field.name() {
            Some("employee_id") => employee_id = Some(field.text().await.map_err(internal)?),
            Some("captured_via") => captured_via = Some(field.text().await.map_err(internal)?),
            Some("source_device_id") => source_device_id = Some(field.text().await.map_err(internal)?),
            Some("photo") => {
                let bytes = field.bytes().await.map_err(internal)?;
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(AppError::Validation {
                        code: "PHOTO_TOO_LARGE",
                        message: "Photo must be ≤ 2 MB".into(),
                    });
                }
                if !(bytes.len() >= 3 && &bytes[0..3] == &[0xFF, 0xD8, 0xFF]) {
                    return Err(AppError::Validation {
                        code: "PHOTO_NOT_JPEG",
                        message: "Photo must be JPEG (magic bytes mismatch)".into(),
                    });
                }
                photo_bytes = Some(bytes.to_vec());
            }
            _ => { /* ignore unknown fields */ }
        }
    }

    let employee_id = employee_id.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR", message: "employee_id required".into(),
    })?;
    let captured_via = captured_via.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR", message: "captured_via required".into(),
    })?;
    let photo_bytes = photo_bytes.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR", message: "photo required".into(),
    })?;

    // Decode + downscale (CPU-bound — wrap in spawn_blocking).
    let normalized = tokio::task::spawn_blocking(move || normalize_face_jpeg(&photo_bytes))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking task panicked: {e}")))?
        .map_err(|e| AppError::Validation {
            code: "PHOTO_INVALID", message: e.to_string(),
        })?;

    // Persist + write photo to disk + spawn pushes
    let resp = enrollments::service::start_enrollment(
        state.clone(), &claims.sub, &employee_id, &captured_via,
        source_device_id.as_deref(), normalized,
    ).await?;

    Ok((StatusCode::ACCEPTED, Json(resp)))
}

fn internal<E: std::error::Error + Send + Sync + 'static>(e: E) -> AppError {
    AppError::Internal(anyhow::anyhow!(e))
}
```

### Schema migration (planner finalizes column types)

```sql
-- 016_enrollments.sql
-- Phase 7 enrollment tables. Mirrors Phase 1/2 conventions:
--   UUID v4 PKs (TEXT), UTC epoch INTEGER timestamps, version on mutable rows,
--   soft-delete via status (no hard deletes for biometric audit).

ALTER TABLE employees ADD COLUMN face_id TEXT UNIQUE;
ALTER TABLE employees ADD COLUMN current_face_enrollment_id TEXT;
ALTER TABLE device_face_mappings ADD COLUMN state TEXT NOT NULL DEFAULT 'active'
    CHECK(state IN ('active','pending_delete'));

CREATE TABLE IF NOT EXISTS face_enrollments (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    captured_via TEXT NOT NULL CHECK(captured_via IN ('device','webcam','upload')),
    source_device_id TEXT REFERENCES devices(id),
    photo_path TEXT NOT NULL,
    face_quality_score TEXT,                    -- JSON: { face_detected, luminance, width, height }
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_face_enrollments_employee ON face_enrollments(employee_id);

CREATE TABLE IF NOT EXISTS enrollments (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    face_enrollment_id TEXT NOT NULL REFERENCES face_enrollments(id),
    status TEXT NOT NULL DEFAULT 'in_progress' CHECK(status IN ('in_progress','success','partial','failed')),
    started_by TEXT NOT NULL REFERENCES users(id),
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_enrollments_employee ON enrollments(employee_id);
CREATE INDEX IF NOT EXISTS idx_enrollments_status ON enrollments(status);

CREATE TABLE IF NOT EXISTS enrollment_device_pushes (
    id TEXT PRIMARY KEY,
    enrollment_id TEXT NOT NULL REFERENCES enrollments(id),
    device_id TEXT NOT NULL REFERENCES devices(id),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','in_progress','success','failed')),
    error_message TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    UNIQUE(enrollment_id, device_id)
);
CREATE INDEX IF NOT EXISTS idx_edp_enrollment ON enrollment_device_pushes(enrollment_id);
CREATE INDEX IF NOT EXISTS idx_edp_status ON enrollment_device_pushes(status);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hikvision legacy `PUT /ISAPI/AccessControl/UserInfo/SetUp` (single-step JSON with embedded base64 photo) | Modern 2-step: `POST /UserInfo/Record` (JSON, person) + `POST /Intelligent/FDLib/FaceDataRecord` (multipart, photo) | Firmware ≥V2.0 on K1T34x series (2022+) | All current target devices use the 2-step flow. Legacy is documented but not used by current firmware. `[CITED: Hikvision TPP wiki, 2024 update]` |
| `face-api.js@0.22.2` (original, TFJS 1.7) | `@vladmandic/face-api@1.7.15` (bundled TFJS 4) | Original abandoned 2024-03; vladmandic fork archived 2025-02 but still functional | The "canonical" name is misleading — vladmandic fork is what teams actually ship. `[VERIFIED: npm view]` |
| `tokio::spawn` + `Vec<JoinHandle>` for fan-out | `tokio::task::JoinSet` | Tokio 1.21 (Sep 2022) introduced JoinSet | Cleaner drain semantics, drop-aborts pending tasks. `[CITED: tokio docs.rs]` |
| TanStack Query `refetchInterval: number \| false` | `refetchInterval: number \| false \| ((query) => number \| false)` (function form) | TanStack Query v5 (Oct 2023) | Lets us stop polling per-query without state-mirroring tricks. `[CITED: TanStack Query v5 docs]` |
| Axum 0.7 `Multipart` extractor | Axum 0.8 `Multipart` extractor | Jan 2025 | API surface is the same; `Multipart` lives in `axum::extract::multipart` either way. |

**Deprecated/outdated:**

- **`face-api.js@0.22.2` (original):** Last published 2024-03; depends on `@tensorflow/tfjs-core@1.7.0` which conflicts with TFJS 4+ used by everything else in a modern React 19 app. Avoid.
- **Hikvision `UserInfo/SetUp` (legacy single-step):** Still works on older firmware but documented as superseded in TPP wiki. Don't build new code against it.
- **Polling via `useEffect` + `setInterval`:** Replaced by TanStack Query's `refetchInterval` function form for v5. Don't hand-roll.

## Assumptions Log

> Tracking claims that are based on training knowledge or pattern-matching rather than this session's tool verification. Discuss-phase / planner should confirm each before locking into an executable plan.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | DS-K1T341 / DS-K1T342 firmware ≥V2.0 supports the modern 2-step `UserInfo/Record` + `FaceDataRecord` flow. | § Standard Stack, Pattern 2 | If a deployed device runs older firmware, the plan needs a fallback to legacy `UserInfo/SetUp` or a firmware-upgrade prerequisite. Mitigation: capture real ISAPI traffic on a dev unit before plan freeze (same blocker pattern as Phase 2 alertStream — STATE.md flag). |
| A2 | Hikvision FDLib uses `faceLibType: "blackFD"` and `FDID: "1"` as the default access-control face library on K1T34x. | Pattern 2 | If the device exposes a different default FDID, all push tasks 400. Mitigation: probe `GET /ISAPI/Intelligent/FDLib?format=json` on first device registration to discover the real FDID. |
| A3 | Hikvision name field caps at 32 BYTES (UTF-8). | Pitfall 3 | If the cap is actually different (some firmware allows 64), the truncate function is unnecessarily aggressive. Low-blast-radius — confirmable on first push. |
| A4 | The Cloudflare tunnel terminates HTTPS at `{slug}.cronometrix.com` so `getUserMedia` always runs in a secure context in production. | Architectural Responsibility Map | If tunnel config differs (e.g., dev/staging exposes plain HTTP), webcam tab silently breaks. Mitigation: Phase 6 deployment plan likely already enforces HTTPS — confirm with Phase 6 docs. |
| A5 | `image` crate 0.25.10 JPEG encoder respects the quality parameter passed to `JpegEncoder::new_with_quality`. | Pattern 3 | Verified in code by inspection, but the iterative downscale loop assumes quality reduction actually shrinks bytes — true for almost all images, but pathological inputs (already-quantized images) may not. Mitigation: the third pass also reduces dimensions, which guarantees shrinkage. |
| A6 | Hikvision delete (`UserInfoDetail/Delete`) returns 200 immediately and processes async; treating the 200 as "submitted" is acceptable for D-15. | Pattern 2 anti-pattern | If a delete silently fails (e.g., user has unsaved local changes on the device), the next sync window catches it. Acceptable in the v1 design. |
| A7 | `multer` 3.1.0 + Axum 0.8 enforces the configured `RequestBodyLimitLayer` correctly without truncation surprises. | Pitfall 8 | Axum issue #1666 (cited in search) hints at edge cases; mitigation is an integration test that uploads a 3 MB file and asserts a 413 — not a 200 with empty fields. |
| A8 | Each Hikvision device handles 4 concurrent ISAPI sessions reliably (D-16 backfill semaphore cap). | Pitfall 5 | Could be 5 or 10; conservative cap of 4 is safe but slow. Tunable via env var. |

## Open Questions (RESOLVED)

### Q1: Should server-side image downscale (currently in CONTEXT.md "Deferred Ideas") be pulled into Phase 7 scope?

**What we know:**
- Hikvision FaceDataRecord enforces a 200KB hard limit on JPEG payloads. `[CITED: tpp.hikvision.com/Wiki/.../GUID-EEC135B9...]`
- Webcam capture at quality 0.92 produces ~50KB images (passes).
- D-04's "Subir JPG" path explicitly says "no server re-encode" — uploads (typically phone-camera 1-4 MB) WILL exceed the limit.

**What's unclear:**
Whether D-04's "no re-encode" was intended as a v1 limitation (knowingly accepting that uploads >200KB will fail) or whether the deferred-ideas note expected research to surface this and trigger a re-decision.

**Recommendation:**
Pull downscale into Phase 7. The cost is one new crate (`image` 0.25.10) and one helper function (~50 LOC, shown in Pattern 3). The benefit is that "Subir JPG" actually works for real-world uploads. Without it, the upload mode is broken-by-design for >50% of admin uploads. Discuss-phase rebound (or planner judgment) needed if this contradicts D-04 deliberately.


**RESOLVED (2026-04-27):** Pulled into Phase 7 per CONTEXT.md Research Lock-In **D-04 SUPERSEDED** — server-side downscale moved into scope; `image = "0.25.10"` added to `backend/Cargo.toml`; iterative downscale loop implemented in 07-01 Task 4 (`backend/src/enrollments/image_pipeline.rs::normalize_face_jpeg`).

### Q2: Is `face-api.js` (the literal package name in CONTEXT.md D-05) interchangeable with `@vladmandic/face-api`?

**What we know:**
- CONTEXT.md says "client-side `face-api.js`, hard gate" (D-05).
- The original `face-api.js@0.22.2` is unmaintained with TFJS 1.7 dep — incompatible with React 19.
- `@vladmandic/face-api@1.7.15` is the maintained fork with bundled TFJS 4 — same API surface, drop-in replacement.

**What's unclear:**
Whether D-05 referred to "any face-api.js fork" or specifically the original package name.

**Recommendation:**
Plan should specify `@vladmandic/face-api` by name. The functional intent of D-05 (face detection in browser, hard gate, lazy load) is unaffected. The choice is operational/dependency hygiene.


**RESOLVED (2026-04-27):** Use `@vladmandic/face-api@1.7.15` per CONTEXT.md Research Lock-In **D-05 SUPERSEDED** (maintained fork with bundled TFJS 4, drop-in API surface replacement for the unmaintained original). Locked in 07-02 Task 1 frontend dependency install + vendored tinyFaceDetector model files under `frontend/public/models/`.

### Q3: Should `D-12`'s capture-from-device flow be implemented inline (handler) or as a 2-step state machine (capture endpoint + poll endpoint)?

**What we know:**
- UI-SPEC API Contract describes `POST /enrollments/capture-from-device` returning a `capture_id`, then polling `GET /enrollments/captures/:capture_id`.
- Hikvision `/ISAPI/AccessControl/CaptureFaceData` puts the device into capture mode but doesn't return the JPG synchronously — the device captures, then the app must `GET` the captured image.
- The actual ISAPI capture flow is multi-step (mode-on → device captures → fetch captured-image → mode-off).

**What's unclear:**
Exactly how the device returns the captured JPG — via a polling endpoint on the device, via a webhook/callback, or via the alertStream we're already listening on.

**Recommendation:**
Treat as Claude's Discretion (CONTEXT.md). Recommended approach: implement the capture flow as a 2-step state machine on the backend (so the UI doesn't hang for 30s). The first endpoint returns 202 + a `capture_id`; a backend task polls the device for the captured image; the UI polls `GET /captures/:id` on the same 1500ms cadence as the enrollment status. Reuses the existing `enrollment_mode()` ISAPI primitive for step 1.


**RESOLVED (2026-04-27):** 2-step state machine per CONTEXT.md Research Lock-In **D-02 LOCKED**. `POST /api/v1/enrollments/capture-from-device` returns 202 + `capture_id` immediately; frontend polls `GET /api/v1/enrollments/captures/:capture_id` on the same 1500ms cadence as enrollment status. Implemented in 07-01 Task 4 (handlers `capture_from_device` + `get_capture`) and consumed by 07-02 Task 3 (`kiosk-capture-tab.tsx`). Captured JPEG bytes are returned inline via `CaptureResponse.photo_b64` (base64) when status==captured — frontend decodes via `atob()` → `Blob` → `URL.createObjectURL` for preview, no second HTTP fetch needed.

### Q4: Should photos be served via a dedicated `GET /enrollments/:id/photo` endpoint, or via a static file route?

**What we know:**
- Photos live on disk (`./data/enrollments/{employee_id}/{enrollment_id}.jpg`) per D-11.
- UI-SPEC doesn't strictly require photo retrieval — the modal already has the bytes from the user-controlled capture/upload.
- Future audit panel (deferred) WILL need photo retrieval.

**What's unclear:**
Phase 7 scope.

**Recommendation:**
Out of Phase 7 scope. Don't build it. If the in-progress list (`InProgressEnrollmentList` per UI-SPEC) ever needs to show a thumbnail, expose a thin `GET /enrollments/:id/photo` returning `image/jpeg` bytes from disk — but only if the UI demands it. Audit-panel phase can add this when it lands.


**RESOLVED (2026-04-27):** Out of Phase 7 scope; **deferred to a future audit-panel phase** per CONTEXT.md Research Lock-In (`GET /enrollments/:id/photo` deferred). Phase 7 endpoints stay scoped to write/push/status only. The kiosk-mode preview is satisfied by `CaptureResponse.photo_b64` inline-base64 (Q3 RESOLVED) — distinct from a generic photo read-back endpoint.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Backend build | ✓ (existing project) | 1.77+ | — |
| Cargo registry access | `image` crate install | ✓ (existing project workflow) | — | — |
| npm registry access | `@vladmandic/face-api` install | ✓ (existing project workflow) | — | — |
| Node 24 (build) | Frontend build | ✓ (Phase 6 Dockerfile pins this) | 24-alpine | — |
| Hikvision DS-K1T341/342 hardware on dev LAN | ISAPI smoke testing | ✗ | — | wiremock-based recorded fixtures (see Pitfall 1 from Phase 2) |
| Browser with HTTPS or localhost-secure context | Webcam testing | ✓ in production via Cloudflare tunnel; dev needs `next dev --experimental-https` | — | Manual cert override for local dev |
| `tiny_face_detector_model-weights_manifest.json` + `tiny_face_detector_model-shard1` files | face-api.js model load | ✗ (must vendor at plan time) | model from `@vladmandic/face-api` repo `model/` folder | — |
| 6 MB+ disk space per ~20 enrollments | `./data/enrollments/` | ✓ (typical install has GBs free) | — | Future: rotation policy if >1k enrollments |

**Missing dependencies with no fallback:**
- Real Hikvision device for ISAPI smoke test. Carry forward Phase 2 STATE.md blocker — capture real ISAPI traffic before implementation.

**Missing dependencies with fallback:**
- tinyFaceDetector model files: download once during plan execution and commit to `frontend/public/models/` (~190KB total).
- Local HTTPS for webcam dev: use `next dev --experimental-https` or run in production-like deployment via the Phase 6 tunnel for dev testing.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Backend framework | `cargo test` + `cargo nextest` (already in dev workflow) `[VERIFIED: Cargo.toml dev-deps]` |
| Backend test config | `[dev-dependencies]` block in `backend/Cargo.toml` (existing — `axum-test`, `wiremock`, `tempfile`, `proptest`) |
| Backend quick run | `cargo test --package cronometrix-api --lib enrollments` |
| Backend full suite | `cargo test --workspace` |
| Frontend framework | Vitest 4.1.5 + Testing Library + jsdom 29 + msw 2.7 `[VERIFIED: frontend/package.json devDeps]` |
| Frontend config | `vite.config.ts` (existing) |
| Frontend quick run | `npm test -- --run src/components/enrollment` |
| Frontend full suite | `npm test -- --run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENRL-01 | Capture face via Hikvision device camera | integration (backend, wiremock-mocked ISAPI) | `cargo test --package cronometrix-api --test enrollment_capture_from_device` | ❌ Wave 0 |
| ENRL-01 | Kiosk capture tab calls `/capture-from-device` and renders preview | unit (frontend) | `npm test -- src/components/enrollment/kiosk-capture-tab.test.tsx --run` | ❌ Wave 0 |
| ENRL-02 | Upload tab rejects non-JPG client-side | unit (frontend) | `npm test -- src/components/enrollment/upload-capture-tab.test.tsx --run` | ❌ Wave 0 |
| ENRL-02 | Backend rejects non-JPEG magic bytes with 400 PHOTO_NOT_JPEG | integration (backend) | `cargo test --package cronometrix-api --test enrollment_validation` | ❌ Wave 0 |
| ENRL-02 | Backend downscales >200KB JPEG to ≤200KB | unit (backend, in `image_pipeline.rs`) | `cargo test --package cronometrix-api --lib enrollments::image_pipeline` | ❌ Wave 0 |
| ENRL-03 | Webcam tab acquires getUserMedia and disposes on unmount | unit (frontend, jsdom-mocked navigator.mediaDevices) | `npm test -- src/components/enrollment/webcam-capture-tab.test.tsx --run` | ❌ Wave 0 |
| ENRL-03 | face-api validation gate disables CTA until 3 checks green | unit (frontend) | `npm test -- src/components/enrollment/validation-panel.test.tsx --run` | ❌ Wave 0 |
| ENRL-04 | POST /enrollments spawns N tasks; all 4 succeed → enrollments.status='success' | integration (backend, wiremock-mocked ISAPI on 4 devices) | `cargo test --package cronometrix-api --test enrollment_fanout` | ❌ Wave 0 |
| ENRL-04 | 1 of 4 devices fails → enrollments.status='partial' | integration | included in `enrollment_fanout` | ❌ Wave 0 |
| ENRL-04 | All 4 devices fail → enrollments.status='failed' | integration | included in `enrollment_fanout` | ❌ Wave 0 |
| ENRL-04 | Re-enrollment keeps employees.face_id stable | integration | `cargo test --package cronometrix-api --test enrollment_re_enroll` | ❌ Wave 0 |
| ENRL-05 | GET /enrollments/:id returns per-device push rows | integration | `cargo test --package cronometrix-api --test enrollment_status_get` | ❌ Wave 0 |
| ENRL-05 | Frontend stops polling when all rows terminal | unit (frontend) | `npm test -- src/components/enrollment/enrollment-modal.test.tsx --run` | ❌ Wave 0 |
| Pitfall 8 | RequestBodyLimitLayer returns 413 for >3 MB upload | integration | included in `enrollment_validation` | ❌ Wave 0 |
| Pitfall 5 | D-16 backfill respects Semaphore(4) — at most 4 concurrent ISAPI calls | integration (wiremock counts in-flight) | `cargo test --package cronometrix-api --test backfill_semaphore` | ❌ Wave 0 |
| Pitfall 10 | Purge worker aborts mid-loop if employee re-activated | integration | `cargo test --package cronometrix-api --test purge_employee_reactivated` | ❌ Wave 0 |
| D-15 | Employee deactivation triggers purge worker | integration | `cargo test --package cronometrix-api --test purge_on_deactivate` | ❌ Wave 0 |
| D-16 | New device registration triggers backfill worker | integration | `cargo test --package cronometrix-api --test backfill_on_register` | ❌ Wave 0 |
| D-17 | Audit triggers fire on enrollments + face_enrollments + device_face_mappings INSERT/UPDATE/DELETE | integration | `cargo test --package cronometrix-api --test enrollment_audit_triggers` | ❌ Wave 0 |
| D-18 | Non-admin role gets 403 on every enrollment endpoint | integration | `cargo test --package cronometrix-api --test enrollment_rbac` | ❌ Wave 0 |
| Manual | End-to-end against real DS-K1T341 hardware | manual | recorded in test plan; cannot automate without hardware | — |

### Sampling Rate

- **Per task commit (Wave N+):** `cargo test --package cronometrix-api --lib enrollments` (Rust) + `npm test -- --run src/components/enrollment` (frontend) — both finish in <30s.
- **Per wave merge:** `cargo test --workspace` + `npm test -- --run`.
- **Phase gate:** Full suite green + manual smoke against at least one real Hikvision device before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] `backend/tests/enrollment_validation.rs` — covers ENRL-02 + Pitfall 8
- [ ] `backend/tests/enrollment_fanout.rs` — covers ENRL-04 (all branches)
- [ ] `backend/tests/enrollment_capture_from_device.rs` — covers ENRL-01
- [ ] `backend/tests/enrollment_re_enroll.rs` — covers D-14
- [ ] `backend/tests/enrollment_status_get.rs` — covers ENRL-05
- [ ] `backend/tests/enrollment_audit_triggers.rs` — covers D-17
- [ ] `backend/tests/enrollment_rbac.rs` — covers D-18
- [ ] `backend/tests/purge_on_deactivate.rs` + `purge_employee_reactivated.rs` — covers D-15 + Pitfall 10
- [ ] `backend/tests/backfill_on_register.rs` + `backfill_semaphore.rs` — covers D-16 + Pitfall 5
- [ ] `backend/src/enrollments/image_pipeline.rs` (with `#[cfg(test)] mod tests`) — covers ENRL-02 downscale path
- [ ] `frontend/src/components/enrollment/*.test.tsx` (one per component) — covers ENRL-01..05 frontend behaviors
- [ ] `frontend/public/models/tiny_face_detector_*` — model files vendored before the validation-panel test runs

Framework install: not needed — backend `[dev-dependencies]` already includes `axum-test`, `wiremock`, `tempfile`, `proptest`; frontend already has `vitest`, `@testing-library/react`, `jsdom`, `msw`.

## Security Domain

> Required because `security_enforcement` is not explicitly disabled in config. Phase 7 handles biometric data — facial photos are PII / sensitive personal data — so security review is non-optional regardless.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Existing JWT + Argon2 (Phase 1) — no new auth surface in Phase 7 |
| V3 Session Management | yes | Existing httpOnly refresh cookie + Bearer access token (Phase 1) — unchanged |
| V4 Access Control | yes | `require_admin` middleware (Phase 1 D-09) gates ALL Phase 7 endpoints |
| V5 Input Validation | yes | (a) Multipart magic-byte check on photo bytes; (b) `validator::Validate` on form fields (employee_id is UUID, captured_via is enum); (c) zod schema mirror in frontend |
| V6 Cryptography | yes | AES-256-GCM device password (Phase 2 D-01) for ISAPI auth; no new crypto introduced in Phase 7 |
| V7 Error Handling | yes | `error_message` on `enrollment_device_pushes` MUST scrub credentials before persisting (devices' digest-auth response can echo username) |
| V8 Data Protection | yes | Photos stored on local disk only — Turso replica is async per DATA-02; consider whether biometric photos should be excluded from cloud sync |
| V9 Communication | yes | HTTPS required for `getUserMedia` (browser-side); ISAPI runs over HTTPS with self-signed cert handled per device — Pitfall 4 |
| V12 File Upload | yes | (a) magic-byte JPEG check; (b) hard 2 MB request body limit + Multer per-field limit; (c) extension whitelist `.jpg/.jpeg`; (d) random server-side filename = `{enrollment_id}.jpg` (no user-controlled filenames) |
| V13 API & Web Service | yes | All endpoints under `/api/v1/enrollments/*` admin-gated; no anonymous endpoints |

### Known Threat Patterns for {Rust+Axum + Hikvision LAN}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Photo upload abused for SSRF (e.g., crafted EXIF that causes a parser to fetch a URL) | Tampering / Information Disclosure | `image` crate decode strips metadata implicitly during re-encode; explicit no-EXIF policy in `image_pipeline.rs` |
| Photo upload abused for image-decode CVE (e.g., libjpeg parsing bug) | Denial of Service / RCE | Use pure-Rust `image` 0.25.10 (no native libjpeg); decode in `tokio::task::spawn_blocking` so a panic/abort in the decoder cannot poison the runtime |
| Path traversal via filename | Tampering | NEVER use the user-supplied filename. Compose path as `./data/enrollments/{uuid}/{uuid}.jpg` server-side; store path in DB column. Phase 2 D-13 already establishes this convention. |
| Plaintext password leakage via error message | Information Disclosure | Use `DeviceWithPlaintext` (no `Serialize`, no `Debug`) — already implemented. Audit ALL `error_message` writes for any string containing `device.password`. Add a unit test that asserts no `error_message` ever contains the substring of any device password. |
| Insufficient rate limiting on enrollment endpoint | Denial of Service | Same JWT auth tower layer applies; admin-only gate provides a low-cardinality attack surface. No additional rate limiter for v1 (admin-only routes). |
| Unauthenticated `GET /enrollments/:id` exposing photo paths | Information Disclosure | Endpoint is `require_admin` — verified via integration test. |
| Cross-tenant data leak in multi-tenant deployment | Information Disclosure | Cronometrix is single-tenant per install (PROJECT.md: "each installation is independent"). No cross-tenant boundary. |
| Replay attack on `POST /enrollments/:id/devices/:dev/retry` | Tampering | Idempotent on the device side (`UPDATE OR REPLACE`). Frontend deduplicates click via TanStack Query mutation pending state. |
| Audit log tampering | Repudiation | Audit triggers run inside the same transaction as the mutation; row-level deletion is enforced by DB-level CHECK or trigger that rejects DELETE on `audit_log` (existing pattern from Phase 1). |
| Biometric data exfil via Turso replica | Information Disclosure | Photos live on disk, NOT in libSQL. Turso only replicates the `face_enrollments` row (path, metadata) — not the JPEG bytes. Operator must be told this when reviewing GDPR-style retention policies. (Note: Venezuela jurisdiction per project memory — GDPR not strictly applicable, but principle holds.) |

**Credential handling rule (carry-forward from Phase 2 RESEARCH):**
- Plaintext device passwords NEVER appear in `Serialize`, `Debug`, or `error_message` strings.
- Phase 7 adds `DeviceWithPlaintext` usage in every push task — the existing redaction discipline must be preserved.

## Sources

### Primary (HIGH confidence)

- [Hikvision TPP Wiki — JSON_AddFaceRecordCond / FaceDataRecord multipart structure (faceLibType, FDID, FPID)](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-EEC135B9-F974-4C17-8188-E74F05F8B536.html) — endpoint + multipart structure verified
- [Hikvision TPP Wiki — UserInfo/SetUp endpoint definition](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-2B87C87F-3573-4DBD-B0FD-492A36C96C2D.html) — legacy single-step alternative
- [Hikvision TPP Wiki — UserInfoDetail/Delete (async delete pattern)](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-8463D245-EB62-45E0-9CB6-73A30943FF04.html) — D-15 ISAPI shape
- [Hikvision TPP Wiki — CaptureFaceData (enrollment mode)](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-740FB5FF-9D84-4069-BFB0-5F32CDA10B32.html) — D-02 kiosk capture trigger
- [Tokio docs.rs — JoinSet](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) — concurrent fan-out pattern
- [Tokio docs.rs — task::spawn](https://docs.rs/tokio/latest/tokio/task/fn.spawn.html) — fire-and-forget detached task semantics
- [Axum docs.rs — Multipart extractor](https://docs.rs/axum/latest/axum/extract/multipart/struct.Multipart.html) — backend multipart receive
- [Tower-http docs.rs — RequestBodyLimitLayer](https://docs.rs/tower-http/latest/tower_http/limit/struct.RequestBodyLimitLayer.html) — body size limit middleware
- [TanStack Query v5 — Polling docs](https://tanstack.com/query/latest/docs/framework/react/guides/polling) — `refetchInterval` function form for dynamic stop
- [MDN — MediaDevices.getUserMedia()](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia) — webcam API contract
- [MDN — HTMLCanvasElement.toBlob()](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/toBlob) — JPEG export
- [npm view face-api.js](https://www.npmjs.com/package/face-api.js) — version + last-publish verified
- [npm view @vladmandic/face-api](https://www.npmjs.com/package/@vladmandic/face-api) — fork version + maintenance status verified
- [image-rs/image GitHub](https://github.com/image-rs/image) — JPEG decoder/encoder, version 0.25.10 verified via `cargo search`
- [Cargo.toml (this repo)](backend/Cargo.toml) — stack lock verified

### Secondary (MEDIUM confidence)

- [@vladmandic/face-api GitHub README](https://github.com/vladmandic/face-api) — tinyFaceDetector model size (~190KB quantized), TFJS 4 bundling verified
- [face-api.js GitHub original README](https://github.com/justadudewhohacks/face-api.js/) — model loading API
- [Shaykhnazar/hikvision-isapi PHP wrapper](https://github.com/Shaykhnazar/hikvision-isapi) — practical FaceService usage pattern (third-party wrapper, treat as guidance not authority)
- [DS-K1T341A Series User Manual](https://assets.hikvision.com/prd/public/all/doc/m000039711/UD17594B-E_Baseline_DS-K1T341A-Series-Face-Recognition-Terminal_User-Manual_V3.3_20230802.pdf) — target firmware features (image limits not explicitly in user manual)
- [Manage Face Records in Face Picture Library — Hikvision TPP wiki index page](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-707BB83B-774A-47A9-8C95-2B5905AD73C6.html) — index of face record management endpoints
- [TanStack Query v5 GitHub Discussion #713](https://github.com/TanStack/query/discussions/713) — `refetchInterval` returning false to stop polling pattern

### Tertiary (LOW confidence — flagged in Assumptions Log)

- [IP Cam Talk — Hikvision DS-K1T320 face detection thread](https://ipcamtalk.com/threads/api-for-hikvision-face-detection-ds-k1t320ewx.80578/) — community-reported endpoint behavior (different model series — directional only)
- [ISAPI Developer Guide for Face Recognition (Scribd 2022 version)](https://www.scribd.com/document/669288741/ISAPI-Developer-Guide-Access-Control-Face-Recognition-Terminals-2022-07-01) — older edition, used only for cross-checking endpoint paths

## Metadata

**Confidence breakdown:**

- Standard stack (versions + crates): HIGH — every version verified via `cargo search`, `npm view`, or existing Cargo.toml.
- Architecture (concurrency, multipart, polling): HIGH — well-documented patterns, all locked stack-side.
- Hikvision ISAPI endpoints: MEDIUM — primary endpoints verified via TPP wiki, but A1/A2 (firmware compatibility, FDLib defaults) need real-hardware validation per Phase 2 STATE.md blocker pattern.
- Pitfalls: HIGH — drawn from documented community experience + this codebase's existing patterns; each has a concrete avoidance strategy.
- Image pipeline (downscale loop): MEDIUM — `image` crate API is verified, but the iterative quality reduction is heuristic; production tuning may want to swap to `turbojpeg` if perf becomes an issue.
- face-api.js choice: HIGH — npm metadata directly verified; the case for the vladmandic fork over the original is open-and-shut.

**Research date:** 2026-04-27
**Valid until:** 2026-05-27 (30 days; reduce to 7 days if Hikvision releases a major firmware update for the K1T34x series in the interim)
