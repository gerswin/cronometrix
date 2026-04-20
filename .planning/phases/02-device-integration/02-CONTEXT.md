# Phase 2: Device Integration - Context

**Gathered:** 2026-04-19
**Status:** Ready for planning

<domain>
## Phase Boundary

The backend maintains live alertStream connections to all registered Hikvision devices, captures attendance events into a persisted, deduplicated event store, exposes a Device Manager REST API (register/edit/disable/command dispatch) with encrypted credential storage, and publishes device online/offline status through the API. Covers: DEV-01..04, EVT-01..04. Pure backend phase — no frontend work.

Out of scope: time calculation (Phase 3), dashboard UI (Phase 4), facial enrollment flows (Phase 7), licensing gate (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Credential Storage
- **D-01:** ISAPI device passwords stored encrypted with AES-256-GCM. Plaintext never hits SQLite. Decryption happens in-memory only when dispatching an outbound ISAPI request or opening an alertStream connection.
- **D-02:** Encryption key is a dedicated `DEVICE_CREDS_KEY` env var (32 bytes, base64 in `.env`). Isolated from `JWT_SECRET` so compromising one does not auto-compromise the other. The installer generates it.
- **D-03:** Device password is never returned in any API response — not raw, not masked. Admin changes the password via PATCH only. Response schema has no password field at all.
- **D-04:** Key rotation tooling is deferred. Phase 2 ships encryption + decryption only. A future `cronometrix rotate-device-key` CLI re-encrypts all device rows with a new key; tracked in Deferred Ideas.

### Dedup & Unknown Faces
- **D-05:** Dedup key is `(employee_id, device_id, direction, 30s-bucket)` where bucket = `floor(epoch_seconds / 30)`. Events on different devices within 30s are BOTH persisted — Phase 3's first-entry/last-exit rule picks the canonical event. Preserves the multi-device audit trail.
- **D-06:** Dedup is enforced by a composite UNIQUE index plus `INSERT OR IGNORE`. DB-level invariant — zero race conditions across concurrent tokio device tasks. No app-level pre-insert SELECT check.
- **D-07:** Unknown face handling: persist the event with `employee_id = NULL`, `is_unknown = 1`, and the captured `face_id` string. Gives forensic coverage (e.g., "who walked in at 03:00") and lets operators reconcile later. Cronometrix does not participate in real-time access decisions — the device firmware owns those.
- **D-08:** face_id → employee mapping lives in a dedicated `device_face_mappings` table keyed on `(device_id, face_id) → employee_id`. Supports distinct face_ids per device (Hikvision ID assignment varies) without schema rework. Phase 7 populates this table; Phase 2 defines it and reads from it.

### ISAPI Command Dispatch
- **D-09:** Command dispatch is synchronous with a 10-second HTTP timeout. `POST /api/v1/devices/:id/commands` blocks until device responds or times out. Returns 200 with the device's result on success, 504 on timeout. Matches typical ISAPI latency with generous headroom and gives operators immediate feedback.
- **D-10:** Single route, command in body: `POST /api/v1/devices/:id/commands { "command": "door_open" | "reboot" | "enrollment_mode" }`. Extensible — new commands add enum values, not routes. Admin-only.
- **D-11:** Every ISAPI command dispatch is written to a dedicated `command_audit_log` table (who, device_id, command, result or error, dispatched_at, completed_at). Matches the project's audit-everything posture and provides legal forensics for door-open events.

### Event Payload Retention
- **D-12:** The full raw alertStream XML block is persisted in `attendance_events.raw_xml`. Allows re-parsing when schema drift surfaces across device models (DS-K1T341, DS-K1T342, future firmware) and forensic re-processing if dedup logic changes. Storage cost (~2–5 KB/event) is trivial at the expected event volume.
- **D-13:** JPEG face captures are stored on the filesystem under `./data/events/YYYY-MM-DD/{event_id}.jpg`. `attendance_events.photo_path` holds the relative path. Keeps SQLite and Turso sync small. Works cleanly with Docker volume mounts. Backup story: copy the directory.
- **D-14:** No event retention / purge policy in Phase 2 — events are legal audit records, so indefinite retention is the safe default. A future phase can add configurable per-client retention via global rules if disk utilization becomes an issue.
- **D-15:** Phase 2 exposes a read API: `GET /api/v1/events?limit&offset&employee_id&device_id&from&to` with the same pagination, error, and RBAC conventions as Phase 1. All three roles can read (Viewer read-only everything per Phase 1 D-09).

### Claude's Discretion
- Exact `attendance_events` and `devices` schema column set — follow Phase 1 conventions (UUID v4 PKs, UTC epoch INTEGER timestamps, `version` column on mutable tables, `status/deleted_at` for soft-delete on devices, audit triggers).
- alertStream tokio task topology and supervisor pattern: one task per enabled device, reconnect loop with exponential backoff + jitter, graceful shutdown on device disable/edit. Exact backoff constants and watchdog thresholds left to research + planner.
- Online/offline status detection mechanism (TCP state vs heartbeat vs last-event timestamp) — planner decides based on alertStream semantics found in research.
- `quick-xml` multipart parser implementation and `diqwest` digest-auth integration details.
- Handler ↔ service ↔ db layering within each new module (`devices/`, `events/`), following the Phase 1 pattern.
- Exact `command_audit_log` schema and whether command audit writes are via SQLite trigger or application code (triggers only fire on table mutations, so app-code insert is likely the pragmatic choice).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/REQUIREMENTS.md` — DEV-01..04, EVT-01..04 are Phase 2 scope; all other IDs are out of scope
- `.planning/PROJECT.md` — constraints (on-prem, Hikvision-only, audit-everything), key decisions table
- `.planning/STATE.md` — accumulated decisions AND the Phase 2 blocker: *"Hikvision ISAPI XML schema varies by device model (DS-K1T341, DS-K1T342) — capture real alertStream traffic before implementation; do not rely on documentation alone"*
- `.planning/phases/01-foundation/01-CONTEXT.md` — carry-forward conventions (UUID PKs, UTC epoch, audit triggers, version column, error envelope, offset pagination, 3-role RBAC, /api/v1 prefix)

### Stack reference
- `CLAUDE.md` — locked stack: `reqwest` 0.13, `diqwest` for digest auth, `quick-xml` 0.39 for multipart XML, `tokio` 1.51, `libsql` 0.9.x. Also ISAPI integration patterns section and alertStream inbound spec reference.
- `backend/src/main.rs` — current router layout; new `devices` and `events` route groups will nest under `/api/v1` with existing `require_auth` / `require_admin` / `require_supervisor_or_above` middleware.
- `backend/src/errors.rs`, `backend/src/common.rs` — reuse `AppError` variants and `PaginatedResponse<T>` for the events list endpoint.
- `backend/src/db/migrations/` — new migrations `003_*.sql` onward follow the existing naming and idempotency conventions.

### External docs (to capture during research)
- Hikvision ISAPI Event Listening spec (referenced in CLAUDE.md sources) — multipart XML format, digest-auth challenge flow, `EventNotificationAlert` block structure.
- Real alertStream traffic samples from the target device models — BLOCKER per STATE.md; research phase must obtain these before planning locks event schema.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppState { db: Arc<Database>, config: Arc<Config> }` — extend `Config` with `device_creds_key` field; no change to AppState shape.
- `AppError` enum + `IntoResponse` impl — reuse variants (`NotFound`, `Unauthorized`, `Forbidden`, `Conflict`, `Validation`, `Internal`); add a new `Timeout` variant for 504 on ISAPI timeout.
- `PaginatedResponse<T>` in `common.rs` — directly usable for `GET /events` and `GET /devices` list endpoints.
- `epoch_to_iso()` helper in `common.rs` — use for all event timestamps in API responses.
- RBAC middleware `require_auth`, `require_supervisor_or_above`, `require_admin` in `auth/middleware.rs` + `auth/rbac.rs` — compose device routes into admin-only, event read routes into viewer-or-above.
- Validator-derive pattern on request DTOs — reuse for `CreateDeviceRequest`, `UpdateDeviceRequest`, `DispatchCommandRequest`.

### Established Patterns
- Module layout: `{domain}/{mod.rs, models.rs, service.rs, handlers.rs}` — new modules `devices/` and `events/` follow this.
- Audit via SQLite triggers in `002_audit_triggers.sql` — extend triggers to cover `devices` and `device_face_mappings`. `attendance_events` is append-only so no triggers needed there. `command_audit_log` is its own audit table and needs no meta-audit.
- Version column + optimistic concurrency on mutable tables — applies to `devices` and `device_face_mappings`; PATCH requires `version`.
- Soft delete via `status` + `deleted_at` — `devices` follows this pattern. Disabling a device sets status=inactive; the alertStream supervisor detects and shuts the task down.
- `/api/v1` router composition in `main.rs` — add `devices_routes` (admin for mutations + commands, viewer for reads) and `events_routes` (viewer for reads) and merge them in.

### Integration Points
- `main.rs` bootstrap — after `init_db`, spawn the alertStream supervisor task that owns the per-device task set. Pass it a handle so the devices handler can signal start/stop/restart on CRUD operations.
- `Config::from_env()` — add `device_creds_key` (required, validated as 32 bytes decoded from base64).
- Migration runner already picks up new `00X_*.sql` files in `backend/src/db/migrations/`.

</code_context>

<specifics>
## Specific Ideas

- The alertStream schema risk flagged in STATE.md is the load-bearing research task. Research phase MUST obtain real multipart XML samples from the target device model(s) before the planner commits to an event schema — otherwise the parser ships blind to the actual traffic.
- Command dispatch is audited per-invocation; timesheet audit (Phase 4) later cross-references `command_audit_log.dispatched_at` for forensic trails ("Admin opened door X at 02:14").
- Event store (`attendance_events`) is the single source of truth for Phase 3 time calculation. Keep it dumb: raw capture + dedup only. No first-entry/last-exit logic here.
- Filesystem photo storage means `data/events/` is part of the container's persistent volume and the backup strategy — call this out in deployment docs later.

</specifics>

<deferred>
## Deferred Ideas

- **Device credential key rotation CLI** (`cronometrix rotate-device-key`) — re-encrypts all `devices` rows with a new `DEVICE_CREDS_KEY`. Needed eventually but not required for v1 ship.
- **Configurable event retention** — expose retention window via `global_rules` with a nightly purge job for raw_xml and/or photo files. Add once a real client pushes against disk limits.
- **Async/queued command dispatch for long-running ops** (e.g., multi-device enrollment batch) — sync dispatch covers Phase 2 commands; revisit when Phase 7 enrollment scope lands.
- **"View password" admin endpoint** — currently never returned. If operators need to migrate a device to new infra and have lost the original password, add a single-use audited reveal. Not required for v1.

</deferred>

---

*Phase: 02-device-integration*
*Context gathered: 2026-04-19*
