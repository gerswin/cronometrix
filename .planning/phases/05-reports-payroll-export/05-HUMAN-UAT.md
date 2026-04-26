---
status: partial
phase: 05-reports-payroll-export
source: [05-VERIFICATION.md]
started: 2026-04-26T17:05:00Z
updated: 2026-04-26T17:05:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. End-to-end Excel download
expected: Workbook opens with `Resumen` sheet showing `Reporte Pre-Nómina` title, client_name + RIF in row 2, period range + generated_at in row 3, 20 column headers in row 5, per-employee data rows grouped by department, `Total {Departamento}` subtotal rows, `Total General` grand-total row at bottom; money columns formatted as $X,XXX.XX
result: [pending]

### 2. Anomaly row tint (WR-05 follow-up)
expected: Generate a report including an employee with `anomaly_count > 0` and visually inspect the Excel file for amber-100 (#FEF3C7) row tinting on that row across all 20 cells
result: [pending]

### 3. End-to-end PDF download
expected: PDF in landscape A4 with branding header on every page, 20 column headers, per-employee rows, per-dept subtotals (slate-100 background), grand-total row (blue-100 background), anomaly rows (amber-100 background), `Página N de M` footer
result: [pending]

### 4. Period boundary parity (picker preview vs backend response)
expected: Pick `Quincenal` / `2da quincena` for current month — picker shows e.g. `2026-04-16 – 2026-04-30` and backend `ReportPayload.header.from_date / to_date` matches exactly after Emitir Reporte
result: [pending]

### 5. Settings/Datos de Empresa flow
expected: Login as Admin, navigate to /settings/tenant-info, fill fields, submit, verify success toast and that subsequent report exports show the new client_name + RIF in the branding header (Excel row 1-2 and PDF first page)
result: [pending]

### 6. RBAC matrix end-to-end
expected: Viewer cannot see Exportar Excel/PDF buttons; Supervisor can see and successfully export; Admin can edit Settings/Datos de Empresa; Supervisor sees Settings as read-only; backend returns 403 on bypass attempts
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
