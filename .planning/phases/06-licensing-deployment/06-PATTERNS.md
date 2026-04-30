# Phase 6: Licensing & Deployment - Pattern Map

**Mapped:** 2026-04-27
**Files analyzed:** 18 (10 backend + 4 frontend + 4 deploy/DO Functions)
**Analogs found:** 14 / 18 (4 deploy/DO files have no analog — first of their kind in repo)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `backend/src/license/mod.rs` | module index | n/a | `backend/src/auth/mod.rs` | exact |
| `backend/src/license/fingerprint.rs` | utility | file-I/O | `backend/src/auth/service.rs` (sha2 hashing) | role-match |
| `backend/src/license/service.rs` | service | request-response | `backend/src/auth/service.rs` | exact |
| `backend/src/license/middleware.rs` | middleware | request-response | `backend/src/auth/middleware.rs` | exact |
| `backend/src/license/handlers.rs` | controller | request-response | `backend/src/setup/handlers.rs` | exact |
| `backend/src/license/pubkey.pem` | config (asset) | n/a | none | new |
| `backend/src/config.rs` (modify) | config | n/a | self (extend pattern) | exact |
| `backend/src/state.rs` (modify) | state | n/a | self (extend pattern) | exact |
| `backend/src/errors.rs` (modify) | error type | n/a | self (extend pattern) | exact |
| `backend/src/setup/handlers.rs` (modify) | controller | request-response | self — extend `setup_status` + add `setup_activate` | exact |
| `backend/src/main.rs` (modify) | entrypoint | n/a | self (router wiring) | exact |
| `backend/Cargo.toml` (modify) | manifest | n/a | self | exact |
| `backend/tests/license_tests.rs` | test | request-response | `backend/tests/auth_tests.rs` + `device_tests.rs` (wiremock) | exact |
| `frontend/src/app/setup/license/page.tsx` | component (page) | request-response | `frontend/src/app/setup/page.tsx` | exact |
| `frontend/src/app/setup/license/layout.tsx` | component (layout) | n/a | `frontend/src/app/setup/layout.tsx` | exact |
| `frontend/src/lib/validations.ts` (modify) | utility (zod schema) | n/a | self (extend pattern) | exact |
| `frontend/src/app/setup/page.tsx` (modify) | component (page) | request-response | self (extend status check) | exact |
| `deploy/Dockerfile.api` | deploy artifact | n/a | none | new (RESEARCH-only) |
| `deploy/Dockerfile.web` | deploy artifact | n/a | none | new (RESEARCH-only) |
| `deploy/docker-compose.yml` | deploy artifact | n/a | none | new (RESEARCH-only) |
| `deploy/install.sh` | deploy artifact (bash) | n/a | none | new (RESEARCH-only) |
| `do-functions/packages/licenses/activate/index.js` | service (serverless) | request-response | none | new (RESEARCH-only) |
| `do-functions/packages/licenses/renew/index.js` | service (serverless) | request-response | none (peer of activate) | sibling-match |
| `do-functions/project.yml` | config | n/a | none | new (RESEARCH-only) |

---

## Pattern Assignments

### `backend/src/license/mod.rs` (module index)

**Analog:** `backend/src/auth/mod.rs` (lines 1-5)

```rust
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod rbac;
pub mod service;
```

**Apply as:**
```rust
pub mod fingerprint;
pub mod handlers;
pub mod middleware;
pub mod service;
```

