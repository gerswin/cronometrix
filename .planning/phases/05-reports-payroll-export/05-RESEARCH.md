# Phase 5: Reports & Payroll Export - Research

**Researched:** 2026-04-25
**Domain:** Period-based payroll aggregation + Excel/PDF export (Rust+Axum backend, Next.js 16/React 19 frontend)
**Confidence:** HIGH (libraries, money math, audit pattern); MEDIUM (jspdf Spanish accents under WinAnsi); HIGH (period boundary math, rust_xlsxwriter API)

## Summary

Phase 5 is a thin aggregation layer plus two export pipelines. Most of the work is **structural** (computing money columns from existing time columns, generating workbooks, wiring a one-row settings table) — there is very little research-novel logic because CONTEXT.md already locked 30 decisions. The real risks are:

1. **Money math precision** — integer-only arithmetic in cents must be applied in a fixed multiplication order to avoid premature rounding; division is the lossy step and must come last per term.
2. **Night premium semantics** — D-04 says "+30% applied to whole shift when shift_type='night'" but doesn't say whether the +30% is *additive on top of* base work_pay (= 130%) or *replaces* it (= 30%). Industry standard reading of LOTTT Art. 117 is ADDITIVE — the night premium is an additional component, the worker still earns base pay. This RESEARCH.md adopts the additive reading and flags it for confirmation.
3. **jsPDF UTF-8** — Helvetica embedded in jsPDF uses WinAnsi (cp1252) which **does** include Spanish accent marks (á é í ó ú ñ Ñ ¿ ¡ ü). Standard usage works without custom font registration. Verified by inspecting jsPDF source mappings.
4. **rust_xlsxwriter version drift** — current crate version is **0.94.0** (verified via `cargo search` 2026-04). This is significantly newer than what CLAUDE.md hints at; pin exactly.
5. **TanStack Table v8 with department subtotals** — v8 has no first-class subtotal-row primitive; subtotals must be inserted as synthetic rows in the data array with a `_kind: 'subtotal' | 'grandtotal' | 'data'` marker to drive conditional cell rendering.

**Primary recommendation:** Adopt 4 plans (not 2) because the dependency graph is genuinely 4-wave: (05-01) tenant_info migration + CRUD + audit triggers, (05-02) reports calculation API + JSON endpoint, (05-03) Excel export endpoint, (05-04) Reports + Settings frontend screens. This isolates each risk surface (schema, money math, server binary export, client PDF render) and keeps each plan's verifiable surface narrow.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Money Math (D-01..D-07):**
- D-01: Money totals computed at report time (not persisted to daily_records); `work_pay`, `OT_pay`, `night_premium`, `rest_day_surcharge`, `late_deduction`, `total_a_pagar` all USD cents.
- D-02: Currency = USD; `departments.base_salary_cents` reinterpreted as USD cents (no schema rename in v1).
- D-03: Sunday/rest-day surcharge = +50% flat (LOTTT Art. 120) when `daily_records.is_rest_day_worked = 1`.
- D-04: Night premium = +30% applied to whole shift when `shift_type = 'night'` (LOTTT Art. 117). No per-minute partition. No premium when `shift_type` is `day` or `mixed`.
- D-05: Late deduction = `late_minutes × (base_salary_cents / ordinary_daily_minutes)`. Additional explicit line item; engine already excluded late time from `work_minutes`.
- D-06: Overtime premium = +50% (LOTTT Art. 118), regardless of cap status.
- D-07: Leave salary treatment per type: `medical` excluded from total_a_pagar (IVSS pays externally); `vacation` paid full; `unpaid` zero pay; `manual` zero pay default.

**Period Model (D-08..D-11):**
- D-08: Dropdown of `Semanal` / `Quincenal` / `Mensual` / `Personalizado`.
- D-09: Bi-weekly = calendar 1–15 / 16–EOM (NOT floating 14-day window).
- D-10: Weekly = Monday–Sunday (ISO 8601), aligned with Phase 4 D-7.
- D-11: No period locking; reports always regenerate from live `daily_records` + overrides.

**Report Scope & Columns (D-12..D-15):**
- D-12: One row per employee per period (aggregated).
- D-13: Filters = department multi-select, include_inactive toggle, single-employee picker, shift_type dropdown.
- D-14: Column set (locked — see CONTEXT.md for full list).
- D-15: Drill-down modal reuses existing `GET /api/v1/daily-records?employee_id=X&from=Y&to=Z`.

**Anomalies (D-16..D-19):**
- D-16: Always generate; flag in `Anomalías` column with code list; Excel applies yellow row tint when count > 0.
- D-17/D-18: Inherited engine semantics (MISSING_EXIT → work=0; EVENTS_ON_LEAVE_DAY → leave wins).
- D-19: OT cap exceeded → still pay all OT minutes at +50%; flag breach.

**RBAC & Audit (D-20..D-21):**
- D-20: Admin + Supervisor can generate/export. Viewer read-only on screen, no export buttons.
- D-21: Audit log entry per export (action='REPORT_EXPORT'); app-code insert (not trigger).

**Generation & Persistence (D-22..D-25):**
- D-22: Excel = sync streaming via `POST /api/v1/reports/excel` returning xlsx bytes inline.
- D-23: PDF = client-side `jspdf` + `jspdf-autotable`; backend exposes `POST /api/v1/reports/json`.
- D-24: No report persistence (no `reports_archive` table).
- D-25: 60s axum timeout; frontend disables button + spinner.

**Excel & PDF Structure (D-26..D-29):**
- D-26: Single 'Resumen' sheet, sorted by department then name.
- D-27: Per-dept subtotals + grand total (bold).
- D-28: Branding header (3 rows): client_name + RIF + period range + generation timestamp.
- D-29: PDF = landscape A4, repeated headers, page numbering footer.

**Tenant Info (D-30):**
- D-30: `tenant_info` single-row table (CHECK id=1), version column, audit trigger; GET (all auth roles) + PATCH (Admin only); new `Configuración / Datos de Empresa` screen.

### Claude's Discretion
- Backend module layout (`reports/` and `tenant_info/` modules).
- Whether reports calc lives under `daily_records/` or its own module (recommend: own module — different domain).
- Exact Rust struct shapes (`EmployeeReportRow`, `ReportPayload`).
- `rust_xlsxwriter` styling choices (cell formats, money format string).
- jspdf-autotable styling (font, color, row striping) — match Phase 4 design tokens.
- Migration numbering (next: `013_tenant_info.sql`, `014_phase5_audit_triggers.sql`).
- Plan count: 2 or 4. **Research recommends 4** (see Open Questions section).

### Deferred Ideas (OUT OF SCOPE)
- Holiday calendar + surcharge (HOL-01..03).
- Period locking / "Cerrar Período".
- Per-department CBA overrides (overtime multiplier, surcharge rates).
- Per-minute night/day partition (Phase 3 D-11 — still deferred).
- Async job queue for very large installations.
- Export history persistence (`reports_archive` table).
- Client logo / palette branding.
- Per-employee individual pay-stub PDFs.
- Multi-currency (VES + USD dual columns).
- Direct payroll system integration.
- Audit panel UI.
- `base_salary_cents` rename to `base_salary_usd_cents`.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PAY-01 | Admin can generate pre-payroll report for a configurable period (weekly/bi-weekly/monthly) | Period boundary math (chrono ISO week + calendar 1–15/16–EOM, no DST in Caracas); period selector dropdown decision D-08 |
| PAY-02 | Report includes work minutes, overtime, late deductions, and leave summary per employee | Money math formulas (LOTTT Art. 117/118/120); leave classification carries forward from Phase 3 `leaves` table; aggregation pattern from `daily_records.service::list` |
| PAY-03 | Report exports to Excel format | `rust_xlsxwriter` 0.94.0 — `Workbook::save_to_buffer() -> Result<Vec<u8>>`; axum binary response pattern with custom Content-Type/Disposition headers |
| PAY-04 | Report exports to PDF format | `jspdf` 4.2.1 + `jspdf-autotable` 5.0.7 client-side; landscape A4; WinAnsi covers Spanish accents (no custom font registration needed) |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Period aggregation across daily_records + overrides + leaves | API / Backend (Rust) | Database (SQL aggregates) | Money math + override-merge logic must be authoritative; SQL aggregation pre-filters then Rust applies LOTTT math per row |
| Money math (work_pay, OT_pay, night_premium, rest_day_surcharge, late_deduction, total_a_pagar) | API / Backend (Rust) | — | Cents-i64 integer math in pure Rust — testable + deterministic; never persisted (D-01) |
| Excel binary generation | API / Backend (Rust, `rust_xlsxwriter`) | — | Synchronous in handler thread; `Workbook::save_to_buffer()` returns Vec<u8> — small enough for sub-5s generation at 1000 employees per D-22 |
| PDF generation | Browser / Client (jspdf) | — | D-23 explicit: client-side. Backend never produces PDF — the `/reports/json` endpoint returns the raw payload that client renders |
| Audit log entry per export | API / Backend (app-code INSERT) | — | App-code insert (D-21) — no trigger because the export is a read action; INSERTs happen on `audit_log` itself with `actor_id` from JWT |
| Tenant info storage | Database (SQLite) | API / Backend (CRUD) | Single-row `tenant_info` table with CHECK(id=1); Admin-only PATCH with optimistic concurrency via `version` column |
| Report rendering UI | Browser / Client (Next.js + TanStack Table v8) | API / Backend (read endpoint) | Backend supplies JSON; frontend handles filters, summary table, drill-down modal, export button gating per RBAC role |
| Settings (`Configuración / Datos de Empresa`) screen | Browser / Client (form) | API / Backend (PATCH) | Standard CRUD pattern mirroring departments/rules forms |

## Project Constraints (from CLAUDE.md and frontend AGENTS.md)

