---
phase: 05-reports-payroll-export
plan: 01
subsystem: database
tags: [tenant_info, employees, audit_log, sqlite, libsql, axum, validator, optimistic_concurrency, writable_schema]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "audit_log table + audit triggers, singleton-row CRUD pattern (global_rules), AppError + epoch_to_iso helpers, RBAC middleware (require_admin / require_supervisor_or_above / require_auth), validator-derive DTO pattern, MIGRATIONS array runner"
  - phase: 04-frontend-ui
    provides: "Employee TypeScript type already declares position + hire_date — backend now matches the contract"
provides:
  - "tenant_info singleton table seeded with empty values + GET (any role) + PATCH (admin-only) endpoints with optimistic concurrency"
  - "audit_log.operation CHECK relaxed to accept 'REPORT_EXPORT' so Plan 05-02 can insert report-export audit rows"
  - "audit_tenant_info_update trigger writing immutable old/new diffs"
  - "employees.position + employees.hire_date columns with full create/update wiring"
  - "audit_employees_update trigger rebuilt to capture position + hire_date in JSON diff"
  - "Bruno requests for tenant-info GET + PATCH with version env-var passthrough"
affects: [05-02-reports-calc-api, 05-03-excel-export, 05-04-frontend-reports-screen]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "PRAGMA writable_schema = 1 idiom for relaxing CHECK constraints (table-rebuild + DROP fails inside libSQL execute_batch even with legacy_alter_table because the audit triggers from migrations 002/006/011 reference audit_log)"
    - "Singleton-row module pattern: {mod, models, service, handlers}.rs mirroring rules/ but with explicit service.rs (rules/ inlines DB calls in handlers; tenant_info/ extracts service for cleaner test seam)"
    - "Optional ISO date column pattern: Option<String> as YYYY-MM-DD on the wire, parsed to UTC midnight epoch i64 via parse_hire_date helper; empty string treated as NULL clear in PATCH"

key-files:
  created:
    - "backend/src/db/migrations/013_tenant_info.sql"
    - "backend/src/db/migrations/014_phase5_audit_triggers.sql"
    - "backend/src/db/migrations/015_employees_position_hire_date.sql"
    - "backend/src/tenant_info/mod.rs"
    - "backend/src/tenant_info/models.rs"
    - "backend/src/tenant_info/service.rs"
    - "backend/src/tenant_info/handlers.rs"
    - "backend/tests/tenant_info_test.rs"
    - "bruno/cronometrix/tenant-info/01_get.bru"
    - "bruno/cronometrix/tenant-info/02_patch.bru"
  modified:
    - "backend/src/db/mod.rs"
    - "backend/src/lib.rs"
    - "backend/src/main.rs"
    - "backend/src/employees/models.rs"
    - "backend/src/employees/service.rs"

key-decisions:
  - "Replaced the planned table-rebuild + legacy_alter_table approach with PRAGMA writable_schema = 1 for relaxing audit_log.operation CHECK (Rule 3 deviation: rebuild path failed inside libSQL execute_batch with 'database table is locked' because 13 audit triggers reference audit_log; the writable_schema idiom rewrites sqlite_master.sql in-place without touching trigger references and preserves all existing rows verbatim)"
  - "tenant_info module gets its own service.rs (rules/ does not) — extra file pays for itself in test isolation: integration tests can construct connections that exercise update_tenant_info without an HTTP layer"
  - "hire_date parses YYYY-MM-DD to UTC midnight epoch and stores as INTEGER nullable; empty string in PATCH means 'clear to NULL' (vs absent field which means 'leave unchanged')"

patterns-established:
  - "When a CHECK constraint must be relaxed and the table has incoming triggers/views/FKs, prefer PRAGMA writable_schema rewrite over rebuild. Modern SQLite recursively validates references during DROP TABLE / RENAME and libSQL execute_batch does NOT honour legacy_alter_table inside its implicit transaction."
  - "Singleton-row PATCH endpoints: dynamic SET via Vec<String> + Vec<libsql::Value>, version param last, WHERE id = <pk> AND version = ?N, rows_affected == 0 returns AppError::Conflict { code: VERSION_CONFLICT }. Identical shape to rules::handlers::update_rules — Plan 05-04 settings UI can ship the same client-side flow."

