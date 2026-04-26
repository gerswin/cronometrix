---
phase: 05-reports-payroll-export
reviewed: 2026-04-26T16:58:42Z
depth: standard
files_reviewed: 51
files_reviewed_list:
  - backend/Cargo.toml
  - backend/src/db/migrations/013_tenant_info.sql
  - backend/src/db/migrations/014_phase5_audit_triggers.sql
  - backend/src/db/migrations/015_employees_position_hire_date.sql
  - backend/src/db/mod.rs
  - backend/src/employees/models.rs
  - backend/src/employees/service.rs
  - backend/src/lib.rs
  - backend/src/main.rs
  - backend/src/reports/excel.rs
  - backend/src/reports/handlers.rs
  - backend/src/reports/mod.rs
  - backend/src/reports/models.rs
  - backend/src/reports/money.rs
  - backend/src/reports/periods.rs
  - backend/src/reports/service.rs
  - backend/src/tenant_info/handlers.rs
  - backend/src/tenant_info/mod.rs
  - backend/src/tenant_info/models.rs
  - backend/src/tenant_info/service.rs
  - backend/tests/fixtures/reports/seed.rs
  - backend/tests/reports_excel_test.rs
  - backend/tests/reports_test.rs
  - backend/tests/tenant_info_test.rs
  - bruno/cronometrix/reports/01_json.bru
  - bruno/cronometrix/reports/02_excel.bru
  - bruno/cronometrix/tenant-info/01_get.bru
  - bruno/cronometrix/tenant-info/02_patch.bru
  - frontend/package.json
  - frontend/src/app/(dashboard)/reports/page.tsx
  - frontend/src/app/(dashboard)/settings/tenant-info/page.tsx
  - frontend/src/components/layout/sidebar.tsx
  - frontend/src/components/reports/__tests__/drill-down-dialog.test.tsx
  - frontend/src/components/reports/__tests__/export-buttons.test.tsx
  - frontend/src/components/reports/__tests__/filters-bar.test.tsx
  - frontend/src/components/reports/__tests__/period-picker.test.tsx
  - frontend/src/components/reports/__tests__/summary-table.test.tsx
  - frontend/src/components/reports/drill-down-dialog.tsx
  - frontend/src/components/reports/export-buttons.tsx
  - frontend/src/components/reports/filters-bar.tsx
  - frontend/src/components/reports/period-picker.tsx
  - frontend/src/components/reports/summary-table.tsx
  - frontend/src/components/settings/__tests__/tenant-info-form.test.tsx
  - frontend/src/components/settings/tenant-info-form.tsx
  - frontend/src/components/ui/dialog.tsx
  - frontend/src/lib/format/__tests__/currency.test.ts
  - frontend/src/lib/format/currency.ts
  - frontend/src/lib/reports/__tests__/pdf.test.ts
  - frontend/src/lib/reports/pdf.ts
  - frontend/src/lib/validations.ts
  - frontend/src/test-utils/msw-handlers.ts
  - frontend/src/types/api.ts
findings:
  critical: 0
  warning: 6
  info: 7
  total: 13
status: issues_found
---

# Phase 5: Code Review Report

**Reviewed:** 2026-04-26T16:58:42Z
**Depth:** standard
**Files Reviewed:** 51
**Status:** issues_found

## Summary

Phase 5 ships the Reports & Payroll Export feature: backend `compute_report` (JSON + Excel) with LOTTT money math, the `tenant_info` singleton CRUD, employee `position`/`hire_date` columns, and a frontend Reports + Settings UI with PDF export. Overall code quality is high — security-critical surfaces are well-defended:

