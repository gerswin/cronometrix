# Architecture

**Analysis Date:** 2026-04-19

## Pattern Overview

**Overall:** Layered API with JWT-based authentication, local-first cloud sync, and role-based access control

**Key Characteristics:**
- Axum HTTP server with Tower middleware for CORS, tracing, compression, and timeout handling
- Stateless JWT authentication (access token in header, refresh token in httpOnly cookie)
- Embedded SQLite with periodic Turso cloud sync (authoritative local replica pattern)
- Separate router groups for public, authenticated viewer, supervisor+, and admin-only endpoints
- Typed error responses via thiserror + IntoResponse pattern
- Frontend-backend separation: Next.js SSR/SPA with TanStack Query for server state management

## Layers

**HTTP & Routing (Axum):**
- Purpose: HTTP request handling, routing, middleware orchestration
- Location: `backend/src/main.rs`
- Contains: Router setup, route handlers, middleware composition
- Depends on: Tower, Tower-HTTP, all handler modules
- Used by: Tokio runtime, clients (frontend, biometric devices)

**Middleware (Tower):**
- Purpose: Cross-cutting concerns (CORS, tracing, compression, timeout)
- Location: `backend/src/main.rs` (TraceLayer, CorsLayer, timeout via tower-http)
- Contains: Axum middleware stack composition
- Depends on: Tower-HTTP
- Used by: Axum router

**Authentication & Authorization:**
- Purpose: JWT validation, role-based access control
- Location: `backend/src/auth/` (middleware, RBAC, service, models)
- Contains:
  - `middleware.rs`: Extracts and validates Bearer token from header
  - `rbac.rs`: Enforces role checks (Admin, Supervisor, Viewer)
  - `service.rs`: Token generation, password hashing, verification
  - `models.rs`: Claims, LoginRequest/Response, UserInfo, Role enum
- Depends on: jsonwebtoken, password-auth, serde, chrono
- Used by: Route handlers via extractor and middleware layers

**Database (libSQL + Turso):**
- Purpose: Local and cloud data persistence
- Location: `backend/src/db/mod.rs`
- Contains:
  - `init_db()`: Routes to local or remote replica based on config
  - `init_db_local()`: Dev-only SQLite mode
  - `init_db_remote()`: Embedded replica with Turso sync
  - `run_migrations()`: Applies SQL migrations from `backend/src/db/migrations/`
- Depends on: libsql, chrono (for timestamps)
- Used by: Handler service layers, AppState

**Domain Services:**
- Purpose: Business logic for each domain (employees, departments, rules, auth, setup)
- Locations:
  - `backend/src/employees/service.rs` - CRUD operations for employees
  - `backend/src/departments/service.rs` - CRUD operations for departments
  - `backend/src/rules/` - GET and PATCH for singleton global rules table
  - `backend/src/auth/service.rs` - Token/password operations
  - `backend/src/setup/handlers.rs` - First-time admin initialization
- Contains: SQL queries, validation logic, error handling
- Depends on: libsql Connection, models, errors
- Used by: Handlers (HTTP request-response layer)

**Handlers (HTTP Request/Response):**
- Purpose: Parse requests, invoke services, format responses
- Locations: `backend/src/{module}/handlers.rs`
- Contains:
  - `employees/handlers.rs` - create, list, get, update, deactivate
  - `departments/handlers.rs` - create, list, get, update
  - `rules/handlers.rs` - get, update (singleton)
  - `auth/handlers.rs` - login, refresh, logout
  - `setup/handlers.rs` - status check, initial admin creation
- Depends on: Axum extractors, service layers, errors
- Used by: Axum router

**Error Handling:**
- Purpose: Standardized error responses
- Location: `backend/src/errors.rs`
- Contains: AppError enum (NotFound, Unauthorized, Forbidden, Conflict, Validation, Internal) implementing IntoResponse
- Depends on: thiserror, axum, serde_json
- Used by: All handler and service functions

**State Management:**
- Purpose: Shared application state (database, config)
- Location: `backend/src/state.rs`
- Contains: AppState struct with Arc<Database> and Arc<Config>
- Depends on: libsql, config module
- Used by: Axum State extractor in all handlers

**Configuration:**
- Purpose: Environment-based settings
- Location: `backend/src/config.rs`
- Contains: Config struct with database_path, turso_url, turso_token, jwt_secret, server_host, server_port, turso_sync_interval
- Depends on: dotenvy, anyhow
- Used by: main.rs, db/mod.rs

