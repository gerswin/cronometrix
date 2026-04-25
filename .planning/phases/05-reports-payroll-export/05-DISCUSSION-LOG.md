# Phase 5: Reports & Payroll Export - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-25
**Phase:** 05-reports-payroll-export
**Areas discussed:** Money math, Period model, Report scope/columns, Anomaly handling, RBAC, Generation flow, Excel/PDF structure, Tenant info

---

## Money Math Depth

| Option | Description | Selected |
|--------|-------------|----------|
| Full money totals | Compute work_pay, OT_pay, night_premium, rest_day_surcharge, late_deduction, total_a_pagar in USD cents per row | ✓ |
| Time + flags only | Time/minute columns + boolean flags; payroll system applies premiums | |
| Time totals + computed pay only at footer | Per-row time-only; footer aggregates money | |

**User's choice:** Full money totals (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| +50% flat (Recommended) | Hardcoded LOTTT Art. 120 default for Sunday/rest-day | ✓ |
| Per-department configurable | departments.rest_day_surcharge_pct column | |
| Global config in global_rules | Single installation-wide rate via Reglas Globales | |

**User's choice:** +50% flat

| Option | Description | Selected |
|--------|-------------|----------|
| Whole shift if shift_type=night (Recommended) | +30% on all work_minutes when dept.shift_type=night | ✓ |
| No night premium in v1 | Document as known limitation | |
| Per-minute partition now | Apportion 7pm-5am minutes; bring forward Phase 3 D-11 | |

**User's choice:** Whole shift if shift_type=night

| Option | Description | Selected |
|--------|-------------|----------|
| Subtract late_minutes from work_minutes | Engine already excludes; report shows informational column only | |
| Pro-rated salary deduction column (Recommended) | late_min × (base_salary / ordinary_daily_min) as 'Descuento por Retraso' | ✓ |
| Configurable: deduction or not | global_rules.deduct_late_pay BOOLEAN | |

**User's choice:** Pro-rated salary deduction column

| Option | Description | Selected |
|--------|-------------|----------|
| VES (bolívar) (Recommended) | base_salary_cents = bolívar centavos, format Bs. X,XX | |
| USD | base_salary_cents = USD cents, format $X.XX | ✓ |
| Per-department currency | departments.currency column | |

**User's choice:** USD (deviation from recommendation; reflects VE dollar-pegged payroll practice)

| Option | Description | Selected |
|--------|-------------|----------|
| Medical=excluded, vacation=paid, unpaid=zero, manual=zero (Recommended) | Medical: IVSS pays externally → 'Días IVSS' informational; Vacation: paid full; Unpaid: zero; Manual: zero unless override | ✓ |
| Treat all leave as zero pay | All leave informational; payroll handles externally | |
| Medical + vacation both paid, manual configurable | Most generous; most complex | |

**User's choice:** Medical=excluded, vacation=paid, unpaid=zero, manual=zero

---

## Period Model

| Option | Description | Selected |
|--------|-------------|----------|
| 3 presets + custom range (Recommended) | Semanal/Quincenal/Mensual + Personalizado (date-range picker) | ✓ |
| 3 presets only | Strict PAY-01 read | |
| Custom range only | Maximum flexibility, no presets | |

**User's choice:** 3 presets + custom range

| Option | Description | Selected |
|--------|-------------|----------|
| Calendar 1-15 / 16-EOM (Recommended) | VE payroll convention; two fixed cuts per month | ✓ |
| Floating 14-day window | Rolling 14-day from picked start | |
| Configurable in global_rules | global_rules.biweekly_mode = 'calendar' or 'floating' | |

**User's choice:** Calendar 1-15 / 16-EOM

| Option | Description | Selected |
|--------|-------------|----------|
| Monday–Sunday (Recommended) | Matches Phase 4 D-7 timesheet boundary; ISO 8601 | ✓ |
| Sunday–Saturday | Some VE companies; diverges from timesheet | |

**User's choice:** Monday–Sunday

| Option | Description | Selected |
|--------|-------------|----------|
| No lock — always live (Recommended for v1) | Always regenerate from current daily_records + overrides; audit_log captures who/when | ✓ |
| Snapshot on first export | Save snapshot row per (period, employee) on first export; reuse afterward | |
| Manual close button | Admin-driven 'Cerrar Período' freeze; v2 | |

**User's choice:** No lock — always live

---

## Report Scope & Columns

| Option | Description | Selected |
|--------|-------------|----------|
| Summary per employee (Recommended) | One row per employee per period; aggregated time + money | ✓ |
| Daily detail rows | One row per (employee, day); verbose | |
| Both — summary sheet + detail sheet | Excel: Resumen + Detalle sheets | |

