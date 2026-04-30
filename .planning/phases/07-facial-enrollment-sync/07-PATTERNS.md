# Phase 7: Facial Enrollment & Sync — Pattern Map

**Mapped:** 2026-04-27
**Files analyzed:** 26 (backend: 12, frontend: 13, migrations: 1 file with 4 logical units)
**Analogs found:** 24 / 26 (2 files have no direct analog — multipart receive + face-api validation — patterns drawn from RESEARCH.md)

---

## File Classification

### Backend — new module `backend/src/enrollments/`

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `backend/src/enrollments/mod.rs` | module index | n/a | `backend/src/devices/mod.rs` | exact |
| `backend/src/enrollments/models.rs` | DTOs | request-response | `backend/src/devices/models.rs` | exact |
| `backend/src/enrollments/service.rs` | service | CRUD | `backend/src/devices/service.rs` | exact |
| `backend/src/enrollments/handlers.rs` | controller | request-response + multipart | `backend/src/devices/handlers.rs` + `backend/src/leaves/handlers.rs` (multipart fields) | role-match |
| `backend/src/enrollments/pusher.rs` | service (fan-out) | event-driven (JoinSet detached) | `backend/src/recompute/worker.rs` (mpsc) + `backend/src/supervisor/mod.rs` (JoinHandle map) | partial |
| `backend/src/enrollments/image_pipeline.rs` | utility | transform | no analog — new (use RESEARCH Pattern 3) | none |
| `backend/src/enrollments/isapi_face.rs` | utility | request-response | extracted helpers similar to `backend/src/isapi/client.rs` body builders | partial |

### Backend — extensions to existing modules

| File | Action | Role | Data Flow | Closest Analog (in same file) | Match Quality |
|---|---|---|---|---|---|
| `backend/src/isapi/client.rs` | EXTEND | utility | request-response | existing `door_open` / `enrollment_mode` / `send_json` methods | exact |
| `backend/src/employees/service.rs` | EXTEND `deactivate` to publish purge | service | event-driven publish | existing `deactivate()` lines 351-370 + `publish_recompute_for_range` in leaves/handlers.rs | exact |
| `backend/src/devices/handlers.rs` | EXTEND `create_device` to publish backfill | controller | event-driven publish | existing `emit_lifecycle()` helper lines 28-34 | exact |
| `backend/src/state.rs` | EXTEND with `purge_tx`, `backfill_tx` | shared state | n/a | existing `recompute_tx`, `lifecycle_tx` Option pattern lines 41-54 | exact |
| `backend/src/main.rs` | EXTEND bootstrap with workers + routes | entrypoint | startup | existing `RecomputeWorker` spawn lines 105-109, `admin_routes` block lines 225-245 | exact |
| `backend/src/workers/purge.rs` | CREATE | worker | event-driven (mpsc) | `backend/src/recompute/worker.rs` | exact |
| `backend/src/workers/backfill.rs` | CREATE | worker | event-driven (mpsc + Semaphore-capped JoinSet) | `backend/src/recompute/worker.rs` + RESEARCH Pattern 1 (JoinSet) | role-match |

### Backend — migrations

| File | Action | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|---|
| `backend/src/db/migrations/016_enrollments.sql` | CREATE | schema | DDL | `backend/src/db/migrations/003_devices.sql` | exact |
| `backend/src/db/migrations/017_phase7_audit_triggers.sql` | CREATE | DB triggers | DDL | `backend/src/db/migrations/006_devices_audit_triggers.sql` | exact |

### Frontend — new components under `frontend/src/components/enrollment/`

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `frontend/src/components/enrollment/enrollment-modal.tsx` | component (modal) | request-response + polling | `frontend/src/components/devices/command-modal.tsx` | role-match |
| `frontend/src/components/enrollment/kiosk-capture-tab.tsx` | component | polling + state machine | `frontend/src/components/devices/command-modal.tsx` (mutation pending) | partial |
| `frontend/src/components/enrollment/webcam-capture-tab.tsx` | component | streaming (getUserMedia) | no analog — new (use RESEARCH Pattern 5) | none |
| `frontend/src/components/enrollment/upload-capture-tab.tsx` | component | file-I/O | `frontend/src/lib/validations.ts` `evidenceFileSchema` (file shape) | partial |
| `frontend/src/components/enrollment/validation-panel.tsx` | component | event-driven (face-api per-frame) | no analog — new (use RESEARCH Pattern 5) | none |
| `frontend/src/components/enrollment/sync-panel.tsx` | component | polling | `frontend/src/components/devices/device-table.tsx` (StatusBadge + role-gated action) | partial |
| `frontend/src/components/enrollment/sync-row.tsx` | component | request-response (retry mutation) | `frontend/src/components/devices/command-modal.tsx` `useMutation` | exact |
| `frontend/src/components/enrollment/employee-enrollment-picker.tsx` | component | CRUD (read employees) | `frontend/src/app/(dashboard)/employees/page.tsx` employee+department `useQuery` | exact |
| `frontend/src/components/enrollment/in-progress-list.tsx` | component | polling (active enrollments) | `frontend/src/app/(dashboard)/devices/page.tsx` `refetchInterval` pattern | exact |
| `frontend/src/components/common/access-restricted.tsx` | component (shared) | n/a | `frontend/src/components/employees/employee-table.tsx` `role === 'admin'` inline pattern | partial |

### Frontend — page + plumbing

| File | Action | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|---|
| `frontend/src/app/(dashboard)/enrollment/page.tsx` | REPLACE | route page | composed | `frontend/src/app/(dashboard)/devices/page.tsx` | exact |
| `frontend/src/app/(dashboard)/employees/page.tsx` | EXTEND (add row action) | route page | n/a | already wires modal trigger pattern via `selectedDevice` state | role-match |
| `frontend/src/components/employees/employee-table.tsx` | EXTEND (add `Enrolar Rostro` action) | component | n/a | existing actions cell lines 58-81 | exact |
| `frontend/src/lib/validations.ts` | EXTEND with `enrollmentSubmitSchema` | utility | n/a | existing `novedadSchema` + `evidenceFileSchema` | exact |
| `frontend/src/types/api.ts` | EXTEND with `Enrollment`, `EnrollmentDevicePush` | types | n/a | existing `Device`, `Leave` shapes | exact |
| `frontend/src/proxy.ts` | EXTEND `PROTECTED_PATHS` (already includes `/enrollment`) — verify, no change needed | config | n/a | existing matcher line 47 already lists `/enrollment` | exact |
| `frontend/src/lib/face-detection.ts` | CREATE | utility | event-driven | no analog — new (RESEARCH § Code Examples — webcam + face-api) | none |
| `frontend/public/models/tiny_face_detector_*` | VENDOR | static asset | n/a | no analog — new (RESEARCH § NEW dependencies) | none |

---

## Pattern Assignments