**Frontend (Next.js):**
- Purpose: Admin UI for time & attendance data management
- Location: `frontend/src/`
- Contains: App Router pages, components, API client, validations
- Depends on: React, Next.js, TanStack Query, React Hook Form, Zod, shadcn/ui
- Used by: Browser clients

## Data Flow

**Login Flow:**

1. User submits username + password via POST `/auth/login` form
2. Handler validates request body (LoginRequest with Validator derive)
3. Service queries users table: `SELECT id, username, full_name, password_hash, role FROM users WHERE username = ? AND status = 'active'`
4. Service verifies password: `password-auth::verify_password()` compares plaintext against Argon2id hash (timing-safe)
5. Service generates tokens:
   - Access token: JWT with sub (user_id), role, exp (20 min), iat, token_type="access", signed with HS256
   - Refresh token: JWT with same claims, exp (7 days), token_type="refresh"
6. Service hashes refresh token with SHA-256 and stores hash in DB: `UPDATE users SET refresh_token_hash = ?, updated_at = ? WHERE id = ?`
7. Response: HTTP 200 with JSON body `{access_token, user: {id, username, full_name, role}}` + httpOnly refresh cookie
8. Frontend stores access_token in memory, browser stores refresh cookie automatically
9. Subsequent requests attach access_token via axios interceptor: `Authorization: Bearer {token}`

**Token Refresh Flow:**

1. Access token expires or returns 401 on next request
2. Axios response interceptor catches 401
3. Frontend auto-triggers POST `/auth/refresh` with credentials: true (sends refresh cookie)
4. Backend middleware validates refresh cookie token via `verify_refresh_token()`
5. Backend generates new access token and refresh token (both rotated)
6. Response: HTTP 200 with new access_token + new refresh cookie
7. Axios interceptor updates in-memory token and retries original request

**Employee List Flow:**

1. Frontend calls `GET /employees?limit=10&offset=0` with access token in header
2. Middleware `require_auth` validates Bearer token, inserts Claims into request extensions
3. Handler extracts authenticated state and query params
4. Service queries employees table with pagination: `SELECT ... FROM employees WHERE status = ? LIMIT ? OFFSET ?`
5. Service counts total: `SELECT COUNT(*) FROM employees WHERE status = ?`
6. Handler returns PaginatedResponse<Employee> JSON

**Audit Log Flow:**

1. Any INSERT, UPDATE, DELETE on employees/departments/users triggers audit log append (via 002_audit_triggers.sql)
2. Trigger captures table_name, record_id, operation, old_data (JSON), new_data (JSON), actor_id (TODO: from Claims), created_at
3. Audit log entry written atomically with the mutation
4. Audit log is append-only: no UPDATE or DELETE allowed (enforced by schema design, not triggers)

**State Management:**
- Shared AppState created once at startup: `AppState { db: Arc::new(db), config: Arc::new(config) }`
- Cloned via Arc to each handler's State extractor (cheap — Arc is just a pointer)
- Database connection obtained per handler: `state.db.connect()` (libSQL internally pools)

## Key Abstractions

**AppState:**
- Purpose: Dependency injection container for handlers
- Examples: `backend/src/state.rs`
- Pattern: Single clone-via-Arc passed to Axum router, then extracted in handlers via State<AppState> extractor

**Claims (JWT Payload):**
- Purpose: Represents decoded access/refresh token
- Examples: `backend/src/auth/models.rs`
- Pattern: Deserialized from JWT, validated in middleware, inserted into request extensions for downstream handler access

**PaginatedResponse<T>:**
- Purpose: Wrapper for list endpoints with metadata
- Examples: `backend/src/common.rs`
- Pattern: Generic over any Serialize type; includes data, total, limit, offset

**AppError (Error Handling):**
- Purpose: Typed errors that convert to HTTP responses
- Examples: `backend/src/errors.rs`
- Pattern: Enum variants map to specific HTTP status codes; IntoResponse impl serializes to JSON error body

**Role (RBAC):**
- Purpose: Represents user authorization level
- Examples: `backend/src/auth/models.rs`
- Pattern: Enum (Admin, Supervisor, Viewer) serialized in JWT; middleware checks role before invoking handler

