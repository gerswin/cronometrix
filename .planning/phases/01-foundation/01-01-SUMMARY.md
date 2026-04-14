---
phase: 01-foundation
plan: 01
subsystem: database
tags: [rust, axum, libsql, sqlite, turso, migrations, audit-triggers, tokio]

requires:
  - phase: 01-foundation-plan-00
    provides: placeholder SQL files, test infrastructure, Cargo.lock bootstrap

provides:
  - Rust/Axum backend compiling with all Phase 1 dependencies
  - AppState (Arc<Database> + Arc<Config>) for handler injection
  - AppError enum with structured JSON IntoResponse per D-11
  - Config struct with JWT_SECRET validation (min 32 chars, panics on startup)
  - Full SQLite schema: users, departments, employees, global_rules, audit_log
  - AFTER INSERT/UPDATE/DELETE audit triggers on all 4 mutable tables
  - Migration runner with _migrations tracking table (idempotent)
  - Turso embedded replica mode with graceful sync failure (local-only degraded mode)
  - Health endpoint performing SELECT 1 database connectivity check
  - Wave 0 db tests passing: schema_creates_all_tables, audit_triggers_fire, utc_epoch

affects:
  - 01-foundation-plan-02 (auth endpoints use AppState, AppError, Config)
  - 01-foundation-plan-03 (employee/department CRUD uses schema from 001_initial_schema)
  - All downstream phases depend on this schema and error handling pattern

tech-stack:
  added:
    - axum 0.8.8 (macros feature)
    - axum-extra 0.12.5 (cookie feature)
    - libsql 0.9.30 (embedded replica + local modes)
    - password-auth 1.0 (Argon2id hashing)
    - jsonwebtoken 10.3.0
    - tower-http 0.6 (cors, trace, compression-gzip, timeout)
    - tower 0.5
    - validator 0.20 (derive)
    - dotenvy 0.15
    - chrono 0.4 (serde feature)
    - uuid 1 (v4, serde features)
    - tracing + tracing-subscriber 0.3 (json + env-filter features)
    - anyhow 1 + thiserror 2
  patterns:
    - AppState with Arc<Database> + Arc<Config> for Axum State extractor
    - AppError enum with IntoResponse producing {"error":{"code","message","status"}} JSON
    - Embedded SQL migrations via include_str!() with _migrations tracking table
    - Turso sync wrapped in match with tracing::warn! for graceful degraded mode
    - SQLite audit triggers using hex(randomblob()) UUID v4 + json_object() snapshots
    - lib.rs exposing pub modules so integration tests can import production code

key-files:
  created:
    - backend/src/lib.rs
    - backend/src/config.rs
    - backend/src/state.rs
    - backend/src/errors.rs
    - backend/src/db/mod.rs
    - backend/src/db/migrations/001_initial_schema.sql
    - backend/src/db/migrations/002_audit_triggers.sql
    - backend/.env.example
    - backend/.gitignore
  modified:
    - backend/Cargo.toml
    - backend/src/main.rs
    - backend/tests/common/mod.rs
    - backend/tests/db_tests.rs

key-decisions:
  - "lib.rs added to expose pub modules for integration test imports — binary crates can't be referenced by test crates without a library target"
  - "Test fixture uses temp file DB (not :memory:) because sqlite3_open_v2(':memory:') creates an isolated DB per connection — migrations and tests would see different databases"
  - "tracing-subscriber env-filter feature added to Cargo.toml — with_env_filter() requires it explicitly"
  - "run_migrations skips SQL starting with '-- Placeholder' to maintain Wave 0 guard compatibility"
  - "actor_id is NULL in audit triggers (Pitfall 4 — Phase 1 acceptable; service layer writes secondary entry with actor context in later plans)"

patterns-established:
  - "Pattern: AppState — Arc<Database> + Arc<Config> shared via Axum State extractor"
  - "Pattern: AppError — thiserror enum implementing IntoResponse with structured JSON body"
  - "Pattern: Migrations — const MIGRATIONS array with include_str!() + _migrations tracking table"
  - "Pattern: DB init — Config.has_turso() selects remote replica vs local mode at startup"
  - "Pattern: Audit — SQLite AFTER triggers write json_object() snapshots, never modified after insert"

requirements-completed: [DATA-01, DATA-02, DATA-03, DATA-04]

duration: 8min
completed: 2026-04-14
---

# Phase 01 Plan 01: Backend Scaffold and Database Foundation Summary

**Rust/Axum backend compiling with SQLite schema (5 tables), idempotent migration runner, Turso sync with graceful degraded mode, and AFTER triggers on all 4 mutable tables producing JSON audit snapshots**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-14T17:24:46Z
- **Completed:** 2026-04-14T17:32:11Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Full Rust/Axum backend compiles with all Phase 1 dependencies (libsql 0.9.30, axum 0.8.8, jsonwebtoken 10.3.0, password-auth 1.0)
- SQLite schema with 5 tables: users, departments, employees, global_rules, audit_log — all with version columns, UTC epoch timestamps, and CHECK constraints
- AFTER INSERT/UPDATE/DELETE audit triggers on all 4 mutable tables using hex(randomblob()) UUID v4 and json_object() snapshots
- Health endpoint performs SELECT 1 database connectivity check (not just HTTP liveness)
- Turso sync failure gracefully logs warning and continues in local-only mode (review fix applied)
- Wave 0 db tests passing: schema creation, audit trigger firing, UTC epoch storage

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold Rust project with Cargo.toml, config, state, and error types** - `9abb709` (feat)
2. **Task 2: Database initialization, migrations, and audit triggers** - `178340f` (feat)

