# Phase 5: Reports & Payroll Export - Context

**Gathered:** 2026-04-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 5 produces the primary client deliverable: a period-based pre-payroll report covering work minutes, overtime, late deductions, and leave summaries per employee, exported to Excel and PDF. The report reads materialized `daily_records` (Phase 3 engine output) joined with `daily_record_overrides` (operator edits, Phase 3 D-04) and `leaves`, then converts time to USD amounts via LOTTT premiums. Excel via `rust_xlsxwriter` server-side. PDF via `jspdf-autotable` client-side.

**In scope (PAY-01..04):**
- Report calculation API: period aggregation across daily_records + overrides + leaves with money math
- Excel export endpoint: server-side workbook generation, sync streaming download
- PDF export: client-side rendering from JSON report payload
- Reports screen: period picker, filters, summary table, drill-down modal, export buttons
- Tenant info table + settings UI: client_name + RIF for report header branding
- Audit log entry per export

**Out of scope (Phase 5):**
- Holiday calendar + surcharge (HOL-01..03, v2)
- Period locking / "Cerrar Período" admin action (deferred)
- Per-department CBA overrides (overtime multiplier, rest-day surcharge rate)
- Per-minute night/day partition for shifts straddling 7pm/5am (Phase 3 D-11 deferred, still deferred)
- Async job queue for very large installations
- Export history persistence (reports_archive)
- Client logo image upload, custom palette
- Per-employee individual pay-stub PDFs
- Direct payroll-system integration (REQUIREMENTS Out of Scope)
- Audit panel UI (separate phase)

**Legal frame (carried from Phase 3):** Target jurisdiction = Venezuela. Labor law = LOTTT. Premiums applied here: Art. 117 (+30% night), Art. 118 (+50% overtime), Art. 120 (+50% Sunday/rest-day). OT caps from Art. 178 surface as anomaly flags only — minutes still paid.

</domain>

<decisions>

## Money Math

**D-01 — Full money totals computed at report time**
Per-row money columns ship: `work_pay`, `OT_pay`, `night_premium`, `rest_day_surcharge`, `late_deduction`, `total_a_pagar` (all USD cents in storage, displayed as `$X.XX`). Engine remains time-only per Phase 3 D-10 — premiums are computed inside the reports module, never persisted on `daily_records`.

**D-02 — Currency is USD**
`departments.base_salary_cents` is interpreted as USD cents from Phase 5 onward (Venezuelan dollar-pegged payroll practice). All money columns format as `$X.XX`. No schema rename in v1; document the semantic in `tenant_info` / settings copy. Multi-currency or VES dual-column deferred.

**D-03 — Sunday / rest-day surcharge = +50% flat (LOTTT Art. 120)**
Hardcoded constant applied to minutes worked when `daily_records.is_rest_day_worked = 1`. Per-department override deferred. Default rest days follow Phase 3 D-12 (Saturday + Sunday).

**D-04 — Night premium = +30% applied to whole shift when `shift_type='night'` (LOTTT Art. 117)**
Approximation matching Phase 3 D-11. Per-minute partition for shifts crossing 7pm/5am stays deferred. If `shift_type` is `day` or `mixed`, no night premium — even if some minutes fall in the night window.

**D-05 — Late deduction column = pro-rated salary**
Formula: `late_minutes × (base_salary_cents / ordinary_daily_minutes)`. Shown as `Descuento por Retraso` column (USD cents). The engine already excluded late time from `work_minutes`, so the deduction column is an additional explicit line item — visible signal to payroll team. Not a configurable knob in v1.

**D-06 — Overtime premium = +50% (LOTTT Art. 118)**
Applied to `overtime_minutes` regardless of cap status. Hardcoded constant.

**D-07 — Leave salary treatment per type**
- `medical`: `work_minutes=0`, **excluded** from `total_a_pagar` (IVSS pays externally). Day count appears as `Días IVSS` informational column.
- `vacation`: paid full at base salary (work_minutes treated as `ordinary_daily_minutes`).
- `unpaid`: zero pay; counted in `Días No Remunerado`.
- `manual`: zero pay by default; admin overrides handled via Phase 4 timesheet edit, not here.