(Note: license module has no separate `models.rs` — claims live in `service.rs` since they're internal-only; no `rbac.rs` because the gate is binary.)

---

### `backend/src/license/service.rs` (service, request-response)

**Analog:** `backend/src/auth/service.rs`

**Imports pattern** (lines 1-7):
```rust
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sha2::{Digest, Sha256};

use crate::errors::AppError;

use super::models::{Claims, Role};
```

**Apply as (license/service.rs imports):**
```rust
use std::sync::OnceLock;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use super::fingerprint;
```

**JWT verify pattern** (auth/service.rs lines 60-74):
```rust
pub fn verify_access_token(token: &str, secret: &[u8]) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized)?;

    if token_data.claims.token_type != "access" {
        return Err(AppError::Unauthorized);
    }

    Ok(token_data.claims)
}
```

**Apply as (license verification — RS256 + soft expiry):**
```rust
const LICENSE_PUBLIC_KEY_PEM: &str = include_str!("pubkey.pem");
static LICENSE_DECODING_KEY: OnceLock<DecodingKey> = OnceLock::new();

fn license_decoding_key() -> &'static DecodingKey {
    LICENSE_DECODING_KEY.get_or_init(|| {
        DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY_PEM.as_bytes())
            .expect("License public key is invalid PEM — recompile required")
    })
}

pub fn verify_license_jwt(token: &str) -> Result<LicenseClaims, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false; // D-07 soft expiry — system keeps running
    let data = decode::<LicenseClaims>(token, license_decoding_key(), &validation)
        .map_err(|_| AppError::Unlicensed)?;
    Ok(data.claims)
}
```

**Token issuance NOT applied** — license JWTs are signed externally by DO Functions (D-01). The Rust binary only verifies, never signs.

**Reqwest call pattern** (`backend/src/isapi/client.rs` lines 50-57 — outbound HTTP):
```rust
let client = Client::builder()
    .timeout(REQUEST_TIMEOUT)
    .connect_timeout(CONNECT_TIMEOUT)
    .danger_accept_invalid_certs(allow_insecure_tls)
    .build()
    .context("build reqwest Client for ISAPI")?;
```

**Apply as (DO Functions activation call):**
```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(15))
    .connect_timeout(Duration::from_secs(5))
    .build()
    .context("build reqwest Client for DO Functions")?;
let resp = client
    .post(&do_functions_url)
    .json(&serde_json::json!({
        "license_key": license_key,
        "hardware_fingerprint": fingerprint,
    }))
    .send()
    .await
    .map_err(|_| AppError::BadGateway { code: "ACTIVATION_UNREACHABLE", message: "Could not reach license server".into() })?;
```

---

### `backend/src/license/fingerprint.rs` (utility, file-I/O)

**Analog:** `backend/src/auth/service.rs` lines 92-98 (sha2 hash pattern)

**Hash pattern excerpt:**
```rust
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

**Apply as:**
```rust
use sha2::{Digest, Sha256};
use std::fs;

pub fn collect_fingerprint() -> Result<String, anyhow::Error> {
    let cpu = read_cpu_model()?;
    let mac = read_primary_mac()?;
    let disk = read_primary_disk_serial().unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(cpu.as_bytes());
    hasher.update(mac.as_bytes());
    hasher.update(disk.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}
```

**No existing /proc-reading code in repo** — RESEARCH § Pattern 3 is the source of truth for the three readers. Helpers must be private to the module (not pub) since they leak Linux-only paths.

---

### `backend/src/license/middleware.rs` (middleware, request-response)

**Analog:** `backend/src/auth/middleware.rs` (entire file, 28 lines — complete pattern)

**Imports + signature:**
```rust
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{auth::service, errors::AppError, state::AppState};

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = service::verify_access_token(token, state.config.jwt_secret.as_bytes())?;

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
```

**Apply as (require_license — even simpler, no token extraction):**
```rust
use std::sync::atomic::Ordering;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{errors::AppError, state::AppState};

pub async fn require_license(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if !state.license_valid.load(Ordering::Relaxed) {
        return Err(AppError::Unlicensed);
    }
    Ok(next.run(req).await)
}
```

---

### `backend/src/license/handlers.rs` AND `backend/src/setup/handlers.rs` (controller, request-response)

The `POST /setup/activate` handler may live in either module. Per CONTEXT.md `code_context`, "License activation endpoint slots into the existing setup wizard flow" — recommendation: put it in `setup/handlers.rs` to keep the public-route surface co-located.

**Analog:** `backend/src/setup/handlers.rs` `setup_init` (lines 50-108)

**Imports pattern** (lines 1-7):
```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{auth::service, errors::AppError, state::AppState};
```

**Validate request body pattern** (lines 36-44, 53-57):
```rust
#[derive(Debug, Deserialize, Validate)]
pub struct SetupInitRequest {
    #[validate(length(min = 1, message = "Full name is required"))]
    pub full_name: String,
    // ...
}

body.validate().map_err(|e| AppError::Validation {
    code: "VALIDATION_ERROR",
    message: e.to_string(),
})?;
```

**Apply as (activate endpoint):**
```rust
#[derive(Debug, Deserialize, Validate)]
pub struct SetupActivateRequest {
    #[validate(length(min = 19, max = 19, message = "License key must be in XXXX-XXXX-XXXX-XXXX format"))]
    pub license_key: String,
}

pub async fn setup_activate(
    State(state): State<AppState>,
    Json(body): Json<SetupActivateRequest>,
) -> Result<impl IntoResponse, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;
    // 1. collect_fingerprint()
    // 2. call DO Functions /licenses/activate via reqwest
    // 3. verify returned JWT (RS256 + fingerprint match)
    // 4. write JWT to config.license_jwt_path
    // 5. set state.license_valid.store(true, Ordering::Relaxed)
    // 6. return 200 { "activated": true }
}
```

**Status endpoint extension** (modify `setup_status` at lines 12-33):
```rust
// Existing:
Ok(Json(json!({ "initialized": count > 0 })))

// Apply as:
Ok(Json(json!({
    "initialized": count > 0,
    "licensed": state.license_valid.load(Ordering::Relaxed)
})))
```

---

### `backend/src/config.rs` (config — modify)

**Analog:** self (extend the existing pattern)

**Existing struct field + Debug redaction pattern** (lines 8-23, 25-41):
```rust
#[derive(Clone)]
pub struct Config {
    pub database_path: String,
    // ...
    pub timezone: chrono_tz::Tz,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("database_path", &self.database_path)
            // ...
            .field("timezone", &self.timezone.name())
            .finish()
    }
}
```

**Existing env-var loading with default** (lines 45-46, 80-83):
```rust
let database_path = std::env::var("CRONOMETRIX_DB_PATH")
    .unwrap_or_else(|_| "cronometrix.db".to_string());

