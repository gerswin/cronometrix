---
phase: 05-reports-payroll-export
plan: 02
subsystem: backend
tags: [reports, money_math, periods, lottt, sql_aggregation, override_merge, audit_log, rbac, libsql, axum, validator, proptest, dos_guard]

# Dependency graph
requires:
  - phase: 05-01
    provides: "tenant_info singleton + GET endpoint (header.client_name/client_rif source); audit_log.operation CHECK accepts 'REPORT_EXPORT'; employees.position column populates EmployeeReportRow.cargo"
  - phase: 03-time-calculation-engine
    provides: "daily_records (work/ot/late minutes, shift_type, is_rest_day_worked, leave_id), daily_record_overrides (override_work_minutes, status), daily_record_anomalies (codes for GROUP_CONCAT), leaves (medical/vacation/unpaid/manual)"
  - phase: 01-foundation
    provides: "AppError + IntoResponse, params_from_iter pattern, require_supervisor_or_above middleware, AuthUser extractor, validator-derive DTO pattern, libSQL connection via AppState"
provides:
  - "POST /api/v1/reports/json — Admin/Supervisor only, 60s timeout, returns ReportPayload with header + rows[] + dept_subtotals[] + grand_total + departments_in_order[]"
  - "reports::service::compute_report — pure aggregation entry point that Plan 05-03 (Excel) and Plan 05-04 (PDF) consume directly with no transformation"
  - "reports::money::{work_pay,ot_pay,night_premium,rest_day_surcharge,late_deduction,total_a_pagar}_cents — pure cents-i64 helpers with property tests; usable from any future code that needs LOTTT premium math"
  - "reports::periods::{PeriodPreset,resolve_period,parse_period} — period boundary math (ISO weekly, calendar bi-weekly, monthly, custom)"
  - "App-code audit insert pattern reused for any future export route (action='REPORT_EXPORT'; payload_json captures filters; actor_id from JWT)"
affects: [05-03-excel-export, 05-04-frontend-reports-screen]

# Tech tracking
tech-stack:
  added: []  # proptest already existed in dev-dependencies
  patterns:
    - "Multi-source aggregation: primary daily_records JOIN + secondary leaves-table aggregation merged into per-employee accumulator (W-5 fix). The daily_records branch handles money math; leave-day counters come exclusively from the leaves aggregation so we never double-count."
    - "W-6 source-of-truth choice: read per-day actual values (dr.shift_type) over policy/default values (d.shift_type) when the engine output is authoritative for what happened on a day."
    - "Pure-module + integration-test split: money.rs and periods.rs ship with inline #[cfg(test)] tests (unit + property); the SQL/HTTP wiring lives in service.rs/handlers.rs and is exercised end-to-end via tests/reports_test.rs. Lets us iterate on math without paying a DB-spinup cost."
    - "App-code audit insert AFTER success (Pitfall 7): the audit row is the LAST thing compute_report does before Ok(...); failed compute paths return without writing, so failed exports never leave a misleading 'this happened' audit trail."
    - "App-supplied actor_id (T-05-14): write_export_audit takes actor_id as a parameter sourced from JWT claims in the handler; request body cannot influence it."

key-files:
  created:
    - "backend/src/reports/mod.rs"
    - "backend/src/reports/money.rs"
    - "backend/src/reports/periods.rs"
    - "backend/src/reports/models.rs"
    - "backend/src/reports/service.rs"
    - "backend/src/reports/handlers.rs"
    - "backend/tests/reports_test.rs"
    - "backend/tests/fixtures/reports/seed.rs"
    - "bruno/cronometrix/reports/01_json.bru"
  modified:
    - "backend/src/lib.rs"
    - "backend/src/main.rs"

key-decisions:
  - "Stub service.rs in Task 1 commit so the module compiles before Task 2 fills it in. Lets the property tests for money + periods green-light independently of the SQL wiring (clean TDD bisection if a regression hits)."
  - "Test fixtures live at backend/tests/fixtures/reports/seed.rs (per plan spec) and are pulled into reports_test.rs via #[path = ...] mod seed; — avoids restructuring the existing fixtures/ binary-blob layout into a Rust module tree."
  - "Bonus test include_inactive_filter_works added beyond the 25 named tests in the plan: directly exercises the include_inactive predicate on both the daily_records JOIN and the leaves aggregation (parameterization + filter symmetry across the two queries)."
  - "Removed scaffold leftover in audit_entry_on_export: cleaner pattern shares the libsql::Database via Arc clone before make_state takes ownership of the original."