## Period Model

**D-08 — Period selection = 3 presets + custom range**
Dropdown: `Semanal` / `Quincenal` / `Mensual` / `Personalizado`. Custom mode exposes a date-range picker. Picker UI uses shadcn Calendar Popover (already in Phase 4 stack).

**D-09 — Bi-weekly anchor = calendar 1–15 / 16–EOM**
VE payroll convention. Selecting `Quincenal` shows a month + cut picker (1ra quincena / 2da quincena). No floating 14-day window in v1.

**D-10 — Weekly boundary = Monday–Sunday**
Matches Phase 4 D-7 (timesheet view). ISO 8601. Same boundary keeps Reports + Timesheet aligned.

**D-11 — No period locking**
Reports always regenerate live from current `daily_records` + `daily_record_overrides`. If an operator edits a timesheet in a previously-exported period, the next regeneration reflects it. Audit trail comes from `audit_log` (D-21), not from frozen snapshots. Period locking + "Cerrar Período" deferred to v2.

## Report Scope & Columns

**D-12 — Row layout = summary per employee per period**
One row per employee for the chosen period. Aggregates across the period (sum of daily work_min, OT_min, etc.). Daily detail accessible via drill-down (D-15).

**D-13 — Filters exposed on Reports screen**
- Department: multi-select dropdown, default = all
- Include inactive employees: boolean toggle, default = off (only employees with status='active' or attendance in period)
- Single employee: search/picker for personal pay-slip generation; same export pipeline scoped to one row
- Shift type: dropdown (`day` / `night` / `mixed` / all), default = all

**D-14 — Column set (v1)**

Identity columns:
- `cédula` (employee national ID), `nombre`, `departamento`, `cargo`

Time columns:
- `work_min`, `OT_min`, `late_min`, `días_trabajados`, `días_ausentes` (display in hours where natural: `480min → 8.00h`)

Money columns (USD cents → `$X.XX`):
- `work_pay`, `OT_pay`, `night_premium`, `rest_day_surcharge`, `late_deduction` (negative), `total_a_pagar`

Leave summary columns (day counts):
- `Días IVSS` (medical), `Días Vacación`, `Días Permiso` (manual), `Días No Remunerado` (unpaid)

Anomaly column:
- `Anomalías`: count + comma-separated codes when present (D-16)

**D-15 — Drill-down modal**
Click a summary row → modal showing that employee's per-day breakdown for the period: anchor_date, entry_at, exit_at, work_min, OT_min, late_min, anomaly codes, override marker. Reuses existing `GET /api/v1/daily-records?employee_id=X&from=Y&to=Z`. No separate export from drill-down.

## Anomaly Handling

**D-16 — Allow + flag column**
Report generates regardless of unresolved anomalies. `Anomalías` column shows count + comma-separated codes (e.g., `MISSING_ENTRY, OT_CAP_EXCEEDED_DAILY`). Excel applies a yellow row tint when count > 0. Operator decides whether to fix before paying. No 409 / blocking on anomalies.

**D-17 — `MISSING_EXIT` → `work_minutes = 0` (engine output, inherited)**
No auto-cap to `shift_end_time` (avoids paying unverified hours). Day flagged in `Anomalías` column. Operator must edit timesheet via Phase 4 to backfill.

**D-18 — `EVENTS_ON_LEAVE_DAY` → leave wins (Phase 3 D-16, inherited)**
Day classified as leave per `leave_type`; no work pay. Anomaly flagged in column for audit. Avoids double-pay when admin error overlaps a captured punch with an approved leave.

**D-19 — OT cap exceeded → pay all OT minutes, flag breach**
All `overtime_minutes` paid at +50% (D-06) regardless of LOTTT Art. 178 caps. Anomaly column shows `OT_CAP_EXCEEDED_DAILY` / `_WEEKLY` / `_ANNUAL` per Phase 3 D-09. No carry-forward, no truncation. Mirrors Phase 3 stance: caps surface visibility, don't block calculation.