let tz_str = std::env::var("TZ").unwrap_or_else(|_| "America/Caracas".to_string());
```

**Apply as:**
```rust
// In struct:
pub license_jwt_path: String,
pub do_functions_activate_url: String,  // for activate handler
pub do_functions_renew_url: String,     // for renewal task

// In Debug impl: print these as-is — they are URLs, not secrets.
.field("license_jwt_path", &self.license_jwt_path)
.field("do_functions_activate_url", &self.do_functions_activate_url)

// In from_env():
let license_jwt_path = std::env::var("LICENSE_JWT_PATH")
    .unwrap_or_else(|_| "/opt/cronometrix/data/license.jwt".to_string());
let do_functions_activate_url = std::env::var("DO_FUNCTIONS_ACTIVATE_URL")
    .unwrap_or_default();
```

---

### `backend/src/state.rs` (state — modify)

**Analog:** self (lines 41-48)

**Existing `Option<...>` pattern for non-required-in-tests fields:**
```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    pub lifecycle_tx: Option<LifecycleTx>,
    pub recompute_tx: Option<UnboundedSender<RecomputeRequest>>,
    pub event_broadcast: Option<broadcast::Sender<AttendanceEventSSEPayload>>,
}
```

**Apply as:**
```rust
pub license_valid: Arc<std::sync::atomic::AtomicBool>,
```

Use `Arc<AtomicBool>` (NOT `Option`) because:
- License flag is checked on every gated request — branch-free atomic load is cheap
- Default value (false) is meaningful — tests can construct AppState with `Arc::new(AtomicBool::new(true))` to bypass the gate
- Same pattern as cited in RESEARCH § AppState code example (lines 622-633)

**Test wiring change in `backend/tests/auth_tests.rs` `build_test_app`** (lines 30-36):
```rust
let state = AppState {
    db: Arc::new(db),
    config,
    lifecycle_tx: None,
    recompute_tx: None,
    event_broadcast: None,
    // Add:
    license_valid: Arc::new(std::sync::atomic::AtomicBool::new(true)),
};
```

All existing test files (15 files in `backend/tests/`) need this single-line addition.

---

### `backend/src/errors.rs` (error type — modify)

**Analog:** self (lines 16-67)

**Existing variant + IntoResponse mapping pattern** (lines 22-27, 75-79):
```rust
#[error("unauthorized")]
Unauthorized,
// ...
AppError::Unauthorized => (
    StatusCode::UNAUTHORIZED,
    "UNAUTHORIZED",
    "Authentication required".to_string(),
),
```

**Apply as:**
```rust
#[error("system not licensed")]
Unlicensed,

