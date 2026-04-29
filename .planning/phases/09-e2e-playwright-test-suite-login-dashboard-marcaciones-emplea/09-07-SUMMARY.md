---
phase: 09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea
plan: 07
subsystem: testing
tags: [playwright, e2e, auth, login, rbac, session, open-redirect]

# Dependency graph
requires:
  - phase: 09-06
    provides: shared fixtures (api.ts, selectors.ts, time.ts), setup project (00-build-and-seed, 01-authenticate), globalTeardown
  - phase: 01-foundation
    provides: login/page.tsx with English copy, loginSchema (min(1) both fields), safeRedirect CR-02 mitigation
  - phase: 09-01
    provides: playwright.config.ts, baseURL, project configuration

provides:
  - "frontend/e2e/login.spec.ts: 12 tests at D-01 Full UAT depth"
  - "English-copy assertions locked: 'Username', 'Password', 'Log in', 'Log in to Cronometrix', 'Invalid username or password.'"
  - "Coverage for: happy login, 401 error, Zod validation, password toggle, session persistence, multi-tab, RBAC, redirect param, open-redirect sanitization, user enumeration prevention"

affects:
  - "i18n work: any login-page Spanish translation must update assertions in this file"
  - "09-08 onwards: login contract locked; all other specs reuse storageState not UI login"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "fillAndSubmit() helper: reusable login form fill-and-submit function shared within login spec"
    - "Fresh context pattern: login.spec.ts is the only spec with no test.use({ storageState })"
    - "Playwright getByLabel / getByRole first: accessible-role-first selectors per RESEARCH"
    - "aria-label toggle detection: page.getByRole('button', { name: 'Show password' }) matches aria-label attribute on Eye icon button"

key-files:
  created:
    - "frontend/e2e/login.spec.ts"
  modified: []

key-decisions:
  - "loginSchema min(1) not min(8): password validation only requires non-empty (not 8 chars); 'short password' plan template test adapted to empty-field tests instead"
  - "T-12 user-enumeration test: non-existent username returns same 401 'Invalid username or password.' as wrong password — no user-enumeration leak"
  - "aria-label Show/Hide password: login/page.tsx assigns explicit aria-label to eye toggle; spec uses getByRole('button', {name: 'Show password'}) for accessibility-first selection"
  - "Viewer RBAC via /devices: viewer can GET /devices (read-only); assertion is that Spanish admin command buttons (Abrir puerta / Reiniciar / Modo enroll) have count=0"

patterns-established:
  - "Pattern: Only one spec file (login.spec.ts) exercises UI-driven login; all other specs use storageState (D-06 hybrid auth)"
  - "Pattern: No page.waitForTimeout() — all waits via Playwright auto-wait (toHaveURL, toBeVisible, toContainText)"

requirements-completed: [E2E-LOGIN, E2E-RBAC, E2E-SELECTORS]

# Metrics
duration: 2min
completed: 2026-04-29
---

# Phase 09 Plan 07: Login E2E Spec Summary

**12 Playwright tests at D-01 Full UAT depth covering login form rendering, happy path, 401 error with English copy, Zod validation, password visibility toggle, session persistence, multi-tab, Viewer RBAC, redirect param, and open-redirect sanitization (CR-02 / T-09-09)**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-29T03:51:17Z
- **Completed:** 2026-04-29T03:53:21Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Created `frontend/e2e/login.spec.ts` with 12 tests (≥ 8 required by D-01), 207 lines (≥ 200 required by artifact spec)
- Locked English-copy assertions per Addendum D-19: "Username", "Password", "Log in", "Log in to Cronometrix", "Invalid username or password." — any future i18n PR will know exactly which strings to translate
- Covered all security requirements: T-09-03 (Viewer cannot reach admin commands), T-09-09 (open-redirect sanitized via safeRedirect), T-01-19 (generic error message — no user enumeration)
- Spec correctly uses fresh browser contexts throughout (no `test.use({ storageState })`), making it the single source of UI-driven login truth per D-06
- Playwright `--list` confirms all 12 tests parse and enumerate correctly

