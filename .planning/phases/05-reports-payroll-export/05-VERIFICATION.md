---
phase: 05-reports-payroll-export
verified: 2026-04-25T23:55:00Z
status: human_needed
score: 4/4 must-haves verified
overrides_applied: 0
human_verification:
  - test: "End-to-end Excel download — start backend + frontend, login as Admin, navigate to /reports, pick a period (Mensual), click 'Emitir Reporte', click 'Exportar Excel', verify the .xlsx file downloads and opens cleanly in Excel/LibreOffice/Numbers"
    expected: "Workbook opens with 'Resumen' sheet showing 'Reporte Pre-Nómina' title, client_name + RIF in row 2, period range + generated_at in row 3, 20 column headers in row 5, per-employee data rows grouped by department, 'Total {Departamento}' subtotal rows, 'Total General' grand-total row at bottom; money columns formatted as $X,XXX.XX"
    why_human: "Visual file rendering correctness can only be confirmed by a spreadsheet client; cell formatting, freeze panes, autofit width, color rendering across applications cannot be verified programmatically (calamine round-trip parses content but not visual output)"
  - test: "Anomaly row tint (WR-05 follow-up) — generate a report including an employee with anomaly_count > 0 and visually inspect the Excel file for amber-100 (#FEF3C7) row tinting on that row"
    expected: "The anomaly row should appear with a clearly visible amber/yellow background across all 20 cells"
    why_human: "Code review WR-05 flagged that `set_row_format` may be overridden by per-cell `write_with_format` so the amber tint silently fails on data rows. The integration test only verifies anomaly column STRING content, not cell color. This is a visual inspection task; either confirm the tint shows or reproduce the WR-05 defect."
  - test: "End-to-end PDF download — same flow, click 'Exportar PDF', verify .pdf file downloads and opens cleanly in browser/PDF viewer"
    expected: "PDF in landscape A4 with branding header on every page, 20 column headers, per-employee rows, per-dept subtotals (slate-100 background), grand-total row (blue-100 background), anomaly rows (amber-100 background), 'Página N de M' footer"
    why_human: "Client-side jspdf-autotable rendering requires running browser; visual fidelity (page breaks, color, footer placement) cannot be verified programmatically"
  - test: "Verify period boundary math matches between picker preview and backend report — pick 'Quincenal' / '2da quincena' for current month, verify the preview displays day-16 to last-day-of-month range, generate the report, verify backend returns the same range in the response header"
    expected: "Picker shows e.g. '2026-04-16 – 2026-04-30' and backend ReportPayload.header.from_date / to_date matches exactly"
    why_human: "I-8 acknowledges intentional duplication of period math between frontend deriveDates and backend periods.rs::resolve_period; visual confirmation that the two stay in sync is a human-in-the-loop check beyond the mirror-suite unit tests"
  - test: "Verify Settings/Datos de Empresa flow — login as Admin, navigate to /settings/tenant-info, verify form is editable, fill fields, submit, verify success toast and that subsequent report exports show the new client_name + RIF in the branding header"
    expected: "Form submits successfully; reports rendered after the update show the updated branding values in row 1-2 of the Excel and the PDF first page"
    why_human: "End-to-end flow connecting tenant_info PATCH → branding header in Excel/PDF is a multi-screen user journey requiring browser interaction"
  - test: "RBAC matrix end-to-end — verify Viewer cannot see Exportar Excel/PDF buttons, Supervisor can see them and successfully export, Admin can edit Settings/Datos de Empresa, Supervisor sees Settings as read-only"
    expected: "Buttons hidden/disabled per role; backend returns 403 if any role bypass attempt is made"
    why_human: "Multi-role manual flow across Reports + Settings screens; backend integration tests cover API-level RBAC but the UI-level role gating needs visual confirmation"
---

# Phase 5: Reports & Payroll Export Verification Report

**Phase Goal:** Admin and supervisors can generate a pre-payroll report for any configurable period and export it to Excel or PDF, producing the primary deliverable clients pay for.