patterns-established:
  - "When a single SQL JOIN cannot see all the relevant rows for a counter (here: full-period leaves with zero engine captures), run a SECOND scoped aggregation and merge into a shared per-entity accumulator BEFORE building the response. Both queries must apply the same filter set (department_ids, employee_id, include_inactive) so they stay coherent."
  - "Reports as derived data: never persist money on daily_records; recompute at read time from the engine's time-only output. Future policy changes (e.g. CBA-specific multipliers) only touch reports/money.rs."

requirements-completed: [PAY-01, PAY-02]

# Metrics
duration: ~70min
completed: 2026-04-25
---

# Phase 5 Plan 2: Reports Calculation JSON API Summary

**LOTTT-correct pre-payroll calculation API with override-merge across daily_records + leaves, secondary leaves aggregation for full-period leaves with zero captures (W-5), per-day shift_type-driven night-premium gating (W-6), 422 DoS guard on > 366-day periods, and app-code audit log entry per export. POST /api/v1/reports/json gated to Admin + Supervisor with a 60s tower_http TimeoutLayer; full backend suite green at 249/249.**

## Performance

- **Duration:** ~70 min
- **Started:** 2026-04-26T00:00Z (worktree base 0448a42)
- **Tasks:** 2
- **Files created:** 9
- **Files modified:** 2
- **Tests added:** 30 (16 unit + 2 proptest in money, 12 in periods, 26 integration in reports_test — bonus include_inactive case beyond the 25 named in the plan)
- **Backend suite:** 249/249 passing (no regressions)

## Accomplishments

- **Pure money math** (`reports/money.rs`) implementing LOTTT Art. 117/118/120 premiums with cents-i64 integer arithmetic, `checked_mul` overflow guards on every multi-step formula, and `saturating_add`/`saturating_sub` on the final composer. 16 unit tests anchor each formula to the reference example (e.g. `work_pay_cents(240, 100_000, 480) == 50_000`); 2 property tests cover monotonicity in worked-minutes and no-panic over realistic ranges (10k cases). Night premium is ADDITIVE per D-31 — the column shows the +30% surcharge separately so payroll teams audit it.
- **Period boundary math** (`reports/periods.rs`) with `PeriodPreset::{Weekly, BiweeklyFirst, BiweeklySecond, Monthly, Custom}` and `resolve_period` / `parse_period`. ISO 8601 Mon-Sun weekly, calendar 1-15 / 16-EOM bi-weekly, full-month default, and a December year-rollover test. Unknown `period_type` strings surface `AppError::Validation` (→ HTTP 422).
- **Reports DTOs** (`reports/models.rs`): `ReportParamsRequest` (validator-derived), `ReportPayload`, `BrandingHeader`, `EmployeeReportRow` (with `#[serde(flatten)]` Aggregates so the wire shape is the column set the Excel/PDF layouts expect), `Aggregates`, `DeptSummary`, `DeptSubtotal`. All seven structs ship in one file.
- **Reports service** (`reports/service.rs`): the calculation engine.
  - **Primary aggregation** — single dynamic SQL JOIN across `daily_records dr` + `employees e` + `departments d` + `LEFT JOIN daily_record_overrides dro ON ... AND dro.status='active'` + `LEFT JOIN leaves l ON l.id = dr.leave_id AND l.status='active'` + a `GROUP_CONCAT(code)` subquery against `daily_record_anomalies`. Override-merge via `override_work_min_opt.unwrap_or(work_minutes)` (Pitfall 3 — operator edits invisible if skipped).
  - **W-5 secondary aggregation** — separate query against `leaves` only, scoped to the same employee filter, computing overlap days with `[from..to]` per leave row and merging into the per-employee accumulator. Leave-day counters (`days_ivss`, `days_vacation`, `days_permission`, `days_unpaid`) come EXCLUSIVELY from this branch so the primary daily_records branch can't double-count. Vacation pay continues to come from the daily_records branch when an overlay is attached (a documented v1 limitation: leave-only days produce counter increments but no synthesized pay).
  - **W-6 night-premium gating** — reads `dr.shift_type` (per-day actual) instead of `d.shift_type` (policy/default). The engine's per-day output is authoritative for what happened on a given day. Two integration sub-cases pin the behaviour: dept='day' + dr='night' → premium applied; dept='night' + dr='day' → no premium.
  - **DoS guard** — `(to - from).num_days() > 366` returns `AppError::Validation` → HTTP 422 BEFORE any DB work.
  - **App-code audit insert** — runs AFTER aggregation succeeds (Pitfall 7), action='REPORT_EXPORT', payload_json captures `period_type`, `from_date`, `to_date`, all four filters, and `format`. `actor_id` sourced from JWT claims (T-05-14).
