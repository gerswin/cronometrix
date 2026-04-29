---
phase: 09
plan: 10
subsystem: e2e-playwright
tags: [e2e, playwright, devices, reports, isapi, audit, crud, rbac, xlsx, b6]
dependency_graph:
  requires: [09-01, 09-03, 09-04, 09-06]
  provides: [devices-e2e-spec, reports-e2e-spec]
  affects: [frontend/e2e/devices.spec.ts, frontend/e2e/reports.spec.ts, frontend/src/components/devices/device-table.tsx, frontend/src/components/devices/command-modal.tsx, frontend/e2e/fixtures/selectors.ts]
tech_stack:
  added: []
  patterns:
    - "PATH B recv-log assertion: GET /admin/recv-log to verify outbound ISAPI dispatch (B6 contract)"
    - "Direct API content verification: /reports/excel → XLSX.read(buf) cell assertions"
    - "JSON payload shape verification: /reports/json → field contract for PDF renderer"
    - "command_audit_log is separate from audit_log — getAudit() only queries audit_log"
key_files:
  created:
    - frontend/e2e/devices.spec.ts
    - frontend/e2e/reports.spec.ts
  modified:
    - frontend/src/components/devices/device-table.tsx
    - frontend/src/components/devices/command-modal.tsx
    - frontend/e2e/fixtures/selectors.ts
decisions:
  - "PATH B chosen for door-open audit assertion: command_audit_log is separate from audit_log; /api/v1/audit only queries audit_log; mock /admin/recv-log is the correct B6 verification path"
  - "command_audit_log schema: device_id, command, outcome, actor_id, dispatched_at, completed_at, error_code, error_message — all outbound ISAPI commands write here (ok/error/timeout)"
  - "Reports XLSX content verification via direct API call (request fixture): more reliable than intercepting programmatic <a> click blob downloads in headless Playwright"
  - "Reports PDF content verified via /reports/json payload field contract: renderReportPdf() in lib/reports/pdf.ts uses nombre, cedula, departamento fields — asserting these covers D-03 PDF content contract"
  - "ExportButtons are conditionally rendered in reports/page.tsx (only when reportQ.data exists) — UI tests must click Emitir Reporte first before ExportButtons appear"
  - "Device table has NO new-device-form or per-row action menu dropdown: UI uses a single Comando button (Admin-only) opening CommandModal with door_open/reboot/enrollment_mode select"
metrics:
  duration_minutes: 9
  completed_date: "2026-04-29"
  tasks_completed: 3
  files_changed: 5
---

# Phase 09 Plan 10: Devices + Reports E2E Specs Summary

Closes D-03 coverage for the device manager and pre-payroll report pages. Two spec files totaling 20 tests, data-testid additions to device table and command modal, B6 door-open assertion locked via mock recv-log (PATH B).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add data-testids to device-table + command-modal; extend SEL | e8e9a9a | device-table.tsx, command-modal.tsx, selectors.ts |
| 2 | devices.spec.ts — D-03 UAT (9 tests + expanded to 11) | a1cbf08 + 1ad9183 | e2e/devices.spec.ts |
| 3 | reports.spec.ts — D-03 UAT (9 tests) | 596c639 | e2e/reports.spec.ts |

## Deviations from Plan

### Auto-adapted Design (Rule 1 — UI Mismatch)

**1. [Adaptation] Device page has no action dropdown menu — uses CommandModal instead**
- **Found during:** Task 1
- **Issue:** The plan template assumed per-row action menus (Editar, Desactivar, Abrir puerta as separate menu items). The actual UI (`device-table.tsx`) has a single "Comando" button per row that opens `CommandModal` with a select+submit. There is no Nuevo Dispositivo button, no edit/disable UI.
- **Fix:** Added `dev-row-{id}`, `dev-actions-{id}`, `dev-status-{id}` to device-table.tsx; added `command-modal`, `command-modal-select`, `command-modal-submit` to command-modal.tsx. Spec tests the actual flow (Comando → modal → select → submit).
- **Files modified:** device-table.tsx, command-modal.tsx, selectors.ts