**Verified:** 2026-04-25T23:55:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| #   | Truth                                                                                                                                                                                                | Status     | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Admin can select a report period (weekly, bi-weekly, or monthly) and generate a pre-payroll report covering all employees                                                                            | VERIFIED   | `frontend/src/components/reports/period-picker.tsx` exposes Semanal/Quincenal (1ra/2da)/Mensual/Personalizado options; `backend/src/reports/periods.rs` (PeriodPreset::{Weekly, BiweeklyFirst, BiweeklySecond, Monthly, Custom}) + `resolve_period`; `POST /api/v1/reports/json` registered in `main.rs:165`; `Emitir Reporte` button at `reports/page.tsx:67` calls `reportQ.refetch()` against `compute_report` (`service.rs:62`) which returns ReportPayload over all employees in scope                                  |
| 2   | Report includes work minutes, overtime hours, late deductions, and leave summary per employee for the selected period                                                                                | VERIFIED   | `Aggregates` struct in `backend/src/reports/models.rs:65-82` exposes `work_min, ot_min, late_min, days_worked, days_absent, work_pay_cents, ot_pay_cents, night_premium_cents, rest_day_surcharge_cents, late_deduction_cents, total_a_pagar_cents, days_ivss, days_vacation, days_permission, days_unpaid` flattened into each EmployeeReportRow; LOTTT money math in `money.rs` (16 unit tests + 2 proptest); leaves W-5 secondary aggregation in `service.rs:415` (`FROM leaves`); 26 integration tests in `reports_test.rs`            |
| 3   | Report downloads as a correctly formatted Excel file                                                                                                                                                  | VERIFIED   | `POST /api/v1/reports/excel` registered in `main.rs:166`; handler in `handlers.rs:49-86` returns `(StatusCode::OK, headers, bytes)` with Content-Type `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` + Content-Disposition `attachment; filename="prenomina_{from}_{to}.xlsx"`; builder in `excel.rs` generates branding header (rows 0-2), 20 column headers (row 4), per-employee rows grouped by dept, dept subtotal rows + grand total; tested via 11 calamine round-trip integration tests in `reports_excel_test.rs` (response headers, branding, column headers, dept subtotals, grand total, RBAC, audit). NOTE: WR-05 (anomaly row tint may be overridden by per-cell formats) flagged in code review; visible Excel formatting requires human verification — see human_verification list                                                                                                                                                |
| 4   | Report downloads as a PDF file with the same data                                                                                                                                                    | VERIFIED   | `frontend/src/lib/reports/pdf.ts::renderReportPdf` uses jspdf 4.2.1 + jspdf-autotable 5.0.7, landscape A4, builds branding header + 20-column body with same data shape as Excel (per-dept subtotals + grand total); anomaly row tint via `didParseCell` callback; page footer "Página N de M" via `didDrawPage`; client-side flow in `export-buttons.tsx:41-48` POSTs to `/reports/json`, then renders PDF locally; jspdf + jspdf-autotable deps confirmed in `frontend/package.json`; vitest covers `pdf.test.ts` |

**Score:** 4/4 truths verified

### Required Artifacts

#### Backend (Wave 1 — Plan 05-01)

| Artifact                                                                | Expected                                                  | Status     | Details                                                                                |
| ----------------------------------------------------------------------- | --------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------- |
| `backend/src/db/migrations/013_tenant_info.sql`                         | tenant_info singleton with CHECK(id=1)                    | VERIFIED   | Exists (701B), contains `CREATE TABLE IF NOT EXISTS tenant_info` + `CHECK (id = 1)`    |
| `backend/src/db/migrations/014_phase5_audit_triggers.sql`               | audit_log CHECK includes REPORT_EXPORT + tenant_info trig | VERIFIED   | Exists (3.6K); writable_schema rewrite (deviation documented) + `audit_tenant_info_update` + DROP/RECREATE `audit_employees_update` |
| `backend/src/db/migrations/015_employees_position_hire_date.sql`        | ALTER TABLE employees + position/hire_date columns        | VERIFIED   | Exists (319B), contains both ALTER statements                                          |
| `backend/src/tenant_info/{mod,models,service,handlers}.rs`              | Singleton CRUD module                                     | VERIFIED   | All 4 files present                                                                    |
| `backend/tests/tenant_info_test.rs`                                     | 5 integration tests                                       | VERIFIED   | 7.7K, all 5 named tests pass (`get_returns_seed_row`, `admin_patch_succeeds`, `supervisor_blocked`, `version_conflict`, `audit_trigger_fires`) |

#### Backend (Wave 2 — Plan 05-02)

