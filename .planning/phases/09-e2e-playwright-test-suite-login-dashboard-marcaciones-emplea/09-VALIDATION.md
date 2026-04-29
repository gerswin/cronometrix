---
phase: 9
slug: e2e-playwright-test-suite-login-dashboard-marcaciones-emplea
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-28
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `09-RESEARCH.md` § Validation Architecture (lines 862–931).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `@playwright/test` 1.59.1 |
| **Config file** | `frontend/playwright.config.ts` (NEW — Wave 0 installs) |
| **Quick run command** | `cd frontend && npx playwright test --project=chromium --grep <pattern>` |
| **Full suite command** | `cd frontend && npx playwright test` (or `make e2e` from repo root) |
| **Estimated runtime** | ~5–15 min (50+ specs, workers=1 for determinism per D-12) |

Backend-side validation also uses:

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (existing) |
| **License-bypass test** | `cargo nextest run --test license_bypass_safety` |
| **Estimated runtime** | <30s |

---

## Sampling Rate

- **After every task commit:** Run touched spec only — `npx playwright test <file>` (~10–30s for one spec).
- **After every plan wave:** Run full E2E suite — `make e2e`.
- **Before `/gsd-verify-work`:** Full suite must be green AND CI `E2E Tests` job must be green on the branch.
- **Max feedback latency:** 30s per-task / 15min per-wave.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 09-01-* | 01 (Tooling + Audit Backend) | 0 | E2E-TOOLING / E2E-AUDIT-API | T-09-01 (license-bypass leak) | Bypass flag aborts startup w/o e2e flag | unit (Rust) + smoke (Playwright) | `cargo nextest run --test license_bypass_safety && npx playwright test --project=setup` | ❌ W0 | ⬜ pending |
| 09-02-* | 02 (Audit UI + Fixtures) | 0 | E2E-AUDIT-UI / E2E-FIXTURES | — | Three storageState files generated; argon2id matches prod hash params | setup project | `npx playwright test --project=setup` | ❌ W0 | ⬜ pending |
| 09-03-* | 03 (Backend Boot + Mock Device) | 0 | E2E-BACKEND / E2E-MOCK | T-09-02 (bypass flag must reject prod startup) | Mock alertStream serves canned XML; backend connects | infrastructure | covered by Wave 1 specs first run | ❌ W0 | ⬜ pending |
| 09-04-* | 04 (Login spec) | 1 | E2E-LOGIN | — | RBAC redirect; session expiry; multi-tab | e2e | `npx playwright test login.spec.ts` | ❌ W1 | ⬜ pending |
| 09-05-* | 05 (Dashboard spec) | 1 | E2E-DASHBOARD | — | KPI calc, donut, ring buffer, photo fallback, SSE banner | e2e | `npx playwright test dashboard.spec.ts` | ❌ W1 | ⬜ pending |
| 09-06-* | 06 (Timesheet/Marcaciones) | 2 | E2E-CRUD-TS | T-09-03 (mutation→audit) | Each mutation produces audit_log entry | e2e + audit assertion | `npx playwright test timesheet.spec.ts` | ❌ W2 | ⬜ pending |
| 09-07-* | 07 (Employees) | 2 | E2E-CRUD-EMP | T-09-03 | mutation→audit | e2e + audit assertion | `npx playwright test employees.spec.ts` | ❌ W2 | ⬜ pending |
| 09-08-* | 08 (Devices) | 2 | E2E-CRUD-DEV | T-09-03 | mutation→audit; ISAPI dispatch | e2e + audit assertion | `npx playwright test devices.spec.ts` | ❌ W2 | ⬜ pending |
| 09-09-* | 09 (Reports) | 2 | E2E-CRUD-REP | — | Excel + PDF content matches | e2e + file parsing | `npx playwright test reports.spec.ts` | ❌ W2 | ⬜ pending |
| 09-10-* | 10 (Audit screen) | 2 | E2E-AUDIT | — | Audit list immutable; filter by user/date | e2e | `npx playwright test audit.spec.ts` | ❌ W2 | ⬜ pending |
| 09-11-* | 11 (RBAC cross-cuts) | 2 | E2E-RBAC | T-09-04 (RBAC enforcement) | Viewer → 403; Supervisor partial; Admin full | e2e | `npx playwright test rbac.spec.ts` | ❌ W2 | ⬜ pending |
| 09-12-* | 12 (CI gate + docs) | 3 | E2E-CI / E2E-DOCS | T-08-15 (least privilege) | Pinned actions; permissions: contents: read; artifacts always uploaded | CI workflow + docs | manual: open PR, see job pass | ❌ W3 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> Plan numbering above is illustrative; final plan IDs determined by gsd-planner.