// In IntoResponse match:
AppError::Unlicensed => (
    StatusCode::FORBIDDEN,
    "UNLICENSED",
    "License required".to_string(),
),
```

UI-SPEC error code mapping (06-UI-SPEC.md lines 271-277) requires the activate endpoint to return distinct codes:
- 404 `LICENSE_NOT_FOUND`
- 403 `HARDWARE_MISMATCH`
- 409 `ALREADY_ACTIVATED`
- 503 network/server error

These should reuse the existing `NotFound { code, message }`, `Forbidden`/new `Unlicensed`, `Conflict { code, message }`, and `BadGateway { code, message }` variants — no new variants needed beyond `Unlicensed`.

---

### `backend/src/main.rs` (entrypoint — modify)

**Analog:** self (lines 105-201, full router construction)

**Existing route grouping + middleware layering** (lines 121-141):
```rust
let viewer_routes = Router::new()
    .route("/employees", get(employees::handlers::list_employees))
    // ...
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::middleware::require_auth,
    ));
```

**Existing public routes group** (lines 106-113):
```rust
let public_routes = Router::new()
    .route("/health", get(health))
    .route("/auth/login", post(auth::handlers::login))
    .route("/setup/status", get(setup::handlers::setup_status))
    .route("/setup/init", post(setup::handlers::setup_init))
    .route("/events/stream", get(events::handlers::events_stream));
```

**Apply as:**
1. Add `setup_activate` to `public_routes`:
```rust
.route("/setup/activate", post(setup::handlers::setup_activate))
```

2. Construct `AppState` with `license_valid` (line 65-71):
```rust
let license_valid = Arc::new(std::sync::atomic::AtomicBool::new(false));

// Load + validate cached JWT BEFORE state construction:
if license::service::load_and_validate_license(&config).await {
    license_valid.store(true, std::sync::atomic::Ordering::Relaxed);
}

let state = AppState {
    // ... existing fields ...
    license_valid: license_valid.clone(),
};
```

3. Wrap `viewer_routes`, `supervisor_*_routes`, `report_routes`, `admin_routes`, `cookie_auth_routes` with `require_license` BEFORE their existing `require_auth` / `require_*` layer (per CONTEXT.md `code_context`: "License gate middleware added to the router builder, applied before `require_auth`"). Public routes stay ungated so first-run activation can complete.

The "before `require_auth`" ordering matters in Axum because `route_layer` applies in reverse order of definition — so `require_license` must be added LAST in the chain to run FIRST on incoming requests:

```rust
let viewer_routes = Router::new()
    .route(/* ... */)
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::middleware::require_auth,
    ))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        license::middleware::require_license,
    ));
```

4. Spawn renewal task (RESEARCH § Pattern at lines 81-103 — same pattern as `nightly_handle`):
```rust
let renewal_handle = tokio::spawn({
    let s = state.clone();
    let c = shutdown.clone();
    async move {
        license::service::renewal_task(s, c).await;
    }
});
// ... later, drain it before exit:
let _ = renewal_handle.await;
```

---

### `backend/Cargo.toml` (manifest — modify)

**Analog:** self (line 18)

**Existing line:**
```toml
jsonwebtoken = { version = "10.3.0", features = ["rust_crypto"] }
```

**Apply as:**
```toml
jsonwebtoken = { version = "10.3.0", features = ["rust_crypto", "use_pem"] }
```

No new dependencies — `sha2`, `reqwest`, `serde`, `serde_json`, `anyhow` all present (lines 7-39).

---

### `backend/tests/license_tests.rs` (test, request-response)

**Analog 1:** `backend/tests/auth_tests.rs` (test app construction, lines 17-60)

**Build test app pattern:**
```rust
mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use axum::routing::{get, post};
use cronometrix_api::auth;
use cronometrix_api::setup;
use cronometrix_api::state::AppState;
use cronometrix_api::config::Config;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use http_body_util::BodyExt;