- **Rust 1.77+ stable, Axum 0.8.8, Tokio 1.51, libSQL 0.9.30** — already pinned in Cargo.toml.
- **Frontend: Next.js 16.2.3, React 19.2.4, TypeScript 5, Tailwind 4, shadcn/ui, TanStack Query 5.99, TanStack Table 8.21.3, react-hook-form 7.72, zod 4.3.6, axios 1.15, sonner 2, recharts 3.8, lucide-react 1.8** — all already in package.json.
- **CRITICAL: Next.js 16 has breaking changes from training data** (per `frontend/AGENTS.md`) — middleware is now `proxy.ts` not `middleware.ts`, function name is `proxy` not `middleware`. Metadata can only be exported from server components. Plan must read `node_modules/next/dist/docs/` before proposing any new Next.js feature.
- **CRITICAL: zod is v4 (not v3)** and **date-fns is v4 (not v3)** — CLAUDE.md table is stale on these. Some v3 patterns will not compile.
- **Audit-everything:** Every mutation generates audit log entry. For Phase 5 the only mutation surface is `tenant_info` (covered by trigger) and the `REPORT_EXPORT` action on the audit_log itself (covered by app-code insert).
- **Spanish UI:** All user-facing strings in Spanish. Currency display `$X,XX` (Venezuelan locale uses comma as decimal but USD payroll context typically uses period — pick one and document; recommend dot per US-style USD formatting consistent with Excel `$#,##0.00` num_format).
- **Desktop-only ≥1280px** (Phase 4 D-3 inherited).
- **GSD workflow enforced** — no direct edits outside a `/gsd-*` command.
- **Bruno collections** (`bruno/cronometrix/`) — Phase 5 plans MUST add Bruno requests for the 3 new endpoints (per `implementation-checklist` skill convention noted in init context).

## Standard Stack

### Core (new in Phase 5)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust_xlsxwriter` | **0.94.0** | Server-side xlsx generation | `[VERIFIED: cargo search 2026-04-25]` — actively maintained by jmcnamara (port of Python XlsxWriter); only mature pure-Rust xlsx writer; `save_to_buffer()` returns `Vec<u8>` for direct axum response |
| `jspdf` | **4.2.1** | Client PDF generation core | `[VERIFIED: registry.npmjs.org/jspdf/latest 2026-04-25]` — de-facto standard for browser PDF; v4 dropped IE11 support; ESM-first |
| `jspdf-autotable` | **5.0.7** | jspdf plugin for tabular layout | `[VERIFIED: registry.npmjs.org/jspdf-autotable/latest 2026-04-25]` — compatible with jspdf ^2 \|\| ^3 \|\| ^4 (peer dep); v5 redesigned hooks API |

### Supporting (already in stack — no new deps)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `chrono` | 0.4 | ISO week math, date arithmetic | Period boundary computation (`IsoWeek`, `weekday().num_days_from_monday()`) — pattern already established in `daily_records/service.rs` (line 145-148) |
| `chrono-tz` | 0.10.4 | TZ-aware "now" for branding header | `Utc::now().with_timezone(&state.config.timezone)` for `Generado: {now}` field |
| `serde_json` | 1.0 | Audit log payload serialization | `payload_json` for REPORT_EXPORT entry |
| `uuid` | 1 | New IDs for tenant_info row, audit row | UUID v4 — same pattern as everywhere else |
| `validator` | 0.20 | DTO validation | `ReportParamsRequest`, `UpdateTenantInfoRequest` |
| `@tanstack/react-table` | 8.21.3 | Summary table | Already used in Phase 4 timesheet (re-use column-def pattern) |
| `@tanstack/react-query` | 5.99 | Mutation for export trigger + JSON fetch | `useMutation` with `onSuccess` triggering blob download or PDF render |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rust_xlsxwriter` | `xlsxwriter` (older bindings to libxlsxwriter C lib) | Rejected: C dependency, smaller community, older API. Pure-Rust `rust_xlsxwriter` has caught up and surpassed it. |
| `rust_xlsxwriter` | `umya-spreadsheet` | Rejected: heavier, designed for read+modify workflows, slower for write-only batch generation |
| `jspdf` (client) | server-side PDF (e.g. `printpdf`, `genpdf`, headless Chromium) | D-23 locked: client-side. Avoids backend PDF dependency, leverages browser fonts, keeps server stateless. Tradeoff: browser memory at 1000+ rows — acceptable per D-22 ≤5s scale. |
| `jspdf-autotable` | `pdf-lib` + manual layout | Rejected: would re-implement table layout logic; autotable handles repeating headers, page breaks, cell styling for free |
| Polling for completion | Sync inline response | D-22 explicit: sync streaming. Simpler client code; 60s timeout cap is generous. |

**Installation:**

Backend (`backend/Cargo.toml`):
```toml
rust_xlsxwriter = { version = "0.94.0", features = ["chrono", "zlib"] }
```
- `chrono` feature → write `chrono::DateTime` directly into cells (used in branding header `Generado` field).
- `zlib` feature → faster compression (xlsx files are zip archives). Pure-Rust default uses miniz_oxide which is fine but `zlib` shells to system zlib for ~30% speedup at large workbook sizes. For 1000-row monthly reports, the difference is negligible (~50ms); recommend leaving `zlib` out to avoid system dependency, fall back if generation slows.
- **Do NOT enable `constant_memory`** — that mode requires writing rows in strict order (top-to-bottom, left-to-right) and disables some formatting features like `set_row_format()`. The 1000-employee scale fits comfortably in heap (~2MB for the workbook), and we need `set_row_format()` for anomaly tinting (D-16).

Frontend (`frontend/package.json`):
```bash
npm install jspdf@4.2.1 jspdf-autotable@5.0.7
```

**Version verification commands the planner can rerun:**
```bash
cargo search rust_xlsxwriter --limit 1
curl -s "https://registry.npmjs.org/jspdf/latest" | jq '.version'
curl -s "https://registry.npmjs.org/jspdf-autotable/latest" | jq '.version'
```

## Architecture Patterns

### System Architecture Diagram

```
                     ┌──────────────────────────────────────────────┐
                     │          User (Admin/Supervisor)             │
                     └──────────────────┬───────────────────────────┘
                                        │ click "Emitir Reporte"
                                        ▼
        ┌──────────────────────────────────────────────────────────────┐
        │  Reports Screen (frontend/src/app/(dashboard)/reports/)      │
        │  ┌──────────────────────────────────────────────────────┐    │
        │  │ FiltersBar: period preset + custom range +           │    │
        │  │             department[] + include_inactive +         │    │
        │  │             employee_id? + shift_type?                │    │
        │  └─────────────┬────────────────────────────────────────┘    │
        │                │ filters validated (zod v4)                  │
        │                ▼                                              │
        │  ┌───────────────────────────────────────────────────────┐   │
        │  │ ReportSummaryTable (TanStack Table v8)                │   │
        │  │   data = report.rows + synthetic subtotal/grand rows  │   │
        │  └─┬─────────────┬─────────────┬─────────────────────────┘   │
        │    │             │             │                              │
        │    ▼             ▼             ▼                              │
        │  Drill-down    Export Excel   Export PDF                      │
        │  Modal         button         button                          │
        └────┼─────────────┼─────────────┼──────────────────────────────┘
             │             │             │
             │ existing    │ POST        │ POST /api/v1/reports/json
             │ /daily-     │ /reports/   │   (then jsPDF render in browser)
             │ records     │ excel       │
             ▼             ▼             │
        ┌────────────────────────────────┼────────────────────────────┐
        │  Backend (Axum) — /api/v1/reports/{json,excel}              │
        │                                                              │
        │  ┌──────────────────────────────────────────────────────┐   │
        │  │ require_supervisor_or_above middleware (D-20)        │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   ▼                                          │
        │  ┌──────────────────────────────────────────────────────┐   │
        │  │ reports::handlers::{generate_json, generate_excel}   │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   ▼                                          │
        │  ┌──────────────────────────────────────────────────────┐   │
        │  │ reports::service::compute_report(filters) →          │   │
        │  │   ReportPayload {                                    │   │
        │  │     header: BrandingHeader{client_name, rif, period},│   │
        │  │     rows: Vec<EmployeeReportRow>,                    │   │
        │  │     dept_subtotals: HashMap<dept_id, Aggregates>,    │   │
        │  │     grand_total: Aggregates,                         │   │
        │  │   }                                                  │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   │  reads ↓                                 │
        │  ┌────────────────┴─────────────────────────────────────┐   │
        │  │ SQL: JOIN daily_records dr                           │   │
        │  │      LEFT JOIN daily_record_overrides dro            │   │
        │  │        (status='active') ON dr.id = dro.daily_       │   │
        │  │      LEFT JOIN leaves l ON dr.leave_id = l.id        │   │
        │  │      JOIN employees e + departments d                │   │
        │  │      WHERE dr.anchor_date BETWEEN ? AND ?            │   │
        │  │        AND filters...                                 │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   ▼                                          │
        │  ┌──────────────────────────────────────────────────────┐   │
        │  │ Money math (pure Rust, cents-as-i64) per row +       │   │
        │  │ aggregation per department + grand total              │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   │                                          │
        │      JSON path ───┘                                          │
        │      Excel path ──┐                                          │
        │                   ▼                                          │
        │  ┌──────────────────────────────────────────────────────┐   │
        │  │ reports::excel::build_workbook(payload) →            │   │
        │  │   Vec<u8> (xlsx binary via                           │   │
        │  │   Workbook::save_to_buffer())                        │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   │                                          │
        │  ┌────────────────┴─────────────────────────────────────┐   │
        │  │ AUDIT INSERT into audit_log (D-21):                  │   │
        │  │   table='reports', operation='REPORT_EXPORT',        │   │
        │  │   actor_id=jwt.sub, new_data=filters_json            │   │
        │  └────────────────┬─────────────────────────────────────┘   │
        │                   ▼                                          │
        │  Response: (HeaderMap, Vec<u8>) with                         │
        │    Content-Type: application/vnd.openxmlformats-...          │
        │    Content-Disposition: attachment; filename="..."           │
        └───────────────────┬──────────────────────────────────────────┘
                            │
        ┌───────────────────┴──────────────────────────────────────────┐
        │  GET /api/v1/tenant-info  (require_auth — all roles)         │
        │  PATCH /api/v1/tenant-info (require_admin, optimistic vers.) │
        │   ↓                                                           │
        │  tenant_info table (CHECK id=1) — single-row settings store  │
        │  ↓ audit trigger fires on UPDATE → audit_log entry           │
        └───────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