### `backend/src/enrollments/mod.rs` (module index)

**Analog:** `backend/src/devices/mod.rs` (verbatim 4-line shape)

**Imports / re-exports** (lines 1-4 of devices/mod.rs):
```rust
pub mod crypto;
pub mod handlers;
pub mod models;
pub mod service;
```

**For Phase 7 enrollments/mod.rs:** drop `crypto` (no enrollment-specific crypto — reuses `devices::crypto`), add `image_pipeline`, `isapi_face`, `pusher`:
```rust
pub mod handlers;
pub mod image_pipeline;
pub mod isapi_face;
pub mod models;
pub mod pusher;
pub mod service;
```

---

### `backend/src/enrollments/models.rs` (DTOs)

**Analog:** `backend/src/devices/models.rs` (lines 1-89, 130-167)

**Imports pattern** (devices/models.rs lines 1-2):
```rust
use serde::{Deserialize, Serialize};
use validator::Validate;
```

**Response struct convention** (devices/models.rs lines 9-27 — `DeviceResponse`):
```rust
#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    // ...
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}
```
**Apply to:** `EnrollmentResponse`, `FaceEnrollmentResponse`, `EnrollmentDevicePushResponse`. ISO-8601 strings for timestamps (Phase 1 D-13 carry-forward via `epoch_to_iso`); `version` only on mutable rows (`enrollments` has it; `enrollment_device_pushes` does not — cf. CONTEXT schema additions).

**Request validator pattern** (devices/models.rs lines 30-48 — `CreateDeviceRequest`):
```rust
#[derive(Debug, Deserialize, Validate)]
pub struct CreateDeviceRequest {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 chars"))]
    pub name: String,
    // ...
}
```
**Apply to:** Multipart fields are NOT decoded as a single Validate struct (multipart bodies are consumed field-by-field — see leaves/handlers pattern below). Instead, after assembling the fields, build a `CreateEnrollmentRequest` struct and validate in service. Use `#[validate(custom = ...)]` for `captured_via ∈ {device,webcam,upload}` and UUID checks for `employee_id` / `source_device_id`.

**Plaintext-safe internal struct** (devices/models.rs lines 141-167 — `DeviceWithPlaintext` — reused, not redeclared):
```rust
pub struct DeviceWithPlaintext {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub username: String,
    pub password: String, // plaintext — short-lived on the stack
    // ...
}

impl std::fmt::Debug for DeviceWithPlaintext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceWithPlaintext")
            // ...
            .field("password", &"[redacted]")
            // ...
    }
}
```
**Apply to:** every `pusher::push_one_device` call site loads a `DeviceWithPlaintext` via `devices::service::get_decrypted` — DO NOT introduce a new plaintext struct in enrollments/models.rs.

**Helpers (validate_*)** (devices/models.rs lines 170-198):
```rust
pub fn validate_status(s: &str) -> Result<(), &'static str> {
    match s {
        "active" | "inactive" => Ok(()),
        _ => Err("status must be 'active' or 'inactive'"),
    }
}
```
**Apply to:** `validate_captured_via`, `validate_enrollment_status`, `validate_push_status` — same shape, same error-string-only contract.

---

### `backend/src/enrollments/service.rs` (service, CRUD)

**Analog:** `backend/src/devices/service.rs` (entire file — same module shape)

**Imports pattern** (devices/service.rs lines 1-12):
```rust
use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;

use super::crypto;
use super::models::{
    validate_direction, validate_ip, validate_scheme, validate_status, Command,
    CreateDeviceRequest, DeviceListQuery, DeviceResponse, DeviceWithPlaintext,
    UpdateDeviceRequest,
};
```

**Row-mapper pattern** (devices/service.rs lines 37-56 — `row_to_device`):
```rust
fn row_to_device(row: libsql::Row) -> Result<DeviceResponse, AppError> {
    let allow_int: i64 = row.get(7).map_err(|e| AppError::Internal(e.into()))?;
    Ok(DeviceResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        // ...
        last_seen_at: epoch_to_iso_opt(row.get(9).map_err(|e| AppError::Internal(e.into()))?),
        // ...
        created_at: epoch_to_iso(row.get(13).map_err(|e| AppError::Internal(e.into()))?),
    })
}

const DEVICE_SELECT_COLS: &str =
    "id, name, ip, port, scheme, username, direction, allow_insecure_tls, \
     connection_state, last_seen_at, status, deleted_at, version, created_at, updated_at";
```
**Apply to:** `row_to_enrollment`, `row_to_face_enrollment`, `row_to_push` — define `ENROLLMENT_SELECT_COLS`, `FACE_ENROLLMENT_SELECT_COLS`, `PUSH_SELECT_COLS` mirroring the format.

**Insert with UUID PK + unixepoch()** (devices/service.rs lines 87-110):
```rust
let id = Uuid::new_v4().to_string();
// ...
conn.execute(
    "INSERT INTO devices (\
         id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, \
         created_at, updated_at\
     ) VALUES (\
         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'offline', 'active', 1, unixepoch(), unixepoch()\
     )",
    params![id.clone(), req.name.clone(), /* ... */],
)
.await
```
**Apply to:** persistence of `enrollments`, `face_enrollments`, `enrollment_device_pushes` rows. Use `unixepoch()` for `started_at` / `created_at`.

**Conflict mapping** (devices/service.rs lines 113-130) — translate UNIQUE-index errors to `AppError::Conflict`:
```rust
match result {
    Err(e) => {
        let msg = e.to_string();
        if msg.contains("UNIQUE constraint failed") && msg.contains("idx_devices_ip_port_active") {
            return Err(AppError::Conflict {
                code: "DEVICE_IP_EXISTS",
                message: format!("Device with IP {}:{} is already active", req.ip, req.port),
            });
        }
        return Err(AppError::Internal(e.into()));
    }
    Ok(_) => {}
}
```
**Apply to:** `face_id` UNIQUE on `employees` (D-10) — `EMPLOYEE_FACE_ID_EXISTS` if a duplicate face_id ever surfaces (defensive — `write_face_id_if_missing` should make this unreachable). Use the same string-match-on-error-message technique.

**`get_decrypted` reuse** (devices/service.rs lines 385-433):
```rust
pub async fn get_decrypted(
    conn: &Connection,
    id: &str,
    key: &[u8; 32],
) -> Result<DeviceWithPlaintext, AppError> {
    let row = conn
        .query(
            "SELECT id, name, ip, port, scheme, username, encrypted_password, \
                    direction, allow_insecure_tls, status, version \
             FROM devices WHERE id = ?1 AND status = 'active'",
            params![id.to_string()],
        )
        // ...
    let password =
        crypto::decrypt_password(&encrypted, key).map_err(|e| AppError::Internal(e.into()))?;
    Ok(DeviceWithPlaintext { /* ... */ })
}
```
**Apply to:** every push task in `pusher.rs` loads its target device via `crate::devices::service::get_decrypted(&conn, &device_id, &state.config.device_creds_key)`. Drop the plaintext as soon as the ISAPI calls complete.

