---
phase: 09
plan: 01
subsystem: e2e-tooling
tags: [playwright, e2e, tooling, wave-0, timezone]
dependency_graph:
  requires: []
  provides:
    - frontend/playwright.config.ts
    - frontend/e2e/ (directory)
    - Makefile e2e targets
  affects:
    - frontend/package.json (devDeps + scripts)
    - .gitignore (e2e artifact exclusions)
    - Makefile (e2e-install, e2e-build, e2e targets)
tech_stack:
  added:
    - "@playwright/test 1.59.1 (exact pin, devDep)"
    - "xlsx 0.18.5 (exact pin, devDep — download verification)"
    - "pdf-parse 2.4.5 (exact pin, devDep — download verification)"
  patterns:
    - "Playwright webServer × 3 (backend binary + mock_hikvision + next dev/start)"
    - "D-20 TZ freeze in 3 places: backend env / browser context timezoneId / Next.js env"
    - "D-12 determinism: fullyParallel=false, workers=1"
    - "CLAUDE.md filesystem-root injection via webServer.env (5 path vars)"
key_files:
  created:
    - frontend/playwright.config.ts
    - frontend/e2e/.gitkeep
  modified:
    - frontend/package.json
    - frontend/package-lock.json
    - .gitignore
    - Makefile
decisions:
  - "Exact version pins (no caret) on @playwright/test, xlsx, pdf-parse per T-09-06 threat mitigation"
  - "fullyParallel=false + workers=1 for D-12 shared-DB determinism"
  - "webServer uses pre-built binary path (debug for local, release for CI) — avoids cargo compilation on test startup"
  - "passWithNoTests not available in Playwright 1.59.1 — documented as known deviation"
  - "D-20 TZ freeze in backend env (TZ=America/Caracas), browser context (timezoneId), and Next.js webServer env"
  - "D-16 chromium-only: single chromium project + setup dependency project; no firefox/webkit"
  - "D-17 no visual snapshots: trace/screenshot/video all retain-on-failure only"
  - "D-13 license bypass: CRONOMETRIX_E2E + CRONOMETRIX_LICENSE_BYPASS both set in backend webServer env"
metrics:
  duration: "4 minutes"
  completed: "2026-04-29T01:36:29Z"
  tasks: 2
  files: 6
---

# Phase 09 Plan 01: Playwright Tooling Scaffold Summary

Install @playwright/test 1.59.1, scaffold `frontend/e2e/`, produce `playwright.config.ts` that boots backend (4001) + mock_hikvision (4400) + Next.js (3001) via webServer × 3, with TZ frozen to America/Caracas in all three places.

## Tasks Completed

| # | Name | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Install Playwright + xlsx + pdf-parse; scaffold e2e dir | 2d7c20e | frontend/package.json, frontend/package-lock.json, frontend/e2e/.gitkeep, .gitignore |
| 2 | Author playwright.config.ts + Makefile e2e targets | 1893777 | frontend/playwright.config.ts, Makefile |

## Pinned Versions

| Package | Version | Reason |
|---------|---------|--------|
| @playwright/test | 1.59.1 | Latest stable as of 2026-04-28; exact pin per T-09-06 (no caret) |
| xlsx | 0.18.5 | SheetJS community edition; used in reports spec download verification |
| pdf-parse | 2.4.5 | PDF export content verification in reports spec |

## Final Port Assignments

| Server | Port | Probe URL |
|--------|------|-----------|
| Rust/Axum backend (cronometrix) | 4001 | `http://127.0.0.1:4001/api/v1/health` |
| Next.js frontend | 3001 | `http://localhost:3001/login` |
| mock_hikvision public | 4400 | `http://127.0.0.1:4400/ISAPI/System/status` |
| mock_hikvision admin | 4401 | (injection endpoint — not a health probe) |

## TZ Freeze Locations (D-20)

All three locations per RESEARCH §Critical clarification on D-20:

1. **Backend process env** — `TZ: 'America/Caracas'` in the first webServer entry's `env` block
2. **Browser context** — `timezoneId: 'America/Caracas'` in `use` block (affects `new Date()` in browser JS)
3. **Next.js process env** — `TZ: 'America/Caracas'` in the third webServer entry's `env` block

The mock_hikvision webServer also receives `TZ: 'America/Caracas'` for consistency.

## Why `make e2e-build` Will Not Yet Succeed (Plan 03 Dependency)

The `e2e-build` Makefile target references two binaries that do not yet exist:
- `mock_hikvision` — requires Cargo feature `mock-hikvision` (shipped in Plan 03)
- `seed_e2e` — requires Cargo feature `seed-e2e` and a corresponding binary crate (shipped in Plan 03)

The `cronometrix` binary itself builds fine. `make e2e-build` will fail until Plan 03 ships these two additional binary targets. This is expected and documented in the plan.

## Notes for Executors Picking Up Plan 02

1. **`playwright.config.ts` is complete** — Plan 02 (auth fixtures + globalSetup) reads this config verbatim. Port assignments are locked: backend 4001, frontend 3001, mock 4400, admin 4401.

2. **`frontend/e2e/` directory exists** — Plan 02 adds subdirectories: `setup/`, `fixtures/`, and per-route spec files.

3. **`CRONOMETRIX_E2E=true`** is injected by the backend webServer env. Backend startup honors `CRONOMETRIX_LICENSE_BYPASS=true` only when `CRONOMETRIX_E2E=true` is also set (D-13). Plan 02 does not need to modify this behavior — it is already wired.

4. **storageState files** will land at `frontend/e2e/.auth/{admin,supervisor,viewer}.json` (gitignored). Plan 02's globalSetup generates them.

5. **TypeScript check** — `npx tsc --noEmit --skipLibCheck playwright.config.ts` exits 0. The project-wide `npx tsc --project tsconfig.json --noEmit` surfaces 1 pre-existing error in `command-modal.test.tsx` (unrelated to this plan's scope).

6. **`npx playwright test --list` exits 1 with 0 tests** — Playwright 1.59.1 does not support `passWithNoTests` and exits 1 when no test files are found. This is a known version behavior. Once Plan 02 adds the first spec file, `--list` will exit 0 normally. The config is syntactically valid (confirmed by `tsc --skipLibCheck`).

## Deviations from Plan

### Known Behavioral Difference (Playwright 1.59.1)

**1. [Rule 1 - Bug] `npx playwright test --list` exits 1 with 0 tests, not 0**
- **Found during:** Task 2 verification
- **Issue:** Plan acceptance criteria states "`npx playwright test --list` exits 0 (config valid; 0 specs is OK)". Playwright 1.59.1 exits 1 when no test files match `testDir`. There is no `passWithNoTests` flag in this version.
- **Fix:** Config is syntactically valid (confirmed by `tsc --skipLibCheck`). The exit-1 behavior resolves automatically when Plan 02 adds the first `.spec.ts` file. No config change needed; documented as expected until Plan 02.
- **Files modified:** None (documented only)
- **Commit:** N/A

## Self-Check: PASSED

- `frontend/playwright.config.ts` — FOUND
- `frontend/e2e/.gitkeep` — FOUND
- Commits 2d7c20e and 1893777 — verified in `git log`
- `@playwright/test: "1.59.1"` (exact) in devDependencies — VERIFIED
- `xlsx: "0.18.5"` (exact) in devDependencies — VERIFIED
- `pdf-parse: "2.4.5"` (exact) in devDependencies — VERIFIED
- `e2e` + `e2e:install` scripts in package.json — VERIFIED
- `.gitignore` Phase 9 block (3 lines) — VERIFIED
- Makefile `.PHONY: e2e e2e-install e2e-build` — VERIFIED
- `coverage-backend:` target unchanged — VERIFIED (1 occurrence)
- webServer × 3 (3 `command:` entries) — VERIFIED
- TZ freeze in all 3 locations — VERIFIED
