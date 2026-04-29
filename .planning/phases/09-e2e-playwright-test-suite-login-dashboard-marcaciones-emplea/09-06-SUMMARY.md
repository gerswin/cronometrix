---
phase: 09
plan: 06
subsystem: e2e-infrastructure
tags: [playwright, e2e, fixtures, setup-project, teardown, authentication]
dependency_graph:
  requires: [09-01, 09-03, 09-05]
  provides: [e2e-fixtures, e2e-setup-project, e2e-storagestate, e2e-teardown]
  affects: [09-07, 09-08, 09-09, 09-10, 09-11, 09-12, 09-13]
tech_stack:
  added: []
  patterns: [playwright-setup-project, storagestate-auth, global-teardown, typed-fixture-catalog]
key_files:
  created:
    - frontend/e2e/fixtures/api.ts
    - frontend/e2e/fixtures/selectors.ts
    - frontend/e2e/fixtures/time.ts
    - frontend/e2e/fixtures/hikvision-events/ana-entrada.xml
    - frontend/e2e/fixtures/hikvision-events/ana-salida.xml
    - frontend/e2e/fixtures/hikvision-events/luis-entrada.xml
    - frontend/e2e/setup/00-build-and-seed.setup.ts
    - frontend/e2e/setup/01-authenticate.setup.ts
    - frontend/e2e/global-teardown.ts
  modified:
    - frontend/playwright.config.ts
decisions:
  - "storageState path uses path.resolve(__dirname) to be location-independent — avoids cwd-relative path bugs"
  - "00-build-and-seed uses fs.existsSync pre-built binary check to fail-fast on cargo build errors (not swallow them in try/catch)"
  - "globalTeardown also removes .db-wal and .db-shm sidecar files that libSQL WAL mode creates"
  - "01-authenticate creates .auth dir via fs.mkdirSync if missing — setup project is self-contained"
  - "ROLES array typed as const in 01-authenticate — TypeScript narrows file literal union for type safety"
metrics:
  duration_minutes: 3
  tasks_completed: 4
  files_created: 9
  files_modified: 1
  completed_date: "2026-04-29"
---

# Phase 09 Plan 06: E2E Setup Project + Shared Fixtures Summary

**One-liner:** Playwright setup project with typed API/selector/time fixtures, 3-role storageState generation, and end-of-run /tmp cleanup.

## What Was Built

### Shared fixtures (`frontend/e2e/fixtures/`)

**`api.ts`** — Typed `APIRequestContext` wrapper. Exports:
- `API_BASE = 'http://127.0.0.1:4001/api/v1'` (single source of truth for backend URL)
- `getAudit(req, params?)` — typed GET /audit with all filter params
- `resetMutableTables(req)` — POST /__test_reset with loud failure if E2E gate is off
- `pushHikvisionEvent(req, xml)` — POST to mock admin port 4401

**`selectors.ts`** — Centralized `SEL` const (data-testid catalog). Current entries:

| Category | Selectors |
|----------|-----------|
| Layout | `topBarTitle` |
| Login | `loginUsername`, `loginPassword`, `loginSubmit` (role-based, not testid) |
| Dashboard KPIs | `kpiPresentes`, `kpiRetraso`, `kpiDispositivos`, `kpiAlertas`, `donutDept`, `ringBuffer`, `sseBanner` |
| Audit | `auditPage`, `auditRow(id)`, `auditFilterActor`, `auditFilterFrom`, `auditFilterTo`, `auditFilterTable` |
| Employees | `employeesPage`, `employeeRow(id)`, `employeeSearch`, `newEmployeeBtn` |
| Devices | `devicesPage`, `deviceRow(id)`, `deviceStatus(id)` |
| Timesheet | `timesheetPage`, `timesheetRow(id)`, `editTimesheetBtn`, `timesheetPeriodPicker` |
| Reports | `reportsPage`, `exportExcelBtn`, `exportPdfBtn` |
| Navigation | `navDashboard`, `navEmployees`, `navTimesheet`, `navDevices`, `navReports`, `navAudit` |
| RBAC | `accessRestricted` |

Plans 07-12 MUST add entries to this file — never hardcode strings in spec files.

**`time.ts`** — Caracas-anchored epoch helpers:
- `SEED_DATE_ISO = '2026-04-15'` — Wednesday; deterministic anchor for all fixtures
- `CARACAS_TZ = 'America/Caracas'` — IANA TZ string (UTC-4, no DST since 2016)
- `caracasEpoch(isoDate, hhmm) → number` — converts Caracas local time to UTC epoch seconds; handles day-overflow for late-night events
- `epochToCaracasHHMM(epochSeconds) → string` — reverse helper for UI display assertions

### Hikvision XML fixtures (`frontend/e2e/fixtures/hikvision-events/`)

All use `dateTime` with `-04:00` (Caracas) offset as required by the mock_hikvision binary.