- **Handler + route** — `POST /api/v1/reports/json` wired in `main.rs` with `require_supervisor_or_above` + `tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60))` (D-25). Returns `Json(ReportPayload)` on success.
- **Integration tests** — 26 tests in `tests/reports_test.rs`: 4 period-preset assertions in one test, override-merge precedence, all four leave types, W-5 full-week vacation with no captures (`days_vacation=5, days_absent=0`), W-5 medical variant, W-5 no-double-count when overlay + leave-row both span Mon-Fri (`days_vacation=5` not 6), W-6 dr-vs-d shift_type test (both sub-cases in one test) plus two existing variants, anomaly column population, dept subtotals reconciling with constituents, grand_total reconciling with subtotals, RBAC matrix (admin 200 / supervisor 200 / viewer 403), audit row on success / no row on failure (Pitfall 7), 422 on 731-day range with the exact error code + message, `department_ids` filter, rest-day surcharge gating, `days_worked` count, weekend-excluded `days_absent`, and a bonus `include_inactive_filter_works` covering the predicate symmetry between the two SQL queries.
- **Bruno smoke** — `bruno/cronometrix/reports/01_json.bru` for monthly POST against `{{baseUrl}}/api/v1/reports/json` with bearer auth.

## Task Commits

Each task committed atomically with `--no-verify` (worktree mode):

1. **Task 1: Pure money + period modules with property tests** — `bd09d67` (feat)
2. **Task 2: Reports JSON API + service + audit + integration tests** — `7f9c5a1` (feat)

## Files Created/Modified

### Created

- `backend/src/reports/mod.rs` — module index re-exporting the public surface.
- `backend/src/reports/money.rs` — pure cents-i64 LOTTT premium math with 16 unit tests + 2 property tests.
- `backend/src/reports/periods.rs` — `PeriodPreset` enum + `resolve_period` + `parse_period` with 12 tests including leap-year and December rollover.
- `backend/src/reports/models.rs` — DTOs (`ReportParamsRequest`, `ReportPayload`, `BrandingHeader`, `EmployeeReportRow`, `Aggregates`, `DeptSummary`, `DeptSubtotal`).
- `backend/src/reports/service.rs` — `compute_report` with primary daily_records JOIN, W-5 secondary leaves aggregation, days_absent computation, dept-subtotal/grand-total accumulators, app-code audit insert.
- `backend/src/reports/handlers.rs` — `generate_json` axum handler.
- `backend/tests/reports_test.rs` — 26 integration tests (all 25 named in the plan + 1 bonus include_inactive case).
- `backend/tests/fixtures/reports/seed.rs` — seed helpers (`seed_dept`, `seed_employee`, `seed_inactive_employee`, `seed_daily_record`, `seed_override`, `seed_leave`, `seed_anomaly`, `set_tenant_branding`).
- `bruno/cronometrix/reports/01_json.bru` — monthly POST /reports/json smoke request with bearer auth.

### Modified

- `backend/src/lib.rs` — `pub mod reports;` added in alphabetical order between `recompute` and `rules`.
- `backend/src/main.rs` — `use cronometrix_api::reports;` import, new `report_routes` group with `TimeoutLayer::new(std::time::Duration::from_secs(60))` + `require_supervisor_or_above`, merged into the `/api/v1` nest in front of `admin_routes`.

## Decisions Made

- **Stub Task 1, full implementation Task 2.** Task 1 ships `service.rs` and `handlers.rs` as compiling stubs returning `AppError::Internal`. This lets the pure-module property tests green independent of the SQL wiring, gives a clean TDD bisection point if a future regression hits, and matches the plan's two-task split.
- **Test fixtures live at `tests/fixtures/reports/seed.rs` per plan spec.** The existing `tests/fixtures/` directory holds binary blobs only; rather than convert it into a Rust module tree (which would force changes across multiple unrelated test binaries), the seed module is pulled into `reports_test.rs` via `#[path = "fixtures/reports/seed.rs"] mod seed;`. Same disk layout, same module access, no collateral changes.
- **Bonus `include_inactive_filter_works` test.** Beyond the 25 named tests, this one directly exercises the `include_inactive` predicate against BOTH the primary daily_records JOIN and the W-5 leaves aggregation in a single test. Filter symmetry between the two queries is a regression-prone area; this test pins it.
- **Display `shift_type` in `EmployeeReportRow` reflects the most-recent day seen** (with dept policy as fallback for leave-only employees). This is display-only — the night-premium gating decision is per-day inside the JOIN, never via this field.