requirements-completed: [PAY-01]

# Metrics
duration: 60min
completed: 2026-04-25
---

# Phase 5 Plan 1: Tenant Info + Employees ALTER + audit_log CHECK Rebuild Summary

**Singleton tenant_info CRUD with optimistic concurrency, employees.position + employees.hire_date columns wired through service+trigger, and audit_log.operation CHECK relaxed via writable_schema to accept REPORT_EXPORT for downstream plans.**

## Performance

- **Duration:** ~60 min (including diagnostic loop on libSQL execute_batch table-rebuild lock)
- **Started:** 2026-04-25T22:42Z
- **Completed:** 2026-04-25T23:42Z
- **Tasks:** 2
- **Files modified:** 5
- **Files created:** 10

## Accomplishments

- `tenant_info` singleton table seeded with empty branding values + full CRUD module (`{mod, models, service, handlers}.rs`) implementing GET (any authenticated role) + PATCH (admin-only) with optimistic concurrency via `version` and `WHERE id = 1` (RESEARCH Pitfall 8).
- `audit_log.operation` CHECK constraint relaxed to accept `'REPORT_EXPORT'` — Plan 05-02 can now insert export audit rows without the planned table-rebuild detour.
- `audit_tenant_info_update` trigger writes immutable JSON diffs of every PATCH.
- `audit_employees_update` trigger rebuilt to capture the new `position` + `hire_date` columns alongside the existing identity columns.
- `employees` schema extended with `position TEXT NOT NULL DEFAULT ''` and `hire_date INTEGER` (nullable epoch seconds, exposed on the wire as `Option<String>` ISO YYYY-MM-DD).
- Five integration tests covering RBAC enforcement, version conflict, and audit trigger firing — full backend suite is green at 189 tests.
- Bruno collection for `/api/v1/tenant-info` with version env-var passthrough between `01_get` and `02_patch`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create migrations 013/014/015 and register in MIGRATIONS array** — `53a3c6b` (feat)
2. **Task 2: Build tenant_info module + extend employees + register routes + Bruno + tests** — `8552398` (feat)

## Files Created/Modified

### Created

- `backend/src/db/migrations/013_tenant_info.sql` — Singleton `tenant_info` table with `CHECK (id = 1)` and `INSERT OR IGNORE` seed row.
- `backend/src/db/migrations/014_phase5_audit_triggers.sql` — `PRAGMA writable_schema = 1` rewrites `audit_log` CHECK to accept `REPORT_EXPORT`; adds `audit_tenant_info_update`; DROP+RECREATE `audit_employees_update` to include `position` + `hire_date` in the JSON diff.
- `backend/src/db/migrations/015_employees_position_hire_date.sql` — `ALTER TABLE employees ADD COLUMN position TEXT NOT NULL DEFAULT ''` + `ALTER TABLE employees ADD COLUMN hire_date INTEGER` (nullable).
- `backend/src/tenant_info/mod.rs` — Module index.
- `backend/src/tenant_info/models.rs` — `TenantInfo` (Serialize) + `UpdateTenantInfoRequest` (Deserialize + Validate, max-length only per CONTEXT D-30 minimal scope).
- `backend/src/tenant_info/service.rs` — `get_tenant_info` + `update_tenant_info` with dynamic SET, optimistic concurrency, and explicit `WHERE id = 1` pinning.
- `backend/src/tenant_info/handlers.rs` — Axum extractors wrapping the service, with explicit `validator::Validate::validate()` on PATCH bodies.
- `backend/tests/tenant_info_test.rs` — Five integration tests: `get_returns_seed_row`, `admin_patch_succeeds`, `supervisor_blocked`, `version_conflict`, `audit_trigger_fires`.
- `bruno/cronometrix/tenant-info/01_get.bru` — GET request that captures `version` into `tenant_info_version` env var.
- `bruno/cronometrix/tenant-info/02_patch.bru` — PATCH request consuming `{{tenant_info_version}}`.