Backend additions:
```
backend/src/
├── reports/
│   ├── mod.rs           # pub use service::compute_report; pub use models::*
│   ├── models.rs        # ReportParamsRequest, ReportPayload, EmployeeReportRow, Aggregates, BrandingHeader
│   ├── service.rs       # compute_report(state, params) — SQL aggregation + money math + audit insert
│   ├── money.rs         # Pure functions: work_pay(), ot_pay(), night_premium(), rest_day_surcharge(),
│   │                    # late_deduction(), total_a_pagar(). All cents-as-i64. Unit testable.
│   ├── periods.rs       # period_from_preset(period_type, ref_date) -> (NaiveDate, NaiveDate);
│   │                    # week_iso(ref_date), biweekly(ref_date, half), month_full(ref_date)
│   ├── excel.rs         # build_workbook(payload) -> Result<Vec<u8>, AppError>;
│   │                    # uses rust_xlsxwriter Workbook+Worksheet+Format
│   └── handlers.rs      # generate_json, generate_excel
├── tenant_info/
│   ├── mod.rs
│   ├── models.rs        # TenantInfo, UpdateTenantInfoRequest (Validator-derive)
│   ├── service.rs       # get_tenant_info(conn), update_tenant_info(conn, req, expected_version)
│   └── handlers.rs      # get_handler, patch_handler
└── db/migrations/
    ├── 013_tenant_info.sql              # CREATE TABLE + INSERT seed row
    └── 014_phase5_audit_triggers.sql    # AFTER UPDATE trigger on tenant_info
```

Frontend additions:
```
frontend/src/app/(dashboard)/
├── reports/
│   ├── page.tsx                              # Reports screen (replaces 397B placeholder)
│   ├── _components/
│   │   ├── filters-bar.tsx                   # period preset + custom range + dept multi-select + toggles
│   │   ├── report-summary-table.tsx          # TanStack Table v8 with subtotal/grand-total rows
│   │   ├── drill-down-dialog.tsx             # shadcn Dialog wrapping daily_records list
│   │   ├── export-buttons.tsx                # Excel + PDF buttons (RBAC-gated)
│   │   └── pdf-renderer.ts                   # client-side jspdf-autotable build from JSON payload
│   └── _hooks/
│       └── use-report.ts                     # TanStack Query hook for /reports/json
└── settings/                                  # NEW route group (sidebar nav addition)
    └── tenant-info/
        ├── page.tsx                          # Datos de Empresa form (Admin edit, others read-only)
        └── _components/
            └── tenant-info-form.tsx
```

Sidebar nav (`frontend/src/components/layout/sidebar.tsx`) needs a new item:
```typescript
{ href: '/settings/tenant-info', icon: Settings, label: 'Configuración' }
```
(Admin-only visibility — already-established `useAuth().role === 'admin'` gating pattern from Phase 4 D-14.)

### Pattern 1: Money Math (cents-as-i64, fixed multiplication order)

```rust
// reports/money.rs
//
// Critical invariant: integer math. Multiply ALL numerators first, then divide
// ONCE at the end. Premature division loses cent precision and silently shaves
// money off line items.
//
// Rounding policy: integer truncation (floor for positives, toward-zero overall).
// LOTTT does not specify cent-level rounding; integer truncation favors the
// employer fractionally — flag during user review if a stricter banker's
// rounding is required (no Phase 5 decision says it is).

/// Base work pay: pro-rated daily salary × minutes worked.
/// `work_pay = work_minutes × base_salary_cents / ordinary_daily_minutes`
pub fn work_pay_cents(work_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64 {
    if ordinary_daily_minutes <= 0 { return 0; } // defensive: dept misconfigured
    work_minutes
        .checked_mul(base_salary_cents)
        .map(|p| p / ordinary_daily_minutes)
        .unwrap_or(0) // overflow safeguard — see Pitfall 4
}

/// Overtime pay: minutes × salary_per_min × 1.5 (LOTTT Art. 118).
/// Pre-multiply by 150 then divide by 100 × ordinary_daily_minutes.
pub fn ot_pay_cents(ot_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64 {
    if ordinary_daily_minutes <= 0 { return 0; }
    ot_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(150))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Night premium: ADDITIVE +30% on top of work_pay (LOTTT Art. 117).
/// Applied to work_minutes when shift_type='night' (D-04 — whole-shift approximation).
/// Total night-shift pay = work_pay + night_premium = work_pay × 1.30.
pub fn night_premium_cents(work_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64 {
    if ordinary_daily_minutes <= 0 { return 0; }
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(30))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Sunday/rest-day surcharge: +50% (LOTTT Art. 120).
/// Applied to work_minutes when daily_records.is_rest_day_worked=1 (D-03).
pub fn rest_day_surcharge_cents(work_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64 {
    if ordinary_daily_minutes <= 0 { return 0; }
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(50))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Late deduction: pro-rated salary × late_minutes (D-05).
/// Returned as POSITIVE value — column header conveys deduction semantics; total_a_pagar
/// SUBTRACTS this value. Sign convention: positive in storage, displayed with leading "-" in Excel/PDF.
pub fn late_deduction_cents(late_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64 {
    if ordinary_daily_minutes <= 0 { return 0; }
    late_minutes
        .checked_mul(base_salary_cents)
        .map(|p| p / ordinary_daily_minutes)
        .unwrap_or(0)
}

/// Per-row total: work_pay + ot_pay + night_premium + rest_day_surcharge − late_deduction.
/// Medical leave excluded upstream (D-07): if daily_records.leave_id resolves to a medical
/// leave, the row contributes 0 to this total (work_minutes is already 0 from engine D-16).
pub fn total_a_pagar_cents(
    work_pay: i64,
    ot_pay: i64,
    night_premium: i64,
    rest_day_surcharge: i64,
    late_deduction: i64,
) -> i64 {
    work_pay
        .saturating_add(ot_pay)
        .saturating_add(night_premium)
        .saturating_add(rest_day_surcharge)
        .saturating_sub(late_deduction)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 480 min/day, $1000.00/day base salary → $1000.00 / 480 = ~$2.083 per minute
    // 240 minutes worked → 240 × 100000 / 480 = 50000 cents = $500.00
    #[test]
    fn work_pay_half_day() {
        assert_eq!(work_pay_cents(240, 100_000, 480), 50_000);
    }

    // 60 OT minutes at +50%: 60 × 100000 × 150 / (100 × 480) = 18750 cents = $187.50
    #[test]
    fn ot_pay_one_hour() {
        assert_eq!(ot_pay_cents(60, 100_000, 480), 18_750);
    }

    // 480 min night shift base = $1000.00; +30% = $300.00 premium ON TOP
    #[test]
    fn night_premium_full_shift() {
        let base = work_pay_cents(480, 100_000, 480);
        let prem = night_premium_cents(480, 100_000, 480);
        assert_eq!(base, 100_000);
        assert_eq!(prem, 30_000);
        // Total night-shift earnings: base + premium = 130000 = $1300.00 = 130% of base
    }

    // 15 minutes late at $1000/day: 15 × 100000 / 480 = 3125 cents = $31.25
    #[test]
    fn late_deduction_quarter_hour() {
        assert_eq!(late_deduction_cents(15, 100_000, 480), 3_125);
    }
}
```

### Pattern 2: Period Boundary Math

```rust
// reports/periods.rs
use chrono::{Datelike, Duration, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeriodPreset {
    Weekly,             // ISO Monday-Sunday containing ref_date
    BiweeklyFirst,      // 1st-15th of ref_date's month
    BiweeklySecond,     // 16th-EOM of ref_date's month
    Monthly,            // 1st-EOM of ref_date's month
    Custom(NaiveDate, NaiveDate),
}

/// Return the (from_date, to_date) inclusive bounds for a period preset.
/// All dates are in installation TZ (`America/Caracas` → no DST → no anomalies).
pub fn resolve_period(preset: PeriodPreset, ref_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    match preset {
        PeriodPreset::Weekly => {
            // ISO 8601 week: Monday is day 0
            let dow = ref_date.weekday().num_days_from_monday() as i64;
            let mon = ref_date - Duration::days(dow);
            let sun = mon + Duration::days(6);
            (mon, sun)
        }
        PeriodPreset::BiweeklyFirst => {
            let first = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 1).unwrap();
            let fifteenth = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 15).unwrap();
            (first, fifteenth)
        }
        PeriodPreset::BiweeklySecond => {
            let sixteenth = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 16).unwrap();
            let eom = last_day_of_month(ref_date.year(), ref_date.month());
            (sixteenth, eom)
        }
        PeriodPreset::Monthly => {
            let first = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 1).unwrap();
            let eom = last_day_of_month(ref_date.year(), ref_date.month());
            (first, eom)
        }
        PeriodPreset::Custom(from, to) => (from, to),
    }
}

/// Last day of a given (year, month) — handles 28/29/30/31 day months and leap years.
fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap() - Duration::days(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_wraps_through_iso_week() {
        // 2026-04-25 is a Saturday → week is 2026-04-20 (Mon) to 2026-04-26 (Sun)
        let (f, t) = resolve_period(PeriodPreset::Weekly, NaiveDate::from_ymd_opt(2026, 4, 25).unwrap());
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
        assert_eq!(t, NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
    }

    #[test]
    fn biweekly_february_leap_year() {
        // 2024 is a leap year — Feb 29 must be the EOM for 2da quincena
        let (f, t) = resolve_period(
            PeriodPreset::BiweeklySecond,
            NaiveDate::from_ymd_opt(2024, 2, 20).unwrap(),
        );
        assert_eq!(f, NaiveDate::from_ymd_opt(2024, 2, 16).unwrap());
        assert_eq!(t, NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
    }

    #[test]
    fn biweekly_february_non_leap_year() {
        let (_, t) = resolve_period(
            PeriodPreset::BiweeklySecond,
            NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        );
        assert_eq!(t, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }
}
```

### Pattern 3: SQL Aggregation Read with Override Merge

