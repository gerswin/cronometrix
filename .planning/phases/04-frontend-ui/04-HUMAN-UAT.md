---
status: partial
phase: 04-frontend-ui
source: [04-VERIFICATION.md]
started: 2026-04-23T22:30:00-04:00
updated: 2026-04-23T22:30:00-04:00
---

## Current Test

[awaiting human testing]

## Tests

### 1. Auth redirect
expected: Navigating to /dashboard without a refresh_token cookie redirects to /login?redirect=/dashboard
result: [pending]

### 2. Real-time SSE dashboard
expected: Triggering a biometric event pushes it to the activity feed; disconnect banner appears when SSE drops; reconnects automatically
result: [pending]

### 3. Timesheet override end-to-end
expected: Submitting the Registrar Novedad modal with justification + PDF writes to daily_record_overrides and fires audit log
result: [pending]

### 4. RBAC role gating
expected: Supervisor sees Emitir Reporte but not Nuevo Empleado; Viewer sees read-only grids with no action buttons
result: [pending]

### 5. ISAPI command dispatch
expected: CommandModal Admin-only; sending door_open dispatches to /devices/{id}/commands with correct payload
result: [pending]

### 6. Modal validation UX
expected: Submitting NovedadModal with empty justification shows Zod validation error inline; form does not submit
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