**`list_active` for fan-out** (devices/service.rs lines 444-495) — bulk decrypt for all devices, skipping rows that fail to decrypt:
```rust
match crypto::decrypt_password(&encrypted, key) {
    Ok(password) => out.push(DeviceWithPlaintext { /* ... */ }),
    Err(e) => {
        tracing::error!(device_id = %device_id, err = %e,
            "failed to decrypt device password during list_active — skipping");
    }
}
```
**Apply to:** D-06 fan-out kickoff — call `devices::service::list_active(&conn, key)` to get every active device once, then spawn one task per row. Identical skip-on-decrypt-fail policy.

---

### `backend/src/enrollments/handlers.rs` (controller, request-response + multipart)

**Analog (multipart shape):** `backend/src/leaves/handlers.rs` lines 51-194 — drains fields, validates each, writes file, persists row.
**Analog (command dispatch + audit):** `backend/src/devices/handlers.rs` lines 218-299 — `dispatch_command` is the closest analog for the kiosk capture endpoint (timeout-wrapped ISAPI + audit).

**Imports pattern** (devices/handlers.rs lines 1-23):
```rust
use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use libsql::params;
use tokio::time::timeout;
use validator::Validate;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;
```
**Apply to:** add `use axum::extract::Multipart;` (already enabled in Cargo) and `use crate::devices::service as devices_service;`.

**Multipart field-drain pattern** (leaves/handlers.rs lines 65-137):
```rust
while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation {
    code: "VALIDATION_ERROR",
    message: format!("malformed multipart: {}", e),
})? {
    let name = field.name().unwrap_or("").to_string();
    match name.as_str() {
        "employee_id" => {
            employee_id = Some(field.text().await.map_err(|e| AppError::Validation {
                code: "VALIDATION_ERROR",
                message: e.to_string(),
            })?);
        }
        "evidence" => {
            let ct = field.content_type().unwrap_or("").to_string();
            evidence_ext = match ct.as_str() {
                "application/pdf" => Some("pdf"),
                "image/jpeg" => Some("jpg"),
                _ => return Err(AppError::Validation { /* ... */ }),
            };
            let bytes = field.bytes().await.map_err(|e| AppError::Validation { /* ... */ })?;
            if bytes.len() > MAX_EVIDENCE_BYTES {
                return Err(AppError::Validation { /* ... */ });
            }
            evidence_bytes = Some(bytes.to_vec());
        }
        _ => { let _ = field.bytes().await; }  // discard unknown fields
    }
}
```
**Apply to:** `POST /api/v1/enrollments` multipart parser. Drain fields into `Option<String> / Option<Vec<u8>>`, then assemble + validate. Add a magic-byte check for the `photo` field (RESEARCH Pattern Pitfall 2 — `bytes[0..3] == [0xFF, 0xD8, 0xFF]`) BEFORE accepting the bytes. Cap at `2 * 1024 * 1024` (CONTEXT D-04).

**Server-generated path + atomic write** (leaves/handlers.rs lines 160-172):
```rust
let evidence_relpath = if let (Some(bytes), Some(ext)) = (evidence_bytes.as_ref(), evidence_ext) {
    let rel = format!("{}.{}", Uuid::new_v4(), ext);
    write_photo_atomic(&service::leaves_root(), &rel, bytes)
        .map_err(AppError::Internal)?;
    Some(rel)
} else { None };
```
**Apply to:** photo path = `format!("{}/{}.jpg", employee_id, enrollment_id)` (CONTEXT D-11). Reuse `crate::events::service::write_photo_atomic` (already public — see leaves/handlers.rs line 33: `use crate::events::service::write_photo_atomic;`). NEVER use the user-supplied filename.

**Atomic write helper** (events/service.rs lines 156-172):
```rust
pub fn write_photo_atomic(root: &Path, relpath: &str, bytes: &[u8]) -> anyhow::Result<()> {
    use std::fs::{self, File};
    use std::io::Write;

    let full = root.join(relpath);
    if let Some(parent) = full.parent() { fs::create_dir_all(parent)?; }
    let tmp = full.with_extension("jpg.tmp");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &full)?;
    Ok(())
}
```
**Apply to:** Phase 7 enrollment storage. Add `pub fn enrollments_root() -> PathBuf` in `enrollments::service` (mirrors `events::service::events_root` and `leaves::service::leaves_root`). Photos persisted as `enrollments_root().join(format!("{}/{}.jpg", employee_id, enrollment_id))`.

**ISAPI dispatch + timeout + audit triple** (devices/handlers.rs lines 249-281):
```rust
let dispatched_at = chrono::Utc::now().timestamp();
let result = match command {
    Command::DoorOpen => timeout(Duration::from_secs(10), isapi.door_open()).await,
    // ...
};
let completed_at = chrono::Utc::now().timestamp();
let audit_outcome = match &result {
    Ok(Ok(body)) => CommandAuditOutcome::Ok(body.clone()),
    Ok(Err(e)) => CommandAuditOutcome::Error { code: "DEVICE_ERROR", message: e.to_string() },
    Err(_) => CommandAuditOutcome::Timeout,
};
service::write_command_audit(&conn, &claims.sub, &device_id, command, &audit_outcome,
    dispatched_at, completed_at).await?;
```
**Apply to:** every per-device push task (`pusher::push_one_device`) AND the kiosk capture handler. Each task wraps both ISAPI calls (`upsert_user` + `upload_face`) inside a single `tokio::time::timeout(Duration::from_secs(30), ...)`. The audit row is `enrollment_device_pushes` (NOT `command_audit_log`) — but every branch (Ok / Err / Timeout) writes a row, exactly like dispatch_command does.

**Error mapping** (devices/handlers.rs lines 283-298):
```rust
match result {
    Ok(Ok(text)) => Ok(Json(CommandResult { /* ... */ })),
    Ok(Err(e)) => Err(AppError::BadGateway { code: "DEVICE_ERROR", message: e.to_string() }),
    Err(_) => Err(AppError::Timeout {
        code: "DEVICE_TIMEOUT",
        message: "Device did not respond within 10 seconds".to_string(),
    }),
}
```
**Apply to:** kiosk capture handler returns these same shapes. The fan-out POST `/enrollments` does NOT return BadGateway — it returns 202 immediately and per-device errors land in `enrollment_device_pushes.error_message`.

**Response code** (devices/handlers.rs lines 76-91):
```rust
pub async fn create_device(
    State(state): State<AppState>,
    Json(body): Json<CreateDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), AppError> {
    body.validate().map_err(|e| AppError::Validation { /* ... */ })?;
    // ...
    Ok((StatusCode::CREATED, Json(device)))
}
```
**Apply to:** `POST /api/v1/enrollments` returns `(StatusCode::ACCEPTED, Json(EnrollmentSubmitResponse))` — 202 because tasks run async (RESEARCH Pattern 1).

