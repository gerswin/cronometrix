---
phase: 09
plan: 05
subsystem: frontend-audit-ui
tags: [audit, react, tanstack-table, vitest, rbac, tdd]
dependency_graph:
  requires: [09-04-audit-read-api, 01-foundation-auth-rbac]
  provides: [AuditPage UI, AuditTable component, AuditFilters component, DiffCell component, AuditEntry type]
  affects: [Plan 09-11 audit.spec.ts, Plan 09-10+ Wave 2 CRUD specs mutation→audit assertions]
tech_stack:
  added:
    - frontend/src/types/audit.ts (AuditEntry interface)
    - frontend/src/components/audit/diff-cell.tsx
    - frontend/src/components/audit/audit-table.tsx
    - frontend/src/components/audit/audit-filters.tsx
  patterns:
    - TanStack Table v8 manualPagination + manualFiltering (mirrors employee-table.tsx)
    - TanStack Query v5 useQuery + api axios client (mirrors employees/page.tsx)
    - Per-file Vitest coverage >= 70% lines / 60% branches (Phase 8 D-22 floor)
key_files:
  created:
    - frontend/src/types/audit.ts
    - frontend/src/components/audit/diff-cell.tsx
    - frontend/src/components/audit/audit-table.tsx
    - frontend/src/components/audit/audit-filters.tsx
    - frontend/src/components/audit/__tests__/diff-cell.test.tsx
    - frontend/src/components/audit/__tests__/audit-table.test.tsx
  modified:
    - frontend/src/app/(dashboard)/audit/page.tsx
decisions:
  - "W6 actor dropdown — OPTION A: derive distinct actor IDs from current page data (not /audit/actors endpoint which was not implemented in Plan 09-04)"
  - "AuditTable pagination controls always visible (not hidden when pageCount <= 1) so E2E specs can always interact with audit-pagination-prev and audit-pagination-next"
  - "DiffCell renders both INSERT-with-null-new_data and UPDATE-path via guard order (both-null guard first)"
  - "audit-filters.tsx date inputs convert YYYY-MM-DD to epoch seconds (from_ts = start of day, to_ts = end of day)"
metrics:
  duration: "7 minutes"
  completed: "2026-04-29"
  tasks_completed: 3
  files_changed: 7
---

# Phase 09 Plan 05: Audit UI Page Summary

**One-liner:** Paginated audit list page (TanStack Table + TanStack Query) with DiffCell JSON diff renderer, 6-axis filter form, RBAC gate, and 33 Vitest tests — replaces the 10-line placeholder.

## What Was Built

### Components Added

| Component | File | Description |
|-----------|------|-------------|
| `AuditEntry` | `frontend/src/types/audit.ts` | TypeScript interface (8 fields) mirroring backend `AuditEntry` serialization |
| `DiffCell` | `frontend/src/components/audit/diff-cell.tsx` | Collapsible `<details>` diff summary for INSERT/UPDATE/DELETE operations |
| `AuditTable` | `frontend/src/components/audit/audit-table.tsx` | TanStack Table v8 with 6 columns, per-row `data-testid`, empty state, pagination |
| `AuditFilters` | `frontend/src/components/audit/audit-filters.tsx` | 6 filter inputs with epoch conversion for date range |

### Audit Page

`frontend/src/app/(dashboard)/audit/page.tsx` replaces the "Próximamente" placeholder with:
- TopBar with title "Auditoría"
- RBAC gate: `role !== 'admin' && role !== 'supervisor'` → renders `<AccessRestricted />`; useQuery `enabled` flag prevents API call for Viewer
- `AuditFilters` with 6 inputs; onChange resets pageIndex to 0
- `AuditTable` with TanStack Query fetching `GET /api/v1/audit?limit=20&offset=...&...filters`
- `data-testid="audit-page"` on root container

## Data-Testids Added (Plan 11 audit.spec.ts dependency — LOCKED)