| Artifact                                                                | Expected                                                  | Status     | Details                                                                                |
| ----------------------------------------------------------------------- | --------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------- |
| `backend/src/reports/money.rs`                                          | Pure cents-i64 LOTTT premium math                         | VERIFIED   | 7.7K; 16 unit tests + 2 proptest (passing)                                             |
| `backend/src/reports/periods.rs`                                        | PeriodPreset + resolve_period                             | VERIFIED   | 7.4K; 12 tests including leap-year + December rollover                                  |
| `backend/src/reports/service.rs::compute_report`                        | Primary daily_records JOIN + leaves W-5 + audit insert    | VERIFIED   | 26.6K; INSERT INTO audit_log REPORT_EXPORT at line 618; SELECT FROM daily_records / leaves / tenant_info / daily_record_anomalies confirmed |
| `backend/src/reports/handlers.rs`                                       | generate_json + generate_excel                            | VERIFIED   | Both handlers present                                                                  |
| `backend/src/reports/models.rs`                                         | DTOs for wire format                                      | VERIFIED   | All 7 structs present (ReportParamsRequest, ReportPayload, BrandingHeader, EmployeeReportRow, Aggregates, DeptSummary, DeptSubtotal) |
| `backend/tests/reports_test.rs`                                         | 25+ integration tests                                     | VERIFIED   | 36.1K; 26 tests passing (full backend suite 264/264)                                    |

#### Backend (Wave 3 — Plan 05-03)

| Artifact                                                                | Expected                                                  | Status     | Details                                                                                |
| ----------------------------------------------------------------------- | --------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------- |
| `backend/src/reports/excel.rs`                                          | rust_xlsxwriter 0.94 build_workbook                       | VERIFIED   | 13.5K; uses `Format::set_background_color(Color)` (W-7 API name pinning) at 4 sites; legacy `set_bg_color` absent |
| `backend/Cargo.toml`                                                    | rust_xlsxwriter 0.94.0 + calamine 0.28                    | VERIFIED   | Both pinned (calamine bumped from 0.27 to 0.28 due to yanked transitive zip 2.5.0; deviation documented) |
| `backend/tests/reports_excel_test.rs`                                   | 5+ calamine round-trip tests                              | VERIFIED   | 23.9K; 11 named integration tests + 1 ignored perf bench, all pass                                  |

#### Frontend (Wave 4 — Plan 05-04)

| Artifact                                                                | Expected                                                  | Status     | Details                                                                                |
| ----------------------------------------------------------------------- | --------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------- |
| `frontend/src/app/(dashboard)/reports/page.tsx`                         | Reports screen replacing 397B placeholder                 | VERIFIED   | 3.1K; wires PeriodPicker + FiltersBar + SummaryTable + DrillDownDialog + ExportButtons + Emitir Reporte button; data flows from `reportQ.data` to all consumers |
| `frontend/src/app/(dashboard)/settings/tenant-info/page.tsx`            | Settings page                                             | VERIFIED   | 1.0K; useQuery to GET /tenant-info, renders TenantInfoForm with `canEdit = role === 'admin'` |
| `frontend/src/components/reports/period-picker.tsx`                     | Period preset selector                                    | VERIFIED   | 5.6K; weekly/biweekly_first/biweekly_second/monthly/custom + I-8 mirror comment to backend periods.rs |
| `frontend/src/components/reports/filters-bar.tsx`                       | Filters: dept multi-select, employee, shift, inactive     | VERIFIED   | 3.8K                                                                                   |
| `frontend/src/components/reports/summary-table.tsx`                     | TanStack Table v8 with all 20 columns                     | VERIFIED   | 7.7K; 20 column headers confirmed via grep                                              |
| `frontend/src/components/reports/drill-down-dialog.tsx`                 | Per-day breakdown via /daily-records                      | VERIFIED   | 3.9K                                                                                   |
| `frontend/src/components/reports/export-buttons.tsx`                    | Excel + PDF export buttons                                | VERIFIED   | 2.5K; Excel posts to `/reports/excel` with responseType:blob → URL.createObjectURL download; PDF posts to `/reports/json` then renderReportPdf(payload) |
| `frontend/src/components/settings/tenant-info-form.tsx`                 | Admin-only edit form                                      | VERIFIED   | 3.9K; react-hook-form + zod + version-based PATCH + 409 handling                         |
| `frontend/src/lib/format/currency.ts`                                   | fmtMoney via Intl.NumberFormat USD                        | VERIFIED   | 1.0K; tested in vitest                                                                  |
| `frontend/src/lib/reports/pdf.ts`                                       | renderReportPdf via jspdf + autotable                     | VERIFIED   | 5.6K; landscape A4, branding, 20 columns, subtotals, grand total, anomaly tint via didParseCell |
| `frontend/src/lib/validations.ts`                                       | tenantInfoSchema zod                                      | VERIFIED   | 3.0K                                                                                   |
| `frontend/src/test-utils/msw-handlers.ts`                               | MSW v2 handlers                                           | VERIFIED   | 5.9K                                                                                   |
| `frontend/src/components/layout/sidebar.tsx`                            | Configuración nav for admin                               | VERIFIED   | NAV item at line 27: `/settings/tenant-info, label: 'Configuración', roles: ['admin']`  |
| `frontend/package.json`                                                 | jspdf 4.2.1 + jspdf-autotable 5.0.7                       | VERIFIED   | Both deps confirmed                                                                    |