---

### `backend/src/enrollments/pusher.rs` (service, JoinSet fan-out — detached)

**Analog (channel + worker shape):** `backend/src/recompute/worker.rs` (lines 1-69) — mpsc + biased select + drain.
**Analog (handle map + spawn):** `backend/src/supervisor/mod.rs` (lines 47-99) — long-lived task that owns child JoinHandles.

**Detached driver pattern (RESEARCH Pattern 1, verified in supervisor.rs spawn pattern):**
```rust
use tokio::task::JoinSet;
use std::sync::Arc;

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
            // ... clone Arcs ...
            set.spawn(async move {
                push_one_device(state, enrollment_id, face_id, photo_bytes, employee_id, device).await
            });
        }
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(err = %e, "push task returned error"),
                Err(e) => tracing::error!(err = %e, "push task panicked"),
            }
        }
        if let Err(e) = finalize_enrollment_status(&state, &enrollment_id).await {
            tracing::error!(err = %e, "failed to finalize enrollment status");
        }
    });
}
```
**Apply to:** D-06 enrollment fan-out (no Semaphore — small device fleet). The handler returns 202 BEFORE this task starts running; the task survives the request lifecycle (D-09 modal-close-doesn't-cancel).

**Inside `push_one_device`** — composes `devices::service::get_decrypted` + new `DeviceConnection::upsert_user` + `DeviceConnection::upload_face` (RESEARCH Pattern 2) + UPDATE `enrollment_device_pushes` row + INSERT OR REPLACE `device_face_mappings` on success. Audit-trail row is the row mutation itself (D-17 trigger picks it up).

---

### `backend/src/enrollments/image_pipeline.rs` (utility, transform)

**Analog:** none in repo — first server-side image processing path. Use RESEARCH Pattern 3 (lines 540-585) verbatim, with one project-specific addition:

**CPU-bound wrapper** — wrap the decode/resize/encode call in `tokio::task::spawn_blocking` from the handler (RESEARCH § Anti-Patterns: "Synchronous decode-resize on the request thread"):
```rust
let normalized = tokio::task::spawn_blocking(move || normalize_face_jpeg(&photo_bytes))
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking task panicked: {e}")))?
    .map_err(|e| AppError::Validation {
        code: "PHOTO_INVALID", message: e.to_string(),
    })?;
```
This pattern matches the project's existing convention of using `tokio::task::spawn_blocking` for CPU-bound work — search results show no other use yet, but this is the canonical Tokio idiom and is explicitly called out in CLAUDE.md by virtue of the locked Tokio runtime.

---

### `backend/src/enrollments/isapi_face.rs` + `backend/src/isapi/client.rs` extension

**Analog:** `backend/src/isapi/client.rs` (entire file, lines 42-142) — the `DeviceConnection` struct + helper methods.

**Existing send_json helper** (isapi/client.rs lines 120-141):
```rust
async fn send_json(
    &self,
    url: &str,
    method: reqwest::Method,
    body: &str,
) -> Result<String> {
    let resp = self
        .client
        .request(method, url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send_digest_auth((self.username.as_str(), self.password.as_str()))
        .await
        .context("ISAPI request failed")?;
    let status = resp.status();
    let text = resp.text().await.context("read ISAPI response body")?;
    anyhow::ensure!(status.is_success(), "device returned non-success status {status}: {text}");
    Ok(text)
}
```
**Apply to:** `upsert_user` (Step 1, JSON body) reuses `send_json` directly.

**Existing handler shape for new methods** (isapi/client.rs lines 89-95 — `enrollment_mode`):
```rust
pub async fn enrollment_mode(&self) -> Result<String> {
    let url = format!("{}/ISAPI/AccessControl/CaptureFaceData", self.base_url);
    let body = r#"{"CaptureInfo":{"captureInfrared":true}}"#;
    self.send_json(&url, reqwest::Method::POST, body).await
}
```
**Apply to:** add `upsert_user(&self, face_id: &str, full_name: &str)`, `upload_face(&self, face_id: &str, jpeg_bytes: Vec<u8>)`, `delete_user(&self, face_id: &str)`, `capture_face_image(&self, face_id: &str)` directly on `impl DeviceConnection`. RESEARCH Pattern 2 (lines 421-518) gives the exact JSON bodies and multipart-form structure. The `upload_face` method is the only new one that bypasses `send_json` (because of the multipart) — see RESEARCH lines 466-498 for the literal `reqwest::multipart::Form::new()` builder calls.

**TLS + Debug redaction** (isapi/client.rs lines 32-57 — already in place; preserve exactly when extending).

---

### `backend/src/workers/purge.rs` (worker, mpsc-driven)

**Analog:** `backend/src/recompute/worker.rs` (entire file)

**mpsc + cancellation token loop** (recompute/worker.rs lines 30-69):
```rust
pub async fn run(self, mut rx: mpsc::UnboundedReceiver<RecomputeRequest>) {
    let debounce = tokio::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            biased;
            _ = self.shutdown.cancelled() => {
                tracing::info!("recompute worker shutdown");
                break;
            }
            maybe_req = rx.recv() => {
                let Some(req) = maybe_req else {
                    tracing::info!("recompute channel closed, worker exiting");
                    break;
                };
                let mut pending: HashSet<(String, NaiveDate)> = HashSet::new();
                pending.insert((req.employee_id, req.anchor_date));
                while let Ok(extra) = rx.try_recv() {
                    pending.insert((extra.employee_id, extra.anchor_date));
                }
                tokio::time::sleep(debounce).await;
                while let Ok(extra) = rx.try_recv() {
                    pending.insert((extra.employee_id, extra.anchor_date));
                }
                for (emp_id, date) in pending.drain() {
                    if let Err(e) = dr_service::recompute_for_day(&self.state, &emp_id, date).await {
                        tracing::warn!(employee_id = %emp_id, anchor_date = %date,
                            err = %e, "recompute failed");
                    }
                }
            }
        }
    }
}
```
**Apply to:** Purge worker request type = `EmployeeId(String)`. Dedup by employee_id (HashSet of String). Inside the for-loop: SELECT every `device_face_mappings` row for the employee, attempt ISAPI `delete_user`, `INSERT OR REPLACE` mapping state to `pending_delete` on per-device failure or DELETE the row on success. **Add the Pitfall 10 guard** — re-read `employees.status` per row inside the loop; if `'active'` again, abort and clear `pending_delete`.

---

### `backend/src/workers/backfill.rs` (worker, mpsc + Semaphore-capped JoinSet)

**Analog (channel/worker scaffold):** `backend/src/recompute/worker.rs`
**Analog (Semaphore concurrency cap):** none in repo — use RESEARCH Pitfall 5 (lines 754-765):
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
**Apply to:** D-16 backfill — request type = `DeviceId(String)`. On receipt: `SELECT id FROM employees WHERE face_id IS NOT NULL AND status='active'` → fan out one push task per employee with `Semaphore::new(4)`. Each task reads the photo from `./data/enrollments/{emp_id}/{current_face_enrollment_id}.jpg` and calls the same `push_one_device` helper as `pusher.rs` (extract that into `pusher` so backfill imports it).

---

### `backend/src/state.rs` (shared state — extend)

**Analog:** existing `recompute_tx` Option pattern (state.rs lines 41-54)

**Existing pattern** (state.rs lines 42-54):
```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    pub lifecycle_tx: Option<LifecycleTx>,
    pub recompute_tx: Option<UnboundedSender<RecomputeRequest>>,
    pub event_broadcast: Option<broadcast::Sender<AttendanceEventSSEPayload>>,
    pub license_valid: Arc<std::sync::atomic::AtomicBool>,
}
```
**Apply to:** add `pub purge_tx: Option<UnboundedSender<PurgeRequest>>,` and `pub backfill_tx: Option<UnboundedSender<BackfillRequest>>,`. Both Option for the same reason (tests build AppState without workers — silently skip publishes).

---

### `backend/src/main.rs` (bootstrap — extend)

**Analog (worker spawn):** main.rs lines 105-119 (RecomputeWorker + nightly):
```rust
// Start the Phase 3 recompute worker (mpsc + 500ms debounce + HashSet dedup).
let recompute_worker = recompute::worker::RecomputeWorker::new(state.clone(), shutdown.clone());
let recompute_handle = tokio::spawn(async move {
    recompute_worker.run(recompute_rx).await;
});
```
**Apply to:** spawn `PurgeWorker::new(state.clone(), shutdown.clone()).run(purge_rx)` and `BackfillWorker::new(state.clone(), shutdown.clone()).run(backfill_rx)` in the same block. Await both handles on shutdown alongside `recompute_handle`.

**Analog (channel construction):** main.rs lines 49-62:
```rust
let (recompute_tx, recompute_rx) = mpsc::unbounded_channel::<RecomputeRequest>();
```
**Apply to:** `let (purge_tx, purge_rx) = mpsc::unbounded_channel::<PurgeRequest>();` and `let (backfill_tx, backfill_rx) = mpsc::unbounded_channel::<BackfillRequest>();` — pass the senders into `AppState`.

**Analog (admin route registration):** main.rs lines 225-245 (`admin_routes`):
```rust
let admin_routes = Router::new()
    .route("/employees/{id}", delete(employees::handlers::deactivate_employee))
    // ...
    .route("/devices/{id}/commands", post(devices::handlers::dispatch_command))
    // ...
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth::rbac::require_admin))
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), license::middleware::require_license));
```
**Apply to:** add Phase 7 routes inside `admin_routes`:
```rust
.route("/enrollments", post(enrollments::handlers::create_enrollment))
.route("/enrollments/{id}", get(enrollments::handlers::get_enrollment))
.route("/enrollments/{id}/devices/{device_id}/retry", post(enrollments::handlers::retry_push))
.route("/enrollments/capture-from-device", post(enrollments::handlers::capture_from_device))
.route("/enrollments/captures/{capture_id}", get(enrollments::handlers::get_capture))
```
All gated by `require_admin` (D-18) automatically since they're inside `admin_routes`. **Pitfall 8 (Axum body limit):** apply `tower_http::limit::RequestBodyLimitLayer::new(3 * 1024 * 1024)` specifically to the `POST /enrollments` route via `.route_layer` on a sub-router so the 2 MB upload cap works cleanly.

---

### `backend/src/employees/service.rs` (extend `deactivate`)

**Analog:** `backend/src/leaves/handlers.rs` lines 332-361 — `publish_recompute_for_range` (publish-to-channel-on-mutation pattern):
```rust
fn publish_recompute_for_range(state: &AppState, employee_id: &str, from_date: &str, to_date: &str) {
    let Some(tx) = state.recompute_tx.as_ref() else { return };
    // ... iterate dates, send on tx ...
    if let Err(e) = tx.send(RecomputeRequest { /* ... */ }) {
        tracing::warn!(err = %e, "recompute_tx send failed (worker down?)");
        return;
    }
}
```
**Apply to:** in employees/handlers.rs `deactivate_employee` (NOT in `service.rs`, because the publish needs `AppState`), after the successful `service::deactivate(...)` call, publish a `PurgeRequest` to `state.purge_tx`. Same Option-skip-if-None pattern.

---

### `backend/src/devices/handlers.rs` (extend `create_device`)

**Analog:** `emit_lifecycle` helper already in this file (handlers.rs lines 28-34):
```rust
fn emit_lifecycle(state: &AppState, ev: DeviceLifecycleEvent) {
    if let Some(tx) = state.lifecycle_tx.as_ref() {
        if let Err(e) = tx.send(ev.clone()) {
            tracing::warn!(err = %e, event = ?ev, "failed to emit lifecycle event");
        }
    }
}

// Used in create_device:
emit_lifecycle(&state, DeviceLifecycleEvent::Start(device.id.clone()));
```
**Apply to:** add a sibling `emit_backfill(&state, device_id)` helper and call it after `emit_lifecycle(&state, DeviceLifecycleEvent::Start(...))` so the alertStream supervisor and the backfill worker both fire on a new device.

---

### `backend/src/db/migrations/016_enrollments.sql`

**Analog:** `backend/src/db/migrations/003_devices.sql` (entire file — table + partial unique index + indexes pattern)

**Table-creation convention** (003_devices.sql lines 5-22):
```sql
CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- ...
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```
**Apply to:** Phase 7 tables. Use the literal SQL from RESEARCH § Code Examples → Schema migration (RESEARCH lines 1010-1058). Idempotent `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS`.

**Adding columns to existing tables** (no perfect analog — most prior migrations create new tables; closest is migration 015):
**Analog:** `backend/src/db/migrations/015_employees_position_hire_date.sql` (319 bytes — exactly the pattern Phase 7 needs for the `employees.face_id` and `employees.current_face_enrollment_id` additions and the `device_face_mappings.state` addition). Read first for shape; the SQL is identical-style `ALTER TABLE ... ADD COLUMN ...` with CHECK constraints.

---

### `backend/src/db/migrations/017_phase7_audit_triggers.sql`

**Analog:** `backend/src/db/migrations/006_devices_audit_triggers.sql` (entire 59 lines — same target table count: 3-trigger-set per table)

**INSERT trigger** (006_devices_audit_triggers.sql lines 12-26):
```sql
CREATE TRIGGER IF NOT EXISTS audit_devices_insert
    AFTER INSERT ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'name', NEW.name, /* ... NO encrypted_password ... */),
        NULL,
        unixepoch()
    );
END;
```
**Apply to:** generate three triggers for each of `enrollments`, `face_enrollments`, `device_face_mappings` (D-17). The UUID-v4 expression and NULL-on-DELETE-new_data shape are verbatim. **Sensitive-column omission rule** (006 line 4-7 comment): exclude `face_quality_score` JSON blob if it grows large; include `id`, `employee_id`, `captured_via`, `source_device_id`, `photo_path`, `created_at`, `created_by` for `face_enrollments`. For `device_face_mappings` include the new `state` column and existing `device_id`, `face_id`, `employee_id`, `version`.

---

### `frontend/src/components/enrollment/enrollment-modal.tsx` (component, modal)

**Analog:** `frontend/src/components/devices/command-modal.tsx` (entire 80 lines)

**Imports + 'use client'** (command-modal.tsx lines 1-8):
```tsx
'use client'
import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import type { Device } from '@/types/api'
```
**Apply to:** add imports for `Tabs, TabsList, TabsTrigger, TabsContent` (new, via shadcn add), `Progress` (new), and `useQuery` for polling. Keep the `'use client'` directive — required for hooks.

**Mutation pattern** (command-modal.tsx lines 27-38):
```tsx
const mutation = useMutation({
  mutationFn: () => api.post(`/devices/${device!.id}/commands`, { command: selectedCommand }),
  onSuccess: () => {
    toast.success(`Comando "${COMMANDS.find(c => c.value === selectedCommand)?.label}" enviado`)
    onClose()
  },
  onError: (err: unknown) => {
    const message = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? 'Error al enviar comando'
    toast.error(message)
  },
})
```
**Apply to:** the `submitEnrollment` mutation that POSTs `/api/v1/enrollments` (multipart). On success → start polling (set state to syncing). On error → toast.error in Spanish with backend `message` fallback. **Modal close is non-destructive (D-09):** do NOT call `mutation.reset()` on close, do NOT call `onClose()` automatically on success — the mutation just transitions the modal to syncing and the polling query takes over.

**Polling query pattern (RESEARCH Pattern 4 — verified) + analog `refetchInterval` from devices/page.tsx line 17:**
```tsx
// devices/page.tsx existing:
const { data, isLoading } = useQuery<PaginatedResponse<Device>>({
  queryKey: ['devices'],
  queryFn: () => api.get('/devices').then(r => r.data),
  refetchInterval: 30_000,
})
```
**Apply to:** enrollment modal uses RESEARCH Pattern 4's function form (returns `false` once terminal):
```tsx
const { data: status } = useQuery<Enrollment>({
  queryKey: ['enrollment', enrollmentId],
  queryFn: () => api.get(`/enrollments/${enrollmentId}`).then(r => r.data),
  enabled: !!enrollmentId,
  refetchInterval: (query) => {
    const data = query.state.data as Enrollment | undefined
    if (!data) return 1500
    const allDone = data.device_pushes.every(p => p.status === 'success' || p.status === 'failed')
    return allDone ? false : 1500
  },
})
```

**Dialog shape** (command-modal.tsx lines 40-79):
```tsx
<Dialog open={open} onOpenChange={(o) => { if (!o) onClose() }}>
  <DialogContent className="max-w-sm">
    <DialogHeader>
      <DialogTitle>Enviar Comando ISAPI</DialogTitle>
    </DialogHeader>
    <div className="space-y-4">
      {/* body */}
    </div>
    <DialogFooter className="gap-2">
      <Button variant="outline" onClick={onClose}>Cancelar</Button>
      <Button onClick={() => mutation.mutate()} disabled={mutation.isPending}>
        {mutation.isPending ? 'Enviando…' : 'Enviar Comando'}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
```
**Apply to:** widen `DialogContent` to `max-w-5xl w-[1120px] max-h-[88vh] overflow-y-auto` (UI-SPEC § Screen Layout Contract). Replace single body with `grid grid-cols-[1fr_360px] gap-8` two-column layout. Footer button uses `aria-disabled` (UI-SPEC §Accessibility Contract — Phase 1 D-09 carry-forward) instead of `disabled` to keep focus.

---

### `frontend/src/components/enrollment/sync-row.tsx` (component, mutation per row)

**Analog:** `frontend/src/components/devices/command-modal.tsx` lines 27-38 — `useMutation` with `toast` integration (extract per-row).

**Per-row retry mutation:**
```tsx
const retryMutation = useMutation({
  mutationFn: () => api.post(`/enrollments/${enrollmentId}/devices/${push.device_id}/retry`),
  onError: (err: unknown) => {
    const message = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? 'Error al reintentar'
    toast.error(message)
  },
})
```
Pure copy of the analog with no `onSuccess` toast (the polling query already surfaces the in-progress state).

**StatusBadge + state colors** (devices/device-table.tsx lines 7-19):
```tsx
function StatusBadge({ status }: { status: Device['status'] }) {
  const map = {
    online: 'bg-green-100 text-green-700',
    offline: 'bg-red-100 text-red-700',
    unknown: 'bg-slate-100 text-slate-600',
  }
  const labels = { online: 'En línea', offline: 'Offline', unknown: 'Desconocido' }
  return (
    <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${map[status]}`}>
      {labels[status]}
    </span>
  )
}
```
**Apply to:** SyncRow status pill — same shape, mapping `pending → bg-slate-100 text-slate-600`, `in_progress → bg-slate-100 text-slate-600` (with Loader2 icon), `success → bg-green-100 text-green-700` (CheckCircle2), `failed → bg-red-100 text-red-700` (XCircle). UI-SPEC § Color § State color tokens locks these tokens.

---

### `frontend/src/components/enrollment/employee-enrollment-picker.tsx`

**Analog:** `frontend/src/app/(dashboard)/employees/page.tsx` lines 13-38 (employee + departments query) — the dual-`useQuery` pattern.

```tsx
const { data: employees } = useQuery<PaginatedResponse<Employee>>({
  queryKey: ['employees', /* ... */],
  queryFn: () => api.get('/employees', { params: { /* ... */ } }).then(r => r.data),
})
```
**Apply to:** picker queries `GET /api/v1/employees?status=active&limit=100` (or use `?name=` filter as the user types). Wrap in shadcn `Select` (new component to add via `npx shadcn add select`).

---

### `frontend/src/app/(dashboard)/enrollment/page.tsx` (REPLACE)

**Analog:** `frontend/src/app/(dashboard)/devices/page.tsx` (entire 56-line file)

**Page wrapper structure** (devices/page.tsx lines 23-54):
```tsx
return (
  <div className="flex flex-col h-full">
    <TopBar title="Dispositivos" />
    <div className="p-6 space-y-4">
      <div className="flex items-center justify-between">
        {/* header */}
      </div>
      <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
        {/* table or content */}
      </div>
    </div>
    <CommandModal open={...} device={...} onClose={...} />
  </div>
)
```
**Apply to:** Phase 7 enrollment page — `TopBar title="Enrolamiento Facial"`, header row with `<EmployeeEnrollmentPicker />`, body with `<InProgressEnrollmentList />` (only when ≥1 active) + empty state. RBAC gate: at the top of the function body, `if (role !== 'admin') return <AccessRestrictedPlaceholder />;` (UI-SPEC § Screen Layout Contract).

**Auth gate** — analog: existing inline `{role === 'admin' && (...)}` pattern in `frontend/src/components/employees/employee-table.tsx` lines 63-72 and `frontend/src/app/(dashboard)/employees/page.tsx` lines 78-87:
```tsx
const { role } = useAuth()
// ...
{role === 'admin' && (
  <button>Nuevo Empleado</button>
)}
```
**Apply to:** wrap the `<EmployeeEnrollmentPicker />` CTA + the page itself. The PAGE-level gate is new — extract `<AccessRestrictedPlaceholder />` as a shared component (UI-SPEC § Component Inventory).

---

### `frontend/src/app/(dashboard)/employees/page.tsx` + `frontend/src/components/employees/employee-table.tsx` — EXTEND

**Analog (row action):** employee-table.tsx lines 58-81 — existing `actions` cell uses `role === 'admin'` to gate buttons:
```tsx
{
  id: 'actions',
  header: 'Acciones',
  cell: ({ row }) => (
    <div className="flex items-center gap-1">
      {role === 'admin' && (
        <button
          className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700"
          aria-label="Editar empleado"
          onClick={() => alert(`Editar: ${row.original.id}`)}
        >
          <Pencil size={14} />
        </button>
      )}
      {/* ... */}
    </div>
  ),
}
```
**Apply to:** add a third Admin-only button with `<UserPlus size={14} />` and `aria-label="Enrolar Rostro"` that calls `onEnrollClick(row.original)`. Lift the modal state up to `employees/page.tsx` (mirrors how `devices/page.tsx` lifts `selectedDevice` + `commandModalOpen`).

---

### `frontend/src/lib/validations.ts` — EXTEND

**Analog:** existing `evidenceFileSchema` (validations.ts lines 24-30) for the JPG-shape check:
```tsx
export const evidenceFileSchema = z
  .instanceof(File)
  .refine(f => f.size <= 5 * 1024 * 1024, 'Máximo 5MB')
  .refine(
    f => ['application/pdf', 'image/jpeg', 'image/png'].includes(f.type),
    'Solo PDF, JPG o PNG'
  )
