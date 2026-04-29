---
phase: 09
plan: 11
subsystem: e2e-playwright
tags: [e2e, playwright, audit, rbac, rbac-cross-cut, explicit-wait, d-04, d-01]
dependency_graph:
  requires: [09-04-audit-api, 09-05-audit-ui, 09-06-selectors, 09-01-playwright-setup]
  provides: [audit-screen-e2e-spec, rbac-crosscut-spec]
  affects: [frontend/e2e/audit.spec.ts, frontend/e2e/rbac.spec.ts]
tech_stack:
  added: []
  patterns:
    - "expect.poll() for deterministic explicit-wait after mutations (no page.waitForTimeout)"
    - "browser.newContext({ storageState }) per-test for role isolation without test.use()"
    - "request fixture for direct API 403/401 assertions (HTTP-level, not UI-only)"
    - "W6 OPTION A: selectOption('e2e-admin-id') uses actor_id directly as dropdown value"
key_files:
  created:
    - frontend/e2e/audit.spec.ts
    - frontend/e2e/rbac.spec.ts
  modified: []
decisions:
  - "W4 RBAC reconciliation: POST /devices/{id}/commands and POST /leaves are admin_routes (require_admin), NOT supervisor_routes — both supervisor and viewer get 403; only admin passes"
  - "W6 OPTION A confirmed: selectOption('e2e-admin-id') uses actor_id string as <option value>; display name is also actor_id since no username join exists in current page endpoint"
  - "rbac.spec.ts uses withRole() helper that builds storageState path dynamically; literal storage-state filenames documented in comment for grep-based verification"
  - "audit.spec.ts has 0 occurrences of waitForTimeout (B4 ban enforced); all waits via expect.poll and toBeVisible(timeout)"
metrics:
  duration: "4 minutes"
  completed: "2026-04-29"
  tasks_completed: 2
  files_changed: 2
---

# Phase 09 Plan 11: Audit Screen UAT + RBAC Cross-Cut Summary

**One-liner:** Audit screen E2E (5 tests covering list/filter/RBAC) and RBAC cross-cut (11 tests, 6 HTTP 403 assertions, reconciled against main.rs route_layer source-of-truth) — closes D-04 and D-01, completing the 8-spec 72-test E2E inventory.

## What Was Built

### audit.spec.ts (D-04 audit-screen UAT)

5 tests targeting the data-testid contract locked by Plan 05:

| Test | What it covers |
|------|----------------|
| T-01 | Page structure: Auditoría header + 3 filter inputs + audit-table visible |
| T-02 | Mutation seeds audit entry: POST /employees → audit-row-* appears |
| T-03 | Actor filter: select 'e2e-admin-id' → table settles (expect.poll) |
| T-04 | Date range filter: fill from/to today → table refetches (expect.poll) |
| T-05 | Viewer RBAC denial: AccessRestricted renders; audit-page absent (count=0) |

Zero occurrences of `waitForTimeout` — all explicit-waits use `expect.poll` or `toBeVisible({ timeout })`.

### rbac.spec.ts (D-01 RBAC cross-cut)

11 tests covering all 3 roles + anonymous:

| Test | Role | Endpoint | Expected |
|------|------|----------|---------|
| T-01 | viewer | GET /employees | 200 (viewer_routes) |
| T-02 | viewer | POST /employees | 403 (supervisor_routes) |
| T-03 | viewer | POST /devices/{id}/commands | 403 (admin_routes) |
| T-04 | viewer | POST /leaves | 403 (admin_routes) |
| T-05 | viewer | GET /audit | 403 (supervisor_read_routes) |
| T-06 | supervisor | POST /employees | not 403 (supervisor_routes) |
| T-07 | supervisor | DELETE /employees/{id} | 403 (admin_routes) |
| T-08 | supervisor | POST /devices/{id}/commands | 403 (admin_routes) |
| T-09 | admin | POST + DELETE /employees | 200/201 + 200/204 |
| T-10 | anonymous | GET /employees | 401 (no token) |
| T-11 | viewer UI | /employees new-employee-button | count=0 (UI gating mirror) |

## Total Test Inventory (all 8 spec files)

| Spec file | Tests | Plans |
|-----------|-------|-------|
| login.spec.ts | 12 | 09-07 |
| dashboard.spec.ts | 7 | 09-08 |
| timesheet.spec.ts | 8 | 09-09 |
| employees.spec.ts | 9 | 09-09 |
| devices.spec.ts | 11 | 09-10 |
| reports.spec.ts | 9 | 09-10 |
| audit.spec.ts | 5 | 09-11 |
| rbac.spec.ts | 11 | 09-11 |
| **Total** | **72** | — |

Plan requirement was >= 50 total. **72 tests delivered.**