## Entry Points

**Backend HTTP Server:**
- Location: `backend/src/main.rs` function `main()`
- Triggers: `cargo run` or Docker container startup
- Responsibilities:
  1. Load .env via dotenvy
  2. Initialize tracing (pretty or JSON output)
  3. Load Config from environment
  4. Initialize database (local or Turso remote)
  5. Create AppState with Arc-wrapped db and config
  6. Build Axum router with nested routes and middleware
  7. Bind TCP listener and serve

**Frontend:**
- Location: `frontend/src/app/layout.tsx` (Root Layout)
- Triggers: Browser navigation to any route
- Responsibilities:
  1. Wrap children with Providers (TanStack Query)
  2. Establish Inter font
  3. Set metadata and HTML structure

**Frontend Setup Check:**
- Location: `frontend/src/proxy.ts` (Next.js middleware via `config.matcher`)
- Triggers: On every request before route handler
- Responsibilities:
  1. Whitelist paths that don't require setup check (setup, api, _next)
  2. Check `GET /setup/status` endpoint
  3. Redirect unauthenticated users to `/setup` if not initialized
  4. Gracefully handle backend unreachable (show login, which will error)

**Frontend Login Page:**
- Location: `frontend/src/app/login/page.tsx`
- Triggers: User navigates to `/login` or middleware redirects after logout
- Responsibilities:
  1. Render login form (username, password)
  2. Submit POST `/auth/login` via axios
  3. Store access_token and user info in memory
  4. Redirect to dashboard on success (404 not found initially; Phase 02 adds dashboard)
  5. Show error message on 401 or network failure

**Frontend Setup Page:**
- Location: `frontend/src/app/setup/page.tsx`
- Triggers: On first visit if no users exist (middleware redirects)
- Responsibilities:
  1. Render setup form (full_name, username, password, confirm_password)
  2. Client-side validation via Zod schema
  3. Submit POST `/setup/init`
  4. Show success message and redirect to login

## Error Handling

**Strategy:** Typed errors with context, generic client-facing messages, detailed server logs

**Patterns:**

1. **Handler Layer:**
   ```rust
   body.validate().map_err(|e| AppError::Validation {
     code: "VALIDATION_ERROR",
     message: e.to_string(),
   })?;
   ```
   Validates request, returns 422 on failure

2. **Service Layer:**
   ```rust
   service::create(&conn, body).await?
   ```
   Uses `?` operator to propagate errors; caller handles specific error variants

3. **Database Errors:**
   ```rust
   conn.execute(...).await.map_err(|e| AppError::Internal(e.into()))?
   ```
   Wraps libSQL errors in Internal variant; logs details server-side, returns generic "internal error" to client

4. **Auth Errors:**
   ```rust
   let claims = service::verify_access_token(token, ...)?; // Returns Unauthorized on decode failure
   ```
   Rejects invalid/expired tokens with 401

## Cross-Cutting Concerns

**Logging:** 
- Tracing framework with subscriber
- Development: Pretty-printed text with colors
- Production: JSON structured logs via `tracing-subscriber::fmt().json()`
- Levels: info (startup, major ops), debug (migrations, middleware), warn (Turso sync failures), error (exceptions)
- Example: `tracing::info!("Cronometrix API listening on {}", addr)`

**Validation:** 
- Validator derive macros on request structs (length, range, custom rules)
- Client-side: Zod schemas in frontend (LoginFormData, SetupFormData)
- Server-side: Validator + Zod (for form pre-fill validation)

**Authentication:** 
- JWT via jsonwebtoken crate (HS256)
- Middleware-first: require_auth validates Bearer header before handler runs
- RBAC: Separate middleware functions for each role level (require_admin, require_supervisor_or_above)
- Token rotation: Access + refresh token issued on login, refresh rotates both on reuse

**Time & Timestamps:**
- All timestamps stored as UTC epoch integers (Unix seconds) per D-13 pattern
- Conversion to ISO 8601 in API responses via `epoch_to_iso()` helper (common.rs)
- Timezone-aware calculations via chrono (attendance tolerance windows)

**Concurrency:**
- Optimistic concurrency: version column on employees, departments, global_rules
- PATCH requests require version field to prevent lost updates (conflict detection)
- SQLite autoincrement handles serial version increments

---

*Architecture analysis: 2026-04-19*