```
**Apply to:** mirror for the upload-tab JPG check (2 MB cap, JPG-only). Reuse the `.refine` chain. Discard `File` and use `Blob` for the modal submit because the webcam tab supplies a `Blob` directly:
```tsx
export const enrollmentSubmitSchema = z.object({
  employee_id: z.string().uuid("Debes seleccionar un empleado válido."),
  captured_via: z.enum(['device', 'webcam', 'upload']),
  source_device_id: z.string().uuid().nullable(),
  photo: z.instanceof(Blob, { message: "Falta la foto a enrolar." }),
}).refine(
  (data) => data.captured_via !== 'device' || data.source_device_id !== null,
  { message: "Selecciona el dispositivo Hikvision usado para capturar.", path: ['source_device_id'] }
)
```
(UI-SPEC § Form Validation Contract — already drafted.)

**Refine chained validation** (validations.ts lines 47-53 — `novedadSchema`):
```tsx
.refine(
  (data) => !data.fecha_inicio || !data.fecha_fin || data.fecha_fin >= data.fecha_inicio,
  { message: '...', path: ['fecha_fin'] },
)
```
**Apply to:** the `source_device_id` conditional refinement above.

---

### `frontend/src/types/api.ts` — EXTEND

**Analog:** existing `Device`, `Leave` shapes (types/api.ts lines 61-70, 93-106).

**Existing pattern:**
```tsx
export interface Device {
  id: string
  name: string
  ip_address: string
  // ...
  status: 'online' | 'offline' | 'unknown'
  last_seen_at: string | null
  created_at: string
  updated_at: string
}
```
**Apply to:** add
```tsx
export interface EnrollmentDevicePush {
  device_id: string
  device_name: string
  status: 'pending' | 'in_progress' | 'success' | 'failed'
  error_message: string | null
  started_at: string | null
  completed_at: string | null
}