## RBAC & Audit

**D-20 — Admin + Supervisor can generate / export**
Locks Phase 4 D-14 as truth (supersedes ROADMAP "Admin can select" wording). Backend handlers compose `require_supervisor_or_above`. Frontend "Emitir Reporte" / "Exportar Excel" / "Exportar PDF" buttons visible to both roles. Viewer sees the screen and can browse data on-screen but no export buttons.

**D-21 — Audit log entry per export**
Every `POST /api/v1/reports/excel` (and any future export route) inserts an `audit_log` row:
- `actor_id` = JWT sub
- `action` = `REPORT_EXPORT`
- `payload_json` = `{period_type, from_date, to_date, filters: {department_ids, include_inactive, employee_id, shift_type}, format}`

Insert is app-code (not trigger) since the export is a read action that doesn't mutate `audit_log`-tracked tables.

## Generation Flow & Persistence

**D-22 — Excel generation = sync streaming**
`POST /api/v1/reports/excel` returns 200 with `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` bytes. `Content-Disposition: attachment; filename="prenomina_{from}_{to}.xlsx"`. No job queue. `rust_xlsxwriter` empirically handles 1000-employee monthly reports under 5s.

**D-23 — PDF generation = client-side `jspdf-autotable`**
Backend exposes `POST /api/v1/reports/json` returning the report payload as JSON. Browser uses `jspdf` + `jspdf-autotable` to render PDF locally. Matches ROADMAP. Adds `jspdf` + `jspdf-autotable` to frontend deps.

**D-24 — No report persistence**
Each export regenerates from live data. No `reports_archive` table. Audit log (D-21) is the canonical record of who/when/what params.

**D-25 — 60s axum timeout, frontend disables button + spinner**
`tower-http::timeout` middleware caps `/reports/*` at 60s. Frontend disables export button while in flight, shows shadcn `Spinner` + Sonner toast on success/error. 60s is comfortable safety margin for current scale (1000 employees ≤5s observed).

## Excel & PDF Structure

**D-26 — Single 'Resumen' sheet, sorted by department then name**
One sheet with all employees. Department visible as a column → in-Excel filtering still works. Multi-sheet (one per dept) deferred until a client with many departments asks. No separate `Anomalías` sheet — anomalies live in the per-row column (D-16).

**D-27 — Per-dept subtotals + grand total**
After the last employee in each department block: subtotal row labeled `Total {Departamento}` summing time + money columns (bold styling). Final row at bottom: `Total General` aggregating all departments.

**D-28 — Branding header**
Top of Excel sheet 1 (rows 1–3) and PDF first page:
```
Reporte Pre-Nómina
{client_name}    RIF: {client_rif}
Período: {from_date} – {to_date}    Generado: {now ISO}
```
No logo image in v1. Empty `client_name` / `client_rif` render as `—`.

**D-29 — PDF = landscape A4, same data as Excel**
`jspdf-autotable` config: orientation `landscape`, format `a4`, header rows repeat per page (`headStyles` + `pageBreak: 'auto'`). Identical column set to Excel for parity. Footer page number on each page.

## Tenant Info (new in this phase)

**D-30a — `employees` ALTER for `position` (cargo) + `hire_date`**
Phase 4 frontend (`employee-table.tsx`) already references `position` (header `Cargo`) and `hire_date` (header `Fecha Ingreso`), but migration `001_initial_schema.sql` lacks both columns. Migration `015_employees_position_hire_date.sql` adds:
```sql
ALTER TABLE employees ADD COLUMN position TEXT NOT NULL DEFAULT '';
ALTER TABLE employees ADD COLUMN hire_date INTEGER;  -- nullable epoch seconds (UTC)
```
Both columns surface in `EmployeeReportRow.cargo` (Phase 5 D-14 identity columns) and the Phase 4 employees table. Empty `position` renders as `—`. `hire_date` null renders as `—`. Backend `employees/handlers.rs` extends create/update payloads to accept these fields (optional). Audit triggers in `002_audit_triggers.sql` already hash the row — no trigger update needed (column added is INSIDE the audit row diff automatically).

