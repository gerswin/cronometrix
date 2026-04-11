# Phase 1: Foundation - Research

**Researched:** 2026-04-11
**Domain:** Rust/Axum backend, libSQL/Turso, JWT auth, SQLite audit triggers, Next.js setup wizard
**Confidence:** HIGH (core stack verified against registries and official docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Audit logging via SQLite triggers — triggers fire on INSERT/UPDATE/DELETE and write to an audit_log table automatically; cannot be bypassed by application bugs
- **D-02:** UUID v4 strings for all primary keys — globally unique, safe for Turso sync across independent installations, no collision risk
- **D-03:** Soft-delete uses both a status column (active/inactive) and a deleted_at timestamp — status for business logic, deleted_at for audit trail of when deactivation happened
- **D-04:** Version column on all mutable tables for optimistic concurrency — integer increments on update, prevents lost-update problems, essential for Turso sync conflict detection
- **D-05:** All timestamps stored as UTC epoch integers in the database
- **D-06:** Short-lived access token (15-30 min) in memory + refresh token (7 days) in httpOnly cookie — refresh silently on expiry
- **D-07:** Initial admin created via interactive setup wizard on first boot — browser flow asks for admin credentials when no users exist
- **D-08:** Password policy: minimum 8 characters, no complexity rules (NIST 800-63B); Argon2id hashing
- **D-09:** Viewer role has full read access to all data — 3 roles: Admin (full), Supervisor (edit subset), Viewer (read-only everything)
- **D-10:** Resource-based REST with /api/v1 prefix — plural nouns, standard HTTP verbs
- **D-11:** Structured JSON error responses with machine-readable error codes — `{"error": {"code": "EMPLOYEE_NOT_FOUND", "message": "...", "status": 404}}`
- **D-12:** Offset-based pagination with limit/offset query params — TanStack Table compatible
- **D-13:** Timestamps serialized as ISO 8601 strings in API responses; stored as UTC epoch integers in DB

### Claude's Discretion

- Rust project module organization and Axum handler/service structure
- Config management approach (dotenvy, environment loading)
- Exact migration file structure and naming
- Loading skeleton and error state patterns for the setup wizard
- Compression algorithm and temp file handling

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DATA-01 | All data stored locally in SQLite via libSQL | libsql 0.9.30 Builder::new_remote_replica() confirmed |
| DATA-02 | Data syncs asynchronously to Turso cloud | sync_interval(Duration) + manual db.sync().await API verified |
| DATA-03 | Local SQLite is authoritative — cloud is a replica | embedded replica architecture confirmed; reads always local |
| DATA-04 | Every administrative mutation generates an immutable audit log entry | SQLite AFTER triggers on INSERT/UPDATE/DELETE verified |
| AUTH-01 | User can log in with username and password | Argon2id verify + jsonwebtoken 10.3.0 encode confirmed |
| AUTH-02 | Admin role has full access to all features | Custom RBAC Tower middleware extracting role from JWT claims |
| AUTH-03 | Supervisor role can edit timesheets, manage employees, view reports | Per-router-group RBAC enforcement pattern researched |
| AUTH-04 | Viewer role has read-only access to dashboards and reports | Role enum in JWT claims drives axum extractor deny logic |
| AUTH-05 | User session persists across browser refresh | Refresh token in httpOnly cookie + access token in memory |
| EMP-01 | Admin can create employee with unique ID, name, department, status | CRUD handler pattern with validator crate confirmed |
| EMP-02 | Admin can search and filter by name, department, status | Offset pagination + SQL WHERE clause pattern researched |
| EMP-03 | Admin can deactivate employee (soft delete — no hard deletes) | D-03 status + deleted_at pattern; DELETE endpoint sets status only |
| EMP-04 | Each employee belongs to exactly one department (1:1) | FK constraint + DB-level enforcement in schema |
| DEPT-01 | Admin can create department with salary, shift schedule | Department table schema with JSON/columns for shift times |
| DEPT-02 | Admin can configure lunch mode per department | lunch_mode enum column (fixed/punch) + minutes column |
| DEPT-03 | Admin can edit department settings | PATCH endpoint; version column for optimistic concurrency |
| RULE-01 | Admin can configure tolerance margins via visual sliders | global_rules singleton table; PATCH endpoint |
| RULE-02 | Admin can configure bonus minutes | bonus_minutes column in global_rules table |
| RULE-03 | Rule changes take effect on next calculation cycle (not retroactive) | effective_from epoch stored with each rule change; Phase 3 calculates using rule version at time of calculation |
</phase_requirements>

---

## Summary

Phase 1 establishes the complete backend foundation: a Rust/Axum service with libSQL embedded replica, JWT authentication with three-role RBAC, SQLite trigger-based audit logging, and CRUD APIs for employees, departments, and global rules. There is no existing code — every pattern established here becomes the project-wide convention.

The tech stack (Rust 1.93.0, axum 0.8.8, libsql 0.9.30, jsonwebtoken 10.3.0) is fully verified against the Cargo registry as of 2026-04-11. The libSQL embedded replica API is stable: `Builder::new_remote_replica("file.db", url, token).sync_interval(Duration::from_secs(N)).build().await`. SQLite audit triggers are a mature, well-documented pattern that satisfies the legal immutability requirement without any application-layer bypass risk.

The most nuanced implementation areas are: (1) the argon2 crate is on `0.6.0-rc.8` — not a stable release — and using the `password-auth` wrapper is safer; (2) the RBAC middleware must be applied at the Axum router-group level, not per-handler, to avoid accidental omission; (3) libSQL's Turso sync mode does not support `PRAGMA` commands through the remote connection, so all schema-level PRAGMAs (WAL, foreign keys) must be applied through the local file connection before sync is enabled.

**Primary recommendation:** Wire the Cargo workspace, run migrations on startup via embedded SQL strings (no external migration runner needed for this scale), and establish the AppState pattern with `Arc<AppState>` holding the libSQL `Database` handle as the single shared resource.

---

## Standard Stack

### Core — Backend

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | 0.8.8 | HTTP routing, extractors, middleware | Tokio-native; CLAUDE.md locked |
| `tokio` | 1.x | Async runtime | Required by axum; `features = ["full"]` |
| `tower-http` | 0.6.8 | CORS, tracing, compression, timeout layers | Must match axum 0.8 (0.5.x is for axum 0.7) |
| `libsql` | 0.9.30 | SQLite local + Turso cloud sync | Official Turso SDK; CLAUDE.md locked |
| `jsonwebtoken` | 10.3.0 | JWT issuance and validation | CLAUDE.md locked; HS256 for on-premise single-secret |
| `axum-extra` | 0.12.5 | CookieJar extractor for httpOnly refresh token | Official axum crate for cookie management |
| `serde` | 1.x | Serialization/deserialization | Universal; `features = ["derive"]` |
| `serde_json` | 1.x | JSON request/response bodies | Required by axum Json extractor |
| `chrono` | 0.4.x | Epoch integer ↔ ISO 8601 conversion | CLAUSE.md locked for time arithmetic |
| `uuid` | 1.23.0 | UUID v4 primary key generation | `features = ["v4", "serde"]` |
| `tracing` | 0.1.x | Structured logging | Tokio ecosystem standard |
| `tracing-subscriber` | 0.3.x | Log output formatting | JSON for production, pretty for dev |
| `anyhow` | 1.x | Error propagation in application code | Ergonomic; use at handler boundary |
| `thiserror` | 2.x | Typed error enum with IntoResponse | Derive custom API error types |
| `validator` | 0.20.0 | Request payload validation | Derive-based validation macros |
| `dotenvy` | 0.15.7 | Environment config loading | Maintained fork of dotenv |
| `tower` | 0.5.x | RBAC middleware composition | Used via axum's `middleware::from_fn` |

### Password Hashing (special case)

| Library | Version | Purpose | Note |
|---------|---------|---------|------|
| `password-auth` | 1.x | Argon2id hashing wrapper | RECOMMENDED over raw `argon2` crate |
| `argon2` | 0.5.3 (stable) | Argon2id directly | `argon2 0.6.0-rc.8` in registry is a release candidate — use `password-auth` which pins a stable version |

[VERIFIED: cargo registry 2026-04-11] The `argon2` crate on crates.io is currently `0.6.0-rc.8` (a release candidate). The `password-auth` crate from RustCrypto wraps a stable `argon2` version and is the safer choice for production.

### Core — Frontend (Setup Wizard only in Phase 1)

| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| `next` | 16.2.3 | App Router, middleware for setup redirect | CLAUDE.md locked |
| `react` | 19.2.5 | UI library | CLAUDE.md locked |
| `typescript` | 5.x | Type safety | CLAUDE.md locked |
| `react-hook-form` | 7.x | Setup wizard form state | Zero re-renders |
| `zod` | 3.x | Schema validation | Single source of truth for form + type |
| `shadcn/ui` | latest | Form components | CLAUDE.md locked |
| `tailwindcss` | 4.x | Utility CSS | CLAUDE.md locked |
| `@tanstack/react-query` | 5.99.0 | API calls with loading/error state | CLAUDE.md locked |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `password-auth` | raw `argon2` crate | argon2 0.6.x is RC; password-auth uses stable argon2 0.5.3 |
| embedded SQL migration strings | `libsql_migration` (0.2.2) or `refinery` (0.9.0) | At this scale, embedded SQL strings in Rust are simpler; no extra binary |
| HS256 JWT | RS256 JWT | RS256 requires key pair management; HS256 is fine for on-premise single-secret deployment |
| Custom RBAC Tower middleware | `axum-login` or `axum-casbin` | Custom 3-role enum is simpler and eliminates an external dep for a well-understood problem |

**Installation — Rust:**
```toml
# Cargo.toml
[dependencies]
axum = { version = "0.8.8", features = ["macros"] }
axum-extra = { version = "0.12.5", features = ["cookie"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip", "timeout"] }
libsql = "0.9.30"
jsonwebtoken = "10.3.0"
password-auth = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
anyhow = "1"
thiserror = "2"
validator = { version = "0.20", features = ["derive"] }
dotenvy = "0.15"
tower = "0.5"
```

---

## Architecture Patterns

### Recommended Project Structure

```
backend/
├── src/
│   ├── main.rs              # Startup: load env, build AppState, wire router, serve
│   ├── config.rs            # Config struct populated from env vars
│   ├── state.rs             # AppState struct (Arc-wrapped for handlers)
│   ├── db/
│   │   ├── mod.rs           # Database init, migration runner
│   │   └── migrations/      # Embedded SQL migration files (mod.rs includes them)
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── handlers.rs      # POST /api/v1/auth/login, /refresh, /logout
│   │   ├── middleware.rs    # JWT validation tower middleware
│   │   ├── rbac.rs          # Role enum, require_role() extractor
│   │   └── service.rs       # password_hash, verify, jwt_issue, jwt_verify
│   ├── employees/
│   │   ├── mod.rs
│   │   ├── handlers.rs      # CRUD handlers
│   │   ├── service.rs       # Business logic (soft delete, validation)
│   │   └── models.rs        # Employee struct, CreateEmployeeDto, etc.
│   ├── departments/
│   │   ├── mod.rs
│   │   ├── handlers.rs
│   │   ├── service.rs
│   │   └── models.rs
│   ├── rules/
│   │   ├── mod.rs
│   │   ├── handlers.rs      # GET/PATCH /api/v1/rules
│   │   └── models.rs
│   ├── audit/
│   │   └── mod.rs           # AuditLog model for query/read (writes via triggers)
│   └── errors.rs            # AppError enum with IntoResponse
frontend/
├── src/
│   ├── app/
│   │   ├── layout.tsx
│   │   ├── page.tsx          # Redirects to /dashboard or /setup
│   │   ├── setup/
│   │   │   └── page.tsx      # Setup wizard (admin creation)
│   │   └── (dashboard)/
│   │       └── ...           # Protected pages (Phase 4+)
│   ├── middleware.ts          # Intercept requests: check /api/v1/setup/status
│   └── lib/
│       └── api.ts            # TanStack Query client + axios/fetch base
```

### Pattern 1: AppState — Shared Database Handle

```rust
// Source: Axum docs https://docs.rs/axum/latest/axum/extract/struct.State.html
use std::sync::Arc;
use libsql::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Config>,
}

// In main.rs:
let db = libsql::Builder::new_remote_replica(
    "cronometrix.db",
    std::env::var("TURSO_DATABASE_URL").unwrap(),
    std::env::var("TURSO_AUTH_TOKEN").unwrap(),
)
.sync_interval(std::time::Duration::from_secs(300))
.read_your_writes(true)  // default; makes writes immediately visible
.build()
.await?;

// Sync on startup before accepting requests
db.sync().await?;

let state = Arc::new(AppState {
    db: Arc::new(db),
    config: Arc::new(config),
});

let app = Router::new()
    .nest("/api/v1", api_router())
    .with_state(state);
```

[VERIFIED: docs.turso.tech/sdk/rust/reference 2026-04-11] `sync_interval` and `read_your_writes` are confirmed API options.

### Pattern 2: Custom Error Type with IntoResponse

```rust
// src/errors.rs
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound { code: &'static str, message: String },
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict")]
    Conflict { code: &'static str, message: String },
    #[error("validation failed")]
    Validation { code: &'static str, message: String },
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound { code, message } => (StatusCode::NOT_FOUND, *code, message.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Authentication required".into()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN", "Insufficient permissions".into()),
            AppError::Conflict { code, message } => (StatusCode::CONFLICT, *code, message.clone()),
            AppError::Validation { code, message } => (StatusCode::UNPROCESSABLE_ENTITY, *code, message.clone()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "An unexpected error occurred".into()),
        };
        let body = Json(json!({
            "error": { "code": code, "message": message, "status": status.as_u16() }
        }));
        (status, body).into_response()
    }
}
```

[CITED: leapcell.io/blog/elegant-error-handling-in-axum-actix-web-with-intoresponse]

### Pattern 3: JWT Claims with Role

```rust
// src/auth/service.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Supervisor,
    Viewer,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // user_id (UUID string)
    pub role: Role,
    pub exp: i64,          // epoch seconds
    pub iat: i64,
}

// Use HS256 with secret from env — appropriate for on-premise single-secret deployment
pub fn issue_access_token(user_id: &str, role: Role, secret: &[u8]) -> Result<String, AppError> {
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: user_id.to_owned(),
        role,
        exp: (now + chrono::Duration::minutes(20)).timestamp(),
        iat: now.timestamp(),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret),
    ).map_err(|e| AppError::Internal(e.into()))
}
```

[VERIFIED: cargo registry — jsonwebtoken 10.3.0 2026-04-11]

### Pattern 4: RBAC Tower Middleware

```rust
// src/auth/middleware.rs
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use axum_extra::extract::CookieJar;

pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Check Authorization: Bearer <token> header
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = verify_access_token(token, state.config.jwt_secret.as_bytes())?;
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

// Per-router-group application (in router.rs):
let admin_routes = Router::new()
    .route("/employees", post(create_employee))
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_admin));

let viewer_routes = Router::new()
    .route("/employees", get(list_employees))
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_auth));
```

[CITED: docs.logto.io/api-protection/rust/axum]

### Pattern 5: Database Migration at Startup

```rust
// src/db/mod.rs
// Migrations embedded as const strings in order — no external migration tool needed
const MIGRATIONS: &[(&str, &str)] = &[
    ("001_initial_schema", include_str!("migrations/001_initial_schema.sql")),
    ("002_audit_triggers", include_str!("migrations/002_audit_triggers.sql")),
];

pub async fn run_migrations(conn: &libsql::Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        (),
    ).await?;

    for (name, sql) in MIGRATIONS {
        let count: i64 = conn
            .query("SELECT COUNT(*) FROM _migrations WHERE name = ?1", [*name])
            .await?
            .next().await?.unwrap()
            .get(0)?;

        if count == 0 {
            conn.execute_batch(sql).await?;
            conn.execute(
                "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
                [*name, &chrono::Utc::now().timestamp().to_string()],
            ).await?;
            tracing::info!("Applied migration: {}", name);
        }
    }
    Ok(())
}
```

[ASSUMED] No libsql-compatible migration runner was verified as battle-tested for this version. The embedded-SQL pattern is widely used in Rust projects without external tooling dependency.

### Pattern 6: SQLite Audit Triggers (migration file)

```sql
-- migrations/002_audit_triggers.sql
-- Audit log table — append-only, never updated or deleted
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,              -- UUID v4
    table_name TEXT NOT NULL,
    record_id TEXT NOT NULL,
    operation TEXT NOT NULL,          -- 'INSERT' | 'UPDATE' | 'DELETE'
    old_data TEXT,                    -- JSON snapshot of old row (NULL for INSERT)
    new_data TEXT,                    -- JSON snapshot of new row (NULL for DELETE)
    actor_id TEXT,                    -- user_id who made the change (NULL for trigger-only ops)
    created_at INTEGER NOT NULL       -- UTC epoch
);

-- Trigger: employees INSERT
CREATE TRIGGER IF NOT EXISTS audit_employees_insert
    AFTER INSERT ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name, 'department_id', NEW.department_id, 'status', NEW.status),
        NULL,
        unixepoch()
    );
END;

-- Trigger: employees UPDATE
CREATE TRIGGER IF NOT EXISTS audit_employees_update
    AFTER UPDATE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name, 'department_id', OLD.department_id, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name, 'department_id', NEW.department_id, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

-- Same pattern for departments, global_rules, users
-- Note: actor_id can be patched by application via a temp table or session variable pattern
-- but for Phase 1, NULL actor_id is acceptable since triggers fire automatically
```

[CITED: til.simonwillison.net/sqlite/json-audit-log] — JSON snapshot approach verified
[CITED: bytefish.de/blog/sqlite_logging_changes.html] — versioning pattern verified

**UUID generation in SQLite:** libSQL (which is a SQLite extension) does not have a built-in `uuid()` function. The `hex(randomblob(N))` pattern above generates UUID v4-compatible strings within triggers without external functions.

### Pattern 7: Setup Wizard — First Boot Detection

```typescript
// frontend/src/middleware.ts
import { NextRequest, NextResponse } from 'next/server'

export async function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl

  // Never intercept setup page itself or API routes
  if (pathname.startsWith('/setup') || pathname.startsWith('/api')) {
    return NextResponse.next()
  }

  // Check backend for setup status
  const res = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/api/v1/setup/status`)
  const { initialized } = await res.json()

  if (!initialized) {
    return NextResponse.redirect(new URL('/setup', req.url))
  }

  return NextResponse.next()
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
}
```

Backend endpoint: `GET /api/v1/setup/status` returns `{"initialized": true/false}` based on COUNT(*) of users table. This endpoint must be public (no auth required).

[ASSUMED] The Next.js middleware fetch-to-backend approach works; however, if the backend is unreachable (startup race), the middleware needs a fallback (treat as uninitialized or serve an error page).

### Anti-Patterns to Avoid

- **Storing JWT in localStorage:** XSS-accessible; always use httpOnly cookies for refresh tokens
- **Hard-deleting records:** Any DELETE SQL that removes rows — use status = 'inactive' + deleted_at
- **Global Mutex on libSQL connection:** libSQL connections are already async-safe; wrapping in Mutex causes deadlock under concurrent requests
- **Bypassing trigger audit log with raw SQL patches:** All schema mutations must go through the trigger-covered tables
- **PRAGMA statements over Turso remote connection:** Will fail — apply WAL and foreign_keys PRAGMAs on the local connection before enabling sync
- **Returning raw database error messages to the client:** Expose only the AppError JSON structure; log the raw error with tracing

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Password hashing | Custom Argon2 parameters | `password-auth` crate | PHC string format, timing-safe comparison, stable argon2 version |
| JWT encode/decode | Custom HMAC token | `jsonwebtoken` 10.3.0 | Handles header, claims, signature, expiry — all edge cases |
| httpOnly cookie management | Manual Set-Cookie headers | `axum-extra` CookieJar | Handles secure flag, path, max-age, SameSite correctly |
| Audit log | Application-layer audit writes | SQLite AFTER triggers | Cannot be bypassed by code bugs or missing function calls |
| UUID generation | Incremental IDs or custom IDs | `uuid::Uuid::new_v4()` | Globally unique, safe for Turso multi-install sync, no collision |
| Env config | Reading env vars inline | `dotenvy` + Config struct | Validates all required vars at startup; fails fast on missing config |
| Optimistic concurrency | Custom locking | Version integer column | libSQL embedded replica needs conflict detection; version column is the standard pattern |

**Key insight:** Every item in this table has caused production incidents when hand-rolled. The libraries exist precisely because the edge cases are non-obvious.

---

## Common Pitfalls

### Pitfall 1: axum-extra Cookie Feature Not Enabled

**What goes wrong:** `axum_extra::extract::CookieJar` fails to compile — "use of undeclared crate or module `cookie`"
**Why it happens:** The `cookie` feature must be explicitly enabled: `axum-extra = { version = "0.12.5", features = ["cookie"] }`
**How to avoid:** Add `features = ["cookie"]` in Cargo.toml
**Warning signs:** Compile error referencing `axum_extra::extract::cookie`

### Pitfall 2: tower-http Version Mismatch with axum 0.8

**What goes wrong:** Compile error — `the trait bound Service<...> is not satisfied`
**Why it happens:** `tower-http 0.5.x` is for axum 0.7; axum 0.8 requires `tower-http 0.6.x`
**How to avoid:** Specify `tower-http = "0.6"` explicitly; do not let Cargo resolve to 0.5
**Warning signs:** Version conflict warnings during `cargo build`

### Pitfall 3: libSQL Turso Sync — PRAGMA Statements Fail Over Remote

**What goes wrong:** `PRAGMA foreign_keys = ON` or `PRAGMA journal_mode = WAL` returns error when sent over Turso remote connection
**Why it happens:** Turso's remote protocol doesn't proxy PRAGMA commands
**How to avoid:** Apply all PRAGMAs through a local connection before enabling remote replica mode; libSQL embedded replicas use WAL by default
**Warning signs:** `unsupported statement` error from libsql on startup

### Pitfall 4: actor_id Missing from Audit Triggers

**What goes wrong:** Audit log shows operation but actor_id is always NULL — no traceability to who made the change
**Why it happens:** SQLite triggers have no access to application session context
**How to avoid:** Two options: (a) accept NULL actor_id in triggers and have application write a secondary audit_log entry with actor context; (b) use a temp table session variable written before each mutation. For Phase 1, approach (a) is pragmatic — the trigger guarantees the record exists; actor attribution can be a secondary write in the service layer.
**Warning signs:** NULL actor_id in all audit_log rows

### Pitfall 5: Refresh Token Rotation Not Implemented

**What goes wrong:** Refresh token reuse after logout — user logs out but can still get access tokens using the old refresh token in the cookie
**Why it happens:** Stateless refresh tokens with no server-side invalidation
**How to avoid:** Store a `refresh_token_hash` in the users table; on use, verify hash matches and immediately rotate (delete old, issue new). On logout, clear the hash.
**Warning signs:** Tokens still valid after logout in manual testing

### Pitfall 6: Setup Wizard Race Condition

**What goes wrong:** Two browser tabs simultaneously hit `GET /api/v1/setup/status` (both get false), then both POST to create admin — second POST fails with opaque DB error
**Why it happens:** Status check and admin creation are not atomic
**How to avoid:** Put a UNIQUE constraint on the users table's role='Admin' (or simply count via SELECT), and return 409 Conflict if an admin already exists when POST /setup is called. The frontend should handle 409 gracefully ("Setup already completed, please log in").
**Warning signs:** Duplicate admin creation in test environment

### Pitfall 7: Optimistic Concurrency Check Missing

**What goes wrong:** Lost update — two admins edit the same employee simultaneously; second save silently overwrites first
**Why it happens:** UPDATE without checking version column
**How to avoid:** All UPDATE queries must include `WHERE id = ? AND version = ?`; if 0 rows affected, return 409 Conflict with code `VERSION_CONFLICT`
**Warning signs:** Missing audit trail entries that should exist

---

## Code Examples

### Startup Sequence (main.rs)

```rust
// Source: Axum docs https://docs.rs/axum/latest/axum/fn.serve.html
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env before anything else
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;  // Fails fast if required vars missing

    // Build libSQL embedded replica
    let db = libsql::Builder::new_remote_replica(
        &config.db_path,
        config.turso_url.clone(),
        config.turso_token.clone(),
    )
    .sync_interval(std::time::Duration::from_secs(300))
    .build()
    .await
    .context("Failed to open libSQL database")?;

    // Initial sync from Turso before accepting requests
    db.sync().await.context("Initial Turso sync failed")?;

    // Run migrations
    let conn = db.connect()?;
    crate::db::run_migrations(&conn).await?;

    let state = Arc::new(AppState { db: Arc::new(db), config: Arc::new(config) });

    let app = Router::new()
        .nest("/api/v1", api_router(state.clone()))
        .layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(tower_http::cors::CorsLayer::permissive())
                .layer(tower_http::timeout::TimeoutLayer::new(
                    std::time::Duration::from_secs(30)
                )),
        );

    let listener = tokio::net::TcpListener::bind(&format!("0.0.0.0:{}", state.config.port))
        .await
        .context("Failed to bind TCP listener")?;

    tracing::info!("Listening on port {}", state.config.port);
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Password Hash + Verify

