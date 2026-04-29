---
phase: 9
slug: e2e-playwright-test-suite-login-dashboard-marcaciones-emplea
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-28
updated: 2026-04-28
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `09-RESEARCH.md` § Validation Architecture (lines 862–931).
> Updated 2026-04-28 to reflect the FINAL 13-plan layout.

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
| **Test-reset gating** | `cargo nextest run --test test_reset_gating` |
| **Estimated runtime** | <30s |

---

## Sampling Rate

- **After every task commit:** Run touched spec only — `npx playwright test <file>` (~10–30s for one spec).
- **After every plan wave:** Run full E2E suite — `make e2e`.
- **Before `/gsd-verify-work`:** Full suite must be green AND CI `E2E Tests` job must be green on the branch.
- **Max feedback latency:** 30s per-task / 15min per-wave.

---

## Per-Plan Verification Map (final 13-plan layout)

| Plan ID | Type | Wave | Focus | Requirements (E2E-*) | Threat Refs | Automated Command | File Status |
|---------|------|------|-------|----------------------|-------------|-------------------|-------------|
| **09-01** | execute | 0 | Tooling — Playwright config + Makefile + .gitignore + package.json | E2E-TOOLING, E2E-CHROMIUM-ONLY, E2E-DOCS, E2E-TZ-FREEZE | T-09-04, T-09-06, T-09-07, T-08-15 | `npx tsc --noEmit frontend/playwright.config.ts && npx playwright test --list` | ⬜ pending |
| **09-02** | tdd | 0 | License-bypass safety: `evaluate_bypass` + main.rs wiring + integration test (locks T-09-01) | E2E-BACKEND, E2E-LICENSE-BYPASS | T-09-01 | `cargo nextest run --test license_bypass_safety` | ⬜ pending |
| **09-03** | execute | 0 | Backend infra: Cargo features + seed_e2e + mock_hikvision (incl. /admin/recv-log per B6) + __test_reset gated route | E2E-BACKEND, E2E-MOCK, E2E-FIXTURES, E2E-TABLE-RESET | T-09-02, T-09-04, T-09-05 | `cargo nextest run --test test_reset_gating && cargo build --bin mock_hikvision --features mock-hikvision` | ⬜ pending |
| **09-04** | tdd | 0 | GET /api/v1/audit endpoint (paginated, RBAC-enforced) | E2E-AUDIT-API | T-09-03 | `cargo nextest run --test audit_api_test` | ⬜ pending |
| **09-05** | execute | 0 | Audit UI: TanStack Table replaces placeholder; data-testids land | E2E-AUDIT-UI, E2E-SELECTORS | T-09-03 | `npx vitest run src/components/audit/__tests__/ && npm run build` | ⬜ pending |
| **09-06** | execute | 1 | Setup project (00-build-and-seed + 01-authenticate) + shared fixtures + globalTeardown | E2E-FIXTURES, E2E-TZ-FREEZE, E2E-TABLE-RESET | T-09-04, T-09-06 | `npx playwright test --project=setup` | ⬜ pending |
| **09-07** | execute | 2 | login.spec.ts (≥ 8 tests, English copy per D-19 Addendum) | E2E-LOGIN | — | `npx playwright test login.spec.ts` | ⬜ pending |
| **09-08** | execute | 2 | dashboard.spec.ts + data-testids (≥ 6 tests) | E2E-DASHBOARD, E2E-SELECTORS | — | `npx playwright test dashboard.spec.ts` | ⬜ pending |
| **09-09** | execute | 2 | timesheet.spec.ts + employees.spec.ts (≥ 14 tests; mutation→audit) | E2E-CRUD-TS, E2E-CRUD-EMP, E2E-SELECTORS | T-09-03 | `npx playwright test timesheet.spec.ts employees.spec.ts` | ⬜ pending |
| **09-10** | execute | 2 | devices.spec.ts + reports.spec.ts (≥ 14 tests; ISAPI dispatch via mock; XLSX/PDF content; non-optional door-open audit per B6) | E2E-CRUD-DEV, E2E-CRUD-REP, E2E-SELECTORS | T-09-03 | `npx playwright test devices.spec.ts reports.spec.ts` | ⬜ pending |
| **09-11** | execute | 2 | audit.spec.ts (no waitForTimeout per B4) + rbac.spec.ts (reconciled vs main.rs per W4); ≥ 10 tests | E2E-AUDIT-SCREEN, E2E-RBAC, E2E-SELECTORS | T-09-03 | `npx playwright test audit.spec.ts rbac.spec.ts` | ⬜ pending |
| **09-12** | execute | 3 | CI E2E Tests job (pinned-action whitelist; B5; W5 step-level TZ) | E2E-CI, E2E-CHROMIUM-ONLY, E2E-TZ-FREEZE | T-08-15, T-09-06, T-09-07 | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` + manual GitHub Actions run | ⬜ pending |
| **09-13** | execute | 3 | CLAUDE.md "Phase 9 E2E" subsection + repository docs | E2E-DOCS | T-08-15 | `grep -q "Phase 9 E2E" CLAUDE.md` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Threat → Plan reconciliation

| Threat ID | Plans that mitigate / accept |
|-----------|------------------------------|
| T-09-01 (license bypass) | **09-02** (TDD-locked safety test) |
| T-09-02 (test_reset destroys audit) | **09-03** (gated registration + handler env-recheck + integration test) |
| T-09-03 (mutation→audit + RBAC) | **09-04** (audit endpoint), **09-05** (audit UI), **09-09** (timesheet + employees CRUD audit assertions), **09-10** (devices + reports audit assertions), **09-11** (audit screen + RBAC cross-cut) |
| T-09-04 (mock binary on prod) | **09-01** (config locks 127.0.0.1 ports), **09-03** (Cargo feature gate) |
| T-09-05 (seed_e2e password leak) | **09-03** (feature + runtime gate) |
| T-09-06 (dep version drift) | **09-01** (exact-pin devDeps), **09-12** (lockfile committed in CI cache) |
| T-09-07 (PII in test artifacts) | **09-12** (14-day retention; private repo), **09-13** (CLAUDE.md docs) |
| T-08-15 (pinned-action policy) | **09-12** (pinned-action whitelist enforcement; B5), **09-13** (docs) |

---

## Eight Validation Dimensions (Nyquist coverage map)

Per RESEARCH § Validation Architecture, every E2E phase must hit eight dimensions:

| # | Dimension | What's Validated | Where Covered |
|---|-----------|------------------|---------------|
| 1 | **Smoke** | App boots, login screen renders | 09-07 first test, infra in 09-06 setup project |
| 2 | **Contract** | Request/response shapes match between frontend and backend | Implicit in every spec — schema drift surfaces as failure |
| 3 | **Journey** | End-to-end user workflows (login → CRUD → logout) | All Wave 1 + Wave 2 specs (09-07..09-11) |
| 4 | **RBAC** | Each role sees only what it should; backend enforces | 09-11 (rbac.spec.ts), role-scoped specs in 09-09/09-10 |
| 5 | **Mutation→Audit** | Every mutating action produces audit_log entry (CLAUDE.md non-negotiable) | 09-09 + 09-10 + 09-11 (each D-03 CRUD test asserts audit row) |
| 6 | **Error states** | Validation errors, 4xx, 5xx surfaced correctly | 09-07 login.spec.ts, 09-09/09-10 CRUD validation tests |
| 7 | **Time-calc determinism** | Tolerance, lunch, overtime calcs reproduce in tests | 09-09 timesheet.spec.ts with seeded events of known timestamps; D-20 TZ freeze in 09-01 |
| 8 | **Real-time/SSE** | Live activity feed, ring buffer, disconnect banner | 09-08 dashboard.spec.ts |

---

## Wave 0 Requirements

Wave 0 must land before Wave 1 specs can run. The 5 plans in Wave 0 (09-01 through 09-05) collectively deliver:

- [ ] `frontend/playwright.config.ts` — webServer × 3 (backend, mock, next), projects (setup + chromium), env injection, TZ freeze (09-01)
- [ ] `frontend/package.json` — add `@playwright/test`, `xlsx`, `pdf-parse` devDeps + `e2e`, `e2e:install` scripts (09-01)
- [ ] `.gitignore` — add `frontend/e2e/.auth/`, `frontend/playwright-report/`, `frontend/test-results/` (09-01)
- [ ] `Makefile` — `make e2e`, `make e2e-install`, `make e2e-build` targets (09-01)
- [ ] `backend/src/license/service.rs` (or `main.rs`) — bypass-flag check gated by `CRONOMETRIX_E2E=true` (09-02)
- [ ] `backend/tests/license_bypass_safety.rs` — locks D-13 / T-09-01 (asserts non-zero exit when bypass set without e2e) (09-02)
- [ ] `backend/Cargo.toml` — `[[bin]]` entries with `required-features` per binary; `seed-e2e` + `mock-hikvision` features (09-03)
- [ ] `backend/src/bin/seed_e2e.rs` — seed binary (gated by `seed-e2e` feature + runtime CRONOMETRIX_E2E flag) (09-03)
- [ ] `backend/src/bin/mock_hikvision.rs` — mock outbound device + `/admin/recv-log` endpoint per B6 (09-03)
- [ ] `backend/src/main.rs` — `__test_reset` route registered behind `CRONOMETRIX_E2E=true` for D-12 table reset (09-03)
- [ ] `backend/tests/test_reset_gating.rs` — locks T-09-02 (404 without flag, 200 with flag) (09-03)
- [ ] `backend/src/audit/mod.rs` + `GET /api/v1/audit` (paginated, RBAC: Admin + Supervisor read; Viewer 403) — Addendum resolution for D-04 (09-04)
- [ ] `frontend/src/app/(dashboard)/audit/page.tsx` — replace placeholder with TanStack Table audit list (filter user/date) — Addendum resolution for D-04 (09-05)
- [ ] `frontend/src/components/audit/{audit-table,audit-filters,diff-cell}.tsx` + Vitest tests (09-05)

After Wave 0 completes, set `wave_0_complete: true` in this file's frontmatter.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Branch protection: `E2E Tests` required status check on `main` | E2E-CI | GitHub UI step — cannot be set via PR | After live CI run is green: Settings → Branches → branch protection rule → Require status checks → add `E2E Tests`. Mirrors Phase 8 Plan 05 deferred follow-up. (09-12) |
| Real Hikvision device QA | (out of scope; deferred per CONTEXT) | Hardware required | Manual smoke against a real device pre-release |

---

## Validation Sign-Off

- [x] Every plan has at least one `<verify><automated>...</automated></verify>` block (Nyquist compliance — see per-plan verification map)
- [x] Sampling continuity: no 3 consecutive plans without automated verify (every plan has its own command)
- [x] Wave 0 covers all MISSING references (audit endpoint + UI; mock device incl. recv-log; license-bypass test; seed binary; test_reset gating)
- [x] No watch-mode flags (full runs only in CI; quick runs via `--grep` locally)
- [x] Feedback latency < 30s per-task, < 15min per-wave
- [x] `nyquist_compliant: true` set in frontmatter (this update)
- [ ] `wave_0_complete: true` will be set after plans 09-01..09-05 are all green

**Approval:** pending live execution
</content>
</invoke>