## Task Commits

1. **Task 1: Author login.spec.ts with 12 tests at UAT depth** - `cdd1c79` (feat)

**Plan metadata:** (docs commit — created with SUMMARY.md, STATE.md, ROADMAP.md)

## Files Created/Modified

- `frontend/e2e/login.spec.ts` — 12 E2E tests at D-01 Full UAT depth; the only spec using UI-driven login (D-06)

## Decisions Made

1. **loginSchema password is min(1), not min(8):** The `loginSchema` in `src/lib/validations.ts` only requires password ≥ 1 character (the plan template assumed ≥ 8). Adapted: replaced the "short password rejected by zod schema" test with two tests covering empty username and empty password validation separately — both are enforced by Zod and produce field errors without any API call.

2. **Added T-12 (user enumeration prevention):** Plan outlined 10 tests; added a 12th test confirming a non-existent username also returns the generic "Invalid username or password." message (T-01-19). This directly tests the security requirement and uses no additional setup.

3. **Eye toggle via aria-label:** `login/page.tsx` assigns `aria-label="Show password"` / `"Hide password"` to the eye icon button. Used `page.getByRole('button', { name: 'Show password' })` — more accessible and more stable than CSS-based locators from the plan template.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Adaptation] loginSchema password min(1) ≠ plan template's assumed min(8)**
- **Found during:** Task 1 (read_first — src/lib/validations.ts)
- **Issue:** Plan template test `'validation: short password rejected by zod schema'` filled password `'a'` and assumed zod would block it. loginSchema uses `z.string().min(1)` — 'a' satisfies this, so the test would make an API call, get 401, and still pass, but for the wrong reason.
- **Fix:** Replaced with two targeted tests: T-04 (empty username) and T-05 (empty password), each asserting the user stays on /login and a field error is visible. Both use Zod's `min(1, 'This field is required.')` rule correctly.
- **Files modified:** frontend/e2e/login.spec.ts
- **Verification:** `npx playwright test --list` confirms both tests enumerate; no API call is needed for empty-field rejection
- **Committed in:** cdd1c79 (Task 1 commit)

---

**Total deviations:** 1 auto-adapted (Rule 1 — plan template assumption about schema min length)
**Impact on plan:** Adaptation necessary for correct test semantics. Final test count (12) exceeds required minimum (8). All original test categories are still covered.

## Issues Encountered

None — plan executed cleanly after adapting the password-validation test to match the actual schema.

## Threat Surface Scan

| Flag | File | Description |
|------|------|-------------|
| No new surface | frontend/e2e/login.spec.ts | Test-only file; does not introduce any new network endpoints, auth paths, or schema changes |

## Known Stubs

None — all tests are fully implemented. No placeholder assertions.

## Copy Strings Asserted (for future i18n PRs)

The following English strings are asserted in this spec. Any Spanish localization of the login page must update these assertions:

| String | Assertion location |
|--------|--------------------|
| `Log in to Cronometrix` | `getByRole('heading', { name: '...' })` T-01 |
| `Username` | `getByLabel('Username')` T-01, T-02, T-03, T-04, T-07, T-08, T-09, T-10, T-11, T-12 |
| `Password` | `getByLabel('Password')` T-01 |
| `Log in` | `getByRole('button', { name: 'Log in' })` all tests |
| `Invalid username or password.` | `getByRole('alert').toContainText(...)` T-03, T-12 |
| `Show password` | `getByRole('button', { name: 'Show password' })` T-06 |
| `Hide password` | `getByRole('button', { name: 'Hide password' })` T-06 |

## Flake Observed

None — spec was not run against live servers (servers not running in dev environment). Static analysis via `playwright --list` confirms all 12 tests parse correctly. Actual runtime results to be confirmed in CI.

## Next Phase Readiness

- `login.spec.ts` is complete and committed at `cdd1c79`
- Plan 09-08 (dashboard.spec.ts) can proceed — it will use storageState from Plan 06's authenticate setup, not UI login
- No blockers

---
*Phase: 09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea*
*Completed: 2026-04-29*