### Key Link Verification

| From                                                  | To                                                           | Via                                                                  | Status     | Details                                                                                                                                                                                                                |
| ----------------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `backend/src/main.rs` route group                     | `tenant_info::handlers::{get,patch}_tenant_info`             | viewer_routes (GET) + admin_routes (PATCH)                            | VERIFIED   | `main.rs:137` GET in viewer_routes (require_auth); `main.rs:186` PATCH in admin_routes (require_admin); `main.rs:14` use cronometrix_api::tenant_info                                                                  |
| `backend/src/db/mod.rs` MIGRATIONS                    | migrations 013/014/015                                       | array append in numeric order                                         | VERIFIED   | All three include_str! tuples present                                                                                                                                                                                  |
| `backend/src/main.rs` report_routes group             | `reports::handlers::{generate_json,generate_excel}`          | require_supervisor_or_above + 60s TimeoutLayer                        | VERIFIED   | `main.rs:165-171`: both routes + TimeoutLayer + RBAC middleware                                                                                                                                                        |
| `backend/src/reports/service.rs::compute_report`      | audit_log table                                              | INSERT INTO audit_log REPORT_EXPORT after success                     | VERIFIED   | `service.rs:618` INSERT with operation='REPORT_EXPORT', payload_json captures filters + format; runs AFTER aggregation succeeds (Pitfall 7); covered by `audit_entry_on_export` + `no_audit_on_failure` tests           |
| `backend/src/reports/service.rs`                      | daily_records + overrides + leaves + employees + departments | single SQL JOIN + W-5 secondary leaves aggregation                    | VERIFIED   | `service.rs:193` `FROM daily_records dr` (primary JOIN); `service.rs:415` `FROM leaves l` (W-5); `service.rs:104` `FROM tenant_info` for branding                                                                       |
| `backend/src/reports/handlers.rs::generate_excel`     | `excel::build_workbook`                                      | tokio::task::spawn_blocking wrapping sync builder                     | VERIFIED   | `handlers.rs:66` `tokio::task::spawn_blocking(move || excel::build_workbook(&payload))`                                                                                                                                |
| `frontend/src/components/reports/export-buttons.tsx`  | `POST /api/v1/reports/excel`                                 | axios responseType:'blob' + URL.createObjectURL download              | VERIFIED   | `export-buttons.tsx:22-35` matches pattern; filename `prenomina_{from}_{to}.xlsx`                                                                                                                                       |
| `frontend/src/components/reports/export-buttons.tsx`  | `POST /api/v1/reports/json` + renderReportPdf                | fetch payload then call renderReportPdf                               | VERIFIED   | `export-buttons.tsx:42-44`                                                                                                                                                                                             |
| `frontend/src/app/(dashboard)/settings/tenant-info`   | `PATCH /api/v1/tenant-info`                                  | react-hook-form + version field for optimistic concurrency            | VERIFIED   | `tenant-info-form.tsx:39-42` includes `version: initialData.version`; 409 handler invalidates cache                                                                                                                    |
| `frontend/src/components/layout/sidebar.tsx`          | `/settings/tenant-info`                                      | NAV_ITEMS entry roles=['admin']                                       | VERIFIED   | sidebar.tsx:27 — Configuración nav with roles=['admin']                                                                                                                                                                |
| `frontend/src/app/(dashboard)/reports/page.tsx`       | `POST /api/v1/reports/json`                                  | reportQ.refetch via Emitir Reporte button                              | VERIFIED   | reports/page.tsx:47 + 67-75; reportQ data drives SummaryTable + ExportButtons                                                                                                                                          |