| data-testid | Element | Purpose |
|-------------|---------|---------|
| `audit-page` | root `<div>` in page.tsx | E2E top-level wait |
| `audit-table` | wrapper `<div>` in audit-table.tsx | E2E explicit-wait visibility check |
| `audit-row-${id}` | `<tr>` per entry | E2E row targeting by audit entry ID |
| `audit-empty` | `<tr>` when data=[] | E2E empty state assertion |
| `audit-pagination-prev` | `<button>` | E2E pagination interaction |
| `audit-pagination-next` | `<button>` | E2E pagination interaction |
| `audit-filter-actor` | `<select>` | E2E actor filter — value is actor_id (e.g. "e2e-admin-id") |
| `audit-filter-table` | `<select>` | E2E table filter |
| `audit-filter-from` | `<input type="date">` | E2E date range from |
| `audit-filter-to` | `<input type="date">` | E2E date range to |
| `audit-filter-operation` | `<select>` | E2E operation filter |
| `audit-filter-record-id` | `<input type="text">` | E2E record ID filter |

## TABLE_OPTIONS (valid table names for the dropdown)

```
employees, departments, leaves, daily_records, daily_record_overrides,
devices, rules, tenant_info, enrollments
```

These match the tables with audit triggers in migrations 002, 006, 011, 014, 017.

## Actor Dropdown — W6 Fix: OPTION A

The plan offered two options for the actor dropdown data source:

**OPTION A (chosen):** Derive distinct actor IDs from the current page's `data` response.
- Reason: Plan 09-04 explicitly did NOT implement `GET /api/v1/audit/actors`; adding it is out of scope for Plan 09-05.
- Implementation: `useMemo` over `data?.data` extracts distinct `actor_id` values; `{ id, username }` where `username === actor_id` (we don't have a join to `users.username` without a new endpoint).
- Plan 11 impact: `selectOption('e2e-admin-id')` works because `<option value={actor_id}>` — the actor_id IS the value. Plan 11 executor must use actor_id strings directly (not usernames).
- Note: `actor_id` in `audit_log` is `users.id`, NOT `employees.id` — different tables.

If `GET /api/v1/audit/actors` is implemented in a later plan, switch to OPTION B by replacing the `useMemo` with a `useQuery(['audit-actors'], ...)` call.

## DiffCell Behavior (all 4 cases)

| operation | old_data | new_data | Renders |
|-----------|----------|----------|---------|
| Any | null | null | `<span>—</span>` |
| INSERT | null | `{a:1, b:2}` | `<details><summary>+ 2 campos</summary>…</details>` |
| DELETE | `{x:1}` | null | `<details><summary>- 1 campos</summary>…</details>` |
| UPDATE | `{a:1, b:2}` | `{a:1, b:3, c:5}` | `<details><summary>~ 2 cambios</summary>…</details>` (b changed + c added) |

## Test Coverage

### Tests Added

| File | Tests |
|------|-------|
| `src/components/audit/__tests__/diff-cell.test.tsx` | 10 (DiffCell — all 4 operation cases + edge cases) |
| `src/components/audit/__tests__/audit-table.test.tsx` | 23 (AuditTable — rows, empty state, pagination, loading) + 16 (AuditFilters — inputs, dropdowns, epoch conversion, clearing) = 33 total |

**Total: 33 new Vitest tests**

### Per-file Coverage (Phase 8 D-22 floor: >= 70% lines / >= 60% branches)

| File | Lines | Branches | Status |
|------|-------|----------|--------|
| `src/components/audit/diff-cell.tsx` | 100% (18/18) | 88.9% (16/18) | PASS |
| `src/components/audit/audit-table.tsx` | 84.0% (21/25) | 75.0% (9/12) | PASS |
| `src/components/audit/audit-filters.tsx` | 100% (21/21) | 88.9% (24/27) | PASS |

`src/app/(dashboard)/audit/page.tsx` is excluded from Vitest per Phase 8 D-10 (`src/app/**` whitelist not in `include`); it is covered by Plan 11 E2E.

### Project-wide Gate

Branch coverage at 83.48% < 85% threshold — **this is a pre-existing failure** confirmed by running coverage on the commit immediately before Plan 09-05 (stash verification). Our new files raise coverage, not lower it. The pre-existing regression is documented as a deferred item.

## RBAC

| Role | Behavior |
|------|---------|
| Admin | Page renders; API call enabled; full audit list |
| Supervisor | Page renders; API call enabled; full audit list |
| Viewer | `<AccessRestricted />` rendered; `useQuery` disabled (no API call) |
| Anonymous | Redirect by Next.js middleware before page renders |

Backend `/api/v1/audit` is authoritative (403 for Viewer) — frontend is defense in depth per T-09-03.

## Deviations from Plan

### Plan deviation: W6 Actor Dropdown — OPTION A instead of OPTION B

**Found during:** Task 3 planning
**Issue:** Plan 09-04 explicitly did NOT implement `GET /api/v1/audit/actors` (documented in 09-04-SUMMARY.md "Plan deviation: GET /audit/actors not implemented"). The plan's template code used OPTION B which requires that endpoint.
**Fix:** Implemented OPTION A — derive distinct actor IDs from the current page's loaded `data` rows via `useMemo`. No new backend endpoint needed.
**Impact on Plan 11:** `selectOption('e2e-admin-id')` in `audit.spec.ts` must use actor_id string as the option value (not display name). Document in Plan 11 executor notes.
**Files modified:** `frontend/src/app/(dashboard)/audit/page.tsx`
**Classification:** [Rule 3 - Blocking] missing endpoint would have caused runtime error; OPTION A avoids the dependency.

### Coverage improvement: 7 extra AuditFilters tests added

**Found during:** Task 3 verification
**Issue:** After running per-file coverage, `audit-filters.tsx` showed 42.9% line coverage (below 70% floor) because `dateToEpoch` and `epochToDate` helper functions were not exercised.
**Fix:** Added 7 additional tests in `audit-table.test.tsx` covering: from/to date epoch conversion, clearing date inputs, clearing actor/operation dropdowns, clearing record_id text input. Coverage rose to 100% lines / 88.9% branches.
**Classification:** [Rule 2 - Missing critical functionality] per-file floor is a Phase 8 correctness requirement.

### Pre-existing issue: project branch coverage gate at 83.48%

**Scope:** Out of scope — pre-existing failure in unrelated files.
**Finding:** `make coverage-frontend` fails with "Coverage for branches (83.48%) does not meet global threshold (85%)". Confirmed via stash test that this failure existed before Plan 09-05 changes. Our new files actually improve coverage.
**Action:** Logged to deferred items. No fix applied (out of scope per deviation scope rule).

## Known Stubs

None — all data is wired to the live `/api/v1/audit` endpoint. The actor dropdown uses OPTION A (real data from current page). No placeholder text, no hardcoded empty arrays in rendered output.

## Threat Model Coverage

| Threat | Disposition | Verification |
|--------|-------------|-------------|
| T-09-03 Elevation of Privilege (Viewer) | mitigated | `role !== 'admin' && role !== 'supervisor'` → AccessRestricted; `enabled: false` prevents API call |
| T-09-03-info Information Disclosure | accept | RBAC restricts to Admin + Supervisor by design |

## Self-Check: PASSED

Files exist:
- `frontend/src/types/audit.ts` — FOUND
- `frontend/src/components/audit/diff-cell.tsx` — FOUND
- `frontend/src/components/audit/audit-table.tsx` — FOUND
- `frontend/src/components/audit/audit-filters.tsx` — FOUND
- `frontend/src/components/audit/__tests__/diff-cell.test.tsx` — FOUND
- `frontend/src/components/audit/__tests__/audit-table.test.tsx` — FOUND
- `frontend/src/app/(dashboard)/audit/page.tsx` — FOUND (no "Próximamente")

Commits exist:
- `df0f658` AuditEntry type + DiffCell + tests — FOUND
- `bf4e3dd` AuditTable + AuditFilters + tests — FOUND
- `f384087` audit page replacement + coverage improvement — FOUND

Verification:
- `npx vitest run src/components/audit/__tests__/` — 33 PASS 0 FAIL
- `npx next build` — Errors: 0 | Warnings: 0
- Per-file coverage floor — all 3 audit component files PASS (70% line / 60% branch)
- "Próximamente" not in audit/page.tsx — CONFIRMED