- **SQL injection** — All user input (`department_ids`, `employee_id`, `shift_type`, custom dates, leaves predicates) flows through `libsql::Value` + `params_from_iter`. No string interpolation of user data observed in either the primary `daily_records` JOIN or the secondary `leaves` aggregation. Predicates are built with positional placeholders (`?N`) only.
- **RBAC** — `/reports/*` correctly mounted under `require_supervisor_or_above`; `PATCH /tenant-info` correctly mounted under `require_admin`; `GET /tenant-info` under `require_auth` (Viewer-readable per D-09). Test suite covers the three role/endpoint matrices.
- **Money math** — `multiply numerators × constants → divide once at the end` order is consistent across `work_pay_cents`, `ot_pay_cents`, `night_premium_cents`, `rest_day_surcharge_cents`. `checked_mul` guards every step; `total_a_pagar_cents` uses `saturating_add`/`sub`. Property tests assert no panic on plausible inputs.
- **Period boundary math** — `resolve_period` correctly handles ISO Mon-Sun, biweekly 1-15 / 16-EOM, monthly 1-EOM with year-rollover via `last_day_of_month`. Tests cover leap-year February for both biweekly and monthly.
- **Audit log on REPORT_EXPORT** — `write_export_audit` is called AFTER aggregation succeeds (Pitfall 7). DoS guard (366-day cap), `from > to` check, and `period_type` parsing all return `AppError::Validation` BEFORE the audit insert, so failed reports leave no row. Verified by `no_audit_on_failure` integration test.
- **DoS guard** — 366-day check at L83 of `service.rs` runs before any DB query. 60-second timeout layer wraps the route group.
- **Frontend XSS** — All user-visible data is rendered through React JSX (auto-escaped) or jsPDF / autoTable (which treat input as text, not HTML). `doc.save(...)` filename is built from server-controlled `header.from_date`/`header.to_date`. No `dangerouslySetInnerHTML`, no `eval`, no string-template HTML construction observed.
- **Optimistic concurrency on `tenant_info`** — Service uses `WHERE id = 1 AND version = ?N` (RESEARCH Pitfall 8 belt-and-braces with `CHECK(id=1)`); `rows_affected == 0` returns `Conflict { code: "VERSION_CONFLICT" }`. Frontend handles 409 via `onError` → `invalidateQueries` to refetch. End-to-end covered by the `version_conflict` integration test.

The findings below are correctness/UX issues and migration-safety concerns, not security bugs.

## Warnings

### WR-01: Excel filename uses raw request dates, not resolved period boundaries

**File:** `backend/src/reports/handlers.rs:70`
**Issue:** The downloaded filename is built with `params.from_date` and `params.to_date` (the raw client-supplied strings, used as anchor/ref dates for non-custom presets) rather than the resolved `(from, to)` from `periods::resolve_period`. For any non-custom period (`weekly`, `biweekly_first`, `biweekly_second`, `monthly`) the user typically passes today's date as the anchor; the filename then misrepresents the actual period the workbook covers.

Example: User clicks "Weekly" on 2026-04-25 (Sat). The picker emits `from_date=2026-04-25`, `to_date=2026-04-25` (anchor). The backend resolves to Mon 2026-04-20..Sun 2026-04-26 and writes the workbook for that range. The downloaded file, however, is named `prenomina_2026-04-25_2026-04-25.xlsx` — looks like a single-day report.

