---
phase: 5
slug: reports-payroll-export
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-25
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Source: 05-RESEARCH.md `## Validation Architecture`.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (backend)** | `cargo nextest` (Rust 1.77+ stable, project default) |
| **Framework (frontend)** | `vitest` + `@testing-library/react` (Phase 4 baseline) |
| **Config file (backend)** | `backend/Cargo.toml` workspace dev-dependencies |
| **Config file (frontend)** | `frontend/vitest.config.ts` |
| **Quick run (backend)** | `cd backend && cargo nextest run -p cronometrix --no-fail-fast` |
| **Quick run (frontend)** | `cd frontend && npx vitest run --reporter=basic` |
| **Full suite** | `cd backend && cargo nextest run && cd ../frontend && npx vitest run` |
| **Estimated runtime** | ~60s backend (~30 unit + integration), ~25s frontend |

---

## Sampling Rate

- **After every task commit:** Run scoped quick command for the touched layer (`cargo nextest run -p cronometrix reports::` for backend reports tasks; `npx vitest run reports/` for frontend reports tasks)
- **After every plan wave:** Run full backend or full frontend suite for the wave's layer
- **Before `/gsd-verify-work`:** Full suite must be green (`cargo nextest run && npx vitest run`)
- **Max feedback latency:** 60s

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | PAY-01 (header source) | T-05-01 (admin-only PATCH) | `require_admin` rejects Supervisor PATCH with 403 | unit + integration | `cargo nextest run -p cronometrix tenant_info::` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | PAY-01 | — | Single-row CHECK constraint enforced | unit | `cargo nextest run -p cronometrix tenant_info::single_row` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | PAY-01 | T-05-02 (audit insert on PATCH) | `audit_log` row appears with `action='UPDATE'`, `entity_type='tenant_info'` | integration | `cargo nextest run -p cronometrix tenant_info::audit_trigger` | ❌ W0 | ⬜ pending |
| 05-01-04 | 01 | 1 | EMP-01..04 (column extension) | — | ALTER preserves existing rows; `position` defaults to `''`; `hire_date` nullable | migration test | `cargo nextest run -p cronometrix migrations::015_employees` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 2 | PAY-01, PAY-02 | T-05-03 (RBAC supervisor+) | `require_supervisor_or_above` rejects Viewer 403 | integration | `cargo nextest run -p cronometrix reports::rbac` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 2 | PAY-02 (money math) | — | `work_pay = work_min × base_cents / ord_min` (integer order) | unit (snapshot) | `cargo nextest run -p cronometrix reports::money::work_pay` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 2 | PAY-02 (D-04 night) | — | Night premium = 30% additive (D-31), only when `shift_type='night'` | unit (property) | `cargo nextest run -p cronometrix reports::money::night_premium` | ❌ W0 | ⬜ pending |
| 05-02-04 | 02 | 2 | PAY-02 (D-03 rest day) | — | Rest-day surcharge = 50%, only when `is_rest_day_worked=1` | unit | `cargo nextest run -p cronometrix reports::money::rest_day` | ❌ W0 | ⬜ pending |
| 05-02-05 | 02 | 2 | PAY-02 (D-05 late) | — | `late_deduction = late_min × base_cents / ord_min`; positive in storage, negative on display | unit | `cargo nextest run -p cronometrix reports::money::late_deduction` | ❌ W0 | ⬜ pending |
| 05-02-06 | 02 | 2 | PAY-02 (D-06 OT) | — | OT premium = 50% on `overtime_minutes` (no cap truncation per D-19) | unit | `cargo nextest run -p cronometrix reports::money::overtime` | ❌ W0 | ⬜ pending |
| 05-02-07 | 02 | 2 | PAY-02 (D-07 leave) | — | Medical leave excluded from `total_a_pagar`; vacation paid full; unpaid=0 | unit (table-driven) | `cargo nextest run -p cronometrix reports::money::leave_treatment` | ❌ W0 | ⬜ pending |
| 05-02-08 | 02 | 2 | PAY-01 (D-08..D-10 periods) | — | Weekly Mon–Sun ISO; quincenal 1–15 / 16–EOM; mensual 1–EOM; custom = passthrough | unit (property) | `cargo nextest run -p cronometrix reports::periods` | ❌ W0 | ⬜ pending |
| 05-02-09 | 02 | 2 | PAY-02 (D-12 row layout) | — | Aggregation: 1 employee → period rows summed correctly across `daily_records + overrides + leaves` | integration | `cargo nextest run -p cronometrix reports::aggregation` | ❌ W0 | ⬜ pending |
| 05-02-10 | 02 | 2 | PAY-02 (D-13 filters) | T-05-04 (filter SQL injection) | dept_ids parameterized; `include_inactive` toggle; employee_id scoped to one row; shift_type filter applied | integration | `cargo nextest run -p cronometrix reports::filters` | ❌ W0 | ⬜ pending |
| 05-02-11 | 02 | 2 | PAY-01 (D-21 audit) | T-05-05 (audit on every export) | `audit_log` row with `action='REPORT_EXPORT'`, `payload_json` contains full filter set | integration | `cargo nextest run -p cronometrix reports::audit_export` | ❌ W0 | ⬜ pending |
| 05-02-12 | 02 | 2 | PAY-02 (D-34 días) | — | `días_trabajados` count of `daily_records.work_minutes > 0`; `días_ausentes` count of weekdays with no record + no leave | unit | `cargo nextest run -p cronometrix reports::dias_count` | ❌ W0 | ⬜ pending |
| 05-02-13 | 02 | 2 | PAY-02 (D-16 anomalies) | — | `anomalies` column = comma-separated codes; count ≥ 0 | integration | `cargo nextest run -p cronometrix reports::anomaly_column` | ❌ W0 | ⬜ pending |
| 05-02-14 | 02 | 2 | PAY-02 (totals invariant) | — | Property: sum of per-employee `total_a_pagar` ≡ grand_total within 1 cent | property | `cargo nextest run -p cronometrix reports::invariants` | ❌ W0 | ⬜ pending |
| 05-03-01 | 03 | 3 | PAY-03 (xlsx) | T-05-06 (binary response headers) | `Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` + `Content-Disposition: attachment` | integration | `cargo nextest run -p cronometrix reports::excel::headers` | ❌ W0 | ⬜ pending |
| 05-03-02 | 03 | 3 | PAY-03 (xlsx round-trip) | — | calamine parses generated bytes; sheet names + cell values match expected | integration | `cargo nextest run -p cronometrix reports::excel::round_trip` | ❌ W0 | ⬜ pending |
| 05-03-03 | 03 | 3 | PAY-03 (D-26..D-28 layout) | — | Single 'Resumen' sheet; sorted dept→name; subtotal rows after dept blocks; grand total at bottom; branding rows 1–3 | snapshot | `cargo nextest run -p cronometrix reports::excel::layout_snapshot` | ❌ W0 | ⬜ pending |
| 05-03-04 | 03 | 3 | PAY-03 (D-16 anomaly tint) | — | Rows with `anomaly_count > 0` have amber-100 background via `set_row_format` | snapshot | `cargo nextest run -p cronometrix reports::excel::anomaly_tint` | ❌ W0 | ⬜ pending |
| 05-03-05 | 03 | 3 | PAY-03 (perf) | — | 1000-employee monthly report < 5s on dev hardware | benchmark | `cargo nextest run -p cronometrix reports::excel::bench --release` | ❌ W0 | ⬜ pending |
| 05-04-01 | 04 | 4 | PAY-01 (Reports screen) | T-05-07 (RBAC button gating) | Viewer cannot see Exportar buttons; Admin/Supervisor can | unit (RTL) | `npx vitest run reports/page.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-02 | 04 | 4 | PAY-01 (period picker) | — | All 4 period types render correct `from`/`to` dates | unit (RTL) | `npx vitest run reports/period-picker.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-03 | 04 | 4 | PAY-01 (filters) | — | Department multi-select, include_inactive toggle, employee picker, shift_type dropdown all populate correctly | unit (RTL) | `npx vitest run reports/filters.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-04 | 04 | 4 | PAY-01 (summary table) | — | TanStack Table renders identity + time + money + leave + anomaly columns; subtotal rows styled bold | unit (RTL) | `npx vitest run reports/summary-table.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-05 | 04 | 4 | PAY-01 (drill-down) | — | Click row opens dialog with per-day breakdown via `GET /daily-records` | unit (RTL + MSW) | `npx vitest run reports/drill-down.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-06 | 04 | 4 | PAY-04 (PDF) | — | jspdf + autotable renders landscape A4; Spanish accents (Iñaki, Núñez) display correctly | unit (jsdom) | `npx vitest run reports/pdf-export.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-07 | 04 | 4 | PAY-04 (PDF parity) | — | PDF column set matches Excel column set (string equality on table headers) | unit | `npx vitest run reports/pdf-parity.test.ts` | ❌ W0 | ⬜ pending |
| 05-04-08 | 04 | 4 | PAY-01 (currency format) | — | All money cells render via `Intl.NumberFormat('en-US', {style:'currency',currency:'USD'})` per D-33 | unit | `npx vitest run reports/currency-format.test.ts` | ❌ W0 | ⬜ pending |
| 05-04-09 | 04 | 4 | PAY-01 (D-30 settings UI) | T-05-08 (Admin-only edit) | Viewer/Supervisor see read-only; Admin sees editable form; PATCH calls `version` for optimistic concurrency | unit (RTL) | `npx vitest run settings/tenant-info.test.tsx` | ❌ W0 | ⬜ pending |
| 05-04-10 | 04 | 4 | PAY-01 (D-25 timeout/spinner) | — | Export button disabled in flight; Spinner visible; Sonner toast on success/error | unit (RTL) | `npx vitest run reports/export-state.test.tsx` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `backend/tests/reports_test.rs` — integration test scaffold for reports module
- [ ] `backend/tests/tenant_info_test.rs` — integration test scaffold
- [ ] `backend/tests/fixtures/reports/` — golden fixture employees + daily_records + leaves for snapshot tests
- [ ] `backend/Cargo.toml` add `[dev-dependencies] calamine = "0.27"` for xlsx round-trip parsing
- [ ] `frontend/src/components/reports/__tests__/` — vitest scaffolds for Reports screen
- [ ] `frontend/src/components/settings/__tests__/` — vitest scaffolds for tenant-info screen
- [ ] `frontend/src/test-utils/msw-handlers.ts` — extend with `/api/v1/reports/json`, `/api/v1/tenant-info` mock handlers
- [ ] Property test framework (`proptest = "1.4"` in backend dev-deps) — for money math invariants and period boundary properties

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Excel file opens in LibreOffice / Microsoft Excel without warnings | PAY-03 | Real xlsx interpreter compatibility cannot be asserted in CI without installing Office tooling | Generate report, open file in both LibreOffice and Excel, verify branding header, subtotal rows, anomaly tints render. Sample fixture: 50 employees, 1 month period, includes 2 anomaly rows + 1 night-shift dept. |
| PDF prints correctly on A4 paper | PAY-04 | Print-driver behavior is browser/printer specific | Open generated PDF, send to printer, verify landscape orientation, header repeats per page, page numbers visible. |
| jspdf Spanish accents render in Acrobat / Preview | PAY-04 | jsdom doesn't fully render font glyphs | Generate PDF with employee names containing `ñ á é í ó ú`, open in Adobe Acrobat AND macOS Preview, visually confirm no `?` or boxes. |
| Performance under realistic load | PAY-03 | CI fixture is too small for prod-scale signal | Run report on production-scale fixture (1000 employees, 1 month) on dev machine; record p95 latency. Should be < 5s per D-22. |
| Audit log entry visible in `audit_log` table after export | PAY-01 (D-21) | Cross-check spans HTTP + DB | Trigger export from frontend, then `sqlite3 backend/data/cronometrix.db "SELECT * FROM audit_log WHERE action='REPORT_EXPORT' ORDER BY created_at DESC LIMIT 1;"` — verify row exists with correct payload_json. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags (`cargo nextest` and `vitest run` are non-watch)
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter (after Wave 0 completes)

**Approval:** pending