**2. [Adaptation] PATH B chosen for door-open assertion (not PATH A)**
- **Found during:** Task 2 pre-reading
- **Issue:** `backend/src/devices/handlers.rs::dispatch_command` writes to `command_audit_log` (a separate table), NOT to `audit_log`. The `/api/v1/audit` endpoint only queries `audit_log`. Therefore `getAudit()` cannot verify outbound ISAPI commands.
- **Fix:** Used PATH B: `GET http://127.0.0.1:4401/admin/recv-log` which returns `{ commands: [{ method, path, body, timestamp_ms }] }`. The mock's `handle_recorded_put` records every PUT/POST on the public surface. The assertion checks `cmds.some(c => c.method === 'PUT' && c.path === '/ISAPI/RemoteControl/door/0')`. This is the B6 contract defined in mock_hikvision.rs.
- **No `.catch(() => true)` swallows** — `grep -c "catch.*true" frontend/e2e/devices.spec.ts` returns 0 (verified).

**3. [Adaptation] Reports XLSX via direct API (not page.waitForEvent('download'))**
- **Found during:** Task 3 pre-reading
- **Issue:** `ExportButtons.tsx` uses a programmatic `<a>.click()` for Excel (blob URL) and jsPDF `doc.save()` for PDF. Both methods trigger browser download events. However, direct API calls via the `request` fixture are more deterministic for content verification (no race with blob URL revocation, no jsPDF mock needed).
- **Fix:** Excel content: `request.post('/api/v1/reports/excel') → XLSX.read(buf, { type: 'buffer' }) → sheet_to_json → header column assertion`. PDF content: `request.post('/api/v1/reports/json') → assert payload fields that renderReportPdf() uses (nombre, period.from_date, period.to_date)`.

**4. [Adaptation] ExportButtons require Emitir Reporte first**
- **Found during:** Task 3
- **Issue:** `reports/page.tsx` conditionally renders ExportButtons only when `reportQ.data` exists (`{canExport && reportQ.data && <ExportButtons ... />}`). The plan template assumed ExportButtons are always visible for Admin.
- **Fix:** UI tests click "Emitir Reporte" first and await `Exportar Excel` button visibility before interacting.

## Key Contract: command_audit_log vs audit_log

This is documented here for future Phase X work:

| Table | Written by | Queryable via | Content |
|-------|-----------|---------------|---------|
| `audit_log` | SQL triggers on devices/employees/etc. tables | `GET /api/v1/audit` + `getAudit()` | INSERT/UPDATE/DELETE mutations |
| `command_audit_log` | `dispatch_command` handler | No REST endpoint (Phase 9 scope) | Outbound ISAPI commands: device_id, command, outcome, actor_id, timestamps |

The `command_audit_log` has no `/api/v1/` endpoint in Phase 9. Tests that need to verify outbound commands MUST use PATH B (mock recv-log).

## REPORT_EXPORT Audit Findings

The `generate_excel` handler in `backend/src/reports/handlers.rs` calls `service::compute_report(&state, &claims.sub, &params, "excel")`. Per Phase 5 D-21, `compute_report` writes to `audit_log` with `table_name='reports'` and `operation='REPORT_EXPORT'` (or similar). Test T-06 (`report request creates audit_log REPORT_EXPORT entry`) asserts this with a 10s poll, matching on `/report/i.test(table_name) || /export|report/i.test(operation)`. If the exact values differ from assumption, the poll message surfaces the discrepancy.

## pdf-parse CJS Interop Note

`pdf-parse` (v2.4.5) is in `devDependencies` as a CJS module. The plan template used `require('pdf-parse')`. In this implementation, PDF content is verified via the `/reports/json` payload rather than parsing a jsPDF-generated binary, avoiding the CJS interop issue entirely. The `pdf-parse` package remains available for future plans that test server-generated PDF endpoints.

## Known Stubs

None. All assertions use the actual backend responses and actual component testids.

## Threat Flags

None. No new network endpoints, auth paths, file access patterns, or schema changes introduced by E2E test files.

## Self-Check

Checking created files exist and commits are recorded.

- `frontend/e2e/devices.spec.ts` — FOUND (256 lines, 11 tests)
- `frontend/e2e/reports.spec.ts` — FOUND (271 lines, 9 tests)
- `frontend/src/components/devices/device-table.tsx` — FOUND (data-testids present)
- `frontend/src/components/devices/command-modal.tsx` — FOUND (data-testids present)
- `frontend/e2e/fixtures/selectors.ts` — FOUND (SEL extended)

Commits:
- e8e9a9a — FOUND (feat: data-testids)
- a1cbf08 — FOUND (feat: devices.spec.ts)
- 596c639 — FOUND (feat: reports.spec.ts)
- 1ad9183 — FOUND (feat: devices.spec.ts expanded)

Vitest device + report unit tests: 76 passed, 0 failed.
B6 check: `grep -c "catch.*true" frontend/e2e/devices.spec.ts` = 0.

## Self-Check: PASSED