The reports module MUST read through the override-merge pattern, not raw `daily_records`. Phase 3 D-04 established `daily_record_overrides` as the operator's truth layer.

```rust
// reports/service.rs — period aggregation query (sketch)
//
// Strategy: ONE wide SQL query that fetches every daily_record in the period
// joined with overrides + leaves + employees + departments. Then in Rust:
//   1. Apply override values where present (override_work_minutes etc.)
//   2. Apply money formulas per row
//   3. Group by employee_id, sum within employee
//   4. Group employee aggregates by department, sum within department
//   5. Sum departments → grand total

const REPORT_QUERY: &str = "
    SELECT
        e.id            AS employee_id,
        e.employee_code AS cedula,
        e.name          AS nombre,
        d.id            AS dept_id,
        d.name          AS dept_name,
        d.base_salary_cents,
        d.ordinary_daily_minutes,
        d.shift_type,
        dr.id           AS dr_id,
        dr.anchor_date,
        dr.work_minutes,
        dr.overtime_minutes,
        dr.late_minutes,
        dr.is_rest_day_worked,
        dr.leave_id,
        l.leave_type,
        dro.override_work_minutes,
        dro.override_entry_at,
        dro.override_exit_at,
        (SELECT GROUP_CONCAT(code) FROM daily_record_anomalies WHERE daily_record_id = dr.id)
            AS anomaly_codes
    FROM daily_records dr
    JOIN employees e   ON e.id = dr.employee_id
    JOIN departments d ON d.id = dr.department_id
    LEFT JOIN daily_record_overrides dro ON dro.daily_record_id = dr.id AND dro.status = 'active'
    LEFT JOIN leaves l ON l.id = dr.leave_id AND l.status = 'active'
    WHERE dr.anchor_date BETWEEN ?1 AND ?2
      AND e.status = 'active'  -- include_inactive toggle flips this predicate
      AND (?3 IS NULL OR d.id = ?3)        -- single department filter (or all when NULL)
      AND (?4 IS NULL OR e.id = ?4)        -- single employee filter
      AND (?5 IS NULL OR d.shift_type = ?5)
    ORDER BY d.name, e.name, dr.anchor_date";
```

For multi-select department filter (`department_ids: Vec<String>`) the planner should build a dynamic IN clause using positional params (same pattern as `daily_records/service.rs::list` lines 439-462 and `anomalies/handlers.rs` lines 52-76). DO NOT use string concatenation — every existing module uses `libsql::Value` enum construction with positional placeholders.

### Pattern 4: rust_xlsxwriter Workbook → Vec<u8>

```rust
// reports/excel.rs
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet};
use crate::errors::AppError;
use super::models::ReportPayload;

pub fn build_workbook(payload: &ReportPayload) -> Result<Vec<u8>, AppError> {
    let mut workbook = Workbook::new();

    // -------- Pre-built formats (reuse to keep file size small) --------
    let header_title  = Format::new().set_bold().set_font_size(14);
    let header_meta   = Format::new().set_font_size(10);
    let col_header    = Format::new().set_bold().set_bg_color(Color::RGB(0xE5E7EB))
                                     .set_align(FormatAlign::Center).set_border(FormatBorder::Thin);
    let money_fmt     = Format::new().set_num_format("$#,##0.00");
    let money_neg     = Format::new().set_num_format("$#,##0.00;[Red]-$#,##0.00");
    let int_fmt       = Format::new().set_num_format("0");
    let hours_fmt     = Format::new().set_num_format("0.00");
    let anomaly_tint  = Format::new().set_bg_color(Color::RGB(0xFEF3C7)); // amber-100
    let subtotal_fmt  = Format::new().set_bold().set_top_border(FormatBorder::Thin);
    let grand_fmt     = Format::new().set_bold().set_bg_color(Color::RGB(0xDBEAFE)) // blue-100
                                     .set_top_border(FormatBorder::Double);

    let sheet = workbook.add_worksheet().set_name("Resumen")?;

    // -------- Branding header (rows 0-2) --------
    sheet.merge_range(0, 0, 0, 16, "Reporte Pre-Nómina", &header_title)?;
    sheet.merge_range(
        1, 0, 1, 16,
        &format!("{}    RIF: {}", payload.header.client_name_or_dash(), payload.header.client_rif_or_dash()),
        &header_meta,
    )?;
    sheet.merge_range(
        2, 0, 2, 16,
        &format!("Período: {} – {}    Generado: {}", payload.header.from_date, payload.header.to_date, payload.header.generated_at_iso),
        &header_meta,
    )?;

    // -------- Column headers (row 4 — leave row 3 blank) --------
    let cols = [
        "Cédula", "Nombre", "Departamento", "Cargo",
        "Min Trab", "Min Extra", "Min Retraso", "Días Trab", "Días Aus",
        "Pago Base", "Pago Extra", "Prima Nocturna", "Recargo Domingo", "Descuento Retraso", "Total a Pagar",
        "Días IVSS", "Días Vacación", "Días Permiso", "Días No Remunerado",
        "Anomalías",
    ];
    for (i, label) in cols.iter().enumerate() {
        sheet.write_with_format(4, i as u16, *label, &col_header)?;
    }

    // -------- Data rows --------
    let mut row = 5u32;
    for dept in &payload.departments_in_order {
        for emp in &payload.rows_by_dept[dept.id.as_str()] {
            // Tint the entire row when anomaly count > 0 (D-16).
            if emp.anomaly_count > 0 {
                sheet.set_row_format(row, &anomaly_tint)?;
            }
            sheet.write_with_format(row, 0, &emp.cedula, &Format::new())?;
            sheet.write_with_format(row, 1, &emp.nombre, &Format::new())?;
            sheet.write_with_format(row, 2, &emp.departamento, &Format::new())?;
            // ... time columns (int), money columns (money_fmt or money_neg for late_deduction), etc.
            // Money columns: write as cents/100.0 f64 so Excel formats correctly.
            sheet.write_with_format(row, 9, emp.work_pay_cents as f64 / 100.0, &money_fmt)?;
            sheet.write_with_format(row, 13, -(emp.late_deduction_cents as f64 / 100.0), &money_neg)?;
            // ... anomalies last column
            row += 1;
        }
        // Per-dept subtotal row (D-27)
        sheet.write_with_format(row, 0, &format!("Total {}", dept.name), &subtotal_fmt)?;
        // ... write subtotal sums in matching columns
        row += 2; // blank row between depts for visual separation
    }

    // Grand total row (D-27)
    sheet.write_with_format(row, 0, "Total General", &grand_fmt)?;
    // ... write grand total sums

    // Freeze the column header row so scrolling keeps headers visible.
    sheet.set_freeze_panes(5, 0)?;

    // Auto-fit column widths (uses enhanced_autofit feature only if enabled — basic
    // autofit ships in stable). Falls back to set_column_width manually if needed.
    sheet.autofit();

    // Convert workbook to bytes.
    workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx generation failed: {}", e)))
}
```

### Pattern 5: Axum Binary Response with Custom Headers

```rust
// reports/handlers.rs
use axum::{
    extract::{Json, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use crate::{auth::rbac::AuthUser, errors::AppError, state::AppState};
use super::{models::ReportParamsRequest, service, excel};

/// POST /api/v1/reports/excel — returns xlsx bytes inline.
/// Audit log entry inserted before the binary response is sent (D-21).
pub async fn generate_excel(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(params): Json<ReportParamsRequest>,
) -> Result<impl IntoResponse, AppError> {
    params.validate().map_err(AppError::from)?;

    // Compute payload (also writes the audit_log REPORT_EXPORT entry — see service::compute_report).
    let payload = service::compute_report(&state, &claims.sub, &params, "excel").await?;

    // Build xlsx bytes.
    let bytes = excel::build_workbook(&payload)?;

    // Build response headers per D-22.
    let filename = format!(
        "prenomina_{}_{}.xlsx",
        payload.header.from_date,
        payload.header.to_date
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::try_from(format!("attachment; filename=\"{}\"", filename))
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid filename header")))?,
    );

    Ok((StatusCode::OK, headers, bytes))
}
```

For the timeout (D-25), apply `tower_http::timeout::TimeoutLayer::new(Duration::from_secs(60))` per route group rather than globally:

```rust
// In main.rs route composition
let report_routes = Router::new()
    .route("/reports/json",  post(reports::handlers::generate_json))
    .route("/reports/excel", post(reports::handlers::generate_excel))
    .route_layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60)))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_supervisor_or_above,
    ));
```

### Pattern 6: jsPDF + jspdf-autotable Client-Side PDF