async fn build_test_app(db: libsql::Database) -> Router {
    let config = Arc::new(Config { /* ... */ });
    let state = AppState { /* ... */ };
    Router::new()
        .nest("/api/v1", public_routes.merge(/* ... */))
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}
```

**Analog 2:** `backend/tests/device_tests.rs` lines 462-485 (wiremock for outbound HTTP — DO Functions activate stub)

**Wiremock stub pattern:**
```rust
let mock = MockServer::start().await;
Mock::given(wm_method("POST"))
    .and(wm_path("/licenses/activate"))
    .respond_with(
        ResponseTemplate::new(200)
            .set_body_json(json!({ "token": "<RS256 JWT here>" }))
    )
    .mount(&mock)
    .await;
// In Config: do_functions_activate_url = mock.uri() + "/licenses/activate"
```

**Test cases derived from RESEARCH § Phase Requirements → Test Map (lines 769-779):**
- `test_license_gate_blocks_requests` — LIC-01 — issue request without license, expect 403 + body `{"error":{"code":"UNLICENSED",...}}`
- `test_fingerprint_deterministic` — LIC-02 — call `collect_fingerprint()` twice, assert equal
- `test_activation_calls_do_functions` — LIC-03 — wiremock stub returns valid JWT, assert state.license_valid becomes true
- `test_startup_loads_cached_jwt` — LIC-04 — pre-write a valid JWT to a temp path, call `load_and_validate_license`, assert returns true
- `test_activation_rejects_fingerprint_mismatch` — LIC-05 — wiremock returns JWT with wrong fingerprint claim, assert state.license_valid stays false
- `test_offline_operation_with_cached_jwt` — DEPL-04 — no DO Functions URL configured, JWT cached on disk, assert app starts and serves requests

For RS256 JWT signing in tests, use `jsonwebtoken::EncodingKey::from_rsa_pem` with a test RSA keypair generated once and stored under `backend/tests/fixtures/test_license_key.pem` + `test_license_pubkey.pem`.

---

### `frontend/src/app/setup/license/page.tsx` (component, request-response)

**Analog:** `frontend/src/app/setup/page.tsx` (entire 281-line file — exact pattern reuse per UI-SPEC §Screen Layout Contract)

**Imports pattern** (lines 1-28):
```tsx
"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { Eye, EyeOff, Loader2, AlertCircle } from "lucide-react"
import axios from "axios"

import { setupSchema, type SetupFormData } from "@/lib/validations"
import { API_BASE } from "@/lib/api"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
```

**Apply as (license/page.tsx):**
```tsx
"use client"
import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { Loader2, AlertCircle, ShieldCheck, Key } from "lucide-react"
import axios from "axios"

