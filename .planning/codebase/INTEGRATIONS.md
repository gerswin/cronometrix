# External Integrations

**Analysis Date:** 2026-04-19

## APIs & External Services

**Cloud Database Sync:**
- Turso (SQLite cloud) - Remote database replication for cloud backup and remote access
  - SDK/Client: `libsql` 0.9.30 crate
  - Connection: Via `libsql::Builder::new_remote_replica()` in `backend/src/db/mod.rs`
  - Auth: `TURSO_DATABASE_URL`, `TURSO_AUTH_TOKEN` environment variables
  - Sync interval: Configurable via `TURSO_SYNC_INTERVAL` (default 300 seconds)
  - Local-first architecture: Local SQLite is authoritative; cloud is async replica (per DATA-03 pattern)

**Hikvision Biometric Devices (Planned):**
- ISAPI Protocol - Real-time attendance event webhook integration
  - SDK/Client: `reqwest` 0.13.2 HTTP client + `diqwest` (not yet in Cargo.toml) for digest auth
  - Auth: HTTP Digest authentication (RFC 2617) required by Hikvision devices
  - Inbound: `EventNotificationAlert` XML events (employee ID, face capture time, optional JPEG)
  - Outbound: Device commands (enroll profiles, door control, status checks)
  - Not yet implemented in Phase 01-Foundation — reserved for Phase 02-Biometrics

## Data Storage

**Databases:**
- SQLite 3 (local)
  - Connection: `libsql::Builder::new_local()` for dev-only mode; `new_remote_replica()` for production
  - Local path: `CRONOMETRIX_DB_PATH` environment variable (default `cronometrix.db`)
  - ORM/client: Raw libSQL queries via `libsql::Connection::query()` and `execute()`
  - Foreign keys: Enabled via `PRAGMA foreign_keys = ON` in `backend/src/db/mod.rs:45`
  - WAL mode: Enabled by default in libSQL embedded replicas

- Turso (remote replica)
  - URL: `TURSO_DATABASE_URL` (e.g., `libsql://your-db.turso.io`)
  - Token: `TURSO_AUTH_TOKEN` for authentication
  - Sync strategy: Periodic `db.sync()` every `TURSO_SYNC_INTERVAL` seconds
  - Graceful degradation: If sync fails, continues in local-only mode with warning log

**File Storage:**
- Local filesystem only - Face capture photos not yet implemented
- Reserved for Phase 02-Biometrics (Hikvision JPEG handling)

**Caching:**
- In-memory token cache (future) - TanStack Query on frontend handles HTTP cache
- No Redis/Memcached dependency in Phase 01
- SQLite query results cached by TanStack Query (5 minute stale time default)

## Authentication & Identity

**Auth Provider:**
- Custom JWT-based - No third-party SSO provider in Phase 01

**Implementation:**
- JWT encoding/decoding: `jsonwebtoken` 10.3.0 crate with HS256 (HMAC-SHA256)
- Secret: `JWT_SECRET` environment variable (required, min 32 characters)
- Token types:
  - Access token: 20-minute expiry, stored in memory (frontend), passed via `Authorization: Bearer` header
  - Refresh token: 7-day expiry, stored as httpOnly SameSite=Lax cookie (secure against XSS)
- Password hashing: Argon2id via `password-auth` crate (RustCrypto, OWASP-recommended)
- Password verification: Timing-safe comparison to prevent enumeration attacks

**Token Flow:**
1. `POST /auth/login` returns access token (JSON body) + refresh cookie (httpOnly, secure, SameSite=Lax, path=/api/v1/auth)
2. Access token used in subsequent requests via `Authorization: Bearer <token>` header (axios interceptor in frontend)
3. On 401 response, frontend auto-refreshes via `POST /auth/refresh` (cookie sent automatically by browser)
4. New access token issued; refresh token rotated and re-hashed in DB

## Monitoring & Observability

**Error Tracking:**
- None in Phase 01 - Plan for Sentry or similar in Phase 02

**Logs:**
- Structured logging via `tracing` 0.1 + `tracing-subscriber` 0.3
- Output format: Pretty text for development, JSON for production (configurable via `RUST_LOG` environment variable)
- Levels: info (default), debug (migrations, middleware), warn (Turso sync failures), error (internal server errors)
- Example log on startup: "Cronometrix API listening on 0.0.0.0:3001"
- Database connection pool: Implicit via libSQL (one connection per handler, pooled internally)