## Deviations from Plan

None. Plan executed exactly as written; W-5 and W-6 fixes from the refined plan are honored in both the service implementation and the integration tests.

## Issues Encountered

- The `tower_http::timeout::TimeoutLayer::new` API is marked deprecated in favor of `with_status_code`, surfacing as a single `#[warn(deprecated)]` warning. The plan's acceptance criteria explicitly require the literal `TimeoutLayer::new(std::time::Duration::from_secs(60))`, so the call shape is preserved verbatim. Migration to `with_status_code` is a harmless follow-up that doesn't affect behavior.
- The pre-existing `common::mock_hikvision::tests::fixture_heartbeat_exists_and_contains_heartbeat_marker` test is flagged as "leaky" by nextest. This is a side effect of `common/` being shared across all integration test binaries, unrelated to Plan 05-02 changes — confirmed by running pre- and post-Task 2 and observing the same status.
- Pre-commit hook bypassed via `--no-verify` per parallel-executor instructions; no formatter divergence in the modified files.

## User Setup Required

None — no external service configuration required.

## Threat Surface Verification

The plan's `<threat_model>` enumerated seven threats; each is mitigated by the implementation:

- **T-05-08 (Tampering / SQLi):** Both the primary daily_records JOIN AND the W-5 leaves aggregation push every user-supplied value into `Vec<libsql::Value>` and execute via `libsql::params_from_iter`. Zero string concatenation of user input. Verified by `department_filter_applied` test.
- **T-05-09 (EoP):** `require_supervisor_or_above` on the `/reports/json` route group; `viewer_blocked_on_export` test asserts 403.
- **T-05-10 (DoS):** `(to - from).num_days() > 366` rejected with HTTP 422 (`period_too_long_rejected` test asserts both status code and `code == "VALIDATION_ERROR"` + the literal "Period range cannot exceed 366 days" message). 60s `TimeoutLayer` caps execution at the route layer.
- **T-05-11 (Repudiation):** `write_export_audit` runs AFTER successful aggregation (Pitfall 7), inserts an `audit_log` row with `action='REPORT_EXPORT'`, `payload_json` containing all filters, and `actor_id` sourced from JWT. `audit_entry_on_export` and `no_audit_on_failure` tests verify both happy path and failure mode.
- **T-05-12 (Information Disclosure):** RBAC middleware blocks Viewer (`viewer_blocked_on_export`); HTTPS terminates outside this layer. Accepted v1 stance: no `Cache-Control: no-store` since data is operational and AuthN is required.
- **T-05-13 (Tampering / race):** Reports are read-only on daily_records; concurrent edits during generation simply show in the next regeneration (D-11 explicitly accepts this).
- **T-05-14 (Tampering / actor_id):** `write_export_audit` takes actor_id as a parameter sourced from `claims.sub` in the handler — request body cannot spoof identity.

## Next Plan Readiness

- **Plan 05-03 (Excel export)** can call `reports::compute_report(&state, actor_id, &params, "excel").await?` directly to get a `ReportPayload`, then feed it into `excel::build_workbook`. The audit insert in compute_report already takes a `format` parameter — passing `"excel"` records the export format in `payload_json`. No new module surface needed.
- **Plan 05-04 (Frontend reports screen + PDF)** can `POST /api/v1/reports/json` with the same `ReportParamsRequest` shape, render the summary table from `payload.rows[]`, the dept blocks from `payload.dept_subtotals[]`, and the totals from `payload.grand_total`. PDF generation feeds `payload` into `jspdf-autotable` client-side per D-23.
- **No blockers.** All Phase 3 daily_records / overrides / leaves / anomalies surfaces are stable; Plan 05-01's `tenant_info` GET + `audit_log.operation` CHECK relaxation are already merged.

## Self-Check: PASSED

Verified files exist and commits are present:

- `backend/src/reports/mod.rs` — FOUND
- `backend/src/reports/money.rs` — FOUND
- `backend/src/reports/periods.rs` — FOUND
- `backend/src/reports/models.rs` — FOUND
- `backend/src/reports/service.rs` — FOUND
- `backend/src/reports/handlers.rs` — FOUND
- `backend/tests/reports_test.rs` — FOUND
- `backend/tests/fixtures/reports/seed.rs` — FOUND
- `bruno/cronometrix/reports/01_json.bru` — FOUND
- Commit `bd09d67` (Task 1) — FOUND in git log
- Commit `7f9c5a1` (Task 2) — FOUND in git log

---
*Phase: 05-reports-payroll-export*
*Completed: 2026-04-25*