import { licenseSchema, type LicenseFormData } from "@/lib/validations"
import { API_BASE } from "@/lib/api"
// (same shadcn Card / Form / Input / Button imports)
```

**Status check on mount pattern** (setup/page.tsx lines 52-68):
```tsx
useEffect(() => {
    const checkStatus = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/v1/setup/status`)
        const data = await res.json()
        if (data.initialized) {
          setAlreadyConfigured(true)
          setTimeout(() => router.push("/login"), 1500)
        }
      } catch {
        // Backend unreachable — show form anyway
      } finally {
        setCheckingStatus(false)
      }
    }
    checkStatus()
}, [router])
```

**Apply as (license/page.tsx — UI-SPEC §Status Check on Mount):**
```tsx
useEffect(() => {
    const checkStatus = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/v1/setup/status`)
        const data = await res.json()
        if (data.licensed === true && data.initialized === false) {
          router.push("/setup")
          return
        }
        if (data.licensed === true && data.initialized === true) {
          router.push("/login")
          return
        }
        // licensed === false: show form
      } catch {
        // Backend unreachable — show form anyway (allow retry)
      } finally {
        setCheckingStatus(false)
      }
    }
    checkStatus()
}, [router])
```

**Submit pattern with axios + 409/422 mapping** (lines 70-101):
```tsx
async function onSubmit(values: SetupFormData) {
    setIsSubmitting(true)
    setServerError(null)
    try {
      await axios.post(`${API_BASE}/api/v1/setup/init`, { /* body */ })
      router.push("/login")
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const status = err.response?.status
        if (status === 409) { /* ... */ }
        if (status === 422) {
          const detail = err.response?.data?.error?.message
          setServerError({ message: detail || "Validation error..." })
        } else {
          setServerError({ message: "Something went wrong. Please try again." })
        }
      }
    } finally {
      setIsSubmitting(false)
    }
}
```

**Apply as (license activation submit):**
```tsx
async function onSubmit(values: LicenseFormData) {
    setIsSubmitting(true)
    setServerError(null)
    try {
      await axios.post(`${API_BASE}/api/v1/setup/activate`, {
        license_key: values.license_key,
      })
      setSuccess(true)
      setTimeout(() => router.push("/setup"), 1500)
    } catch (err) {
      if (axios.isAxiosError(err)) {
        const code = err.response?.data?.error?.code
        // UI-SPEC error code mapping table
        const messages: Record<string, string> = {
          LICENSE_NOT_FOUND: "License key not found. Check the key and try again.",
          HARDWARE_MISMATCH: "This license is registered to different hardware. Contact support to transfer your license.",
          ALREADY_ACTIVATED: "This license is already active on another installation.",
          ACTIVATION_UNREACHABLE: "Could not reach the activation server. Check your internet connection and try again.",
        }
        setServerError({ message: messages[code] || "Could not reach the activation server. Check your internet connection and try again." })
      } else {
        setServerError({ message: "Could not reach the activation server. Check your internet connection and try again." })
      }
    } finally {
      setIsSubmitting(false)
    }
}
```

**Layout shell** (lines 121-132 — copy verbatim, swap copy):
```tsx
<div className="min-h-screen flex items-center justify-center px-4">
  <Card className="max-w-md w-full shadow-md">
    <CardHeader>
      <CardTitle className="text-2xl font-semibold">Activate your license</CardTitle>
      <CardDescription>Enter the license key provided with your Cronometrix installation.</CardDescription>
    </CardHeader>
    <CardContent>
      {/* error banner, form, button — see Shared Patterns below */}
    </CardContent>
  </Card>
</div>
```

**Single FormField pattern** (one field — lines 149-166 simplified):
```tsx
<FormField
  control={form.control}
  name="license_key"
  render={({ field, fieldState }) => (
    <FormItem>
      <FormLabel>License key</FormLabel>
      <FormControl>
        <Input
          {...field}
          autoComplete="off"
          spellCheck={false}
          maxLength={19}
          placeholder="XXXX-XXXX-XXXX-XXXX"
          className="font-mono uppercase"
          aria-describedby={fieldState.error ? "license_key-error" : undefined}
          aria-invalid={!!fieldState.error}
        />
      </FormControl>
      <FormMessage id="license_key-error" />
    </FormItem>
  )}
/>
```

---

### `frontend/src/app/setup/license/layout.tsx` (component, layout)

**Analog:** `frontend/src/app/setup/layout.tsx` (entire 13-line file)

**Full pattern (copy verbatim, swap title):**
```tsx
import type { Metadata } from "next"

export const metadata: Metadata = {
  title: "Cronometrix — License Activation",
}

export default function LicenseLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return <>{children}</>
}
```

---

### `frontend/src/lib/validations.ts` (utility — modify)

**Analog:** self (lines 1-22 — `setupSchema` + `loginSchema` pattern)

**Existing pattern:**
```ts
import { z } from 'zod'

