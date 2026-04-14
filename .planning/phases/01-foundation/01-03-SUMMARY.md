---
phase: 01-foundation
plan: 03
subsystem: api
tags: [rust, axum, libsql, sqlite, crud, rbac, pagination, soft-delete, optimistic-concurrency]

# Dependency graph
requires:
  - phase: 01-foundation plan 01
    provides: database schema (employees, departments, global_rules tables), AppState, AppError
  - phase: 01-foundation plan 02
    provides: auth middleware (require_auth, require_admin, require_supervisor_or_above), JWT Claims

provides:
  - Employee CRUD REST API (create, list, get, update, soft-delete)
  - Department CRUD REST API (create, list, get, update)
  - Global rules singleton GET/PATCH API
  - PaginatedResponse<T> and epoch_to_iso shared utilities
  - Full router wiring with 3-tier RBAC (Viewer/Supervisor/Admin)

affects: [02-attendance, 03-timesheet, 04-reports, 05-devices]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Dynamic WHERE clause building with positional params for libsql
    - Row-to-struct mapping via column index (row.get(N))
    - Thin handlers calling service layer (handlers = extractor + service call + status code)
    - Optional field PATCH with dynamic SET clause and optimistic concurrency
    - Soft delete via status + deleted_at (no SQL DELETE anywhere in codebase)

key-files:
  created:
    - backend/src/common.rs
    - backend/src/employees/mod.rs
    - backend/src/employees/models.rs
    - backend/src/employees/service.rs
    - backend/src/employees/handlers.rs
    - backend/src/departments/mod.rs
    - backend/src/departments/models.rs
    - backend/src/departments/service.rs
    - backend/src/departments/handlers.rs
    - backend/src/rules/mod.rs
    - backend/src/rules/models.rs
    - backend/src/rules/handlers.rs
  modified:
    - backend/src/main.rs
    - backend/src/lib.rs
    - backend/tests/employee_tests.rs
    - backend/tests/department_tests.rs
    - backend/tests/rules_tests.rs

key-decisions:
  - "Soft delete uses API-level verification (GET by id + status filter) instead of direct DB connection in tests — libsql::Database does not implement Clone so sharing the DB handle between app state and test assertions is not straightforward"
  - "Dynamic WHERE clause built with positional params (param index tracks across count and fetch queries) to avoid SQL injection while supporting optional filters"
  - "Rules update always sets effective_from = unixepoch() regardless of which fields changed — per RULE-03 any rule change invalidates the prior effective period"
  - "Axum 0.8 path syntax uses {id} not :id for path parameters"

patterns-established:
  - "Service layer: pure async fn(conn, req) -> Result<Model, AppError> — no state, just a connection"
  - "Handler layer: extract State + Path/Query/Json, call service, return StatusCode + Json"
  - "Conflict detection: check rows_affected == 0 after UPDATE, then query for existence to distinguish NOT_FOUND from VERSION_CONFLICT"
  - "Pagination: clamp limit to 1..=100 default 20; offset >= 0 default 0; always return total count"
  - "Test app builder: construct Router with same middleware layers as production, use TEST_JWT_SECRET"

requirements-completed: [EMP-01, EMP-02, EMP-03, EMP-04, DEPT-01, DEPT-02, DEPT-03, RULE-01, RULE-02, RULE-03]

# Metrics
duration: 35min
completed: 2026-04-14
---

# Phase 01 Plan 03: Employee/Department/Rules CRUD Summary

**REST CRUD for employees (soft-delete + FK validation), departments (lunch mode validation), and global rules singleton (effective_from tracking) — all with optimistic concurrency, offset pagination, and 3-tier RBAC**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-14T18:00:00Z
- **Completed:** 2026-04-14T18:35:00Z
- **Tasks:** 3
- **Files modified:** 15

## Accomplishments

- Employee CRUD: create with department FK validation, list with name/dept/status filters and offset pagination, get by id, PATCH with optimistic concurrency, DELETE as soft-delete (status=inactive + deleted_at, no SQL DELETE)
- Department CRUD: create with lunch_mode/lunch_duration_min consistency validation, list with pagination, get by id, PATCH with optimistic concurrency
- Global rules singleton: GET returns seeded defaults (10/10/0 min tolerances), PATCH updates fields and always sets effective_from = unixepoch() per RULE-03
- Full router wiring in main.rs: Viewer reads all, Supervisor creates/edits employees, Admin manages departments/rules/employee deactivation
- Shared common.rs: PaginatedResponse<T> and epoch_to_iso / epoch_to_iso_opt for D-13 timestamp serialization
- 10 integration tests un-ignored and all passing (4 employee + 3 department + 3 rules)
- Total test suite: 18 passing, 1 pre-existing ignored