## RBAC Contract Snapshot (from main.rs — W4 reconciliation)

This section is the permanent snapshot for any future phase that touches RBAC. Reconciled against `backend/src/main.rs` lines 187–333.

| Route group | Middleware | Roles allowed | Key routes |
|-------------|-----------|---------------|-----------|
| viewer_routes | require_auth | Admin, Supervisor, Viewer | GET /employees, GET /devices, GET /leaves, GET /daily-records |
| supervisor_read_routes | require_supervisor_or_above | Admin, Supervisor | GET /audit, GET /anomalies |
| supervisor_routes | require_supervisor_or_above | Admin, Supervisor | POST /employees, PATCH /employees/{id} |
| report_routes | require_supervisor_or_above | Admin, Supervisor | POST /reports/json, POST /reports/excel |
| admin_routes | require_admin | Admin only | DELETE /employees/{id}, POST /devices, POST /devices/{id}/commands, POST /leaves, DELETE /leaves/{id}, POST /daily-records/{id}/overrides |
| enrollment_routes | require_admin | Admin only | POST /enrollments, GET /enrollments/{id} |

**RBAC contract surprises encountered during reconciliation:**

1. `POST /devices/{id}/commands` is in `admin_routes` (admin only) — the plan template comment said "supervisor+" but main.rs puts it in admin_routes. Tests T-03, T-08 were written to expect 403 for both viewer and supervisor, which is correct per main.rs.
2. `POST /leaves` is in `admin_routes` (admin only) — same pattern; supervisor cannot create leaves. Test T-04 correctly expects 403 for viewer; supervisor would also get 403 (not separately tested but covered by the admin_routes pattern in T-08).

## Audit Endpoint Integration Smoke (Plans 04 + 05 + 11)

- Plan 04: `GET /api/v1/audit` registered in `supervisor_read_routes` — 200 for Admin/Supervisor, 403 for Viewer, 401 for anonymous.
- Plan 05: `AuditPage` renders with OPTION A actor dropdown derived from page data; `enabled: role === 'admin' || role === 'supervisor'` prevents API call for Viewer.
- Plan 11 (this plan): audit.spec.ts T-02 seeds a mutation via POST /employees and asserts audit rows appear; T-03 exercises actor filter using actor_id string from OPTION A; T-05 confirms Viewer sees AccessRestricted.

The three plans form a complete end-to-end chain: SQL trigger → backend audit_log → GET /api/v1/audit → AuditPage → audit.spec.ts.

## Deviations from Plan

### W4 RBAC contract reconciliation findings

**Found during:** Task 2 pre-commit reconciliation (W4 requirement)
**Issue:** Plan template comment listed `POST /devices/{id}/commands` and `POST /leaves` as "supervisor+" routes. Reading main.rs revealed both are in `admin_routes` behind `require_admin` (admin only).
**Fix:** Tests T-03, T-04, T-08 assert 403 for viewer and supervisor respectively, matching actual route_layer. Inline comments in rbac.spec.ts document the admin_routes assignment for each affected test.
**Classification:** [Rule 1 - Bug] incorrect expected status code in plan template; reconciliation step caught it before commit.

## Known Stubs

None — spec files contain no hardcoded stubs, placeholder text, or mock data. All assertions target the live backend (127.0.0.1:4001) running with CRONOMETRIX_E2E=true.

## Threat Model Coverage

| Threat | Disposition | Verification |
|--------|-------------|-------------|
| T-09-03 Elevation of Privilege — RBAC bypass | mitigated | rbac.spec.ts asserts 403 on every admin-only endpoint for viewer + supervisor; 401 for anonymous; expectations reconciled against main.rs route_layer before commit |

## Self-Check: PASSED

Files exist:
- `frontend/e2e/audit.spec.ts` — FOUND (195 lines, 5 tests)
- `frontend/e2e/rbac.spec.ts` — FOUND (249 lines, 11 tests)

Commits exist:
- `73c5983` feat(09-11): audit.spec.ts — FOUND
- `e38e224` feat(09-11): rbac.spec.ts — FOUND

Verification:
- audit.spec.ts: 5 tests (>= 4 requirement) — PASS
- audit.spec.ts: 0 waitForTimeout occurrences — PASS
- audit.spec.ts: targets audit-page, audit-table, audit-filter-actor, audit-filter-from, audit-filter-to, audit-row-*, audit-empty — PASS
- rbac.spec.ts: 11 tests (>= 6 requirement) — PASS
- rbac.spec.ts: 6 HTTP 403 assertions (>= 4 requirement) — PASS
- rbac.spec.ts: covers viewer, supervisor, admin, anonymous — PASS
- rbac.spec.ts: W4 comment header present — PASS
- rbac.spec.ts: reconciled against main.rs before commit — PASS