**User's choice:** Summary per employee

| Option | Description | Selected |
|--------|-------------|----------|
| Department filter (Recommended) | Multi-select dropdown, default all | ✓ |
| Employee status: include inactive (Recommended) | Toggle to include terminated employees with attendance in period | ✓ |
| Single employee filter | Personal pay-slip generation, scoped to one row | ✓ |
| Shift type filter (day/night/mixed) | Filter by department.shift_type | ✓ |

**User's choice:** All four filters (multi-select)

| Option | Description | Selected |
|--------|-------------|----------|
| Identity: cédula, nombre, departamento, cargo (Recommended) | Identity columns | ✓ |
| Time: work_min, OT_min, late_min, days_worked, days_absent (Recommended) | Time aggregates from daily_records | ✓ |
| Money: work_pay, OT_pay, night_premium, rest_surcharge, late_deduction, total_a_pagar (Recommended) | Money math from Area 1 (USD cents) | (clarifier added) |
| Leave summary: días IVSS, vacación, permiso, no remunerado (Recommended) | Day counts per leave_type | ✓ |

**User's choice:** Identity + Time + Leave summary; Money columns clarified afterward.

**Clarifier Q (conflict check):** Area 1 chose Full money totals but Money columns weren't ticked. What did you mean?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep money cols per row (Recommended) | Per-row money columns ship; matches Area 1 D-01 | ✓ |
| Money totals only at footer | Per-row time-only; footer aggregates money | |
| No money in v1 | Drop money columns and footer totals | |

**User's choice:** Keep money cols per row — Area 1 D-01 confirmed.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — click row opens drill-down modal/page (Recommended) | Modal showing daily breakdown via existing GET /api/v1/daily-records | ✓ |
| Detail only via separate Excel sheet | Drill-down lives in Excel 'Detalle' sheet | |
| No drill-down — use Phase 4 timesheet | Operator switches screens for per-day view | |

**User's choice:** Yes — click row opens drill-down modal

---

## Anomaly Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Allow + flag column (Recommended) | Generate regardless; 'Anomalías' column shows count + codes; yellow row tint when count > 0 | ✓ |
| Block report if any anomaly | 409 Conflict listing affected rows; operator must resolve first | |
| Separate 'Anomalías' tab in Excel | Main sheet excludes anomalous rows; second sheet lists them | |
| Silent — no anomaly indicator | Don't surface in report | |

**User's choice:** Allow + flag column

| Option | Description | Selected |
|--------|-------------|----------|
| Zero work_minutes for that day + flag (Recommended) | Engine output preserved; flag in anomaly column | ✓ |
| Cap at shift_end_time | Pay unverified hours; fraud risk | |
| Exclude from total entirely | Day appears only in anomaly section | |

**User's choice:** Zero work_minutes + flag

| Option | Description | Selected |
|--------|-------------|----------|
| Leave wins, flag anomaly (Recommended) | Phase 3 D-16 inherited; no work pay; flag for audit | ✓ |
| Pay both — leave + work | Double-pay risk | |
| Block until resolved | Won't generate until punches invalidated or leave canceled | |

**User's choice:** Leave wins, flag anomaly

| Option | Description | Selected |
|--------|-------------|----------|
| Pay all OT minutes, flag cap breach (Recommended) | Pay all overtime_minutes at +50%; anomaly flag for cap breach; no truncation | ✓ |
| Cap-and-park excess | Pay only minutes within cap; carry forward excess | |
| Pay capped portion, drop excess | Aggressive labor-law compliance; risks shortchange | |

**User's choice:** Pay all OT minutes, flag cap breach

---

## RBAC Lock

| Option | Description | Selected |
|--------|-------------|----------|
| Admin + Supervisor (matches Phase 4 D-14) (Recommended) | Both roles export; matches existing 'Emitir Reporte' button visibility | ✓ |
| Admin only (matches ROADMAP wording) | Update Phase 4 D-14 (cosmetic) | |
| Admin + Supervisor + Viewer (read-only) | All view; only Admin/Supervisor export | |

**User's choice:** Admin + Supervisor — locks Phase 4 D-14 as truth.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — log every export with params (Recommended) | audit_log row per export; payload captures period, filters, format | ✓ |
| No audit — reports are read-only | No log; loses 'who pulled December payroll' trace | |
| Log only sensitive periods | Threshold-based; minor savings | |

**User's choice:** Yes — log every export with params

---

## Generation Flow & Persistence