```typescript
// frontend/src/app/(dashboard)/reports/_components/pdf-renderer.ts
import { jsPDF } from 'jspdf'
import { autoTable } from 'jspdf-autotable'
import type { ReportPayload, EmployeeReportRow, Aggregates } from '@/lib/types'

export function renderReportPdf(payload: ReportPayload): void {
  // Landscape A4 — D-29
  const doc = new jsPDF({ orientation: 'landscape', format: 'a4' })

  // -------- Branding header (D-28) --------
  doc.setFontSize(16)
  doc.setFont('helvetica', 'bold')
  doc.text('Reporte Pre-Nómina', 14, 14)
  doc.setFontSize(10)
  doc.setFont('helvetica', 'normal')
  doc.text(
    `${payload.header.client_name || '—'}    RIF: ${payload.header.client_rif || '—'}`,
    14, 22,
  )
  doc.text(
    `Período: ${payload.header.from_date} – ${payload.header.to_date}    Generado: ${payload.header.generated_at_iso}`,
    14, 28,
  )

  // -------- Build body with synthetic subtotal/grand-total rows --------
  const head = [[
    'Cédula','Nombre','Depto','Min Trab','Min Extra','Min Retr','Días T','Días A',
    'Pago Base','Pago Extra','Prima Noc','Recargo Dom','Desc Retr','Total',
    'IVSS','Vac','Perm','No Rem','Anom',
  ]]
  const body: (string | number)[][] = []
  for (const dept of payload.departments_in_order) {
    for (const r of payload.rows_by_dept[dept.id]) {
      body.push([
        r.cedula, r.nombre, r.departamento,
        r.work_min, r.ot_min, r.late_min, r.days_worked, r.days_absent,
        fmtMoney(r.work_pay_cents), fmtMoney(r.ot_pay_cents),
        fmtMoney(r.night_premium_cents), fmtMoney(r.rest_day_surcharge_cents),
        '-' + fmtMoney(r.late_deduction_cents),
        fmtMoney(r.total_a_pagar_cents),
        r.days_ivss, r.days_vacation, r.days_permission, r.days_unpaid,
        r.anomaly_codes.join(', ') || '',
      ])
    }
    const sub = payload.dept_subtotals[dept.id]
    body.push([
      '', `Total ${dept.name}`, '',
      sub.work_min, sub.ot_min, sub.late_min, sub.days_worked, sub.days_absent,
      fmtMoney(sub.work_pay_cents), fmtMoney(sub.ot_pay_cents),
      fmtMoney(sub.night_premium_cents), fmtMoney(sub.rest_day_surcharge_cents),
      '-' + fmtMoney(sub.late_deduction_cents),
      fmtMoney(sub.total_a_pagar_cents),
      sub.days_ivss, sub.days_vacation, sub.days_permission, sub.days_unpaid, '',
    ])
  }
  const grand = payload.grand_total
  body.push([
    '', 'TOTAL GENERAL', '',
    grand.work_min, grand.ot_min, grand.late_min, grand.days_worked, grand.days_absent,
    fmtMoney(grand.work_pay_cents), fmtMoney(grand.ot_pay_cents),
    fmtMoney(grand.night_premium_cents), fmtMoney(grand.rest_day_surcharge_cents),
    '-' + fmtMoney(grand.late_deduction_cents),
    fmtMoney(grand.total_a_pagar_cents),
    grand.days_ivss, grand.days_vacation, grand.days_permission, grand.days_unpaid, '',
  ])

  autoTable(doc, {
    head,
    body,
    startY: 34,
    showHead: 'everyPage',
    styles: { font: 'helvetica', fontSize: 7, cellPadding: 1.5, overflow: 'linebreak' },
    headStyles: { fillColor: [30, 41, 59], textColor: 255, fontStyle: 'bold' }, // slate-800
    didParseCell: (hook) => {
      // Tint anomaly rows yellow (D-16) — anomaly column is index 18
      const anomalyText = String(hook.row.raw[18] ?? '')
      if (hook.section === 'body' && anomalyText.length > 0) {
        hook.cell.styles.fillColor = [254, 243, 199] // amber-100
      }
      // Bold subtotal/grand rows — detect by 'Total' marker in column 1
      const labelCell = String(hook.row.raw[1] ?? '')
      if (hook.section === 'body' && labelCell.startsWith('Total')) {
        hook.cell.styles.fontStyle = 'bold'
        hook.cell.styles.fillColor = [219, 234, 254] // blue-100 for grand
      }
    },
    didDrawPage: (data) => {
      const pageSize = doc.internal.pageSize
      const pageHeight = pageSize.height
      const pageNum = data.pageNumber
      const pageCount = doc.getNumberOfPages()
      doc.setFontSize(8)
      doc.text(
        `Página ${pageNum} de ${pageCount}`,
        pageSize.width - 14,
        pageHeight - 6,
        { align: 'right' },
      )
    },
  })

  const fileName = `prenomina_${payload.header.from_date}_${payload.header.to_date}.pdf`
  doc.save(fileName)
}

function fmtMoney(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`
}
```

### Pattern 7: TanStack Table v8 with Synthetic Subtotal Rows

```typescript
// frontend/src/app/(dashboard)/reports/_components/report-summary-table.tsx
//
// TanStack Table v8 has no first-class "subtotal" primitive — use synthetic
// rows in the data array tagged with `_kind` to drive conditional rendering.

type RowKind = 'data' | 'subtotal' | 'grandtotal'
type TableRow = (EmployeeReportRow | AggregatesRow) & { _kind: RowKind, _key: string }

function buildTableRows(payload: ReportPayload): TableRow[] {
  const rows: TableRow[] = []
  for (const dept of payload.departments_in_order) {
    for (const r of payload.rows_by_dept[dept.id]) {
      rows.push({ ...r, _kind: 'data', _key: `${dept.id}:${r.employee_id}` })
    }
    const sub = payload.dept_subtotals[dept.id]
    rows.push({
      ...sub, nombre: `Total ${dept.name}`, _kind: 'subtotal', _key: `${dept.id}:subtotal`,
    } as TableRow)
  }
  rows.push({
    ...payload.grand_total, nombre: 'TOTAL GENERAL',
    _kind: 'grandtotal', _key: 'grand',
  } as TableRow)
  return rows
}

// In the table cell renderer, switch styling on row._kind:
// - data         → default
// - subtotal     → font-semibold, border-t
// - grandtotal   → font-bold, bg-blue-50, border-t-2