**D-30 — `tenant_info` single-row table + Settings UI**
Migration `013_tenant_info.sql`:
```sql
CREATE TABLE tenant_info (
  id INTEGER PRIMARY KEY CHECK (id = 1),  -- enforce single row
  client_name TEXT NOT NULL DEFAULT '',
  client_rif TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  version INTEGER NOT NULL DEFAULT 1,
  updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
INSERT INTO tenant_info (id) VALUES (1);
```
Endpoints: `GET /api/v1/tenant-info` (Admin/Supervisor/Viewer read), `PATCH /api/v1/tenant-info` (Admin only, optimistic concurrency via `version`). Audit trigger on UPDATE writes to `audit_log`. New frontend screen: `Configuración / Datos de Empresa` (Admin-only edit, others read-only). Empty fields render as `—` in report header.

## Open Questions Resolved (post-research, locked 2026-04-25)

**D-31 — Night premium = ADDITIVE (+30% on top of work_pay)**
LOTTT Art. 117 standard reading. `night_premium` column = 30% premium ONLY (not 130% total). For night-shift minutes, total earnings = `work_pay + night_premium = 130% × base × minutes / ordinary_daily_minutes`. The Excel column shows the 30% premium so payroll team can audit the surcharge separately. `total_a_pagar` sums work_pay + night_premium (and other surcharges) — never double-counts. Locks researcher assumption A1.

**D-32 — Plan count = 4**
- 05-01: `tenant_info` (migration 013, audit trigger 014, CRUD module, GET/PATCH endpoints, Bruno requests) + `employees` ALTER (migration 015 — D-30a)
- 05-02: Reports calculation API (`reports/` module — money.rs, periods.rs, models.rs, service.rs, handlers.rs for `POST /reports/json`, audit insert per D-21, Bruno requests)
- 05-03: Excel export endpoint (`reports/excel.rs`, handler for `POST /reports/excel`, response headers per D-22, audit reuse, calamine round-trip integration tests)
- 05-04: Frontend Reports screen + Settings tenant-info screen + sidebar nav update + jspdf+jspdf-autotable integration

Wave 1 = 05-01 (independent); Wave 2 = 05-02 (depends on 05-01 schema); Wave 3 = 05-03 (depends on 05-02 service layer); Wave 4 = 05-04 (depends on 05-02 + 05-03 endpoints + 05-01 tenant-info GET).

**D-33 — Currency display = dot decimal `$1,234.56` (US/USD format)**
Backend stays in cents-as-i64. Frontend formatter: `Intl.NumberFormat('en-US', {style: 'currency', currency: 'USD'})` for parity with Excel `$#,##0.00`. PDF uses the same formatter. Avoids confusion when payroll team cross-checks UI vs exported file.

**D-34 — Días Trabajados / Días Ausentes definitions**
- `días_trabajados` = count of `daily_records` rows in period where `work_minutes > 0` (after `daily_record_overrides` merge per Phase 3 D-04)
- `días_ausentes` = count of weekdays (Monday–Friday, ISO 8601) in period where employee was active AND has no `daily_records` row AND no `leaves` row covering that date
- Saturday/Sunday excluded from `días_ausentes` regardless of `is_rest_day_worked` (avoids penalizing weekend off as absence). Per-department `rest_days` resolution deferred to v2 — uniform Mon–Fri default is the simplest defensible rule.

## Claude's Discretion