### Modified

- `backend/src/db/mod.rs` — Append migrations 013/014/015 to the `MIGRATIONS` array (numeric order preserved).
- `backend/src/lib.rs` — `pub mod tenant_info;` so the integration test crate can reach it.
- `backend/src/main.rs` — Register `GET /tenant-info` in `viewer_routes` and `PATCH /tenant-info` in `admin_routes`; import `cronometrix_api::tenant_info`.
- `backend/src/employees/models.rs` — Add `position: String` + `hire_date: Option<String>` to `Employee` response, and the matching optional fields to `CreateEmployeeRequest` + `UpdateEmployeeRequest` with max-100 validator on `position`.
- `backend/src/employees/service.rs` — Add `parse_hire_date` (YYYY-MM-DD → epoch) and `epoch_to_iso_date_opt` helpers; rewrite INSERT to include `position, hire_date`; extend dynamic UPDATE SET with the two new columns (empty string clears `hire_date` to NULL); shift column indices in `row_to_employee` to match the new SELECT order.

## Decisions Made

- **Migration 014 strategy switched from table-rebuild to `PRAGMA writable_schema = 1`.** The plan's intended `CREATE _new + INSERT SELECT + DROP + RENAME` pattern fails inside libSQL `execute_batch` with `database table is locked`. Modern SQLite (>= 3.25) recursively validates trigger/view references during `DROP TABLE` + `RENAME`, and the 13 audit triggers from migrations 002/006/011 all `INSERT INTO audit_log`. `PRAGMA legacy_alter_table = ON` is NOT honoured inside libSQL's implicit `execute_batch` transaction. Rewriting `sqlite_master.sql` text is the canonical workaround, preserves every existing row, and does not touch trigger references.
- **`tenant_info` ships with its own `service.rs`** even though `rules/` inlines DB work in handlers. The extra file is paid for by `tenant_info_test::audit_trigger_fires`, which holds a side-channel connection to the test DB and reads `audit_log` directly — service indirection makes that pattern reusable for Plan 05-02 reports tests.
- **`hire_date` empty-string semantics in PATCH = clear to NULL.** Absent field = leave unchanged. This matches the Phase 4 frontend's `hire_date: ""` "no value" idiom while still allowing operators to explicitly delete a hire date.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Migration 014 table-rebuild path fails inside libSQL `execute_batch`**
- **Found during:** Task 1 (`cargo nextest run -p cronometrix-api --test db_tests` — `schema_creates_all_tables`, `audit_triggers_fire_on_employee_insert`, `utc_epoch_storage_verified` all panicked with `SQLite failure: \`database table is locked\``).
- **Issue:** The plan specified rebuilding `audit_log` via `CREATE audit_log_new + INSERT SELECT + DROP audit_log + ALTER ... RENAME`. In modern SQLite (>= 3.25) this DROP fails because the 13 audit triggers from migrations 002/006/011 reference `audit_log`. Even after adding `PRAGMA legacy_alter_table = ON`, libSQL's `execute_batch` wraps the migration in an implicit transaction that prevents the pragma from taking effect. The repro in Python sqlite3 surfaced the underlying cause (`error in trigger audit_employees_insert: no such table: main.audit_log`); libSQL surfaces it as a generic lock error.
- **Fix:** Rewrote `014_phase5_audit_triggers.sql` to use `PRAGMA writable_schema = 1` — directly rewrite the CHECK clause text in `sqlite_master.sql`, then `PRAGMA writable_schema = 0`. No DROP, no RENAME, no trigger references touched, all existing rows preserved verbatim. Header comment in the migration documents why this path was chosen so future migrators don't try to "clean it up" back to a table rebuild.
- **Files modified:** `backend/src/db/migrations/014_phase5_audit_triggers.sql`
- **Verification:** All 7 `db_tests` pass; full backend suite at 189 tests passes; `sqlite3 .schema audit_log` confirms the new constraint includes `'REPORT_EXPORT'`.
- **Committed in:** `53a3c6b` (Task 1 commit).