## Task Commits

Each task was committed atomically:

1. **Task 1: Shared utilities and Employee CRUD** - `6de1baf` (feat)
2. **Task 2: Department CRUD** - `6d7caa7` (feat)
3. **Task 3: Global rules + router wiring** - `e2d4b48` (feat)

## Files Created/Modified

- `backend/src/common.rs` — PaginatedResponse<T>, epoch_to_iso, epoch_to_iso_opt
- `backend/src/employees/models.rs` — Employee, CreateEmployeeRequest, UpdateEmployeeRequest, EmployeeListQuery
- `backend/src/employees/service.rs` — create, list, get_by_id, update, deactivate
- `backend/src/employees/handlers.rs` — create_employee, list_employees, get_employee, update_employee, deactivate_employee
- `backend/src/employees/mod.rs` — module re-exports
- `backend/src/departments/models.rs` — Department, CreateDepartmentRequest, UpdateDepartmentRequest, DepartmentListQuery
- `backend/src/departments/service.rs` — create, list, get_by_id, update (with lunch validation)
- `backend/src/departments/handlers.rs` — create_department, list_departments, get_department, update_department
- `backend/src/departments/mod.rs` — module re-exports
- `backend/src/rules/models.rs` — GlobalRules, UpdateRulesRequest
- `backend/src/rules/handlers.rs` — get_rules, update_rules
- `backend/src/rules/mod.rs` — module re-exports
- `backend/src/main.rs` — full router wiring with viewer/supervisor/admin route groups
- `backend/src/lib.rs` — added pub mod common, employees, departments, rules
- `backend/tests/employee_tests.rs` — 4 tests un-ignored and implemented
- `backend/tests/department_tests.rs` — 3 tests un-ignored and implemented
- `backend/tests/rules_tests.rs` — 3 tests un-ignored and implemented

## Decisions Made

- Soft delete verification in tests done via API (GET by id returning status=inactive + deleted_at set) rather than direct DB connection — `libsql::Database` does not implement `Clone`, making it non-trivial to share the DB handle between app state and test-level assertions without restructuring the test DB helper
- Dynamic WHERE clause with positional parameter indexing — tracks param position across count and fetch queries to avoid SQL injection while supporting optional filters cleanly
- `effective_from` always updated on any rule PATCH — per RULE-03, any rule change takes effect on the next calculation cycle, so the effective period must be reset on every mutation
- Axum 0.8 path syntax confirmed: `{id}` (curly braces) not `:id` (colon) for path parameters

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- `libsql::Database` does not implement `Clone`, so the initial `soft_delete_only_no_hard_delete` test design (which tried to clone the DB handle for direct SQL inspection) failed to compile. Resolved by restructuring the test to use the REST API itself (GET by id + status filter listing) to verify soft-delete behavior — which is actually a stronger test since it validates the full API contract, not just the DB state.

## Known Stubs

None — all endpoints return live data from SQLite. No placeholder values or hardcoded responses.

## Threat Flags

No new threat surface introduced beyond what is in the plan's threat model. All T-01-13 through T-01-18 mitigations are implemented:
- T-01-13: Version column WHERE clause enforced in all UPDATE operations
- T-01-14: Pagination clamped to max 100 per page
- T-01-15: RBAC middleware at router-group level
- T-01-16: No SQL DELETE statement exists — only soft-delete via status/deleted_at
- T-01-17: department_id FK validated against active departments before INSERT
- T-01-18: SQLite triggers from Plan 01 cover audit logging for all mutations

## Next Phase Readiness

- Employee, department, and global rules data layer is complete and tested
- Ready for Phase 02: attendance webhook processing (employees and departments are the FK targets)
- Ready for Phase 03: timesheet editor (employee/department CRUD APIs are the foundation)

---
*Phase: 01-foundation*
*Completed: 2026-04-14*