export interface Enrollment {
  id: string
  employee_id: string
  status: 'in_progress' | 'success' | 'partial' | 'failed'
  started_at: string
  completed_at: string | null
  device_pushes: EnrollmentDevicePush[]
}
```
The `'in_progress'`-style state machine matches the existing string-union conventions throughout the file.

---

### `frontend/src/proxy.ts` — verify (no edit needed)

**Analog:** existing matcher (proxy.ts lines 47-49):
```tsx
export const config = {
  matcher: ['/dashboard/:path*', '/timesheet/:path*', '/employees/:path*', '/devices/:path*', '/enrollment/:path*', '/setup/:path*'],
}
```
`/enrollment/:path*` is already listed (Phase 4 D-12 carry-forward). PROTECTED_PATHS line 3 also already includes `'/enrollment'`. **No change required** — confirm during implementation.

---

## Shared Patterns

### Authentication / RBAC gating

**Source:** `backend/src/auth/rbac.rs` lines 31-51
**Apply to:** every Phase 7 backend route — all 5 (`POST /enrollments`, `GET /enrollments/:id`, retry, capture-from-device, get-capture) sit inside `admin_routes` in main.rs (already wraps `require_admin` middleware).

**Auth extractor for actor_id** (rbac.rs lines 11-27):
```rust
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser where S: Send + Sync {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Claims>().cloned().map(AuthUser).ok_or(AppError::Unauthorized)
    }
}
```
**Apply to:** every handler signature receives `AuthUser(claims): AuthUser` and uses `&claims.sub` for `created_by` / `started_by` columns. Pattern used identically in devices/handlers.rs `dispatch_command` line 220 and leaves/handlers.rs `create_leave` line 53.

### Error handling

**Source:** `backend/src/errors.rs` (entire file)
**Apply to:** Phase 7 reuses `AppError::Validation`, `AppError::NotFound`, `AppError::Conflict`, `AppError::BadGateway`, `AppError::Timeout`, `AppError::Internal`. **Do NOT add new variants** — every Phase 7 error fits an existing one (CONTEXT.md `code_context` — "add no new variants unless strictly needed").

**Concrete code excerpt** (errors.rs lines 38-49):
```rust
#[error("validation failed")]
Validation { code: &'static str, message: String },

#[error("gateway timeout")]
Timeout { code: &'static str, message: String },

#[error("bad gateway")]
BadGateway { code: &'static str, message: String },
```

### File-based JPEG storage

**Source:** `backend/src/events/service.rs` `write_photo_atomic` (lines 156-172) — already public, already used by leaves.
**Apply to:** Phase 7 reuses verbatim. Add a thin `pub fn enrollments_root() -> PathBuf` helper in `enrollments::service` matching `events_root()` and `leaves_root()` shape:
```rust
pub fn enrollments_root() -> PathBuf {
    std::env::var("ENROLLMENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data/enrollments"))
}
```
(Pattern inferred from leaves/handlers.rs line 167 `service::leaves_root()` reference.)

### Audit triggers (D-17)

**Source:** `backend/src/db/migrations/006_devices_audit_triggers.sql` (entire file is the template)
**Apply to:** Phase 7's three new tables — `enrollments`, `face_enrollments`, `device_face_mappings` (the last fulfils the deferral note in 006 line 9). Each table gets a 3-trigger set (INSERT/UPDATE/DELETE). The UUID-v4 hex expression and JSON column whitelisting rules carry over verbatim. Sensitive-column omission (006 line 4-7 comment): exclude any column the team flags as PII-sensitive (e.g., the raw `face_quality_score` JSON if it grows large — though current shape is small enough to keep).

### Multipart receive

**Source:** `backend/src/leaves/handlers.rs` (lines 51-194 — closest analog) + RESEARCH Pattern 7 (multipart receive backend, lines 920-1006).
**Apply to:** `POST /enrollments` handler. The leaves multipart pattern (drain → text/bytes per name → server-generated path → write_photo_atomic) is the project's established convention. RESEARCH § Code Examples → "Multipart enrollment receive (backend)" gives the literal file shape including the JPEG magic-byte check.

### Polling lifecycle

**Source:** `frontend/src/app/(dashboard)/devices/page.tsx` line 17 (`refetchInterval: 30_000`) — existing simple-interval pattern.
**Apply to:** Phase 7's polling uses RESEARCH Pattern 4's **function-form** `refetchInterval` (TanStack Query v5) — devices/page.tsx uses the simple integer form, but Phase 7 needs the dynamic-stop variant since enrollment polling MUST stop when all device_pushes are terminal. Extend the existing convention with the function form.

### Toast / `useMutation` error handling

**Source:** `frontend/src/components/devices/command-modal.tsx` lines 27-38 + `frontend/src/lib/api.ts` lines 47-75 (axios 401 interceptor).
**Apply to:** every Phase 7 mutation. The `(err as { response?: { data?: { message?: string } } })?.response?.data?.message` extraction pattern is the project's standard for surfacing backend `AppError` `message` strings. 401 handling is automatic via the global axios interceptor — no Phase 7 code needs to re-implement it (Phase 4 D-13 carry-forward).

### RBAC UI gating

**Source:** `frontend/src/contexts/auth-context.tsx` (`useAuth` hook) + `frontend/src/components/employees/employee-table.tsx` lines 58-81 (inline gate).
**Apply to:** use `const { role } = useAuth()` and `{role === 'admin' && (...)}` for inline gates (action buttons, secondary controls). For the **page-level** gate (Phase 7's new use case), introduce the new shared `<AccessRestrictedPlaceholder />` component. **Security note** (auth-context.tsx lines 6-19): the role decode is unverified — backend `require_admin` is the authoritative gate. UI gating is UX, not security.

---

## No Analog Found

Files where the codebase has no close match — planner uses RESEARCH.md patterns directly:

| File | Role | Data Flow | Why no analog | Recommended source |
|---|---|---|---|---|
| `backend/src/enrollments/image_pipeline.rs` | utility | transform | First server-side image processing path in the project | RESEARCH § Code Examples → Pattern 3 (`normalize_face_jpeg`, lines 540-585) |
| `frontend/src/components/enrollment/webcam-capture-tab.tsx` | component | streaming | First `getUserMedia` consumer | RESEARCH § Code Examples → Pattern 5 (lines 627-662) + Pitfall 6 (cleanup, lines 770-784) |
| `frontend/src/components/enrollment/validation-panel.tsx` | component | event-driven | First `@vladmandic/face-api` consumer | RESEARCH § Code Examples → "Webcam capture + face validation" hook (lines 836-918) |
| `frontend/src/lib/face-detection.ts` | utility | event-driven | First face-detection helper | RESEARCH § Architecture → Browser tier (validation pipeline diagram) |
| `frontend/public/models/tiny_face_detector_*` | static asset | n/a | First vendored ML model | RESEARCH § Standard Stack → "Models bundle (separate from npm install)" lines 159-162 |

**Multipart-receive on backend** is borderline — `backend/src/leaves/handlers.rs` and `backend/src/daily_records/handlers.rs` both already do it, so the pattern IS in-repo (analog above). The face-api.js pieces and image pipeline are the only true greenfield surfaces.

---

## Metadata

**Analog search scope:**
- `backend/src/devices/` (full module — closest analog for the new `enrollments/` module)
- `backend/src/isapi/client.rs` (extension target)
- `backend/src/events/service.rs` (filesystem write helpers — `write_photo_atomic`, root dir convention, `lookup_employee_for_event` confirms read side already wired)
- `backend/src/leaves/handlers.rs` (multipart receive pattern + recompute publish)
- `backend/src/recompute/worker.rs` (mpsc + cancellation token + biased select)
- `backend/src/supervisor/mod.rs` (lifecycle event channel + JoinHandle map)
- `backend/src/db/migrations/{003_devices.sql, 006_devices_audit_triggers.sql, 015_employees_position_hire_date.sql}` (schema + audit + ALTER TABLE patterns)
- `backend/src/main.rs` (worker bootstrap + admin route registration + body limit middleware)
- `backend/src/auth/rbac.rs` (admin gate)
- `backend/src/errors.rs` + `backend/src/common.rs` (reusable types)
- `frontend/src/components/devices/{command-modal.tsx, device-table.tsx}` (modal pattern + StatusBadge state colors)
- `frontend/src/components/employees/employee-table.tsx` (row action gating)
- `frontend/src/app/(dashboard)/{devices,employees,enrollment}/page.tsx` (page wrapper pattern + RBAC inline gate + existing placeholder)
- `frontend/src/lib/{api.ts, validations.ts}` (axios + TanStack Query setup; Zod schema patterns)
- `frontend/src/contexts/auth-context.tsx` (`useAuth` + role display-hint)
- `frontend/src/proxy.ts` (Next.js proxy — confirmed no change needed)
- `frontend/src/types/api.ts` (existing TypeScript shapes for extension)

**Files scanned:** ~30 backend + ~12 frontend.
**Pattern extraction date:** 2026-04-27.
**Stop criterion:** strong analogs found for 24/26 new-or-modified files; the remaining 2 surfaces (image pipeline + face-api) have explicit, vetted RESEARCH patterns. No diminishing returns to widen the search.