---

**Total deviations:** 1 auto-fixed (1 bug fix to plan-specified migration approach)
**Impact on plan:** The deviation does not change Task 1's contract — `audit_log.operation` accepts `REPORT_EXPORT`, all existing audit rows are preserved, and the new triggers are registered. Downstream plans (05-02 audit insert) are unaffected. This is a libSQL-specific workaround documented in-file for future migration authors.

## Issues Encountered

- libSQL `execute_batch` swallows the upstream `error in trigger audit_employees_insert: no such table` SQLite error and reports `database table is locked` instead. Direct Python sqlite3 reproduction surfaced the real cause within ~5 minutes; without that side-channel the libSQL error is misleading. Documented in migration 014 comment header so the next migrator hits a soft landing.
- Pre-commit hook attempted to format files but was bypassed via `--no-verify` per parallel-executor instructions; no formatter divergence in the modified files.

## User Setup Required

None — no external service configuration required.

## Threat Surface Verification

The plan's `<threat_model>` enumerated seven threats; each remains mitigated by the implementation:

- **T-05-01 (EoP, tenant_info PATCH):** `require_admin` in `admin_routes` group; `supervisor_blocked` test verifies 403.
- **T-05-02 (Repudiation, tenant_info UPDATE):** `audit_tenant_info_update` trigger fires on every UPDATE; `audit_trigger_fires` test asserts the row appears.
- **T-05-03 (Tampering, request payload):** `validator::Validate::validate()` enforces 200/50/500-char caps on `client_name`/`client_rif`/`address` before service runs.
- **T-05-04 (Tampering, race):** `WHERE id = 1 AND version = ?` rejects stale writers; `version_conflict` test asserts 409 + `VERSION_CONFLICT` error code.
- **T-05-05 (Info disclosure):** Accepted — all authenticated roles can read non-secret branding metadata; audit log captures every UPDATE.
- **T-05-06 (Tampering, audit_log rebuild):** New CHECK accepts only `INSERT/UPDATE/DELETE/REPORT_EXPORT`; rebuild done in-place via `writable_schema` (no rows lost, INSERT SELECT not used).
- **T-05-07 (Repudiation, employees UPDATE with new columns):** `audit_employees_update` rebuilt to JSON-encode `position` + `hire_date` in old_data and new_data.

No new threat flags surfaced during execution — no new network endpoints, no new file-IO surfaces, no new auth paths.

## Next Plan Readiness

- Plan 05-02 (Reports calculation API) can now `INSERT INTO audit_log (..., operation, ...) VALUES (..., 'REPORT_EXPORT', ...)` without CHECK violation.
- Plan 05-04 (Frontend Reports + Settings screens) can `GET /api/v1/tenant-info` and render the branding header with `client_name` + `client_rif`.
- Plan 05-02 column population: `EmployeeReportRow.cargo` reads from the new `employees.position`; identity column for hire date will use `employees.hire_date`.
- No blockers.

## Self-Check: PASSED

Verified files exist and commits are present:

- `backend/src/db/migrations/013_tenant_info.sql` — FOUND
- `backend/src/db/migrations/014_phase5_audit_triggers.sql` — FOUND
- `backend/src/db/migrations/015_employees_position_hire_date.sql` — FOUND
- `backend/src/tenant_info/mod.rs` — FOUND
- `backend/src/tenant_info/models.rs` — FOUND
- `backend/src/tenant_info/service.rs` — FOUND
- `backend/src/tenant_info/handlers.rs` — FOUND
- `backend/tests/tenant_info_test.rs` — FOUND
- `bruno/cronometrix/tenant-info/01_get.bru` — FOUND
- `bruno/cronometrix/tenant-info/02_patch.bru` — FOUND
- Commit `53a3c6b` (Task 1) — FOUND in git log
- Commit `8552398` (Task 2) — FOUND in git log

---
*Phase: 05-reports-payroll-export*
*Completed: 2026-04-25*
