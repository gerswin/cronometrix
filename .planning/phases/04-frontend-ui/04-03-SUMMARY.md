---
phase: 04-frontend-ui
plan: "03"
subsystem: timesheet-editor
tags:
  - timesheet
  - override
  - daily-records
  - tdd
  - tanstack-table
  - react-hook-form
  - zod
  - audit
dependency_graph:
  requires:
    - 04-01  # app shell, auth context, api client, base types
    - 03-03  # leaves service (write_photo_atomic, multipart pattern)
  provides:
    - POST /api/v1/daily-records/{id}/overrides endpoint
    - daily_record_overrides table writes (with audit trigger)
    - timesheet screen at /timesheet (week nav + table + modal)
  affects:
    - daily_records table (recompute trigger after override)
    - audit_log (via SQLite trigger on daily_record_overrides INSERT)
tech_stack:
  added:
    - "@base-ui/react/dialog (Dialog UI wrapper)"
    - "novedadSchema (Zod v4, justification min(1))"
    - "TimesheetTable (@tanstack/react-table v8, manualPagination)"
    - "WeekNavigator (date-fns weekStartsOn:1)"
    - "NovedadModal (react-hook-form + zodResolver + Controller for file)"
  patterns:
    - "TDD RED/GREEN cycle: failing tests committed before implementation"
    - "UUID-named evidence paths under data/overrides/ (T-4-10)"
    - "require_admin middleware on override route (T-4-09, T-4-13)"
    - "Role-gated edit icon: role==='admin' only (D-14)"
    - "evidenceFileSchema reused from validations.ts (5MB frontend cap, TS-04)"
key_files:
  created:
    - backend/src/daily_records/handlers.rs  # create_override handler appended
    - backend/src/daily_records/models.rs    # OverrideResponse struct added
    - frontend/src/components/timesheet/week-navigator.tsx
    - frontend/src/components/timesheet/timesheet-table.tsx
    - frontend/src/components/timesheet/novedad-modal.tsx
    - frontend/src/components/ui/dialog.tsx
  modified:
    - backend/src/main.rs                                      # route registered under admin_routes
    - frontend/src/lib/validations.ts                          # novedadSchema + NovedadFormData added
    - frontend/src/app/(dashboard)/timesheet/page.tsx          # full implementation replacing stub
    - frontend/src/__tests__/novedad-modal.test.tsx            # real tests (TDD RED then GREEN)
    - frontend/src/__tests__/timesheet-table.test.tsx          # real tests (week navigation)
decisions:
  - "Dialog built on @base-ui/react/dialog (not @radix-ui) — matches project shadcn base from plan 04-01"
  - "novedadSchema uses z.string().min(1) not .nonempty() — Zod v4 compatibility (Pitfall from plan)"
  - "TimesheetTable columns spread with role check: [...(role==='admin' ? [editCol] : [])] — cleaner than conditional render inside cell"
  - "Recompute uses single SQL query (SELECT anchor_date, employee_id) — avoids second round-trip versus plan's two-query pattern"
  - "node_modules installed in worktree (npm install) — worktrees share git history but not node_modules"
metrics:
  duration_minutes: 16
  completed_date: "2026-04-23"
  tasks_completed: 2
  files_changed: 11
---

# Phase 04 Plan 03: Timesheet Editor Summary

**One-liner:** POST /daily-records/{id}/overrides multipart handler with audit trigger + full timesheet screen (week nav, TanStack Table v8, Registrar Novedad modal with mandatory justification and evidence).

## Tasks Completed

| # | Name | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Backend POST overrides handler | 575f3d4 | handlers.rs, models.rs, main.rs |
| 2 (RED) | TDD failing tests | 8b73543 | novedad-modal.test.tsx, timesheet-table.test.tsx |
| 2 (GREEN) | Frontend timesheet components | cb30e66 | week-navigator.tsx, timesheet-table.tsx, novedad-modal.tsx, dialog.tsx, page.tsx, validations.ts |

## Verification Results

