---
phase: 01-foundation
plan: "02"
subsystem: auth
tags: [jwt, argon2id, rbac, axum, refresh-tokens, cookie, setup-wizard]

requires:
  - phase: 01-01
    provides: AppState, AppError, Config, DB migrations (users table with refresh_token_hash)

provides:
  - JWT authentication with access token (20min) + refresh token rotation (7d httpOnly cookie)
  - Argon2id password hashing via password-auth crate
  - Three-role RBAC middleware: require_auth, require_admin, require_supervisor_or_above
  - AuthUser extractor (FromRequestParts) for handler-level role access
  - Login / refresh / logout HTTP handlers
  - Setup wizard endpoints: GET /setup/status, POST /setup/init (first-boot admin creation)
  - All routes wired under /api/v1 in main.rs using Axum 0.8 {param} syntax

affects:
  - All subsequent plans requiring authenticated endpoints (01-03 employees, 01-04 departments)
  - Frontend plan that consumes access tokens and refresh cookies

tech-stack:
  added:
    - sha2 = "0.10" (SHA-256 hashing for refresh token storage)
    - time = "0.3" (cookie max_age duration for axum-extra)
    - jsonwebtoken rust_crypto feature (avoids rustls CryptoProvider requirement in tests)
  patterns:
    - Access token in Authorization Bearer header; refresh token in httpOnly SameSite=Lax cookie
    - Refresh tokens stored as SHA-256 hash in users.refresh_token_hash (never raw token)
    - Token rotation: every refresh call issues new access + refresh tokens and updates DB hash
    - RBAC via route_layer middleware applied at router group level (not per-handler)
    - Public routes, cookie-auth routes, and bearer-auth routes merged under /api/v1
    - Setup wizard blocks after first admin exists (SELECT COUNT + 409 Conflict)

key-files:
  created:
    - backend/src/auth/mod.rs
    - backend/src/auth/models.rs (Role enum, Claims, LoginRequest, LoginResponse, UserInfo)
    - backend/src/auth/service.rs (hash_password, verify_password, issue/verify tokens, hash_token)
    - backend/src/auth/middleware.rs (require_auth Tower middleware)
    - backend/src/auth/rbac.rs (require_admin, require_supervisor_or_above, AuthUser extractor)
    - backend/src/auth/handlers.rs (login, refresh, logout)
    - backend/src/setup/mod.rs
    - backend/src/setup/handlers.rs (setup_status, setup_init)
  modified:
    - backend/Cargo.toml (sha2, time, jsonwebtoken rust_crypto feature)
    - backend/src/lib.rs (expose auth and setup modules)
    - backend/src/main.rs (full router wiring with public + cookie-auth route groups)
    - backend/tests/auth_tests.rs (all 5 tests un-ignored and implemented)

key-decisions:
  - "SameSite=Lax (not Strict) on refresh cookie: allows navigation from third-party links (email, portals) while still blocking CSRF POST attacks — per review fix in plan"
  - "refresh and logout routes are NOT behind require_auth Bearer middleware: they self-authenticate via the refresh cookie; Bearer middleware would block legitimate refresh flows"
  - "jsonwebtoken rust_crypto feature enabled: avoids requiring a rustls CryptoProvider::install_default() call in test environments where no TLS stack is initialized"
  - "SHA-256 hash of refresh token stored in DB: prevents token theft from DB dump per T-01-10"
  - "Test app builds real AppState with Config struct (not mock): ensures test behavior matches production handler signatures exactly"

patterns-established:
  - "Cookie auth pattern: refresh/logout handlers extract cookie directly — not gated by Bearer middleware"
  - "Role enforcement pattern: use route_layer(from_fn_with_state(...)) on a Router group, not per-handler"
  - "Error messaging: AppError::Unauthorized returned for all login failures (no username enumeration per T-01-08)"
  - "Test fixture pattern: build_test_app() helper constructs real AppState with test DB + TEST_JWT_SECRET"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05]

duration: 6min
completed: "2026-04-14"
---

# Phase 01 Plan 02: Auth & Setup Wizard Summary

**JWT auth with Argon2id password hashing, refresh token rotation via httpOnly SameSite=Lax cookie, three-role RBAC middleware, and first-boot setup wizard — all 5 auth tests passing**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-14T17:34:50Z
- **Completed:** 2026-04-14T17:40:32Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments

- Complete auth service: Argon2id password hashing (password-auth crate), JWT issuance/verification (access 20min + refresh 7d), SHA-256 refresh token storage
- Three-role RBAC middleware stack: require_auth (any role), require_admin (Admin only), require_supervisor_or_above (Admin or Supervisor) with AuthUser extractor
- Login handler: credential verification, token issuance, httpOnly SameSite=Lax refresh cookie; Refresh handler: token rotation; Logout handler: hash invalidation + cookie expiry
- Setup wizard: GET /setup/status (initialized check), POST /setup/init (first-boot admin with 409 on duplicate per T-01-11)
- All 5 auth tests un-ignored and passing: password_hashing_uses_argon2id, auth_login_returns_jwt, rbac_middleware_blocks_unauthorized, jwt_refresh_rotates_tokens, setup_wizard_creates_admin

