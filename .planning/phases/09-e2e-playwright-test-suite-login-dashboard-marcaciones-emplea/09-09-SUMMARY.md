---
phase: 09
plan: 09
subsystem: e2e-playwright
tags: [e2e, playwright, timesheet, employees, audit, crud, rbac]
dependency_graph:
  requires: [09-01, 09-04, 09-06]
  provides: [timesheet-e2e-spec, employees-e2e-spec]
  affects: [frontend/e2e/timesheet.spec.ts, frontend/e2e/employees.spec.ts]
tech_stack:
  added: []
  patterns:
    - mutation-to-audit assertion via getAudit + expect.poll (reusable pattern for Plan 10)
    - employee CRUD dialogs (create/edit/deactivate) wired to backend REST endpoints
    - data-testid on Radix UI Dialog via DialogContent props forwarding
key_files:
  created:
    - frontend/e2e/timesheet.spec.ts
    - frontend/e2e/employees.spec.ts
  modified:
    - frontend/e2e/fixtures/selectors.ts
    - frontend/src/components/timesheet/novedad-modal.tsx
    - frontend/src/components/timesheet/timesheet-table.tsx
    - frontend/src/app/(dashboard)/timesheet/page.tsx
    - frontend/src/components/employees/employee-table.tsx
    - frontend/src/app/(dashboard)/employees/page.tsx
    - frontend/src/types/api.ts
    - frontend/src/components/enrollment/__tests__/employee-enrollment-picker.test.tsx
    - frontend/src/components/enrollment/__tests__/in-progress-list.test.tsx
    - frontend/src/components/enrollment/__tests__/enrollment-modal.test.tsx
    - frontend/src/components/enrollment/__tests__/enrollment-modal-extra.test.tsx
    - frontend/src/components/employees/__tests__/employee-table.test.tsx
decisions:
  - "Employee CRUD dialogs built inline in employees/page.tsx — no separate component file; sufficient for E2E test surface without adding component coverage scope"
  - "deactivate button rendered only when onDeactivateClick prop supplied AND row.original.status === 'active' — avoids inactive-row deactivate attempts"
  - "Employee.version: number added to frontend API type to support optimistic concurrency in edit mutations — aligned with backend UpdateEmployeeRequest"
  - "hire_date: string | null in Employee type — aligns with backend Option<String> serialization"
  - "Pre-existing dashboard-activity-feed-extra.test.tsx failure (1 test) is out-of-scope — confirmed pre-existing by stash verification; not caused by this plan"
metrics:
  duration_minutes: 11
  completed_date: "2026-04-29"
  tasks_completed: 3
  files_created: 2
  files_modified: 11
---

# Phase 9 Plan 9: Timesheet + Employees D-03 UAT Specs Summary

**One-liner:** D-03 CRUD E2E specs for timesheet (marcaciones) and employees — 17 tests with mutation→audit assertions, RBAC negative test, full create/edit/deactivate lifecycle wired to backend REST API.

## What Was Built

### Task 1 — Data-testid additions to mutation entry points

Added deterministic `data-testid` attributes to all mutation surfaces:

**novedad-modal.tsx:**
- `data-testid="novedad-modal"` on `DialogContent`
- `data-testid="novedad-justification"` on the textarea
- `data-testid="novedad-evidence"` on the file input
- `data-testid="novedad-submit"` on the submit button

**timesheet/page.tsx + timesheet-table.tsx:**
- `data-testid="open-novedad-modal"` on the global Registrar Novedad button (page level) and on each row's pencil edit button (table level)

**employees/page.tsx:**
- `data-testid="new-employee-button"` on the Nuevo Empleado button
- `data-testid="new-employee-form"` on the create dialog's `DialogContent`
- `data-testid="new-employee-submit"` on the create dialog's submit button
- `data-testid="edit-employee-form"` on the edit dialog's `DialogContent`

**employee-table.tsx:**
- `data-testid={emp-actions-${id}}` on the actions div wrapper per row
- `data-testid={emp-action-edit-${id}}` on the edit pencil button per row
- `data-testid={emp-action-deactivate-${id}}` on the new deactivate button per active row

**selectors.ts (SEL catalog):**
- `openNovedadModal`, `novedadModal`, `novedadJustification`, `novedadEvidence`, `novedadSubmit`
- `newEmpButton`, `newEmpForm`, `newEmpSubmit`
- `empActions(id)`, `empActionEdit(id)`, `empActionDeactivate(id)`

### Task 1b — Employee CRUD dialogs (Rule 2 deviation)

The employees page only had a stub "Nuevo Empleado" button with no form. Full CRUD required for specs to exercise audit mutations. Added:

- **Create dialog** — `new-employee-form` dialog with name/employee_code/department_id fields, Zod validation, POST to `/employees`
- **Edit dialog** — `edit-employee-form` dialog pre-populated from row data, PATCH to `/employees/:id` with optimistic concurrency version
- **Deactivate confirm dialog** — DELETE `/employees/:id` (soft delete) with confirmation step
- **Employee.version** added to `src/types/api.ts` (aligns with backend model)
- **hire_date: string | null** corrected in Employee type
- 5 test fixture files updated to include `version: 1` (required after type change)

### Task 2 — timesheet.spec.ts (8 tests)

| Test | Coverage |
|------|----------|
| T-01 | Marcaciones heading visible |
| T-02 | Admin sees open-novedad-modal button |
| T-03 | Grid lists Ana Pérez after events pushed |
| T-04 | Week navigator present for period navigation |
| T-05 | Clicking Registrar Novedad opens novedad-modal |
| T-06 | Empty justification validation blocks submit |
| T-07 | Happy path: novedad with evidence → audit_log daily_record_overrides INSERT |
| T-08 | Global button (no record) → audit_log leaves INSERT |