- Backend module layout: likely `reports/` (calc + handlers) + `tenant_info/` (CRUD) following the `{mod, models, service, handlers}` convention. Planner finalizes split.
- Whether reports calc is its own module or sub-module under `daily_records/`.
- Exact Rust struct shapes for `EmployeeReportRow`, `ReportAggregates`, `ReportPayload`. Snapshot tests pin behavior.
- `rust_xlsxwriter` API choices: `Workbook::new()` vs builder, cell-format reuse, money formatting (`Format::new().set_num_format("$#,##0.00")`).
- jspdf-autotable styling details (font, color, row striping) — match Phase 4 design tokens.
- Migration numbering: next available is `013_tenant_info.sql`. Audit triggers may go in `014_phase5_audit_triggers.sql` if separated.
- Whether to ship `reports_routes` under `/api/v1/reports/*` (recommended, matches existing pattern) or split exports under separate prefixes.
- Plan-count: ROADMAP header says 4 plans, body lists 2 (05-01, 05-02). Planner can split into 4 logical waves if scope demands, or keep 2. No hard preference here.
- Frontend Reports screen file structure: leverage existing `frontend/src/app/(dashboard)/reports/page.tsx` (currently 397B placeholder). Component breakdown (filters bar, summary table via TanStack Table, drill-down dialog) is Claude's call.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/REQUIREMENTS.md` — PAY-01..04 are Phase 5 scope; HOL-01..03 are v2 (no holiday surcharge in this phase)
- `.planning/PROJECT.md` — constraints (on-prem, audit-everything, Venezuela target market, USD-pegged payroll)
- `.planning/STATE.md` — accumulated decisions
- `.planning/phases/01-foundation/01-CONTEXT.md` — UUID PKs, UTC epoch INTEGER, version column, audit triggers, error envelope, offset pagination, 3-role RBAC, `/api/v1` router pattern
- `.planning/phases/02-device-integration/02-CONTEXT.md` — event store invariants (consumed indirectly via daily_records)
- `.planning/phases/03-time-calculation-engine/03-CONTEXT.md` — engine output schema (D-01), override layer (D-04), anomaly codes (D-18), leave overlay (D-13/D-16), shift_type + ordinary_daily_minutes (D-09/D-11), is_rest_day_worked (D-12). Phase 5 builds on top of all of these.
- `.planning/phases/04-frontend-ui/04-CONTEXT.md` — UI conventions, design tokens, RBAC button-level gating (D-14: Admin + Supervisor for "Emitir Reporte"), desktop-only ≥1280px (D-3), Spanish UI strings, TanStack Query patterns, sidebar nav with Reports already wired

### Backend code
- `backend/src/daily_records/models.rs` — `DailyRecordResponse`, `DR_SELECT_COLS` (Phase 5 reads aggregations)
- `backend/src/daily_records/service.rs` — read patterns + override-merge logic to reuse
- `backend/src/leaves/models.rs` + `service.rs` — leave classification (medical/vacation/unpaid/manual)
- `backend/src/anomalies/` — anomaly codes + severities (Phase 5 surfaces in column)
- `backend/src/departments/models.rs` — `base_salary_cents`, `shift_type`, `ordinary_daily_minutes`, `is_overnight_shift`
- `backend/src/employees/` — employees schema + status field
- `backend/src/auth/middleware.rs` + `rbac.rs` — `require_admin` (tenant_info PATCH), `require_supervisor_or_above` (report endpoints), `require_auth` (tenant_info GET)
- `backend/src/main.rs` — router composition; new `reports_routes` + `tenant_info_routes` nest under `/api/v1`
- `backend/src/errors.rs` + `common.rs` — reuse `AppError`, `epoch_to_iso()`; add `ReportError` variant if domain failures need typed errors
- `backend/src/state.rs` — `AppState` pattern (no new fields needed; tenant info is DB-resident, not cached in state)
- `backend/src/db/migrations/` — next available is `013_tenant_info.sql`; Phase 3 used through `012_shift_type_to_departments.sql`

### Frontend code
- `frontend/src/app/(dashboard)/reports/page.tsx` — current 397B placeholder, replace with full Reports screen
- `frontend/src/components/layout/sidebar.tsx` — Reports nav item already wired to Admin + Supervisor
- `frontend/src/lib/api.ts` — TanStack Query setup + 401 handler
- `frontend/src/components/ui/*` — shadcn primitives (Table, Dialog, Button, Select, DateRangePicker, Spinner, Sonner toast)
- `frontend/src/hooks/use-auth.ts` — role-based UI gating
- design tokens — locked in Phase 4 04-CONTEXT.md