## Task Commits

Each task was committed atomically:

1. **Task 1: Auth service — password hashing, JWT issuance/validation, refresh token rotation** - `b104df0` (feat)
2. **Task 2: RBAC middleware, auth handlers, setup wizard, router wiring, and auth tests** - `027af30` (feat)

## Files Created/Modified

- `backend/src/auth/models.rs` — Role enum (Admin/Supervisor/Viewer), Claims struct, LoginRequest/Response types
- `backend/src/auth/service.rs` — hash_password, verify_password, issue_access_token, issue_refresh_token, verify_access_token, verify_refresh_token, hash_token
- `backend/src/auth/middleware.rs` — require_auth Tower middleware (Bearer token extraction + Claims insertion into extensions)
- `backend/src/auth/rbac.rs` — require_admin, require_supervisor_or_above middlewares; AuthUser FromRequestParts extractor
- `backend/src/auth/handlers.rs` — login (Argon2id verify + cookie), refresh (rotation), logout (DB clear + cookie expire)
- `backend/src/setup/handlers.rs` — setup_status (count check), setup_init (first-boot admin creation with 409 guard)
- `backend/src/main.rs` — full router: public routes + cookie-auth routes merged under /api/v1
- `backend/src/lib.rs` — exposes auth and setup modules
- `backend/Cargo.toml` — added sha2, time, jsonwebtoken rust_crypto feature
- `backend/tests/auth_tests.rs` — all 5 auth tests implemented and passing

## Decisions Made

- SameSite=Lax (not Strict) on refresh cookie per plan review fix: allows third-party link navigation (emails, portals) in on-premise deployments
- refresh/logout endpoints are NOT behind require_auth Bearer middleware: they self-authenticate via the refresh cookie — Bearer middleware would block legitimate refresh flows (this was a correctness fix discovered during testing)
- jsonwebtoken `rust_crypto` feature enabled: avoids rustls CryptoProvider panic in test environments

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] refresh/logout routes moved out of require_auth Bearer middleware**
- **Found during:** Task 2 (jwt_refresh_rotates_tokens test failure)
- **Issue:** Plan placed refresh/logout behind require_auth middleware which checks Authorization: Bearer header. These endpoints validate via the refresh cookie, not a Bearer token — so they were returning 401.
- **Fix:** Moved refresh/logout to a separate `cookie_auth_routes` group with no Bearer middleware; both routes self-authenticate by verifying the refresh JWT from the cookie
- **Files modified:** backend/src/main.rs, backend/tests/auth_tests.rs
- **Verification:** jwt_refresh_rotates_tokens test passes; logout accessible without Bearer token
- **Committed in:** 027af30 (Task 2 commit)

**2. [Rule 3 - Blocking] Added time crate and jsonwebtoken rust_crypto feature**
- **Found during:** Task 2 (compilation errors and test panics)
- **Issue:** axum-extra Cookie.max_age() requires `time::Duration` from the `time` crate (not in Cargo.toml). jsonwebtoken 10.3.0 panics at runtime without a rustls CryptoProvider unless rust_crypto feature is enabled.
- **Fix:** Added `time = "0.3"` to dependencies; changed jsonwebtoken to `{ version = "10.3.0", features = ["rust_crypto"] }`
- **Files modified:** backend/Cargo.toml
- **Verification:** Cargo check passes; all 5 auth tests pass
- **Committed in:** 027af30 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for correct runtime behavior. No scope creep — no new functionality added beyond plan spec.

## Issues Encountered

- jsonwebtoken 10.x requires explicit crypto provider configuration at runtime — the `rust_crypto` feature (using pure-Rust RustCrypto crates) is the correct solution for environments without a full TLS stack (like tests)

## Known Stubs

None — all handlers are fully implemented with real DB queries, no placeholder data.

## Threat Flags

None — all threat model items (T-01-05 through T-01-12, T-01-26) were addressed as specified. No new network endpoints, auth paths, or schema changes beyond the plan's threat register.

## Next Phase Readiness

- Auth foundation complete: all protected endpoints in Phase 1 (employees, departments, rules) can now apply require_auth or role-gated middleware
- Setup wizard ready: frontend can call GET /setup/status on first load to gate the admin creation flow
- Concern: common::create_test_admin() uses a placeholder Argon2id hash that won't verify — tests needing real login should call hash_password() directly (as auth_login_returns_jwt and jwt_refresh_rotates_tokens do)

---
*Phase: 01-foundation*
*Completed: 2026-04-14*