```rust
// Source: RustCrypto password-auth https://github.com/RustCrypto/password-hashes/tree/master/password-auth
use password_auth::{generate_hash, verify_password};

pub fn hash_password(password: &str) -> String {
    generate_hash(password)  // Uses Argon2id by default
}

pub fn verify_password_hash(password: &str, hash: &str) -> Result<(), AppError> {
    verify_password(password, hash)
        .map_err(|_| AppError::Unauthorized)
}
```

### ISO 8601 Serialization from Epoch

```rust
// src/models.rs — epoch stored in DB, ISO 8601 in responses
use chrono::{DateTime, Utc};
use serde::Serialize;

fn epoch_to_iso(epoch: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}
```

### Global Rules — Singleton Table Pattern

```sql
-- Schema for global_rules — exactly one row
CREATE TABLE IF NOT EXISTS global_rules (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    late_arrival_tolerance_min INTEGER NOT NULL DEFAULT 10,
    early_departure_tolerance_min INTEGER NOT NULL DEFAULT 10,
    bonus_minutes INTEGER NOT NULL DEFAULT 0,
    effective_from INTEGER NOT NULL,  -- UTC epoch: rules apply to cycles AFTER this timestamp
    version INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL
);

-- Seed on first migration
INSERT OR IGNORE INTO global_rules (id, late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, effective_from, version, updated_at)
VALUES ('singleton', 10, 10, 0, unixepoch(), 1, unixepoch());
```