The frontend export-buttons component duplicates this same bug at `export-buttons.tsx:31` (uses `filters.from_date`/`filters.to_date` for the `<a download="...">` attribute, which overrides the server's `Content-Disposition`).

**Fix:** Return the resolved period in the JSON response (already in `payload.header.from_date`/`payload.header.to_date`) and use those values in the handler, then propagate to the frontend:
```rust
// handlers.rs — use resolved boundary, not raw input
let payload = service::compute_report(&state, &claims.sub, &params, "excel").await?;
let filename = format!(
    "prenomina_{}_{}.xlsx",
    payload.header.from_date, payload.header.to_date
);
```
And on the frontend, surface the resolved range from a separate `/reports/json` call or have the backend echo it in a header (e.g. `X-Period-Resolved: 2026-04-20..2026-04-26`) so the `<a download>` matches.

---

### WR-02: Drill-down dialog passes anchor dates instead of resolved period

**File:** `frontend/src/app/(dashboard)/reports/page.tsx:91-93`
**Issue:** `<DrillDownDialog from={filters.from_date} to={filters.to_date} ... />` passes the raw filter values to the daily-records query. For non-custom presets these are the anchor date the picker emitted, NOT the resolved period the report actually covers. So when an admin runs a Weekly report anchored on Sat 2026-04-25 (which on the backend resolves to Mon 2026-04-20..Sun 2026-04-26) and clicks an employee row, the drill-down only fetches `daily_records` for `2026-04-25..2026-04-25` — empty for any captures earlier in the week. The user sees "Sin registros" and assumes data loss.

**Fix:** Read from the resolved header in the report payload after `reportQ.refetch()` succeeds:
```tsx
const resolvedFrom = reportQ.data?.header.from_date ?? filters.from_date
const resolvedTo   = reportQ.data?.header.to_date   ?? filters.to_date
// ...
<DrillDownDialog from={resolvedFrom} to={resolvedTo} ... />
```
Alternatively, mirror the backend's `resolve_period` in `period-picker.tsx::deriveDates` (already done) and have the page commit resolved dates into `filters.from_date`/`filters.to_date` for non-custom presets — the picker already does this in `handlePeriodTypeChange`/`handleHalfChange`. Audit the call sites to ensure the resolved values stay there until the next preset switch.

---

### WR-03: `shift_type` filter is not applied to the leaves aggregation, only the daily_records JOIN

**File:** `backend/src/reports/service.rs:162-165, 377-407`
**Issue:** The `shift_type` filter (`params.shift_type`) is added as a predicate on `dr.shift_type` in the primary daily_records JOIN, but is omitted from the W-5 secondary `leaves` aggregation. Result: when a user filters "show me only night-shift days," the response correctly scopes worked days to night shifts, but leave days (vacation, medical, etc.) for those same employees are still counted regardless of the employee's shift policy.

This may be intentional (leaves aren't tied to a specific shift). However it produces inconsistent aggregates — the "Días Vacación" column shows numbers that would not match a sanity check based on the same shift_type filter. At minimum this should be documented; if undesired, the leaves aggregation should join `departments d` and apply the same predicate against `d.shift_type` (the policy default, since leaves rows have no per-day shift).

**Fix:** Either (a) add a comment in `service.rs` documenting the deliberate divergence, or (b) extend the leaves predicate:
```rust
if let Some(st) = &params.shift_type {
    leave_predicates.push(format!("d.shift_type = ?{}", leave_values.len() + 1));
    leave_values.push(libsql::Value::Text(st.clone()));
}
```
A test case (`shift_type_filter_excludes_unrelated_leaves`) would lock the chosen behavior in.

---

### WR-04: Migration 014 uses `PRAGMA writable_schema` without integrity_check; risk to Turso embedded-replica sync

**File:** `backend/src/db/migrations/014_phase5_audit_triggers.sql:19-29`
**Issue:** The migration directly mutates `sqlite_master.sql` text via `UPDATE sqlite_master SET sql = replace(...)` under `PRAGMA writable_schema = 1`. The header comment correctly explains why this idiom is needed (modern SQLite recursively validates trigger references during the standard table-rebuild path). Two concerns remain:

1. **No `PRAGMA integrity_check` or `PRAGMA schema_version` increment after the rewrite.** SQLite's documentation for `writable_schema` recommends running `PRAGMA integrity_check` immediately after to confirm the schema is still parseable, and bumping `PRAGMA schema_version` so other connections re-read the schema. Without these, a malformed `replace()` (e.g. if a future migration restored the original CHECK text exactly) would be silently accepted and detonate at the next INSERT.
2. **Turso embedded-replica sync semantics for raw `sqlite_master` mutations are undocumented and possibly unsafe.** `init_db_remote` builds a `Builder::new_remote_replica(...)` with `read_your_writes(true)` and calls `db.sync()` afterwards. Whether the cloud replica receives — and correctly applies — a row-level UPDATE against `sqlite_master` is not guaranteed. If the cloud schema diverges (still has the old CHECK), a sync that pulls rows down (or any future Turso-driven rebuild from the cloud snapshot) could fail to insert `REPORT_EXPORT` audit rows.

**Fix:**
1. Add an explicit guard at the end of the migration:
```sql
PRAGMA writable_schema = 0;
PRAGMA integrity_check;
-- Force connections to re-read the schema (defensive; see SQLite writable_schema docs).
PRAGMA schema_version = schema_version;
```
2. Verify Turso behavior: in CI, run a sync round-trip against a Turso staging instance and assert `REPORT_EXPORT` insertions succeed both locally and post-sync. If sync is unsafe, the alternative is the legacy table-rebuild path with triggers temporarily dropped under `legacy_alter_table=ON` — more code but cloud-safe.
3. As long as Turso compatibility remains uncertain, document the constraint in the migration header (e.g. "Local-only — Turso replicas must run `cronometrix-api migrate` independently after update").

---

### WR-05: `set_row_format` followed by per-cell `write_with_format` does NOT tint anomaly rows

**File:** `backend/src/reports/excel.rs:179-187, 234-345`
**Issue:** The intent (D-16) is for rows with `anomaly_count > 0` to render with an amber-100 background. The implementation calls `sheet.set_row_format(row, &anomaly_tint)` and then writes every cell in that row via `write_with_format(row, col, val, &plain | &money_fmt | &int_fmt)`. In `rust_xlsxwriter`, when both a row-level format and a cell-level format are present for the same cell, the **cell format wins** — so the amber tint is overridden everywhere a cell has an explicit format (which is every cell on data rows). The amber tint visually shows up only on empty cells past column 19.

This is a visible product defect: the "anomaly rows highlighted in amber" UX promise from D-16 silently fails. The integration test (`excel_anomaly_data_present`) only asserts the anomaly column STRING content, not the cell color, so the regression is not caught.

**Fix:** Build cell-level formats with the amber background baked in for anomaly rows. One pattern:
```rust
let make_cell_fmt = |is_anomaly: bool, base: &Format| {
    if is_anomaly {
        base.clone().set_background_color(Color::RGB(0xFEF3C7))
    } else {
        base.clone()
    }
};
// then in write_employee_row, pass in the per-cell tinted formats
```
Alternatively, set the background on a "row default" format AND clear cell-level fill on data formats so the row format applies — see rust_xlsxwriter docs on format precedence.

To prevent regression, add a calamine assertion in `reports_excel_test.rs` that reads cell formatting (calamine 0.28's `Range` exposes `formula` + `cell_style`) and checks the fill-color hex on a known-anomaly row.

---

### WR-06: `UpdateTenantInfoRequest::version` accepts negative / zero values without validation

**File:** `backend/src/tenant_info/models.rs:31`
**Issue:** `version: i64` has no `validate(range(min = 1))` constraint. A client posting `version: 0` or `version: -42` will hit the WHERE clause, no row matches (real version starts at 1 and only increments), and the user gets a 409 VERSION_CONFLICT — which is technically correct but conflates "real concurrency loss" with "client sent garbage." Tightens error messages for API consumers.

**Fix:**
```rust
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTenantInfoRequest {
    // ...
    #[validate(range(min = 1, message = "version must be ≥ 1"))]
    pub version: i64,
}
```
Same hardening on `UpdateEmployeeRequest::version` (`employees/models.rs:51`) for consistency, since version semantics are identical.

## Info

### IN-01: `chrono::Datelike::num_days_from_monday` returns `u32`; cast to `i64` is unnecessary

**File:** `backend/src/reports/service.rs:493`
**Issue:** `d.weekday().num_days_from_monday() < 5` already compares `u32 < i32` (literal `5` is `i32` by default but Rust infers `u32` here). The earlier `let dow = ref_date.weekday().num_days_from_monday() as i64;` (`periods.rs:27`) does cast to `i64`, but the comparison form in service.rs works without it. Minor — keeps the code cleaner.

**Fix:** No change required; flagged for awareness. If style consistency matters, drop the explicit `as i64` from `periods.rs:27` since `Duration::days` accepts `i64` already and the conversion is implicit via `num::cast`.

---

### IN-02: Frontend `tenantInfoSchema` regex is laxer than backend; mismatch surfaces only on PATCH

**File:** `frontend/src/lib/validations.ts:68-70`
**Issue:** Frontend allows `^[VJG]-\d+-\d$` (any number of digits in the middle group). Backend (`tenant_info/models.rs:25`) only enforces `length(max=50)` — no RIF format check. A user could PATCH a malformed RIF directly via the API and the backend would store it. The frontend regex is purely cosmetic.

**Fix:** Either (a) drop the frontend RIF regex and document that v1 has no RIF format constraint (matches `D-30 minimal scope`), or (b) port the regex to a `validator::custom` on the backend so the frontend and backend agree. (b) is preferable for defense-in-depth — server-side validation of RIF prevents corrupted data when an integrator hits the REST API directly.

---

### IN-03: `dept_seen` ordering is independent of `compute_report` SQL `ORDER BY`

**File:** `backend/src/reports/service.rs:535-539`
**Issue:** `departments_in_order` is built from `dept_seen: BTreeMap<String, String>` (sorted by dept *id*) then re-sorted by `name`. The intent (D-26) is "alphabetical by name." This works correctly today because the final `.sort_by(|a,b| a.name.cmp(&b.name))` re-imposes the order, but the intermediate BTreeMap-by-id is wasted work and slightly misleading — readers might assume the BTreeMap key gives the order.

**Fix:** Use `BTreeMap<String /* name */, String /* id */>` keyed on name to preserve order naturally, OR switch to `Vec<DeptSummary>` collected via the SQL `ORDER BY d.name` already in the JOIN. Keep the secondary leaves aggregation in mind — it currently inserts via `dept_seen.entry(l_dept_id).or_insert(l_dept_name)`, so the data structure choice must support both insertion sites.

---

### IN-04: `frontend/src/components/reports/summary-table.tsx::buildTableRows` rebuilds on every render

**File:** `frontend/src/components/reports/summary-table.tsx:34-74, 83-86`
**Issue:** `buildTableRows` is exported as a pure helper for tests, then called inside a `useMemo` keyed on `[payload]`. Because `payload` changes reference whenever react-query refetches, the memoization is effectively just the standard tanstack-query referential stability. No bug — flagged for awareness because the function does an O(D × E) filter over `departments_in_order × rows`. For 1000-employee reports the filter walks 1000+ rows per dept; consider grouping via a `Map<dept_id, rows[]>` once at the top and looking up inside the dept loop.

**Fix:**
```tsx
const rowsByDept = useMemo(() => {
  if (!payload) return new Map<string, EmployeeReportRow[]>()
  const m = new Map<string, EmployeeReportRow[]>()
  for (const r of payload.rows) {
    const arr = m.get(r.dept_id) ?? []
    arr.push(r)
    m.set(r.dept_id, arr)
  }
  return m
}, [payload])
// then inside buildTableRows: rowsByDept.get(dept.id) ?? []
```
v1 scope (per phase plan) treats this as a performance improvement deferred unless real-world reports exceed the 5-second SLO.

---

### IN-05: `frontend/src/components/reports/period-picker.tsx::deriveDates` `half` parameter is dead code

**File:** `frontend/src/components/reports/period-picker.tsx:31, 45-46`
**Issue:** `half: '1' | '2' = '1'` parameter is declared, then explicitly discarded with `void half`. The TODO-style comment ("reserved for future range-style biweekly variants") is fine but the parameter contributes nothing today and is uncovered by tests.

**Fix:** Remove the parameter and the `void` line:
```tsx
export function deriveDates(periodType: PeriodType, ref: Date): { from: string; to: string } {
  // ...
}
```
Update call sites and the test file accordingly. Reintroduce the parameter when the feature actually arrives.

---

### IN-06: ExportButtons `_payload` prop is unused

**File:** `frontend/src/components/reports/export-buttons.tsx:13-18`
**Issue:** The component accepts `payload: _payload` but discards it (`void _payload`) and re-fetches via `api.post('/reports/json', filters)` on PDF export. The Excel button POSTs to `/reports/excel` directly, also ignoring the prop. Either remove the prop or use the in-memory payload for the PDF export to skip a duplicate audit insert.

**Fix:** If the audit-twice pattern is intentional ("every download = one audit row"), remove the prop entirely:
```tsx
interface Props {
  filters: ReportFilters
}
export function ExportButtons({ filters }: Props) { /* ... */ }
```
Otherwise, use the in-memory `payload` for PDF rendering and only re-POST for Excel:
```tsx
const exportPdfMutation = useMutation({
  mutationFn: async () => {
    if (_payload) renderReportPdf(_payload)
    else {
      const resp = await api.post<ReportPayload>('/reports/json', filters)
      renderReportPdf(resp.data)
    }
  },
  // ...
})
```
The phase plan should clarify which behavior is canonical for D-21.

---

### IN-07: `epoch_to_iso_date_opt` discards sub-day precision silently

**File:** `backend/src/employees/service.rs:11-16`
**Issue:** `parse_hire_date` stores the YYYY-MM-DD as midnight UTC epoch seconds, and `epoch_to_iso_date_opt` reads the date back via `.naive_utc().date()`. If a future migration ever stores a non-midnight epoch (e.g. accidentally using `unixepoch()` on a TIMESTAMP column), the date round-trip will silently truncate to whatever date UTC midnight falls on, possibly off by one in non-UTC timezones. Today this is correct because all writes go through `parse_hire_date`. Add a defensive comment.

**Fix:**
```rust
fn epoch_to_iso_date_opt(epoch: Option<i64>) -> Option<String> {
    // Assumes epoch is at UTC midnight (parse_hire_date contract). Sub-day
    // precision is silently truncated to the UTC date — fine for hire_date,
    // but DO NOT reuse this helper for fields that may carry a non-midnight
    // timestamp (use crate::common::epoch_to_iso instead).
    epoch.and_then(|t| {
        chrono::DateTime::<chrono::Utc>::from_timestamp(t, 0)
            .map(|dt| dt.naive_utc().date().to_string())
    })
}
```

---

_Reviewed: 2026-04-26T16:58:42Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