**Audit assertions:** 2 tests (T-07, T-08) use `getAudit + expect.poll` pattern.

### Task 3 — employees.spec.ts (9 tests)

| Test | Coverage |
|------|----------|
| T-01 | Lists seeded employees with Spanish heading |
| T-02 | Search by name filters list |
| T-03 | Department filter shows only Producción employees |
| T-04 | Nuevo Empleado button opens create dialog |
| T-05 | Validation: missing name shows alert + keeps dialog open |
| T-06 | Happy create: fill form → list update + audit employees INSERT |
| T-07 | Edit employee: name change → list update + audit employees UPDATE |
| T-08 | Deactivate employee: confirm → audit employees DELETE (soft delete) |
| T-09 | RBAC: Viewer cannot see new-employee-button (count=0) |

**Audit assertions:** 3 tests (T-06, T-07, T-08) use `getAudit + expect.poll` pattern.

## Test Totals

| Spec | Tests | Audit assertions | RBAC tests |
|------|-------|-----------------|------------|
| timesheet.spec.ts | 8 | 2 | 0 |
| employees.spec.ts | 9 | 3 | 1 |
| **Total** | **17** | **5** | **1** |

## Mutation → Audit Pattern (reuse in Plan 10)

```ts
// Standard pattern: expect.poll + getAudit with 15s timeout
await expect.poll(
  async () => {
    const r = await getAudit(request, {
      table_name: 'employees',
      operation: 'INSERT',
      limit: 5,
    })
    if (r.status() !== 200) return null
    const body = await r.json()
    return body.total ?? body.data?.length ?? 0
  },
  { timeout: 15_000, message: 'Expected audit_log entry for employees INSERT' },
).toBeGreaterThanOrEqual(1)
```

Plan 10 (devices + reports) should mirror this pattern.

## SEL Catalog Additions (for Plan 10 reference)

```ts
openNovedadModal: 'open-novedad-modal',
novedadModal:     'novedad-modal',
novedadJustification: 'novedad-justification',
novedadEvidence:  'novedad-evidence',
novedadSubmit:    'novedad-submit',
newEmpButton:     'new-employee-button',
newEmpForm:       'new-employee-form',
newEmpSubmit:     'new-employee-submit',
empActions:       (id) => `emp-actions-${id}`,
empActionEdit:    (id) => `emp-action-edit-${id}`,
empActionDeactivate: (id) => `emp-action-deactivate-${id}`,
```

## Deviations from Plan

### Auto-added Missing Critical Functionality

**1. [Rule 2 - Missing critical functionality] Employee CRUD dialogs (create/edit/deactivate)**
- **Found during:** Task 1 — the "Nuevo Empleado" button had no form; employee table had no deactivate action
- **Issue:** Specs required `new-employee-form`, `new-employee-submit`, `emp-action-deactivate-{id}` but none existed in the UI
- **Fix:** Added inline create dialog, edit dialog, deactivate confirm dialog to `employees/page.tsx`; added deactivate button + `onEditClick`/`onDeactivateClick` props to `employee-table.tsx`
- **Files modified:** `frontend/src/app/(dashboard)/employees/page.tsx`, `frontend/src/components/employees/employee-table.tsx`
- **Commits:** 8ace212

**2. [Rule 1 - Bug] Employee.version missing from frontend type**
- **Found during:** Task 1b — TypeScript error TS2339 on `editEmployee!.version` in the new edit mutation
- **Issue:** The frontend `Employee` interface did not include `version: number` (present in backend model) and `hire_date` was `string` instead of `string | null`
- **Fix:** Added `version: number` and corrected `hire_date: string | null` in `src/types/api.ts`; updated 5 test fixtures that required `version` field
- **Files modified:** `frontend/src/types/api.ts`, 5 test fixture files
- **Commit:** 8ace212

### Out-of-scope pre-existing failure (not fixed)

`src/__tests__/dashboard-activity-feed-extra.test.tsx` — 1 test failing with "Found multiple elements with the text: /—/". Confirmed pre-existing by git stash verification. Logged here for tracking; not caused by Plan 09-09.

## Known Stubs

None — all E2E spec test flows are wired to real backend API calls.

## Threat Coverage

| Threat ID | Mitigation |
|-----------|-----------|
| T-09-03 (Audit Bypass) | All mutation tests (T-07, T-08 in timesheet; T-06, T-07, T-08 in employees) assert audit_log entry via getAudit |
| T-09-03-rbac (RBAC elevation) | employees spec T-09 asserts viewer cannot see new-employee-button (count=0) |

## Self-Check

Files exist:

- `frontend/e2e/timesheet.spec.ts` — FOUND
- `frontend/e2e/employees.spec.ts` — FOUND
- `frontend/e2e/fixtures/selectors.ts` (openNovedadModal, newEmpButton entries) — FOUND
- `frontend/src/components/timesheet/novedad-modal.tsx` (novedad-modal testid) — FOUND
- `frontend/src/components/employees/employee-table.tsx` (emp-action-edit-{id}) — FOUND
- `frontend/src/app/(dashboard)/employees/page.tsx` (new-employee-button, dialogs) — FOUND

Commits exist:

- 7c8f8f5 feat(09-09): data-testid additions — FOUND
- 8ace212 feat(09-09): employee CRUD dialogs + type fix — FOUND
- 2ee0704 feat(09-09): timesheet.spec.ts — FOUND
- a52e048 feat(09-09): employees.spec.ts + timesheet type fix — FOUND

## Self-Check: PASSED