| Option | Description | Selected |
|--------|-------------|----------|
| Sync — axum returns bytes directly (Recommended) | rust_xlsxwriter; Excel bytes streamed; no job queue | ✓ |
| Async — enqueue job, poll for status, download | reports_jobs table + worker; premature for v1 | |
| Sync with progress SSE | Stream progress events; complexity not justified | |

**User's choice:** Sync — axum returns bytes directly

| Option | Description | Selected |
|--------|-------------|----------|
| No — always live regenerate (Recommended) | Each export hits live data; audit_log captures generation events | ✓ |
| Persist last N exports per period | reports_archive table for last-3 per (period, range) | |
| Persist all — full export history | Indefinite storage | |

**User's choice:** No — always live regenerate

| Option | Description | Selected |
|--------|-------------|----------|
| Client-side jspdf-autotable (Recommended) | Browser renders PDF from JSON payload; matches ROADMAP | ✓ |
| Server-side via printpdf or wkhtmltopdf | Consistent rendering; new dep + fonts | |
| Server HTML → client print | window.print(); no real PDF artifact | |

**User's choice:** Client-side jspdf-autotable

| Option | Description | Selected |
|--------|-------------|----------|
| 60s axum timeout, frontend disabled-button + spinner (Recommended) | tower-http timeout middleware; UI disables button + spinner | ✓ |
| 30s timeout (CLAUDE.md default) | Tighter cap; risks edge cases | |
| No timeout for reports | Skip middleware; zombie connection risk | |

**User's choice:** 60s axum timeout

---

## Excel & PDF Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Single 'Resumen' sheet (Recommended) | All employees, sorted by dept then name; dept as column | ✓ |
| Multi-sheet: one per dept + 'Total' summary | Per-dept sheet + aggregate sheet | |
| Single sheet + 'Anomalías' sheet | Main + secondary anomalies sheet | |

**User's choice:** Single 'Resumen' sheet

| Option | Description | Selected |
|--------|-------------|----------|
| Per-dept subtotals + grand total (Recommended) | Subtotal row per dept + final 'Total General' | ✓ |
| Grand total only, no subtotals | Single bottom total | |
| No totals row — use Excel formulas | User adds SUM() manually | |

**User's choice:** Per-dept subtotals + grand total

| Option | Description | Selected |
|--------|-------------|----------|
| Client name + RIF + period header (Recommended) | Header section: 'Reporte Pre-Nómina | {client} | RIF: {rif} | Período: ... | Generado: ...'; no logo | ✓ |
| Plain — no branding header | Just data + totals | |
| Full branding incl. logo + address + colors | v2 territory | |

**User's choice:** Client name + RIF + period header

| Option | Description | Selected |
|--------|-------------|----------|
| Landscape A4, same data as Excel (Recommended) | jspdf-autotable landscape; same column set; auto-paging | ✓ |
| Portrait A4, summary-only (drop OT/late detail) | Trim to fit; loses time-detail | |
| Landscape A4, money-only | PDF as 'pay' artifact; Excel keeps full data | |

**User's choice:** Landscape A4, same data as Excel

---

## Tenant Info

| Option | Description | Selected |
|--------|-------------|----------|
| New tenant_info table + settings UI page (Recommended) | Migration creates single-row tenant_info (client_name, client_rif, address); 'Configuración / Datos de Empresa' Admin-only screen | ✓ |
| Env vars set by installer | Phase 6 installer prompts; .env-resident; read-only | |
| Extend setup wizard | Add fields to Phase 1 setup endpoint | |

**User's choice:** New tenant_info table + settings UI page

---

## Claude's Discretion

- Backend module layout (`reports/`, `tenant_info/`) following `{mod, models, service, handlers}` convention
- Exact Rust struct shapes for `EmployeeReportRow`, `ReportAggregates`, `ReportPayload`
- `rust_xlsxwriter` API specifics (Workbook builder, cell-format reuse, money formatting)
- jspdf-autotable styling (font, color, row striping) matching Phase 4 design tokens
- Migration numbering (`013_tenant_info.sql`, `014_phase5_audit_triggers.sql`)
- Frontend Reports screen component decomposition (filters bar, table, drill-down dialog, export buttons)
- Plan wave count: ROADMAP says 4, lists 2 — planner has freedom to split

## Deferred Ideas

(See CONTEXT.md `<deferred>` section for full list)
- Holiday calendar surcharge (HOL-01..03 v2)
- Period locking / "Cerrar Período"
- Per-department CBA premium overrides
- Per-minute night/day partition
- Async job queue
- Export history persistence
- Logo/palette branding
- Per-employee individual pay-stub PDF
- Multi-currency reports
- Direct payroll-system integration
- Audit panel UI
- `base_salary_cents` rename to `base_salary_usd_cents`