| File | employeeNoString | name | dateTime |
|------|------------------|------|----------|
| `ana-entrada.xml` | EMP001 | Ana Pérez | 2026-04-15T08:05:00-04:00 |
| `ana-salida.xml` | EMP001 | Ana Pérez | 2026-04-15T17:05:00-04:00 |
| `luis-entrada.xml` | EMP002 | Luis García | 2026-04-15T08:30:00-04:00 |

### Setup project (`frontend/e2e/setup/`)

**`00-build-and-seed.setup.ts`** — Three sequential setup steps:
1. `verify backend health` — GET /health, asserts 200
2. `seed e2e database (idempotent)` — prefers pre-built binary at `target/{debug,release}/seed_e2e`; falls back to `cargo run` with normalized whitespace; passes full env (CRONOMETRIX_E2E, JWT_SECRET, DEVICE_CREDS_KEY, filesystem roots)
3. `reset mutable tables (clean slate per run)` — calls `resetMutableTables(request)` from fixtures/api

**`01-authenticate.setup.ts`** — Three setup steps (one per role):
- Calls `POST /api/v1/auth/login` with seeded credentials
- Asserts `access_token` in response body
- Writes `request.storageState` to `e2e/.auth/{admin,supervisor,viewer}.json`

Seeded credentials used:
- `e2e_admin` / `e2e-admin-pass`
- `e2e_supervisor` / `e2e-supervisor-pass`
- `e2e_viewer` / `e2e-viewer-pass`

### Setup project step ordering

Alphabetical naming is intentional and load-bearing:
- `00-build-and-seed.setup.ts` always runs before `01-authenticate.setup.ts`
- DO NOT rename without updating all dependent plan references

### `frontend/e2e/global-teardown.ts`

Removes after every full test run:
- `/tmp/cronometrix-e2e-{RUN_ID}/` (paths root directory)
- `/tmp/cronometrix-e2e-{RUN_ID}.db` + `-wal` + `-shm` (DB + WAL sidecar files)

`.auth/*.json` files are intentionally preserved between local runs (CI always starts fresh).

### `playwright.config.ts` change

Added `globalTeardown: require.resolve('./e2e/global-teardown')` on the second line of `defineConfig({ ... })`.

## Seeded Employee Codes (for Hikvision fixtures + spec assertions)

| Code | Name | Role in specs |
|------|------|---------------|
| EMP001 | Ana Pérez | Primary attendance subject (full day: entrada 08:05 + salida 17:05) |
| EMP002 | Luis García | Secondary subject (late arrival: entrada 08:30) |
| EMP003–EMP006 | (seeded by Plan 03) | Additional employees for CRUD + RBAC specs |

## Deviations from Plan

### Auto-added improvements (within task scope)

**1. [Rule 2 - Missing critical functionality] globalTeardown also removes WAL/SHM sidecar files**
- **Found during:** Task 4 implementation
- **Issue:** libSQL WAL mode creates `.db-wal` and `.db-shm` alongside the `.db` file. The plan only specified removing `.db` and the paths root directory.
- **Fix:** Added parallel `fs.rm` calls for both sidecar files
- **Files modified:** `frontend/e2e/global-teardown.ts`

**2. [Rule 2 - Missing critical functionality] 01-authenticate auto-creates .auth directory**
- **Found during:** Task 3 implementation
- **Issue:** The `.auth/` directory is gitignored (created by Plan 01's `.gitignore` entry), so it doesn't exist on a fresh clone. Playwright's `request.storageState({ path: ... })` would fail if the parent directory doesn't exist.
- **Fix:** Added `fs.mkdirSync(AUTH_DIR, { recursive: true })` at module top-level.
- **Files modified:** `frontend/e2e/setup/01-authenticate.setup.ts`

**3. [Rule 2 - Type safety] ROLES array uses `as const` for literal narrowing**
- **Found during:** Task 3 implementation
- **Issue:** Without `as const`, TypeScript widens `file: 'admin'` to `file: string`, losing the union type.
- **Fix:** Added `as const` to the ROLES array definition.
- **Files modified:** `frontend/e2e/setup/01-authenticate.setup.ts`

**4. [Rule 2 - Correctness] time.ts handles day-overflow for late-night Caracas events**
- **Found during:** Task 1 implementation
- **Issue:** The plan's inline snippet used `h+4` without handling the overflow case where `h+4 >= 24` (e.g., 22:00 Caracas = 02:00+1 UTC).
- **Fix:** Added explicit day-overflow handling using UTC date arithmetic.
- **Files modified:** `frontend/e2e/fixtures/time.ts`

## Known Stubs

None. All fixtures are fully implemented with no placeholder data.

## Threat Flags

None. All new files are test-scope only (gitignored .auth files; /tmp ephemeral DB; 127.0.0.1-bound mock admin port). These surfaces are already covered by T-09-02, T-09-04, T-09-05, and T-09-07 in the plan's threat model.

## Self-Check: PASSED

All 9 created files exist on disk. All 4 task commits (b5af3f7, bffbfbd, 651743d, 83f95f6) exist in git log.