export const setupSchema = z.object({
    full_name: z.string().min(1, 'This field is required.'),
    // ...
})
export type SetupFormData = z.infer<typeof setupSchema>
```

**Apply as (per UI-SPEC §Form Validation Contract lines 258-267):**
```ts
export const licenseSchema = z.object({
  license_key: z
    .string()
    .min(1, "License key is required.")
    .regex(/^[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}$/i,
      "License key must be in XXXX-XXXX-XXXX-XXXX format."),
})
export type LicenseFormData = z.infer<typeof licenseSchema>
```

---

### `frontend/src/app/setup/page.tsx` (modify — extend status check)

**Analog:** self (lines 52-68 — `checkStatus` `useEffect`)

**Modification:** add a redirect to `/setup/license` if the backend responds with `licensed: false`. The `licensed` field is added to the response by Phase 6 work on `setup_status` handler. Existing behavior (redirect to /login if `initialized`) stays.

```tsx
// In existing checkStatus:
if (data.licensed === false) {
  router.push("/setup/license")
  return
}
// existing initialized check stays
```

---

### Deploy artifacts (no in-repo analog)

The following 4 files have **no existing analog** — none of these patterns currently exist in the repo:

1. `deploy/Dockerfile.api` — multi-stage Rust build
2. `deploy/Dockerfile.web` — multi-stage Next.js standalone build (requires `output: "standalone"` added to `frontend/next.config.ts`)
3. `deploy/docker-compose.yml` — 3-service compose
4. `deploy/install.sh` — bash installer

**Source patterns:** RESEARCH.md §Architecture Patterns 6-8 (lines 421-545) and §Pattern 7 (lines 462-515). Planner should treat RESEARCH.md as the authority for these files. No codebase reference excerpts apply.

**Note for planner:** `frontend/next.config.ts` (lines 1-7) currently has no settings. Phase 6 must add:
```ts
const nextConfig: NextConfig = {
  output: "standalone",
}
```
This change is required by `Dockerfile.web` (RESEARCH § Pattern 8 line 544).

---

### DO Functions (no in-repo analog)

`do-functions/packages/licenses/activate/index.js` and `renew/index.js` plus `do-functions/project.yml` are net-new. RESEARCH.md §Pattern 4-5 (lines 359-419) is the authority. Planner uses those excerpts directly.

---

## Shared Patterns

### Error response shape (apply to all backend handlers)

**Source:** `backend/src/errors.rs` lines 113-122

```rust
let body = Json(json!({
    "error": {
        "code": code,
        "message": message,
        "status": status.as_u16()
    }
}));
```

**Apply to:** every handler that returns `Result<_, AppError>` — automatic via `IntoResponse` impl. Frontend reads `err.response?.data?.error?.code` to map to UI banner copy (UI-SPEC §Form Validation Contract).

---

### Tower middleware layer attachment

**Source:** `backend/src/auth/middleware.rs` + `backend/src/main.rs` lines 138-141

```rust
.route_layer(axum::middleware::from_fn_with_state(
    state.clone(),
    auth::middleware::require_auth,
))
```

**Apply to:** `require_license` middleware — wrap every protected route group (`viewer_routes`, `supervisor_*_routes`, `report_routes`, `admin_routes`, `cookie_auth_routes`). Public routes (`public_routes` containing `/health`, `/auth/login`, `/setup/*`, `/events/stream`) MUST stay ungated so first-run activation works.

Layer ordering matters — see `main.rs` modification block above.

---

### Validate request bodies

**Source:** `backend/src/setup/handlers.rs` lines 36-44, 53-57

```rust
#[derive(Debug, Deserialize, Validate)]
pub struct SetupInitRequest {
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}
// In handler:
body.validate().map_err(|e| AppError::Validation {
    code: "VALIDATION_ERROR",
    message: e.to_string(),
})?;
```

**Apply to:** `SetupActivateRequest` body validation in license activation handler.

---

### Secret redaction in Debug

**Source:** `backend/src/config.rs` lines 25-41 + `backend/src/isapi/client.rs` lines 32-40

```rust
.field("jwt_secret", &"[redacted]")
.field("device_creds_key", &"[redacted 32 bytes]")
```

**Apply to:** Any new struct that holds license-secret-adjacent data. The license public key is NOT a secret, but the cached JWT contains the hardware fingerprint claim, so any future struct holding the raw JWT must redact it. (Current plan stores it on disk only — not in any in-memory struct — so this rule is preventative.)

---

### Frontend status check + redirect on mount

**Source:** `frontend/src/app/setup/page.tsx` lines 52-68 (and `alreadyConfigured` state + redirect at lines 105-119)

```tsx
useEffect(() => { /* fetch /setup/status, conditionally redirect */ }, [router])
if (checkingStatus) return <Loader2 ... />
```

**Apply to:** new `/setup/license` page (UI-SPEC §Status Check on Mount lines 226-235); also extend existing `/setup/page.tsx` to redirect to `/setup/license` when `licensed === false`.

---

### Frontend error banner

**Source:** `frontend/src/app/setup/page.tsx` lines 134-141

```tsx
<div
  className="flex items-center gap-3 p-4 mb-4 rounded border-l-4 border-destructive bg-destructive/10"
  role="alert"