### Stack reference
- `CLAUDE.md` — locked stack; planning step adds `rust_xlsxwriter` (latest) to backend `Cargo.toml`; `jspdf` + `jspdf-autotable` to frontend `package.json`. Versions to confirm during research.

### Venezuelan labor law (LOTTT — applied here, full citations in Phase 3 03-CONTEXT.md)
- Art. 117 — Jornada nocturna: +30% premium on night shifts (applied via D-04)
- Art. 118 — Horas extraordinarias: +50% premium on OT (applied via D-06)
- Art. 120 — Prima dominical: +50% surcharge on Sunday/rest-day work (applied via D-03, hardcoded rate)
- Art. 173 — Jornada ordinaria thresholds (already enforced by Phase 3 engine via `ordinary_daily_minutes`)
- Art. 178 — OT caps surface as anomaly flags only (Phase 3 D-09); Phase 5 still pays the minutes (D-19)
- Reference PDF: [LOTTT INCES official copy](https://www.inces.gob.ve/wp-content/uploads/2017/10/lot.pdf)

### External (research phase will deepen)
- `rust_xlsxwriter` docs — Workbook API, cell formatting (`#,##0.00`), conditional formatting / row tinting for anomalies
- `jspdf` + `jspdf-autotable` docs — landscape A4 config, repeated header rows, page numbering, headStyles theming
- VE payroll convention sources — bi-weekly cut conventions, USD-pegged salary practice, RIF format

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets
- `DailyRecordResponse` + `DR_SELECT_COLS` (`backend/src/daily_records/models.rs`, `service.rs`) — reports build aggregations on top.
- Override-merge read pattern (Phase 3 D-04) — `daily_records` joined with `daily_record_overrides` at read time. Reports module MUST use the same join helper, not raw `daily_records` reads.
- `AppError` + `IntoResponse` impl — reuse; add `ReportError` variant if needed for domain failures.
- `epoch_to_iso()` helper — for branding header `Generado: {now}` and any timestamp display.
- RBAC middleware — `require_supervisor_or_above` for report exports, `require_admin` for tenant-info PATCH, `require_auth` (3-role read-all) for GET endpoints.
- `Validator`-derive DTO pattern — for `ReportParamsRequest { period_type, from_date, to_date, department_ids?, include_inactive?, employee_id?, shift_type?, format }`.
- Phase 3 leaves classification (`leave_type` enum) — directly drives leave summary columns + medical-IVSS exclusion.
- Phase 4 sidebar already routes Reports to Admin + Supervisor; the page.tsx placeholder is the only frontend hook to replace.

### Established Patterns
- Module layout `{domain}/{mod.rs, models.rs, service.rs, handlers.rs}` — new modules `reports/` and `tenant_info/`.
- Audit triggers (Phase 1 `002_audit_triggers.sql`, Phase 3 `011_phase3_audit_triggers.sql`) — extend in `014_phase5_audit_triggers.sql` to cover `tenant_info` UPDATE. Report exports use app-code audit insert (D-21), not triggers.
- Version column + optimistic concurrency — applies to `tenant_info` (single-row PATCH).
- `/api/v1` router composition in `main.rs` — add `reports_routes` and `tenant_info_routes`, merge under existing auth groups.
- TanStack Query patterns — Phase 4 conventions for query keys, `defaultOptions.queries.queryFn`, 401 handler reuse.

### Integration Points
- `main.rs` bootstrap — no new background workers (sync export flow). Just route registration.
- `Config` — no new env vars (tenant info is DB-resident).
- `frontend/src/app/(dashboard)/reports/page.tsx` — 397B placeholder, replace with full screen. Component breakdown (filters bar, summary table, drill-down dialog, export buttons) is planner/Claude's call.
- `frontend/src/components/layout/sidebar.tsx` — Reports nav already wired; add a new `Configuración` item for tenant-info (Admin-only).
- Migration runner picks up `013_tenant_info.sql` and `014_phase5_audit_triggers.sql` automatically.
- Phase 3 daily_record_overrides override layer — reports read MUST use the existing helper that joins overrides on top of engine output.

</code_context>

<specifics>

## Specific Ideas

- Currency = USD. Venezuelan target market routinely pegs payroll to USD due to bolívar inflation. `departments.base_salary_cents` is interpreted as USD cents from this phase onward; no schema rename in v1 (rename to `base_salary_usd_cents` deferred unless naming confusion arises).
- Money math is **computed at report time**, never persisted. Engine stays time-only per Phase 3 D-10. If money policies change (e.g., Sunday +75% under a new contract), only the reports module changes — `daily_records` rows remain valid history.
- LOTTT Art. 120 +50% flat is hardcoded. The "research per-CBA override" path is intentionally deferred — adding a knob now without a client demanding it would just slow down v1.
- Phase 4 D-14 wins over ROADMAP wording: Admin + Supervisor both export. The ROADMAP success-criteria sentence ("Admin can select a report period…") was loose phrasing, not a deliberate restriction. Recommend updating ROADMAP language during the next milestone audit.
- Plan count: ROADMAP header says "4 plans" but the body lists 2 (05-01, 05-02). Planner has freedom to split into 4 waves if scope warrants (e.g., 05-01 calc API, 05-02 Excel exporter, 05-03 reports UI, 05-04 tenant-info CRUD + settings UI), or keep 2 if the dependency graph stays clean. Either is acceptable.
- Tenant info is intentionally minimal (client_name, client_rif, address). Logo image upload, custom palette, multi-tenant config — all v2.
- Report timeout 60s is empirically generous; if 1000-employee runs ever creep above 30s, revisit before adding a job queue.
- Drill-down (D-15) reuses Phase 4 timesheet's per-day data shape — no new backend endpoint needed.

</specifics>

<deferred>

## Deferred Ideas

- **Holiday calendar + surcharge (HOL-01..03)** — v2 per REQUIREMENTS.md. Adds calendar table, `salary_surcharge_pct` per holiday, engine overlay, and a column in reports.
- **Period locking / "Cerrar Período"** — admin-driven freeze of `daily_records` for a closed period; edits after close require explicit unlock. Strongest legal posture; v2.
- **Per-department CBA overrides** — `overtime_multiplier_pct`, `rest_day_surcharge_pct`, `night_premium_pct` columns. Add when a client's CBA diverges from LOTTT defaults.
- **Per-minute night/day partition** — precise apportionment of shifts straddling 7pm/5am (LOTTT Art. 117 boundary). Phase 3 D-11 deferred; still deferred.
- **Async job queue** — `reports_jobs` table + worker. Worth it only at >10k employee scale or multi-minute generation; not justified in v1.
- **Export history persistence** — `reports_archive` table, last-N or all-time. Defers to audit log (D-21) for now; revisit if compliance demands a binary artifact archive.
- **Client logo + palette branding** — image upload, color picker, font selection. v2 polish.
- **Per-employee individual pay-stub PDF** — separate template, optional bulk-zip download, optional email-to-employee. v2 feature.
- **Multi-currency reports** — VES + USD dual columns, FX rate config per period. Add when a client straddles currencies.
- **Direct payroll system integration** — REQUIREMENTS Out of Scope; Excel/CSV export covers it.
- **Audit panel UI** (`Panel de Auditoría` mockup `73MPC`) — separate phase; backend audit_log already populated.
- **`base_salary_cents` rename to `base_salary_usd_cents`** — cosmetic. Defer unless naming confusion arises.

</deferred>

<open_questions>

None — all gray areas resolved.

</open_questions>

---

*Phase: 05-reports-payroll-export*
*Context gathered: 2026-04-25*