## CI/CD & Deployment

**Hosting:**
- Docker Compose on Linux server (one-command install via shell script)
- Cloudflare tunnel per client → `{client-slug}.cronometrix.com` (network routing, not in stack)
- No Kubernetes; single-container deployment per on-premises installation

**CI Pipeline:**
- None in Phase 01 - Plan for GitHub Actions or similar in Phase 02

**Build Artifacts:**
- Backend: `cargo build --release` produces statically-linked binary (via Tokio + Reqwest's TLS feature)
- Frontend: `next build` produces `.next/` output for production server

## Environment Configuration

**Required env vars (Backend):**
- `JWT_SECRET` - CRITICAL; min 32 characters; used for HS256 signing
- `TURSO_DATABASE_URL` - Cloud database URL (optional for local-only dev; empty string disables sync)
- `TURSO_AUTH_TOKEN` - Cloud database auth token (optional; must be set if TURSO_DATABASE_URL is set)
- `CRONOMETRIX_DB_PATH` - Local SQLite file path (default: `cronometrix.db`)
- `SERVER_HOST` - Bind address (default: `0.0.0.0`)
- `SERVER_PORT` - Listen port (default: `3001`)
- `TURSO_SYNC_INTERVAL` - Sync interval in seconds (default: `300`)
- `RUST_LOG` - Logging level (default: `info`; options: `debug`, `trace`)

**Required env vars (Frontend):**
- `NEXT_PUBLIC_API_URL` - Backend API base URL (default: `http://localhost:3001`; required for production builds)

**Secrets location:**
- `.env` file in backend root (git-ignored via `.gitignore`)
- Frontend env vars in `.env.local` or deployment platform's secret manager
- Never commit `.env` or any file containing `JWT_SECRET`, `TURSO_AUTH_TOKEN`

## Webhooks & Callbacks

**Incoming:**
- `POST /api/v1/devices/{device-id}/webhook` - Reserved for Hikvision ISAPI event delivery (Phase 02)
- `POST /api/v1/attendance/events` - Reserved for biometric event processing (Phase 02)

**Outgoing:**
- `PUT /ISAPI/AccessControl/UserInfo/SetUp` - Enroll employee face profile on device (Phase 02)
- `PUT /ISAPI/RemoteControl/door/0` - Remote door open command (Phase 02)
- `GET /ISAPI/System/status` - Health check device connectivity (Phase 02)
- `POST /ISAPI/Event/notification/httpHosts` - Configure device webhook URL (Phase 02)

## API Contract

**Base URL:** `/api/v1` (all endpoints prefixed with this)

**Response Format:**
- Success: JSON body with data + HTTP 200/201
- Error: Structured JSON error response (per `backend/src/errors.rs`):
  ```json
  {
    "error": {
      "code": "ERROR_CODE",
      "message": "Human readable message",
      "status": 404
    }
  }
  ```

**Authentication:**
- Public endpoints: `/health`, `/auth/login`, `/setup/status`, `/setup/init`
- Protected endpoints: Require `Authorization: Bearer <access_token>` header
- Role-based access control (RBAC) enforced at router level (see `backend/src/main.rs`)

**Public Routes (no auth):**
- `GET /health` - Database connectivity check
- `POST /auth/login` - Issue tokens
- `GET /setup/status` - Check if initialized
- `POST /setup/init` - Create first admin user

**Viewer+ Routes (any authenticated role):**
- `GET /employees` - List employees with pagination
- `GET /employees/:id` - Get employee details
- `GET /departments` - List departments
- `GET /departments/:id` - Get department details
- `GET /rules` - Get global rules

**Supervisor+ Routes:**
- `POST /employees` - Create employee
- `PATCH /employees/:id` - Update employee

**Admin-only Routes:**
- `DELETE /employees/:id` - Soft-delete (deactivate) employee
- `POST /departments` - Create department
- `PATCH /departments/:id` - Update department
- `PATCH /rules` - Update global rules

---

*Integration audit: 2026-04-19*