---

## Eight Validation Dimensions (Nyquist coverage map)

Per RESEARCH § Validation Architecture, every E2E phase must hit eight dimensions:

| # | Dimension | What's Validated | Where Covered |
|---|-----------|------------------|---------------|
| 1 | **Smoke** | App boots, login screen renders | `login.spec.ts` first test, infra in setup project |
| 2 | **Contract** | Request/response shapes match between frontend and backend | Implicit in every spec — schema drift surfaces as failure |
| 3 | **Journey** | End-to-end user workflows (login → CRUD → logout) | All Wave 1 + Wave 2 specs |
| 4 | **RBAC** | Each role sees only what it should; backend enforces | `rbac.spec.ts`, role-scoped specs |
| 5 | **Mutation→Audit** | Every mutating action produces audit_log entry (CLAUDE.md non-negotiable) | Every D-03 CRUD test asserts audit row |
| 6 | **Error states** | Validation errors, 4xx, 5xx surfaced correctly | login.spec.ts, CRUD validation tests, devices error states |
| 7 | **Time-calc determinism** | Tolerance, lunch, overtime calcs reproduce in tests | `timesheet.spec.ts` with seeded events of known timestamps |
| 8 | **Real-time/SSE** | Live activity feed, ring buffer, disconnect banner | `dashboard.spec.ts` |

---

## Wave 0 Requirements

Wave 0 must land before Wave 1 specs can run:

- [ ] `frontend/playwright.config.ts` — webServer × 2, projects (setup + chromium), env injection
- [ ] `frontend/e2e/setup/00-build-and-seed.setup.ts` — DB seed + storageState generation
- [ ] `frontend/e2e/fixtures/{api.ts,selectors.ts,time.ts,hikvision-events/*.xml}` — shared fixtures
- [ ] `.gitignore` — add `frontend/e2e/.auth/`, `frontend/playwright-report/`, `frontend/test-results/`
- [ ] `frontend/package.json` — add `@playwright/test`, `xlsx`, `pdf-parse` devDeps + `e2e`, `e2e:install` scripts
- [ ] `backend/src/bin/seed_e2e.rs` — seed binary (gated by `[features] seed-e2e`)
- [ ] `backend/src/bin/mock_hikvision.rs` — mock outbound device (gated by `[features] mock-hikvision`)
- [ ] `backend/Cargo.toml` — `[[bin]]` entries with `required-features` per binary
- [ ] `backend/src/license/service.rs` (or `main.rs`) — bypass-flag check gated by `CRONOMETRIX_E2E=true`
- [ ] `backend/tests/license_bypass_safety.rs` — locks D-13 (asserts non-zero exit when bypass set without e2e)
- [ ] `backend/src/main.rs` — `__test_reset` route registered behind `CRONOMETRIX_E2E=true` for D-12 table reset
- [ ] `backend/src/audit/mod.rs` + `GET /api/v1/audit` (paginated, RBAC: Admin + Supervisor read; Viewer 403) — Addendum resolution for D-04
- [ ] `frontend/src/app/(dashboard)/audit/page.tsx` — replace placeholder with TanStack Table audit list (filter user/date) — Addendum resolution for D-04
- [ ] `Makefile` — `make e2e`, `make e2e-install` targets

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Branch protection: `E2E Tests` required status check on `main` | E2E-CI | GitHub UI step — cannot be set via PR | After live CI run is green: Settings → Branches → branch protection rule → Require status checks → add `E2E Tests`. Mirrors Phase 8 Plan 05 deferred follow-up. |
| Real Hikvision device QA | (out of scope; deferred per CONTEXT) | Hardware required | Manual smoke against a real device pre-release |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (audit endpoint + UI; mock device; license-bypass test; seed binary)
- [ ] No watch-mode flags (full runs only in CI; quick runs via `--grep` locally)
- [ ] Feedback latency < 30s per-task, < 15min per-wave
- [ ] `nyquist_compliant: true` set in frontmatter (after planner completes)

**Approval:** pending