**Plan metadata:** (docs commit to follow)

## Files Created/Modified

- `backend/Cargo.toml` - Updated to cronometrix-api package with all Phase 1 verified dependencies
- `backend/src/lib.rs` - Library target exposing pub modules for integration test access
- `backend/src/main.rs` - Tokio entrypoint with health route (SELECT 1 check), CORS, tracing
- `backend/src/config.rs` - Config struct with from_env(), JWT_SECRET min-32-chars validation
- `backend/src/state.rs` - AppState with Arc<Database> + Arc<Config>
- `backend/src/errors.rs` - AppError enum with IntoResponse producing structured JSON
- `backend/src/db/mod.rs` - init_db, init_db_local, init_db_remote, run_migrations
- `backend/src/db/migrations/001_initial_schema.sql` - 5 tables with indexes and global_rules seed
- `backend/src/db/migrations/002_audit_triggers.sql` - 11 triggers covering all 4 mutable tables
- `backend/.env.example` - All 7 required env var templates
- `backend/.gitignore` - Excludes /target, .env, *.db, *.db-wal, *.db-shm
- `backend/tests/common/mod.rs` - Updated to use run_migrations + temp file DB
- `backend/tests/db_tests.rs` - Un-ignored 3 tests; fixed schema_creates_all_tables assertion

## Decisions Made

- Added `lib.rs` to expose pub modules — binary crates cannot be referenced from integration test crates without a library target
- Switched test fixture from `:memory:` to unique temp file path — `sqlite3_open_v2(":memory:")` creates a completely isolated database per connection; migrations on one connection are invisible to a second connection
- Added `env-filter` feature to `tracing-subscriber` — `with_env_filter()` requires it explicitly and was missing from plan's Cargo.toml spec

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] tracing-subscriber missing env-filter feature**
- **Found during:** Task 1 (cargo build)
- **Issue:** `with_env_filter()` call failed to compile — feature not enabled in plan's Cargo.toml spec
- **Fix:** Added `"env-filter"` to tracing-subscriber features in Cargo.toml
- **Files modified:** backend/Cargo.toml
- **Verification:** cargo build passes
- **Committed in:** 9abb709 (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added lib.rs for integration test access**
- **Found during:** Task 2 (updating common/mod.rs to use run_migrations)
- **Issue:** Integration tests cannot import from a binary-only crate; `cronometrix_api::db::run_migrations` requires a library target
- **Fix:** Created `src/lib.rs` with `pub mod config; pub mod db; pub mod errors; pub mod state;`
- **Files modified:** backend/src/lib.rs, backend/src/main.rs
- **Verification:** cargo test compiles and passes
- **Committed in:** 178340f (Task 2 commit)

**3. [Rule 1 - Bug] SQLite :memory: isolation — test fixture returned wrong DB**
- **Found during:** Task 2 (cargo test db_tests — all 3 un-ignored tests failed)
- **Issue:** `Builder::new_local(":memory:")` + two separate `db.connect()` calls = two isolated SQLite databases. Migrations applied to connection A are invisible on connection B. All 3 db tests failed with "no such table"
- **Fix:** Changed test_db() to use unique temp file path (`/tmp/cronometrix_test_{uuid}.db`) so all connections share the same on-disk SQLite file
- **Files modified:** backend/tests/common/mod.rs
- **Verification:** cargo test: 3 passed, 16 ignored
- **Committed in:** 178340f (Task 2 commit)

**4. [Rule 1 - Bug] schema_creates_all_tables test had no actual assertion**
- **Found during:** Task 2 (reviewing test after un-ignoring)
- **Issue:** Test queried sqlite_master then dropped the Rows iterator without asserting — would always pass even if tables didn't exist
- **Fix:** Changed to `SELECT COUNT(*)` and `assert_eq!(count, 1, "Table '{}' should exist")`
- **Files modified:** backend/tests/db_tests.rs
- **Verification:** Test correctly fails before fix, passes after schema migration runs
- **Committed in:** 178340f (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (2 bugs, 1 missing critical, 1 bug in test)
**Impact on plan:** All fixes were necessary for correctness. The :memory: isolation issue was a non-obvious SQLite behavior (consistent with Pitfall notes in RESEARCH.md). No scope creep.

## Issues Encountered

- libsql 0.9.30 `execute_batch` returns `BatchRows` instead of `()` — API changed from 0.6.x. The new signature works correctly; the type change doesn't affect usage since we discard the return value.

## User Setup Required

None — local-only mode works without Turso credentials (TURSO_DATABASE_URL can be empty). To run the server, copy `.env.example` to `.env` and set `JWT_SECRET` to at least 32 characters.

## Next Phase Readiness

- Backend compiles and starts with `cargo run` after setting JWT_SECRET in `.env`
- Database schema and migrations ready for Plan 01-02 (auth endpoints)
- AppState, AppError, Config patterns established for all downstream handlers
- Blocker: Plan 01-02 must create the first admin user (users table is empty after fresh install)

---
*Phase: 01-foundation*
*Completed: 2026-04-14*

## Self-Check: PASSED

- All 12 files verified present on disk
- Commits 9abb709 (Task 1) and 178340f (Task 2) confirmed in git log
- cargo test: 3 passed, 16 ignored, 0 failed