- `cargo build` — exits 0, no errors, POST /daily-records/{id}/overrides registered under `require_admin`
- `vitest run` — 17 tests pass across 6 files (6 new schema + week-nav tests all green)
- novedadSchema rejects empty justification (TS-03 frontend enforcement)
- evidenceFileSchema enforces 5MB cap + PDF/JPG/PNG types (TS-04)
- Backend: missing justification → 422 VALIDATION_ERROR; missing evidence → 422 VALIDATION_ERROR (TS-03, TS-04)
- Audit trigger (migration 011) fires on daily_record_overrides INSERT (TS-05) — no extra code needed
- Edit icon spread only when `role === 'admin'` (D-14)
- Estado badges: Normal/Ausente/Justificado/Ausente Justificado (D-8)
- Modal Estado Inicial shows read-only "Aprobado" label (D-1)
- Week defaults to current Monday–Sunday with weekStartsOn:1 (D-7, Pitfall 7)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] node_modules absent in worktree — installed via npm install**
- **Found during:** Task 2 (TDD RED vitest run)
- **Issue:** Git worktrees share git history but not node_modules; vitest run failed with `MODULE_NOT_FOUND: vitest/config`
- **Fix:** `npm install` in frontend worktree directory
- **Files modified:** frontend/package-lock.json (committed as part of Task 2)
- **Commit:** cb30e66

**2. [Rule 1 - Bug] Timezone-naive date constructor caused wrong weekStartOfWeek assertion**
- **Found during:** Task 2 TDD RED phase (first vitest run)
- **Issue:** `new Date('2026-04-20')` parsed as UTC midnight, shifted to Sunday in America/Caracas (UTC-4), making `startOfWeek` return the previous Monday
- **Fix:** Changed to `new Date(2026, 3, 20, 12, 0, 0)` (noon local time, month 0-indexed) in timesheet-table.test.tsx
- **Files modified:** frontend/src/__tests__/timesheet-table.test.tsx
- **Commit:** 8b73543 (fixed before RED commit)

**3. [Rule 2 - Missing] Plan used two separate SQL queries in recompute block; consolidated to one**
- **Found during:** Task 1 handler implementation
- **Issue:** Plan's recompute block fetched anchor_date and employee_id in separate queries, creating unnecessary round-trips and a second cursor after the first was drained
- **Fix:** Single `SELECT anchor_date, employee_id FROM daily_records WHERE id = ?1` query
- **Files modified:** backend/src/daily_records/handlers.rs
- **Commit:** 575f3d4

**4. [Rule 3 - Blocking] shadcn Dialog component not pre-installed; built from @base-ui/react/dialog**
- **Found during:** Task 2 modal creation
- **Issue:** `npx shadcn add dialog` requires interactive terminal (Bash permission denied); existing components use `@base-ui/react` not `@radix-ui`
- **Fix:** Created `frontend/src/components/ui/dialog.tsx` wrapping `@base-ui/react/dialog` primitives (Root, Popup, Backdrop, Portal, Title) — same pattern as existing button.tsx uses ButtonPrimitive
- **Files modified:** frontend/src/components/ui/dialog.tsx (new file)
- **Commit:** cb30e66

## TDD Gate Compliance

| Gate | Commit | Status |
|------|--------|--------|
| RED (test) | 8b73543 | PASS — 5 novedad tests failed, 4 week-nav passed |
| GREEN (feat) | cb30e66 | PASS — all 17 tests pass |
| REFACTOR | (none needed) | N/A — no cleanup required |

## Known Stubs

None. All data paths are wired:
- TimesheetTable receives `data` from TanStack Query (not mock)
- NovedadModal POSTs to `/daily-records/{id}/overrides` or `/leaves` (real endpoints)
- WeekNavigator drives real query key `[weekStart, weekEnd]`

## Threat Flags

No new trust boundaries beyond plan's threat model. All mitigations applied:

| Threat | Mitigation | Status |
|--------|-----------|--------|
| T-4-09 Spoofing | require_admin on override route | Applied — main.rs admin_routes |
| T-4-10 Path traversal | UUID filename, no user path | Applied — `Uuid::new_v4()` under data/overrides/ |
| T-4-11 Repudiation | SQLite trigger on INSERT | Applied — migration 011 fires unconditionally |
| T-4-12 Justification bypass | Backend trim().is_empty() check | Applied — handlers.rs ~line 160 |
| T-4-13 Privilege escalation | require_admin middleware | Applied — admin_routes layer |

## Self-Check: PASSED

| Item | Status |
|------|--------|
| frontend/src/components/timesheet/week-navigator.tsx | FOUND |
| frontend/src/components/timesheet/timesheet-table.tsx | FOUND |
| frontend/src/components/timesheet/novedad-modal.tsx | FOUND |
| frontend/src/components/ui/dialog.tsx | FOUND |
| 575f3d4 (backend handler) | FOUND |
| 8b73543 (TDD RED tests) | FOUND |
| cb30e66 (frontend GREEN) | FOUND |