### Data-Flow Trace (Level 4)

| Artifact                                  | Data Variable          | Source                                                                                                                            | Produces Real Data | Status                                                                                                                  |
| ----------------------------------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| reports/page.tsx                          | `reportQ.data`         | `api.post<ReportPayload>('/reports/json', filters)` → backend `compute_report` SELECTs from daily_records + leaves + employees    | Yes                | FLOWING                                                                                                                  |
| reports/page.tsx                          | `departmentsQ.data`    | `api.get<PaginatedResponse<Department>>('/departments')` → existing departments handler (Phase 1)                                  | Yes                | FLOWING                                                                                                                  |
| settings/tenant-info/page.tsx             | `data` (TenantInfo)    | `api.get('/tenant-info')` → `tenant_info::handlers::get_tenant_info` → `service::get_tenant_info` SELECT from tenant_info row 1   | Yes                | FLOWING                                                                                                                  |
| ExportButtons (Excel)                     | `resp.data` (blob)     | POST `/reports/excel` → `excel::build_workbook(payload)` where payload comes from `compute_report` SQL aggregation                 | Yes                | FLOWING                                                                                                                  |
| ExportButtons (PDF)                       | `resp.data` (payload)  | POST `/reports/json` → same as above                                                                                              | Yes                | FLOWING                                                                                                                  |
| TenantInfoForm                            | `initialData`          | passed from page-level useQuery; PATCH submits via mutation                                                                       | Yes                | FLOWING                                                                                                                  |

### Behavioral Spot-Checks

| Behavior                                                    | Command                                              | Result                          | Status |
| ----------------------------------------------------------- | ---------------------------------------------------- | ------------------------------- | ------ |
| Backend builds clean                                        | `cargo build` in `backend/`                          | 0 errors, 1 deprecation warning  | PASS   |
| Backend test suite passes                                   | `cargo nextest run --no-fail-fast`                   | 264 passed, 2 skipped            | PASS   |
| Frontend test suite passes                                  | `npx vitest run`                                     | 80 passed in 14 files            | PASS   |
| Reports module exposed in lib.rs                            | `grep "pub mod reports" backend/src/lib.rs`          | match                            | PASS   |
| Tenant_info module exposed in lib.rs                        | `grep "pub mod tenant_info" backend/src/lib.rs`      | match                            | PASS   |
| All migrations registered                                   | `grep 013_tenant_info\|014_phase5\|015_employees`    | 3 matches                        | PASS   |
| Audit insert with REPORT_EXPORT exists                      | `grep REPORT_EXPORT backend/src/reports/service.rs`  | line 618 INSERT                  | PASS   |
| W-7 background-color binding (no legacy set_bg_color)        | `grep set_bg_color backend/src/reports/excel.rs`     | 0 matches                        | PASS   |
| `set_background_color` calls present                         | `grep set_background_color backend/src/reports/excel.rs` | 4 call sites + 3 RGB(0xDBEAFE) variants | PASS   |
| Excel response Content-Type + Content-Disposition           | `grep Content-Disposition backend/src/reports/handlers.rs` | header inserted with quoted filename | PASS   |

### Requirements Coverage

| Requirement | Source Plan(s)                  | Description                                                                                                  | Status     | Evidence                                                                                                                                                                                                  |
| ----------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PAY-01      | 05-01, 05-02, 05-04             | Admin can generate pre-payroll report for a configurable period (weekly/bi-weekly/monthly)                   | SATISFIED  | period-picker.tsx exposes Semanal/Quincenal/Mensual/Personalizado; backend `periods::resolve_period` covers all 5 presets; Reports screen wires the picker → POST /reports/json end-to-end                |
| PAY-02      | 05-02, 05-04                    | Report includes work minutes, overtime, late deductions, and leave summary per employee                      | SATISFIED  | Aggregates struct (`models.rs:65-82`) includes work_min, ot_min, late_min, days_worked, days_absent, days_ivss/vacation/permission/unpaid; rendered in summary-table.tsx + Excel + PDF                    |
| PAY-03      | 05-03, 05-04                    | Report exports to Excel format                                                                               | SATISFIED  | `POST /reports/excel` with rust_xlsxwriter; calamine round-trip tests (11) confirm structure; frontend ExportButtons triggers download. Visual fidelity (anomaly tint per WR-05) needs human verification |
| PAY-04      | 05-04                           | Report exports to PDF format                                                                                 | SATISFIED  | jspdf + jspdf-autotable client-side; renderReportPdf with branding, 20 columns, dept subtotals, grand total, anomaly tint via didParseCell                                                                |