Note: `effective_from` implements RULE-03 (changes take effect on next calculation cycle). Phase 3 will use the `effective_from` timestamp to select which rule version applies to each attendance calculation period.

---

## Database Schema Reference

```sql
-- Core entities for Phase 1
-- All timestamps: UTC epoch integers
-- All IDs: UUID v4 strings

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'supervisor', 'viewer')),
    refresh_token_hash TEXT,          -- nullable; cleared on logout
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,               -- UTC epoch; NULL if active
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS departments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    base_salary_cents INTEGER NOT NULL DEFAULT 0,
    shift_start_time TEXT NOT NULL,   -- "HH:MM" in 24h format
    shift_end_time TEXT NOT NULL,     -- "HH:MM" in 24h format
    lunch_mode TEXT NOT NULL CHECK(lunch_mode IN ('fixed', 'punch')),
    lunch_duration_min INTEGER,       -- non-null when lunch_mode = 'fixed'
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS employees (
    id TEXT PRIMARY KEY,
    employee_code TEXT NOT NULL UNIQUE,   -- business ID (e.g., "EMP-001")
    name TEXT NOT NULL,
    department_id TEXT NOT NULL REFERENCES departments(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for common filter patterns
CREATE INDEX IF NOT EXISTS idx_employees_department ON employees(department_id);
CREATE INDEX IF NOT EXISTS idx_employees_status ON employees(status);
CREATE INDEX IF NOT EXISTS idx_employees_name ON employees(name);
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| bcrypt password hashing | argon2id (via `password-auth`) | ~2020 (OWASP) | bcrypt has 72-byte limit + slower than argon2id; PHC competition winner |
| axum 0.7 with tower-http 0.5 | axum 0.8 with tower-http 0.6 | January 2025 | Breaking API changes; `axum::serve()` replaces `Server::bind()` |
| react-query v3/v4 | @tanstack/react-query v5 | 2024 | Improved TypeScript inference, streaming, App Router SSR hydration |
| next-auth v4 | next-auth v5 (Auth.js) or custom JWT middleware | 2024 | v4 was Pages Router only; v5 is App Router compatible |
| RS256 JWT for all apps | HS256 for on-premise, RS256 for multi-tenant | Ongoing | HS256 is appropriate when the signing secret never leaves the server |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Embedded SQL migration strings (no external runner) are sufficient for this project | Architecture Patterns, Pattern 5 | libsql_migration 0.2.2 may be needed if embedded approach has libsql async compat issues |
| A2 | Next.js middleware fetch to backend is performant enough for setup redirect check | Architecture Patterns, Pattern 7 | Adds ~20ms to every request if backend is local; negligible for admin tools |
| A3 | `actor_id = NULL` in triggers is acceptable for Phase 1 with secondary service-layer audit write | Pitfall 4 | Audit trail lacks actor attribution if secondary write is forgotten; enforce in service layer code review |
| A4 | HS256 JWT with env secret is appropriate security for on-premise single-installation deployment | Pattern 3 | If multi-tenant cloud SaaS expansion happens, must migrate to RS256 |
| A5 | libSQL `execute_batch()` works for DDL + trigger creation in a single migration file | Pattern 5 | If execute_batch requires statement splitting, migration code needs adjustment |

---

## Open Questions (RESOLVED)

1. **Turso sync when Turso is unavailable at startup**
   - What we know: `db.sync().await` can fail if TURSO_DATABASE_URL is unreachable
   - What's unclear: Should startup fail hard, or should the service start with local-only mode and retry sync in background?
   - Recommendation: Start in degraded mode (local only) if sync fails; log warning; background retry with tokio::spawn. DATA-04 only requires local SQLite to be authoritative, which holds.
   - RESOLVED: Degraded local-only mode with background retry (implemented in Plan 01-01 init_db_local)

2. **actor_id attribution in SQLite triggers**
   - What we know: Triggers have no session context; they fire on SQL events without application-layer metadata
   - What's unclear: Whether to implement a `temp.actor_id` session variable table or accept service-layer double-write
   - Recommendation: Use double-write for Phase 1 (trigger creates the row, service layer updates actor_id in the same transaction). This is a single atomic transaction so no partial-write risk.
   - RESOLVED: NULL actor_id for Phase 1 triggers; service-layer double-write updates actor_id in same transaction

3. **libSQL connection pooling**
   - What we know: libsql `Database::connect()` is cheap to call; each connection is independent
   - What's unclear: Whether to pool connections or create per-request connections
   - Recommendation: Create a connection per request from the shared `Arc<Database>`. libSQL embedded replica does not benefit from connection pooling the way network databases do — the WAL file handles concurrency.
   - RESOLVED: Per-request connection from Arc<Database> via state.db.connect() in handlers

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Backend compilation | Yes | 1.93.0 (stable) | — |
| Cargo | Dependency management | Yes | 1.93.0 | — |
| Node.js | Frontend build | Yes | v24.13.0 | — |
| npm | Frontend packages | Yes | 11.6.2 | — |
| Docker | Container packaging | Yes | 29.2.0 | — |
| Turso cloud | DATA-02 sync | Unknown | — | Local-only mode (DATA-03 says local is authoritative) |
| TURSO_DATABASE_URL env | libSQL sync | Unknown | — | Local-only mode acceptable for dev |
| TURSO_AUTH_TOKEN env | libSQL sync | Unknown | — | Local-only mode acceptable for dev |

**Missing dependencies with no fallback:** None that block Phase 1 development. Turso sync requires credentials but can be omitted during local dev by using `Builder::new_local()` instead of `new_remote_replica()`.

**Developer action needed:** Set `TURSO_DATABASE_URL` and `TURSO_AUTH_TOKEN` in `.env` before running with sync enabled. For local dev without Turso, the Config struct should detect missing Turso vars and fall back to local-only mode.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest (Rust) + Vitest (frontend) |
| Config file | No config file — `cargo nextest run` uses default discovery |
| Quick run command | `cargo nextest run` |
| Full suite command | `cargo nextest run --all-features` |
| Frontend quick | `cd frontend && npx vitest run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DATA-01 | SQLite file created on startup | Integration | `cargo nextest run db::tests` | No — Wave 0 |
| DATA-02 | Turso sync called without panic | Integration (mocked) | `cargo nextest run db::sync_tests` | No — Wave 0 |
| DATA-04 | Audit trigger fires on employee update | Integration | `cargo nextest run audit::tests` | No — Wave 0 |
| AUTH-01 | Login returns access token + sets refresh cookie | Integration | `cargo nextest run auth::tests::test_login` | No — Wave 0 |
| AUTH-05 | Refresh endpoint issues new access token from cookie | Integration | `cargo nextest run auth::tests::test_refresh` | No — Wave 0 |
| EMP-01 | POST /api/v1/employees creates employee | Integration | `cargo nextest run employees::tests::test_create` | No — Wave 0 |
| EMP-03 | DELETE /api/v1/employees/:id sets status=inactive | Integration | `cargo nextest run employees::tests::test_soft_delete` | No — Wave 0 |
| DEPT-01 | POST /api/v1/departments creates department | Integration | `cargo nextest run departments::tests::test_create` | No — Wave 0 |
| RULE-01 | PATCH /api/v1/rules updates tolerance margins | Integration | `cargo nextest run rules::tests::test_update` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run` (full backend suite — fast with nextest)
- **Per wave merge:** `cargo nextest run --all-features`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `backend/tests/common/mod.rs` — shared test fixtures (in-memory libSQL, test AppState)
- [ ] `backend/tests/db_tests.rs` — migration and audit trigger tests
- [ ] `backend/tests/auth_tests.rs` — login, refresh, RBAC tests
- [ ] `backend/tests/employee_tests.rs` — CRUD + soft delete tests
- [ ] `backend/tests/department_tests.rs` — CRUD tests
- [ ] `backend/tests/rules_tests.rs` — global rules update tests
- [ ] `backend/Cargo.toml` — add `cargo-nextest` to dev dependencies note

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Yes | Argon2id via `password-auth`; minimum 8 chars; no complexity (NIST 800-63B) |
| V3 Session Management | Yes | Access token in memory (15-30 min); refresh token in httpOnly + Secure + SameSite=Strict cookie (7 days); server-side refresh_token_hash for revocation |
| V4 Access Control | Yes | 3-role RBAC enforced at Axum router group via Tower middleware; never trust client-supplied role |
| V5 Input Validation | Yes | `validator` crate on all DTOs; Zod on frontend; reject unknown fields via serde `deny_unknown_fields` |
| V6 Cryptography | Yes | JWT HS256 with >= 256-bit secret from env; never use default/weak secret; argon2id for passwords |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via employee name/filter | Tampering | libsql parameterized queries only; never string-interpolate SQL |
| JWT secret hardcoded | Information Disclosure | Load from env via dotenvy; fail fast if `JWT_SECRET` missing or < 32 chars |
| Refresh token theft via XSS | Elevation of Privilege | httpOnly cookie; access token in memory only (not localStorage) |
| RBAC bypass by omitting middleware | Elevation of Privilege | Apply `route_layer` at router group level, not per-handler — prevents accidental omission |
| Setup wizard replay attack | Elevation of Privilege | `POST /api/v1/setup` returns 409 if admin already exists (UNIQUE constraint + explicit check) |
| Audit log deletion | Repudiation | No DELETE endpoint or SQL for audit_log table; no trigger on audit_log table itself |
| Version conflict lost update | Tampering | All PATCH/PUT handlers check `WHERE id = ? AND version = ?`; return 409 on mismatch |

---

## Project Constraints (from CLAUDE.md)

The following directives from CLAUDE.md are binding on all planning and implementation:

| Directive | Impact on Phase 1 |
|-----------|-------------------|
| Rust with Axum backend | No deviation to other frameworks or languages |
| axum 0.8.8 (current) | tower-http must be 0.6.x; axum::serve() not Server::bind() |
| libsql (latest = 0.9.30) with embedded replica mode | Builder::new_remote_replica() API |
| jsonwebtoken 10.3.0 | API changed from 8.x; use EncodingKey::from_secret() |
| argon2 (RustCrypto) 0.5.x | Use password-auth wrapper — argon2 0.6.x is RC in registry |
| No diesel ORM | Raw libsql queries with typed structs only |
| No actix-web or warp | Axum only |
| chrono 0.4.x | For epoch ↔ ISO 8601 conversion |
| Next.js 15.x App Router | No Pages Router patterns |
| react-hook-form 7.x + zod 3.x | Form state and validation |
| shadcn/ui | Component library for setup wizard |
| tailwindcss 4.x | CSS utility (different config format from v3) |
| Audit compliance | Every mutation → immutable audit_log entry (via triggers) |
| UTC epoch integers in DB | All created_at, updated_at, deleted_at columns |
| Tauri future-compatible | Keep business logic in Rust, not Next.js Server Actions |

---

## Sources

### Primary (HIGH confidence)

- [crates.io cargo registry](https://crates.io) — all crate versions verified 2026-04-11 via `cargo search`
- [docs.turso.tech/sdk/rust/reference](https://docs.turso.tech/sdk/rust/reference) — libsql Builder API, sync_interval, read_your_writes confirmed
- [docs.rs/axum/latest/axum/](https://docs.rs/axum/latest/axum/) — axum 0.8.8 API
- [docs.rs/axum-extra/latest/axum_extra/extract/cookie/](https://docs.rs/axum-extra/latest/axum_extra/extract/cookie/) — CookieJar extractor confirmed
- CLAUDE.md — full stack table with locked crate versions

### Secondary (MEDIUM confidence)

- [leapcell.io/blog/elegant-error-handling-in-axum-actix-web-with-intoresponse](https://leapcell.io/blog/elegant-error-handling-in-axum-actix-web-with-intoresponse) — IntoResponse pattern with thiserror
- [til.simonwillison.net/sqlite/json-audit-log](https://til.simonwillison.net/sqlite/json-audit-log) — SQLite JSON audit log trigger pattern
- [bytefish.de/blog/sqlite_logging_changes.html](https://www.bytefish.de/blog/sqlite_logging_changes.html) — SQLite versioning/history trigger pattern
- [codevoweb.com/rust-and-axum-jwt-access-and-refresh-tokens/](https://codevoweb.com/rust-and-axum-jwt-access-and-refresh-tokens/) — JWT + refresh token + httpOnly cookie pattern
- [docs.logto.io/api-protection/rust/axum](https://docs.logto.io/api-protection/rust/axum) — RBAC middleware for Axum

### Tertiary (LOW confidence)

- [github.com/tursodatabase/embedded-replica-examples](https://github.com/tursodatabase/embedded-replica-examples) — embedded replica usage patterns (not directly fetched)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against cargo registry 2026-04-11
- Architecture: HIGH — patterns verified against official docs; one ASSUMED (migration approach)
- Pitfalls: MEDIUM — some from training knowledge + corroborated by community sources
- Security domain: HIGH — ASVS categories apply directly to stack choices

**Research date:** 2026-04-11
**Valid until:** 2026-05-11 (stable ecosystem; axum 0.9 release would require re-check)