>
  <AlertCircle className="h-4 w-4 text-destructive shrink-0" />
  <p className="text-sm text-destructive">{serverError.message}</p>
</div>
```

**Apply to:** license activation page error banner (UI-SPEC §Error Banner lines 215-218 — exact same pattern). Also UI-SPEC §Success Banner uses the same structure with `border-green-600 bg-green-50` + `ShieldCheck` icon.

---

### Frontend submit button (no `disabled` attr)

**Source:** `frontend/src/app/setup/page.tsx` lines 259-273

```tsx
<Button
  type="submit"
  className="w-full"
  aria-disabled={isSubmitting}
  onClick={isSubmitting ? (e) => e.preventDefault() : undefined}
>
  {isSubmitting ? (
    <>
      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
      Creating account…
    </>
  ) : (
    "Create account"
  )}
</Button>
```

**Apply to:** license activation submit button (UI-SPEC §Submit Button lines 207-211 — same `aria-disabled` not `disabled` rule).

---

### Test app builder + body-to-json helper

**Source:** `backend/tests/auth_tests.rs` lines 17-66 + `backend/tests/common/mod.rs` lines 25-66

**Apply to:** new `backend/tests/license_tests.rs` — duplicate the `build_test_app` helper (with `license_valid` field added to `AppState`), reuse `common::test_db()`, `common::TEST_JWT_SECRET`, `common::test_device_creds_key()`, and the `body_to_json` helper.

For wiremock-based outbound stubbing, copy the pattern from `backend/tests/device_tests.rs` lines 462-485.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `backend/src/license/pubkey.pem` | config asset | n/a | First PEM asset embedded via `include_str!` in repo; no `.pem` files exist today |
| `deploy/Dockerfile.api` | container build | n/a | No Dockerfiles exist in repo; first multi-stage Rust build artifact |
| `deploy/Dockerfile.web` | container build | n/a | No Dockerfiles exist in repo; first Next.js standalone artifact |
| `deploy/docker-compose.yml` | orchestration | n/a | No compose files exist in repo |
| `deploy/install.sh` | bash installer | n/a | No shell installers exist in repo |
| `do-functions/packages/licenses/activate/index.js` | serverless function | request-response | First serverless artifact in repo; new tier (DO Functions) |
| `do-functions/packages/licenses/renew/index.js` | serverless function | request-response | Same — peer of `activate` |
| `do-functions/project.yml` | DO config | n/a | First DO Functions config |

For these 8 files, the planner should reference RESEARCH.md sections directly (Patterns 4-8, lines 359-545) rather than expecting in-repo analogs.

---

## Metadata

**Analog search scope:**
- `backend/src/auth/` (5 files — middleware, service, handlers, mod, rbac, models)
- `backend/src/setup/` (2 files — handlers, mod)
- `backend/src/config.rs`, `state.rs`, `errors.rs`, `main.rs`, `lib.rs`, `common.rs`
- `backend/src/isapi/client.rs` (reqwest pattern)
- `backend/Cargo.toml`
- `backend/tests/auth_tests.rs`, `device_tests.rs`, `common/mod.rs`
- `frontend/src/app/setup/page.tsx`, `setup/layout.tsx`
- `frontend/src/lib/validations.ts`, `api.ts`
- `frontend/package.json`, `next.config.ts`, `components.json`, `.env.example`

**Files scanned:** ~25
**Pattern extraction date:** 2026-04-27