**No orphaned requirements found.** All 4 PAY requirements are mapped to the phase, all are addressed by at least one plan's implementation.

### Anti-Patterns Found

| File                                            | Line        | Pattern                                          | Severity | Impact                                                                                                                                       |
| ----------------------------------------------- | ----------- | ------------------------------------------------ | -------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `backend/src/reports/excel.rs`                  | 234-345     | (WR-05) `set_row_format` overridden by `write_with_format` per-cell formats; anomaly amber-100 tint may not visibly render on data rows | Warning  | Cosmetic UX defect — anomaly rows may not show amber background in Excel; integration test only checks anomaly column STRING content. Code review documented; needs human visual confirmation |
| `backend/src/reports/service.rs`                | 415         | (WR-03) `shift_type` filter applied to daily_records JOIN but NOT to W-5 leaves aggregation                                              | Info     | Edge case: filtering by shift_type='night' might still count vacation days for an employee whose dept policy is 'day'. Documented in review.  |
| `backend/src/reports/handlers.rs`               | 70          | (WR-01) Excel filename uses raw `params.from_date` / `to_date` instead of resolved period boundaries                                      | Info     | When user picks a preset (e.g. weekly), filename shows the anchor date instead of the resolved Mon..Sun range. Cosmetic, non-blocking         |
| `backend/src/main.rs`                           | 167         | `TimeoutLayer::new` deprecated, recommended `with_status_code`                                                                            | Info     | Compiler warning; functional equivalence; documented as follow-up in Plan 05-02 SUMMARY                                                       |

No blocker anti-patterns found. No TODO/FIXME/HACK in any phase 5 source file. The "placeholder" matches in service.rs are SQL `?N` parameter placeholders (legitimate). The `placeholder=` in tenant-info-form.tsx is an HTML input attribute.

### Human Verification Required

See `human_verification` section in frontmatter — 6 items requiring browser-based / spreadsheet-client / PDF-viewer manual confirmation:

1. End-to-end Excel download + open in spreadsheet client (visual fidelity)
2. Anomaly row tint (WR-05 follow-up — visual confirmation that amber-100 actually renders)
3. End-to-end PDF download + open (visual fidelity)
4. Period boundary parity between picker preview and backend report (I-8 contract sanity)
5. Settings/Datos de Empresa → branding header in exports (multi-screen flow)
6. RBAC matrix end-to-end on Reports + Settings UI (role-based visibility)

### Gaps Summary

No goal-blocking gaps found. All 4 ROADMAP success criteria are programmatically verified at the data-flow + wiring + test-suite level:

- All 4 plans executed (05-01..05-04) with commits in main
- All 4 PAY requirements addressed by at least one plan's deliverables
- Backend test suite green at 264/264, frontend at 80/80
- All artifacts exist with substantive content (no stubs)
- All key links verified (route registrations, module exports, audit insert, data flow from DB through API to UI)
- Real DB queries flow through compute_report (daily_records + leaves + tenant_info + daily_record_anomalies)
- Excel + PDF endpoints both reach the same compute_report → ReportPayload pipeline

The phase has reached production-ready status programmatically; the 6 items in `human_verification` are visual / cross-screen flows that cannot be confirmed via grep, build, or test runner. The code review (`05-REVIEW.md`) flagged 0 critical, 6 warning, 7 info — none blocking. Status is `human_needed` rather than `passed` because:

- WR-05 (Excel anomaly row tint may visually fail) is a possible product-visible defect that the integration test does not catch
- End-to-end visual confirmation of Excel and PDF rendering is required for SC-3 and SC-4 to be incontrovertibly true (both downloads programmatically work; "correctly formatted" is the part needing human eyes)

---

_Verified: 2026-04-25T23:55:00Z_
_Verifier: Claude (gsd-verifier)_