// Anomaly tinting: in the row-level <tr className=...> apply bg-amber-50
// when row._kind === 'data' && row.anomaly_count > 0
```

### Anti-Patterns to Avoid

- **Floating-point money math.** Never use `f64` for cents-to-dollar conversion until the FINAL display step. All arithmetic stays in `i64` cents.
- **Re-fetching each daily_record individually.** The aggregation query MUST be a single JOIN — N+1 queries at 1000 employees × 30 days = 30k roundtrips and will blow the 5s budget.
- **Persisting computed money columns.** D-01 explicit: money is computed at report time. Persisting would create a stale-cache invalidation problem when overrides change.
- **Using `INSERT OR REPLACE` on `tenant_info`.** The CHECK(id=1) constraint would still pass but it would lose `created_at` and replace `version`. Use plain `UPDATE ... WHERE id = 1 AND version = ?`.
- **Setting `Content-Type: application/octet-stream` for xlsx.** Excel and Numbers identify xlsx by both magic bytes and Content-Type; the correct MIME is `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`. Octet-stream works but produces a generic icon and breaks "Open in Excel" intents on some browsers.
- **Forgetting to URL-encode the filename.** RFC 6266 requires `filename*=UTF-8''...` for non-ASCII filenames. Phase 5 filenames are `prenomina_YYYY-MM-DD_YYYY-MM-DD.xlsx` (ASCII-only) so this isn't an issue, but if the planner wants to embed `client_name` in the filename, it must encode properly.
- **Using TanStack Table v8 grouping API for subtotals.** v8's `getGroupedRowModel` is for collapse/expand grouping; it does NOT generate visible subtotal rows. Use synthetic row insertion instead.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| xlsx file generation | A custom OPC/zip writer | `rust_xlsxwriter` 0.94.0 | xlsx is a zip of XML parts with strict relationships; hand-rolling silently breaks Excel's "Repair" prompt |
| PDF rendering | A custom PDF byte writer | `jspdf` + `jspdf-autotable` | PDF spec is 1300+ pages; multi-page table layout with repeating headers is non-trivial |
| Money rounding policy | `f64` arithmetic | Integer cents-as-i64 with explicit final-divide | Floating-point silently loses cents at scale; demo a $0.01 drift across 1000 employees and the client will notice in their bank reconciliation |
| ISO week math | `(captured_at / 7) % ...` | `chrono::Datelike::weekday().num_days_from_monday()` | ISO 8601 week edge cases (year boundary weeks 52/53/1) are subtle |
| Last-day-of-month | `match month {2 => 28 \| 29, ...}` | "first of next month minus one day" trick | Handles leap years correctly without per-month logic |
| Audit log row writer | Re-implementing per module | App-code `INSERT INTO audit_log` with `actor_id` from JWT | Existing pattern for non-trigger audits — see `command_audit_log` in Phase 2 D-11 (referenced from CONTEXT.md), but for `REPORT_EXPORT` the row goes directly into `audit_log` with `table_name='reports'`, `record_id` = a synthetic UUID per export, `operation='REPORT_EXPORT'` |

**Key insight:** xlsx and PDF are deceptively complex binary/structured formats. Both `rust_xlsxwriter` and `jspdf` are mature single-purpose libraries with thousands of production users. Use them.

## Common Pitfalls

### Pitfall 1: Premature division loses cents
**What goes wrong:** Computing `(work_minutes / ordinary_daily_minutes) × base_salary_cents` instead of `(work_minutes × base_salary_cents) / ordinary_daily_minutes`. The first form does integer division before multiplication, throwing away the fractional part of the day.
**Why it happens:** Reads naturally left-to-right; matches mental model of "what fraction of a day did they work × daily rate."
**How to avoid:** Always multiply ALL numerators first; divide ONCE at the very end. The `money.rs` patterns above bake this in.
**Warning signs:** Pay totals are slightly off by the cent for any partial-day shift. Snapshot test should fail immediately with a known-input fixture.

### Pitfall 2: i64 overflow for premium-stacked computations
**What goes wrong:** `work_minutes × base_salary_cents × 150` in `i64` overflows when `base_salary_cents` is large. Worst case: 30 days × 1440 min/day = 43200 min × $1,000,000 base salary in cents (100,000,000) × 150 = 6.48 × 10^14. i64::MAX is 9.22 × 10^18, so we have headroom — but the multiplication ORDER matters: `i64::MAX / (1440 × 100_000_000 × 150)` ≈ 4.27, meaning we're safe up to ~4 months at max parameters. Per-row computation (single-day minutes) is comfortably safe.
**Why it happens:** Naïve multiplication chain without bounds-check.
**How to avoid:** Use `checked_mul` in money math; on overflow return 0 and log. The patterns above use this. Add a property test: random `(work_minutes, base_salary_cents, ordinary_daily_minutes)` inputs in realistic ranges (0..43200, 0..100_000_000_00, 360..600) — assert no panic.
**Warning signs:** Negative pay totals at very high salaries.

### Pitfall 3: Override-merge skipped on report read
**What goes wrong:** Reading `daily_records.work_minutes` directly without checking for an active `daily_record_overrides.override_work_minutes`. Operator edits silently invisible in the report.
**Why it happens:** It's tempting to write `SELECT work_minutes FROM daily_records` because that's what the engine writes.
**How to avoid:** Always `LEFT JOIN daily_record_overrides ON daily_record_id = dr.id AND status = 'active'` and then `COALESCE(dro.override_work_minutes, dr.work_minutes)` either in SQL or in Rust.
**Warning signs:** Reports diverge from what the user sees in the Phase 4 timesheet. Integration test: edit a daily_record via override, regenerate report, assert the edited value appears.

### Pitfall 4: Anomaly count vs anomaly codes mismatch
**What goes wrong:** Computing anomaly_count from one source (e.g., `LEFT JOIN COUNT`) and anomaly_codes from another (`GROUP_CONCAT`). With `LEFT JOIN`, rows with no anomalies still produce one result row with NULL → `COUNT(*)` returns 1, `GROUP_CONCAT` returns NULL.
**Why it happens:** `COUNT(*)` includes the NULL "no match" row; `COUNT(dra.id)` does not.
**How to avoid:** Either use a subquery `(SELECT GROUP_CONCAT(code) FROM daily_record_anomalies WHERE daily_record_id = dr.id)` (simpler, used in Pattern 3 above), or `COUNT(dra.id)` not `COUNT(*)`. Derive anomaly_count from the codes string after the read (split by ',' and count) — single source of truth.
**Warning signs:** Excel rows tinted yellow despite empty anomaly column, or vice versa.

### Pitfall 5: jsPDF + Spanish accents under WinAnsi
**What goes wrong:** Some sources online say jsPDF can't render Spanish accents and requires custom font registration.
**Reality:** jsPDF's default Helvetica uses WinAnsi (cp1252). cp1252 INCLUDES the full set of Spanish characters: á (E1), é (E9), í (ED), ó (F3), ú (FA), ñ (F1), Ñ (D1), ¡ (A1), ¿ (BF), ü (FC). They render correctly with no custom font setup. The "needs custom font" advice applies to non-cp1252 scripts (Cyrillic, Chinese, Arabic, Greek, etc.).
**How to avoid:** Test early with `doc.text('áéíóúñÑ¡¿üÜ', 14, 14)` and confirm visually. Add a smoke test fixture with Spanish names like "Iñaki Núñez" / "María José Peña".
**Warning signs:** Boxes / "?" characters appearing where accents should be. If this happens, revisit — it would indicate a different root cause (e.g., source string is not actually UTF-8 decoded).

### Pitfall 6: Workbook::save_to_buffer() blocking the async runtime
**What goes wrong:** `rust_xlsxwriter::Workbook::save_to_buffer()` is synchronous CPU-bound work. Called directly inside an async axum handler, it blocks the tokio worker thread for ~1-5s on a 1000-employee workbook. With Axum 0.8's default ~CPU-count worker threads, blocking one is fine — but at high concurrency this could starve other handlers.
**How to avoid:** Wrap the call in `tokio::task::spawn_blocking`:
```rust
let bytes = tokio::task::spawn_blocking(move || excel::build_workbook(&payload))
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;
```
This moves the CPU work to the blocking thread pool (default 512 threads). For 1-2 concurrent users (typical Cronometrix scale), the direct synchronous call is fine; for safety, use `spawn_blocking` from day one.
**Warning signs:** SSE event feed (Phase 4) stutters during a report export; dashboard KPI queries time out.

### Pitfall 7: Audit log entry written even when generation fails
**What goes wrong:** Inserting the `REPORT_EXPORT` audit row before computing the report, then having the computation fail. The audit log shows an export that didn't actually deliver bytes.
**How to avoid:** Insert the audit row AFTER `compute_report()` succeeds and BEFORE the `Ok(...)` response is built. The audit entry represents "data was assembled and prepared for delivery to actor X" — bytes leaving the wire after that point is the operational concern, not the audit boundary. Alternative: insert audit row inside the same transaction that materializes the read snapshot, so they atomic.
**Warning signs:** Phantom audit entries after backend errors.

### Pitfall 8: `tenant_info` UPDATE without `WHERE id = 1`
**What goes wrong:** `UPDATE tenant_info SET client_name = ?, version = version + 1 WHERE version = ?` accidentally leaving off `id = 1`. Currently safe because the table has only one row, but it's a foot-gun if someone ever inserts a second row (no defense-in-depth beyond the CHECK).
**How to avoid:** Always include `WHERE id = 1 AND version = ?`. The CHECK(id=1) will reject any INSERT with id != 1, but the UPDATE doesn't trip the CHECK.
**Warning signs:** None during normal operation. Defensive habit only.

### Pitfall 9: Filename in Content-Disposition not properly quoted
**What goes wrong:** Using raw `format!("attachment; filename={}", name)` without quotes. Browsers handle spaces and special characters differently when filename is unquoted.
**How to avoid:** Always wrap in double quotes: `format!("attachment; filename=\"{}\"", name)`. For Phase 5 the filename is generated server-side as `prenomina_{from}_{to}.xlsx` (ASCII, no spaces) so this is precautionary.

## Runtime State Inventory

This section is included for completeness even though Phase 5 is largely additive (no rename/refactor work).

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no existing data is renamed or migrated. New `tenant_info` row is seeded by migration `013_tenant_info.sql` with empty defaults. | None — verified by reviewing migrations 001-012 and CONTEXT.md scope |
| Live service config | None — no external service configuration changes. The Reports module is purely additive HTTP endpoints. | None |
| OS-registered state | None — no new background tasks, schedulers, or system services. | None |
| Secrets/env vars | None — no new env vars (D-30 explicit: tenant info is DB-resident). | None |
| Build artifacts | New crate dependency `rust_xlsxwriter` requires `cargo build` to fetch + compile (~2 min first build, cached after). New npm packages `jspdf` + `jspdf-autotable` require `npm install` in `frontend/`. | After Plan 05-01 / 05-04 land: run `cargo build` and `cd frontend && npm install` |

**The canonical question:** *After every file in the repo is updated, what runtime systems still have the old string cached, stored, or registered?* — Answer for Phase 5: nothing, because Phase 5 only adds new tables, new endpoints, and new screens.

## Code Examples

All code examples are in the "Architecture Patterns" section above (Patterns 1-7). Quoting them here would be duplicative.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Server-side PDF rendering with headless Chromium | Client-side jspdf-autotable | ~2020s — bundle sizes shrunk, browser memory grew | Removes a heavy backend dependency; works for ≤10k row reports |
| `xlsxwriter` (libxlsxwriter C bindings) | `rust_xlsxwriter` (pure Rust) | 2022-2024 — pure-Rust matured | No system C library dep; better Rust ergonomics; same Excel compat |
| Polling for long-running export jobs | Sync inline response with timeout | Always preferred when generation < 30s | Simpler client; no job state machine; no UI for "in progress" |
| TanStack Table v7 grouping API | Synthetic data rows + conditional row classes | TanStack Table v8 (2023+) | v8 dropped automatic subtotal generation; teams use synthetic rows |
| Persisted "report snapshots" | On-demand regeneration | When data is small + audit trail covers it | Eliminates cache invalidation; operator edits surface immediately (D-11) |

**Deprecated/outdated:**
- `react-table` v7 — replaced by `@tanstack/react-table` v8 (already on v8 in this project).
- `jspdf` v2 with `jspdf-autotable` v3 — both have been replaced by jspdf v4 + autotable v5; v5's hooks API (`didParseCell`, `didDrawPage`) is the current way to do cell theming and per-page footer.

## Validation Architecture

Phase 5 has rich invariants that map cleanly to property tests + snapshot tests + integration tests. The validation strategy below satisfies Nyquist Dimension 8 (test density proportional to behavior surface).

### Test Framework

| Property | Value |
|----------|-------|
| Backend framework | `cargo test` + `cargo nextest` (already in use); `proptest` 1.11 for property tests; `axum-test` 16 for HTTP integration |
| Frontend framework | `vitest` 4.1.5 + `@testing-library/react` 16.3.2 (already in use) |
| Backend config file | `Cargo.toml` `[dev-dependencies]` already configured |
| Frontend config file | `vitest.config.ts` (already in `frontend/`) |
| Quick run command (backend) | `cargo test --lib reports` (unit tests for money, periods); `cargo test --test calc_tests` for inherited engine sanity |
| Quick run command (frontend) | `cd frontend && npm test -- --run reports` |
| Full suite command (backend) | `cargo nextest run` |
| Full suite command (frontend) | `cd frontend && npm test -- --run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PAY-01 | Period preset Weekly returns ISO Mon–Sun for any ref_date | unit | `cargo test --lib reports::periods::tests` | ❌ Wave 0 (`backend/src/reports/periods.rs`) |
| PAY-01 | Period preset BiweeklyFirst returns 1st-15th of month | unit | `cargo test --lib reports::periods::tests::biweekly_first` | ❌ Wave 0 |
| PAY-01 | Period preset BiweeklySecond returns 16th-EOM (handles Feb 28/29) | unit | `cargo test --lib reports::periods::tests::biweekly_february` | ❌ Wave 0 |
| PAY-01 | Period preset Monthly returns 1st-EOM | unit | `cargo test --lib reports::periods::tests::monthly` | ❌ Wave 0 |
| PAY-01 | POST /reports/json with each preset returns expected date range in payload header | integration | `cargo test --test report_tests period_presets_in_payload` | ❌ Wave 0 (`backend/tests/report_tests.rs`) |
| PAY-02 | work_pay = work_minutes × base_salary / ordinary_daily_minutes (single row) | unit | `cargo test --lib reports::money::tests::work_pay_half_day` | ❌ Wave 0 (`backend/src/reports/money.rs`) |
| PAY-02 | OT pay = ot_min × base × 1.5 | unit | `cargo test --lib reports::money::tests::ot_pay_one_hour` | ❌ Wave 0 |
| PAY-02 | Night premium = work_min × base × 0.30 (additive on top) | unit | `cargo test --lib reports::money::tests::night_premium_full_shift` | ❌ Wave 0 |
| PAY-02 | Rest-day surcharge = work_min × base × 0.50 | unit | `cargo test --lib reports::money::tests::rest_day_surcharge` | ❌ Wave 0 |
| PAY-02 | late_deduction = late_min × base / ordinary | unit | `cargo test --lib reports::money::tests::late_deduction_quarter_hour` | ❌ Wave 0 |
| PAY-02 | total_a_pagar = work + ot + night + rest − late (saturating) | unit | `cargo test --lib reports::money::tests::total_a_pagar_composition` | ❌ Wave 0 |
| PAY-02 | Money math is monotonic in work_minutes (more minutes ⇒ more pay) | property | `cargo test --lib reports::money::tests::work_pay_monotonic` (proptest) | ❌ Wave 0 |
| PAY-02 | Money math never panics on random in-range inputs | property | `cargo test --lib reports::money::tests::no_panic_on_random_inputs` (proptest, 10k cases) | ❌ Wave 0 |
| PAY-02 | Subtotals = sum of constituent employee rows (per dept) | property | `cargo test --test report_tests subtotals_match_constituents` | ❌ Wave 0 |
| PAY-02 | Grand total = sum of all department subtotals | property | `cargo test --test report_tests grand_total_matches_subtotals` | ❌ Wave 0 |
| PAY-02 | Override merge: when override_work_minutes is set, report uses it (not engine's) | integration | `cargo test --test report_tests override_takes_precedence` | ❌ Wave 0 |
| PAY-02 | Medical leave row contributes 0 to total_a_pagar | integration | `cargo test --test report_tests medical_leave_excluded` | ❌ Wave 0 |
| PAY-02 | Anomaly column populated from daily_record_anomalies | integration | `cargo test --test report_tests anomaly_codes_in_payload` | ❌ Wave 0 |
| PAY-03 | xlsx bytes parse back as a valid Workbook (round-trip via calamine in test deps) | integration | `cargo test --test report_tests excel_round_trip` | ❌ Wave 0 |
| PAY-03 | xlsx contains expected branding header rows 0-2 | integration | `cargo test --test report_tests excel_branding_header_present` | ❌ Wave 0 |
| PAY-03 | xlsx contains per-dept subtotal row labeled "Total {Dept}" | integration | `cargo test --test report_tests excel_dept_subtotals_present` | ❌ Wave 0 |
| PAY-03 | xlsx response Content-Type = openxmlformats and Content-Disposition has filename | integration | `cargo test --test report_tests excel_response_headers` | ❌ Wave 0 |
| PAY-03 | Anomaly row tint applied to rows where anomaly_count > 0 | snapshot | `cargo test --test report_tests excel_anomaly_tint_snapshot` (compare cells.styles bytes) | ❌ Wave 0 |
| PAY-04 | PDF render produces non-empty Blob | unit (vitest) | `cd frontend && npm test -- --run pdf-renderer.test.ts` | ❌ Wave 0 (`frontend/src/.../pdf-renderer.test.ts`) |
| PAY-04 | PDF text() calls include Spanish accents without truncation | unit (vitest) | `cd frontend && npm test -- --run pdf-spanish-accents.test.ts` | ❌ Wave 0 |
| PAY-04 | PDF body has subtotal + grand total rows | unit (vitest) | `cd frontend && npm test -- --run pdf-row-composition.test.ts` | ❌ Wave 0 |
| RBAC | Viewer cannot POST /reports/excel (403 Forbidden) | integration | `cargo test --test report_tests viewer_blocked_on_export` | ❌ Wave 0 |
| RBAC | Admin can POST /reports/excel (200 OK) | integration | `cargo test --test report_tests admin_can_export` | ❌ Wave 0 |
| RBAC | Supervisor can POST /reports/excel (200 OK) | integration | `cargo test --test report_tests supervisor_can_export` | ❌ Wave 0 |
| Audit | Successful export inserts audit_log row with action='REPORT_EXPORT' | integration | `cargo test --test report_tests audit_entry_on_export` | ❌ Wave 0 |
| Audit | Failed export does NOT insert audit_log row | integration | `cargo test --test report_tests no_audit_on_failure` | ❌ Wave 0 |
| Tenant | GET /tenant-info returns single row | integration | `cargo test --test tenant_info_tests get_returns_seed_row` | ❌ Wave 0 (`backend/tests/tenant_info_tests.rs`) |
| Tenant | PATCH /tenant-info as Admin succeeds, increments version | integration | `cargo test --test tenant_info_tests admin_patch_succeeds` | ❌ Wave 0 |
| Tenant | PATCH /tenant-info as Supervisor returns 403 | integration | `cargo test --test tenant_info_tests supervisor_blocked` | ❌ Wave 0 |
| Tenant | PATCH with stale version returns 409 conflict | integration | `cargo test --test tenant_info_tests version_conflict` | ❌ Wave 0 |
| Tenant | UPDATE on tenant_info fires audit trigger → row in audit_log | integration | `cargo test --test tenant_info_tests audit_trigger_fires` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib reports` (unit tests, < 5s)
- **Per wave merge:** `cargo nextest run` (full backend suite, < 60s on M-class hardware) + `cd frontend && npm test -- --run` (frontend suite)
- **Phase gate:** Both full suites green before `/gsd-verify-work`

### Wave 0 Gaps

The following test files do not yet exist and must be created in the planning phase's Wave 0:

- [ ] `backend/src/reports/money.rs` — pure functions + inline `#[cfg(test)] mod tests` (8 unit tests + 2 proptest)
- [ ] `backend/src/reports/periods.rs` — pure functions + inline `#[cfg(test)] mod tests` (4 unit tests covering each preset + leap year)
- [ ] `backend/tests/report_tests.rs` — integration tests via `axum-test` (15 integration + property tests)
- [ ] `backend/tests/tenant_info_tests.rs` — integration tests for CRUD + RBAC + audit (5 tests)
- [ ] `frontend/src/app/(dashboard)/reports/_components/pdf-renderer.test.ts` — vitest unit tests (3 tests)
- [ ] No new framework installs required — proptest, axum-test, vitest already in dev deps.
- [ ] Add `calamine = "0.27"` to `[dev-dependencies]` for xlsx round-trip parsing in `excel_round_trip` test.

### Reconciliation Invariants (Property-Test Targets)

These are the LOAD-BEARING invariants that the property tests must enforce. Every plan produced by the planner must include explicit task verification of at least one property from this list.

1. **Period sum invariant:** For any period [from, to], `report.rows[i].work_min == sum(daily_records.work_minutes for that employee in that range, with overrides applied)`.
2. **Subtotal aggregation:** For any department D in any period P, `subtotals[D].work_min == sum(rows[i].work_min for i where rows[i].dept_id == D)`.
3. **Grand total aggregation:** `grand_total.work_min == sum(subtotals[D].work_min for all D)`.
4. **Money composition:** For any row, `total_a_pagar == work_pay + ot_pay + night_premium + rest_day_surcharge - late_deduction` (modulo saturating arithmetic).
5. **Medical exclusion:** For any row where `daily_records.leave_id` resolves to a `medical` leave on every day in the period, `total_a_pagar == 0`.
6. **Anomaly count consistency:** `anomaly_count == anomaly_codes.len()` (always — derive from a single source).
7. **No negative pay:** `total_a_pagar >= 0` UNLESS deductions legitimately exceed earnings (e.g., entire period was 100% late). Flag for visual review in report; do NOT block.
8. **xlsx round-trip:** `parse(build_workbook(payload)) == payload` (modulo styling that doesn't survive the round-trip — restrict comparison to data cells, not formatting).
9. **PDF determinism:** `render_pdf(payload)` produces byte-identical output for byte-identical input (jsPDF includes a `creationDate` — set it explicitly via `doc.setProperties({ creationDate: payload.header.generated_at })` to make tests deterministic).

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Existing JWT (HS256, jsonwebtoken 10) — Phase 5 endpoints attach `require_supervisor_or_above` / `require_admin` middleware |
| V3 Session Management | yes | Existing httpOnly refresh cookie + access token in memory; Phase 5 inherits, no changes |
| V4 Access Control | yes | Existing 3-role RBAC; Phase 5 explicit gates: Viewer cannot export (D-20); Admin-only PATCH on tenant_info (D-30) |
| V5 Input Validation | yes | `validator` 0.20 derive on `ReportParamsRequest` and `UpdateTenantInfoRequest`; reject malformed dates, unknown shift_type values, oversized text fields |
| V6 Cryptography | no | No new crypto — JWT signing already established; reports do not encrypt payloads |
| V7 Error Handling | yes | Existing `AppError::IntoResponse` with structured `{error: {code, message, status}}` envelope (D-11) — no leakage of internal SQL or stack traces |
| V8 Data Protection | yes | xlsx/PDF contain employee PII (name, cédula, salary). Audit log captures every export; no caching headers (use `Cache-Control: no-store` on export responses) |
| V12 File Resources | yes | xlsx is generated server-side and streamed inline — no temp files on disk; no path traversal surface |
| V13 API/Web Service | yes | All endpoints under `/api/v1/`; CORS already configured per origin; Content-Disposition uses safe ASCII filename |

### Known Threat Patterns for Rust+Axum + Next.js

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection in dynamic IN clause for department_ids | Tampering | Use `libsql::Value` enum + positional placeholders; never string-concatenate user input. The pattern in `daily_records/service.rs` lines 439-462 is the reference implementation. |
| Authorization bypass on export endpoint | Elevation of Privilege | Apply `require_supervisor_or_above` middleware at route registration; verify with negative-path test (Viewer → 403) |
| Unbounded period range causing OOM | DoS | Validate `to_date - from_date` ≤ 366 days at handler entry. Reject with 400 + clear error code. |
| PII leak via overly broad CORS | Information Disclosure | CORS already configured with explicit allowlist (no wildcard). xlsx response inherits same headers. |
| XSS in PDF generation | Tampering | jsPDF's `text()` escapes by default; never construct PDF from raw HTML. autoTable's body array values are typed and not rendered as HTML. |
| Audit-log injection via filename | Tampering | Filename is server-built from validated dates; user input never reaches the filename field. |
| `tenant_info` race condition (two admins editing simultaneously) | Tampering | Optimistic concurrency via `version` column; PATCH with stale version → 409 (existing pattern from departments/employees). |

## Sources

### Primary (HIGH confidence)
- `cargo search rust_xlsxwriter --limit 1` (executed 2026-04-25) — version 0.94.0 confirmed
- `https://registry.npmjs.org/jspdf/latest` (queried 2026-04-25) — version 4.2.1 confirmed
- `https://registry.npmjs.org/jspdf-autotable/latest` (queried 2026-04-25) — version 5.0.7 confirmed; peer dep `^2 || ^3 || ^4`
- `https://crates.io/api/v1/crates/rust_xlsxwriter` — confirms feature flags: `chrono`, `constant_memory`, `enhanced_autofit`, `jiff`, `polars`, `rust_decimal`, `serde`, `wasm`, `zlib`
- `backend/src/daily_records/service.rs` (lines 145-148, 439-462) — established ISO week math + dynamic predicate construction pattern
- `backend/src/anomalies/handlers.rs` (lines 52-108) — established dynamic SQL with positional params
- `backend/src/db/migrations/011_phase3_audit_triggers.sql` — exact AFTER UPDATE trigger pattern to mirror in `014_phase5_audit_triggers.sql`
- `backend/src/auth/rbac.rs` lines 31-77 — exact `require_admin` and `require_supervisor_or_above` extractor patterns
- `backend/src/main.rs` lines 144-176 — route group composition with `route_layer(from_fn_with_state(...))` pattern

### Secondary (MEDIUM confidence)
- `https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/` — Workbook + Format + Color API summary
- `https://users.rust-lang.org/t/how-to-return-a-vec-u8-as-file-from-axum/93009` — Vec<u8> → axum response with custom headers
- `https://rustxlsxwriter.github.io/workbook/saving.html` — `save_to_buffer()` method confirmation
- `https://xlsxwriter.readthedocs.io/format.html` — Format API behavior reference (Python parent project, similar API surface)
- `https://github.com/parallax/jsPDF/issues/12` and #2093 — UTF-8 / Unicode discussion (WinAnsi covers Latin-1 supplement which includes Spanish accents)

### Tertiary (LOW confidence — flagged for verification during execution)
- jsPDF Spanish accent rendering: confidence MEDIUM-HIGH (cp1252 includes the chars, jsPDF helvetica metric tables verified to map them) but recommend a smoke test fixture with `Iñaki Núñez` / `María José Peña` early in Plan 05-04 execution.
- `Workbook::save_to_buffer()` thread-safety on `Send` boundary — recommend wrapping in `tokio::task::spawn_blocking` for safety even though direct call appears to work.
- `enhanced_autofit` feature trade-off: not enabled in our Cargo.toml; basic `worksheet.autofit()` should suffice but the planner should compare visual output if column widths look wrong.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Night premium is ADDITIVE (+30% on top of work_pay), not REPLACING (= 30% of work_pay). Total night-shift earnings = 130% of base. | Pattern 1: Money Math; Money Math D-04 | Mid: pay totals would be ~77% of correct value if interpretation is wrong. LOTTT Art. 117 industry consensus is additive — but explicit user confirmation recommended before final implementation. |
| A2 | jsPDF Helvetica WinAnsi covers all Spanish accented characters used in employee names (á é í ó ú ñ Ñ ü ¡ ¿). | Pitfall 5 | Low: smoke test with real Spanish names early in Plan 05-04 will detect immediately. Fallback: register a TrueType font (Roboto, Inter, or DejaVu Sans) via `addFileToVFS()`. |
| A3 | 1000 employees × monthly period (30 days) = ~30k daily_record rows fits comfortably in 5s budget for SQL aggregation + money math + xlsx generation. | D-22, Common Pitfalls | Low: based on CONTEXT.md D-22 stating "empirically <5s observed". If wrong, Pitfall 6 mitigation (`spawn_blocking`) preserves async runtime; planner should benchmark with a fixture. |
| A4 | `rust_xlsxwriter::Workbook::save_to_buffer()` returns `Result<Vec<u8>, XlsxError>` (synchronous). | Pattern 4 | Very low: confirmed in docs.rs summary; specific error type to be verified during implementation. |
| A5 | Money math truncation rounding (integer floor) is acceptable; banker's rounding not required. | Pattern 1: Money Math | Low-Mid: LOTTT does not specify cent-level rounding policy. If client demands banker's rounding (round-half-to-even), the change is local to `money.rs`. |
| A6 | The `Anomalías` column tint applies based on `anomaly_count > 0`, with anomaly count derived from the `GROUP_CONCAT(code)` subquery (split by comma). | Pattern 3 SQL | Low: single-source-of-truth pattern verified safe; addressed by Pitfall 4. |
| A7 | The reports module does NOT need to apply soft-delete filtering on `daily_record_overrides` — the migration `009_daily_record_overrides.sql` introduced `status` column, and active overrides have `status='active'`. | Pattern 3 SQL | Low: pattern matches Phase 4 timesheet edit semantics. |
| A8 | Plan count = 4 is preferred over 2 because the dependency graph genuinely has 4 isolation surfaces. | Open Questions | Low: planner has explicit discretion (CONTEXT.md "Claude's Discretion") to choose. Recommendation only. |
| A9 | `cd frontend && npm install jspdf jspdf-autotable` does not introduce peer-dep conflicts with React 19 / Next.js 16 — both packages are framework-agnostic and ship pure JS bundles. | Standard Stack | Very low: no React peer dep; jspdf is browser-only ES module. |
| A10 | The "Cargo" / "Fecha Ingreso" columns referenced in Phase 4 D-11 employee table also exist in the Phase 5 report (D-14 lists `cargo` as identity column). The `employees` table currently has no `cargo` or `hire_date` columns (verified migration 001). | D-14 | Mid: Phase 5 may need an `ALTER TABLE employees ADD COLUMN cargo TEXT, hire_date INTEGER` migration. **OPEN QUESTION** — see Open Questions section. |

## Open Questions (RESOLVED 2026-04-25)

All 5 questions resolved during plan-phase orchestration. Locked answers live in `05-CONTEXT.md` D-30a, D-31, D-32, D-33, D-34.

1. **Night premium semantics (additive vs. replacing) — A1 above**
   - **RESOLVED → CONTEXT.md D-31:** ADDITIVE (+30% on top of work_pay). Industry-standard LOTTT Art. 117 reading. `night_premium` column carries the 30% surcharge only; `total_a_pagar` = work_pay + night_premium + others. Total night-shift earnings = 130% × base.

2. **Plan count — 2 or 4?**
   - **RESOLVED → CONTEXT.md D-32:** 4 plans.
     - 05-01: tenant_info CRUD + employees ALTER (cargo/hire_date) — Wave 1
     - 05-02: Reports calculation JSON API — Wave 2
     - 05-03: Excel export endpoint — Wave 3
     - 05-04: Frontend Reports + Settings + jspdf-autotable — Wave 4

3. **Does the `employees` table need new columns to display `cargo` and `Fecha Ingreso`? — A10 above**
   - **RESOLVED → CONTEXT.md D-30a:** YES. Migration `015_employees_position_hire_date.sql` adds `position TEXT NOT NULL DEFAULT ''` + `hire_date INTEGER NULL`. Phase 4 frontend already references both. Audit trigger `audit_employees_update` requires DROP+RECREATE in migration 014/016 to capture the new columns (PATTERNS gotcha #2).

4. **Should "Días Trabajados" / "Días Ausentes" be derived from daily_records, or include holidays/leaves?**
   - **RESOLVED → CONTEXT.md D-34:** Adopt researcher definition.
     - `días_trabajados` = count of daily_records in period where `work_minutes > 0` (after override merge)
     - `días_ausentes` = count of weekdays (Mon–Fri ISO 8601) in period with no daily_record AND no leave covering that date
     - Saturday/Sunday excluded from `días_ausentes`. Per-department `rest_days` resolution deferred to v2.

5. **Cents display: dot or comma decimal separator in Spanish UI?**
   - **RESOLVED → CONTEXT.md D-33:** Dot decimal `$1,234.56` via `Intl.NumberFormat('en-US', {style:'currency', currency:'USD'})`. Matches Excel `$#,##0.00` num_format for parity between UI and exported file.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Backend build | ✓ | 1.77+ stable (verified by existing build) | — |
| `cargo` | Cargo.toml dep resolution | ✓ | bundled with Rust | — |
| `npm` / `npx` | Frontend dep install | ✓ | bundled with Node | — |
| Node.js 22+ | Next.js 16 runtime | assumed ✓ (Phase 4 already shipped) | — | — |
| `jspdf` registry availability | Plan 05-04 install | ✓ | 4.2.1 confirmed available | — |
| `rust_xlsxwriter` registry availability | Plan 05-03 build | ✓ | 0.94.0 confirmed available | — |
| `calamine` (test-only xlsx parser) | Round-trip test (Wave 0) | not yet installed; available on crates.io | 0.27.x | Skip round-trip test; rely on snapshot bytes comparison only |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:**
- `calamine` for round-trip test — if it conflicts with existing dev-deps, fall back to snapshotting the xlsx bytes (less semantic but still catches regressions).

## Metadata

**Confidence breakdown:**
- Standard stack (rust_xlsxwriter, jspdf versions): HIGH — verified live against registries 2026-04-25
- Architecture / SQL aggregation patterns: HIGH — directly mirrors existing `daily_records/service.rs` and `anomalies/handlers.rs` patterns
- Money math formulas: HIGH for D-03/D-05/D-06/D-07; MEDIUM for D-04 (additive vs replacing — A1 assumption flagged)
- Period boundary math: HIGH — chrono ISO week pattern already in codebase; calendar 1-15/16-EOM is straightforward
- jsPDF Spanish accent rendering: MEDIUM-HIGH — WinAnsi covers cp1252 which includes Spanish; recommend smoke test
- Pitfalls: HIGH — drawn from concrete pattern matches in existing codebase + library docs
- Validation Architecture: HIGH — every test target maps to a concrete unit/integration/property/snapshot test
- Audit pattern: HIGH — exact pattern reuse from existing migration 011 + main.rs route composition

**Research date:** 2026-04-25
**Valid until:** 2026-05-25 (libraries are stable; jsPDF v4 was released > 1y ago, rust_xlsxwriter 0.94 is recent)
