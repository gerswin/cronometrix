# Phase 9: E2E Playwright Test Suite - Research

**Researched:** 2026-04-28
**Domain:** End-to-end browser testing of a Next.js 16 admin frontend against a real Rust/Axum backend with seeded SQLite, mock Hikvision device layer, and CI-required-to-merge gate.
**Confidence:** HIGH

## Summary

Phase 9 is a pure-additive testing phase: install `@playwright/test`, scaffold `frontend/e2e/`, build a Playwright config that boots the real `cronometrix` Rust binary on a test port against an ephemeral SQLite database, mock the inbound and outbound Hikvision ISAPI surface so tests can drive marcaciones without hardware, and add a hard-fail `E2E Tests` job to `.github/workflows/ci.yml` alongside the Phase 8 coverage gates.

The technical risk is low — Playwright's `webServer`, `globalSetup`, project-dependencies, and storageState patterns are all stable, well-documented, and battle-tested. The execution risk is medium and concentrated in three places: (1) backend startup race + license-bypass flag leak, (2) mock Hikvision ISAPI surface (digest-auth inbound + tiny mock outbound server), (3) parallel-worker safety against a single ephemeral SQLite. All three have a recommended mitigation below.

**Primary recommendation:** Use the **project-dependencies pattern** (a `setup` project running `*.setup.ts` files that boot the mock device server, seed the DB, generate the three role storageStates, then dispatch tests with `test.use({ storageState })`). Boot the Rust backend via `webServer` with `reuseExistingServer: !process.env.CI`. Build the backend with `cargo build --release --bin cronometrix` once at install time and have `webServer` run the compiled binary, not `cargo run`, to remove compilation from the test critical path. Pin `@playwright/test@1.59.1` (current stable, verified on npm).

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Test Depth & Coverage**
- **D-01** — Login: Full UAT depth. Cover happy login, invalid credentials, password validation rules, session expiry, refresh-token rotation, multi-tab session behavior, RBAC redirect (Viewer cannot reach `/devices`). Approx. 8+ tests. `login.spec.ts` is the only file using UI-driven login; all other specs reuse storageState.
- **D-02** — Dashboard: Full UAT depth. Cover all KPI tile calculations (Empleados Presentes, % Retraso Hoy, Dispositivos Activos, Alertas Diurnas), donut chart by department (Phase 4 D-5), 20-event ring buffer (Phase 4 D-6), photo fallback to initials (Phase 4 D-2), SSE disconnect banner with backoff (Phase 4 D-4), every empty state. Approx. 6+ tests.
- **D-03** — CRUD routes: Full UAT depth. For each of `timesheet`, `employees`, `devices`, `reports`: full CRUD coverage, all filter combinations, all validation errors, and assertions that mutating actions produce the expected immutable audit-log entry. Reports tests must verify Excel + PDF export content, not just download success. Approx. 30+ tests across the four routes.
- **D-04** — Audit screen tests: required. 1–2 tests asserting (a) audit log lists immutable entries and (b) filter by user/date works.
- **D-05** — Total target ≈ 50+ tests. Planner sizes waves and parallelism accordingly.

**Authentication Fixtures**
- **D-06** — Hybrid auth strategy. Inside `login.spec.ts`, run UI-driven login through the real `/login` form. Every other spec reuses a pre-built `storageState` JSON.
- **D-07** — Three role fixtures: Admin, Supervisor, Viewer. Each role has its own storageState file.
- **D-08** — Seed users via `globalSetup`. Idempotent SQL INSERT … ON CONFLICT against the test DB to seed `e2e_admin`, `e2e_supervisor`, `e2e_viewer` users with argon2id password hashes.
- **D-09** — storageState files at `frontend/e2e/.auth/{role}.json`, gitignored. `globalSetup` regenerates all three on every run.

**Backend Orchestration & Test Data**
- **D-10** — Real backend via Playwright `webServer`. `cargo run --release --bin cronometrix` in CI; `cargo run` locally. No network mocks at the API layer.
- **D-11** — Ephemeral SQLite per run at `/tmp/cronometrix-e2e-${RUN_ID}.db`. All migrations + seed users + departments + employees + devices fixtures applied. `globalTeardown` deletes it. Filesystem-root injection convention applies.
- **D-12** — Reset mutable tables (`attendance_events`, `leaves`, `audit_log`, time-calc derived) between describe blocks. Static tables (`users`, `departments`, `employees`, `devices`, `holidays`) stay intact across the run for performance.
- **D-13** — License bypass via env flag, gated by `CRONOMETRIX_E2E=true`. `CRONOMETRIX_LICENSE_BYPASS=true` is honored ONLY when `CRONOMETRIX_E2E=true` is also set; in any other configuration, the bypass flag must abort startup.
- **D-14** — Mock Hikvision device layer. Inbound: tests POST canned `EventNotificationAlert` XML to `/api/v1/webhooks/hikvision`. Outbound: mock server (wiremock-rs OR hand-rolled Axum) on `localhost:${MOCK_DEVICE_PORT}` impersonates a Hikvision unit.

**CI Gate**
- **D-15** — Required to merge. Branch protection on `main` adds `E2E Tests` to required status checks alongside Phase 8 jobs. Hard-fail.
- **D-16** — Chromium only. Single Playwright project.
- **D-17** — Behavioral assertions only, no visual snapshots. `toMatchSnapshot` not used; per-test screenshot-on-failure stays enabled.
- **D-18** — Upload all artifacts always (`playwright-report/` + `test-results/`), retention 14 days, pinned actions.
- **D-19** — Spanish locale assumption. Tests prefer accessible roles + test-ids; Spanish strings only when matching user-visible copy.
- **D-20** — Time zone freeze. Tests run with `TZ=America/Caracas`. `globalSetup` fixes the system clock or backend clock so time-calc assertions are deterministic.

### Claude's Discretion

The planner / executor decides without further user input:
- Test-runner organization style (page-object vs fixture-based — fixture-based recommended).
- Final test port numbers, mock device port, backend release-vs-debug build mode in CI.
- Parallelism / sharding strategy and worker count (must keep determinism per D-12).
- Retry policy on flake (default 1 retry CI / 0 locally is reasonable).
- Concrete test-ID convention added to React components (e.g., `data-testid="kpi-empleados-presentes"`) — list belongs in PLAN.md.
- Whether to write a small Rust seed binary or use SQL files for fixture seeding.
- Whether mock outbound ISAPI uses `wiremock-rs` or hand-rolled Axum (both acceptable).
- File structure under `frontend/e2e/` (subdirectories per route group, shared fixtures in `frontend/e2e/fixtures/`).
- Whether `webServer` boots only the backend, also the frontend dev server, or `next start` against pre-built frontend.

### Deferred Ideas (OUT OF SCOPE)

- Cross-browser matrix (Firefox, WebKit) — defer to follow-up only on customer request.
- Visual regression / screenshot diffs — defer until design system stabilizes.
- Mobile/responsive E2E — Phase 4 D-3 locked desktop ≥1280px.
- Real Hikvision device tests — manual QA only.
- Turso cloud-sync E2E (offline replica, conflict resolution).
- E2E for `/setup` and `/setup/license` flows — owned by Phase 6.
- E2E for `/(dashboard)/settings` and `/(dashboard)/enrollment` — defer.
- Performance / load-testing E2E (k6, Lighthouse CI) — separate concern.

## Phase Requirements

Phase 9 has no traditional functional requirements — it is a quality-engineering phase. Requirements derive from CONTEXT.md decisions. The mapping for the planner:

| ID | Description | Research Support |
|----|-------------|------------------|
| E2E-TOOLING | Install + configure Playwright; scaffold `frontend/e2e/` | §Domain Research §Standard Stack |
| E2E-FIXTURES | 3 role storageStates via `globalSetup` + DB seed | §Authentication Fixtures pattern, §Code Examples |
| E2E-BACKEND | Boot real Rust binary via `webServer`, ephemeral SQLite | §Backend Orchestration, §Implementation Risks (race) |
| E2E-MOCK | Inbound webhook + outbound mock device | §Mock Hikvision Device Layer |
| E2E-LICENSE-BYPASS | Backend env flag gated by `CRONOMETRIX_E2E` | §Don't Hand-Roll, §Implementation Risks (flag leak) |
| E2E-LOGIN | login.spec.ts UI-driven (D-01) | §Test Inventory |
| E2E-DASHBOARD | dashboard.spec.ts (D-02) | §Test Inventory |
| E2E-CRUD | timesheet/employees/devices/reports specs (D-03) | §Test Inventory |
| E2E-AUDIT | audit.spec.ts (D-04) | §Test Inventory + §Open Questions |
| E2E-CI | New `E2E Tests` job in `.github/workflows/ci.yml` | §CI Integration |
| E2E-DOCS | CLAUDE.md test-only flag documentation | §Standard Stack |

The `## Phase Requirements → Test Map` lives in §Validation Architecture.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| UI-driven login flow | Browser / Client | Frontend Server (Next.js) | Cookies set via response headers; UI form is the test subject |
| Programmatic login (storageState gen) | API / Backend | Browser / Client | `request.post('/api/v1/auth/login')` exchanges credentials for JWT cookie; browser context replays |
| RBAC role gating UI elements | Browser / Client | API / Backend | Frontend reads JWT claim role; backend enforces 403 (authoritative) |
| Inbound Hikvision webhook | API / Backend | Mock Device Layer | Tests POST `EventNotificationAlert` XML directly; backend ingests as if from a real device |
| Outbound ISAPI commands (door open, enrollment) | API / Backend | Mock Device Layer (test-only HTTP server) | Backend's outbound reqwest calls hit `http://localhost:MOCK_PORT` impersonating Hikvision |
| Time-calc determinism | API / Backend | Test runner (TZ env) | Backend owns `chrono::DateTime<Tz>`; `TZ=America/Caracas` propagated to backend process |
| Excel/PDF export verification | Browser / Client (download trigger) | Test runner (Node fs + xlsx/pdf-parse) | Browser triggers download; test runner reads file from disk and parses |
| Audit log assertion after mutation | API / Backend | Database / Storage | Tests query `/api/v1/audit` (or DB directly) to verify audit_log row created |
| SSE disconnect banner | Browser / Client | API / Backend | Browser EventSource auto-retries; test simulates by closing backend or kicking the route |
| License bypass enforcement | API / Backend | — | Pure backend startup gate; tests verify by negative integration test |

**Observation for the planner:** the E2E surface mostly exercises the API tier through the browser. The two tiers we DON'T directly exercise are CDN/Static (no edge concerns on-premise) and Database/Storage (touched only via the backend). This map informs the per-test sampling decision: most assertions are UI assertions, but every mutating CRUD test must verify a corresponding API-tier audit_log row exists (D-03).

## Domain Research

### Playwright Fundamentals (verified 2026-04-28)

| Concept | Verified Behavior | Source |
|---------|-------------------|--------|
| Latest stable | `@playwright/test@1.59.1` (2026-04-09); 1.60.0 in alpha | npm registry [VERIFIED: npm view @playwright/test version] |
| Node compatibility | Node 18+ LTS; CI uses Node 20 (matching Phase 8) | [CITED: playwright.dev/docs/ci] |
| webServer | Spawns shell command, waits for `url` to return 2xx/3xx/4xx; default timeout 60s; supports `env`, `cwd`, `gracefulShutdown` | [CITED: playwright.dev/docs/test-webserver] |
| globalSetup | Single function exported from a TS file referenced via `globalSetup: require.resolve('./global-setup')`; lacks fixture support | [CITED: playwright.dev/docs/test-global-setup-teardown] |
| Project dependencies (preferred) | Define a `setup` project with `testMatch: '**/*.setup.ts'` and `dependencies: ['setup']` on consuming projects; integrates with HTML report + traces | [CITED: playwright.dev/docs/test-global-setup-teardown] |
| storageState | JSON file written by `page.context().storageState({ path })` or `request.storageState({ path })`; loaded via `use: { storageState: 'path' }` per project or per test via `test.use(...)` | [CITED: playwright.dev/docs/auth] |
| API request fixture | `request` fixture provides `APIRequestContext` for direct HTTP calls; multipart, headers, cookies all supported; `request.storageState()` writes auth state without browser | [CITED: playwright.dev/docs/api-testing] |
| Download handling | `page.waitForEvent('download')` → `download.path()` / `download.saveAs()` / `download.suggestedFilename()`; race-safe with Promise.all | [CITED: playwright.dev/docs/api/class-download] |
| Browser caching in CI | Official docs recommend AGAINST caching browser binaries — restore time ≈ download time. Install fresh with `npx playwright install --with-deps` | [CITED: playwright.dev/docs/ci] |
| Test runner timezone | `TZ` env var on the Node process (e.g., `TZ=America/Caracas npx playwright test`) controls Date in test runner; `use: { timezoneId }` controls the BROWSER timezone separately | [CITED: stefanjudis.com / playwright.dev/docs/emulation] |

**Critical clarification on D-20 timezone freeze:** there are TWO timezones to set:
1. **Backend process** receives `TZ=America/Caracas` via `webServer.env`. This drives `chrono::Local` and the Phase 3 time-calc engine.
2. **Test runner process** receives `TZ=America/Caracas` via shell env when launching Playwright. This drives `Date` in test code.
3. **Browser context** receives `use: { timezoneId: 'America/Caracas' }` in `playwright.config.ts`. This drives `Date` and `Intl` inside the page.

All three must agree. Setting only one is a known flake source. [CITED: codewithhugo.com/jest-set-timezone-tz-env-var/]

### Rust/Axum Boot Orchestration

The backend's `main.rs` (verified at `/Users/gerswin/Proyectos/cronometrix/backend/src/main.rs`) does the following at startup, in this order:
1. Loads `.env` via `dotenvy::dotenv().ok()` (line 37).
2. Initializes tracing.
3. Builds `Config::from_env()` — reads `TURSO_DATABASE_URL`, `JWT_SECRET`, `SERVER_HOST`, `SERVER_PORT`, `DEVICE_CREDS_KEY`, `LICENSE_JWT_PATH`, etc.
4. Initializes DB via `db::init_db(&config)` — runs migrations.
5. Calls `license::service::load_and_validate_license(...)` → if false, sets `license_valid = false` (system gated, only `/setup/activate` reachable). [VERIFIED: backend/src/main.rs:75-80]
6. Spawns supervisor / watchdog / recompute / nightly / renewal / purge / backfill workers.
7. Builds router and binds to `{server_host}:{server_port}` (line 321).
8. `axum::serve` runs until SIGINT.

**Implications for `webServer`:**
- The `url` for readiness probe should be `http://localhost:${SERVER_PORT}/api/v1/health`. The `/health` handler does `SELECT 1` so a 200 there proves the migration ran. [VERIFIED: backend/src/main.rs:351-368]
- The license gate is a hard wall — if `license_valid = false` and a test hits `/api/v1/employees`, it gets 403. So `webServer.env` MUST set `CRONOMETRIX_E2E=true` AND `CRONOMETRIX_LICENSE_BYPASS=true` together (D-13), OR the license file must exist and validate (impossible in CI without real hardware).
- `dotenvy::dotenv().ok()` reads `.env` if present. To prevent dev `.env` leaking into the test process, the planner should set `webServer.cwd` to a clean directory (the repo root works because `backend/.env` lives in `backend/`, not the repo root) OR pass `--no-default-features`-style isolation via a wrapper script that explicitly clears env. **Verify before implementing.**

**Build mode:**
- `cargo run --bin cronometrix` (debug) on first invocation = ~60-180s compile.
- `cargo run --release --bin cronometrix` = ~120-300s release compile.
- Once built, both invocations skip compilation if no source changed.
- Recommended: `webServer.command = './target/release/cronometrix'` (in CI) and `webServer.command = './target/debug/cronometrix'` locally, with a separate "Build backend" step before Playwright. This keeps `webServer.timeout` low (30s) and produces clearer failure modes (build failure vs runtime failure).

### Ephemeral SQLite Fixture

The backend uses `libsql` 0.9.30 with embedded-replica mode controlled by `TURSO_DATABASE_URL`. [CITED: backend/src/db/mod.rs] Setting `TURSO_DATABASE_URL=file:/tmp/cronometrix-e2e-${RUN_ID}.db` (or just a local SQLite path) selects the local-only path; no Turso sync required. [VERIFIED: backend/src/db/mod.rs has init_db_local + init_db_remote branches]

Migrations live in `backend/src/db/migrations/` and run automatically at `init_db` startup. [VERIFIED: backend/src/db/migrations/ has 001..017 SQL files] The seed users + departments + employees + devices fixtures must be inserted AFTER migrations complete — i.e., AFTER the backend has started AND BEFORE tests run. Two approaches:

**Option A — Seed via API (simpler, slower).** `globalSetup` calls `POST /api/v1/setup/init` to create the first admin, then logs in as admin and creates supervisor + viewer users via `POST /api/v1/employees` (or via direct DB write — see below). Tradeoff: needs the backend running, takes ~3-5s.

**Option B — Seed via SQL file (faster, more invasive).** Before starting the backend, `globalSetup` opens the SQLite file directly via the Node `better-sqlite3` package, runs migrations + INSERT statements, closes the connection, then starts the backend. Tradeoff: re-implements migration runner; password hashing must run in Node. RustCrypto's `argon2` Rust crate is not callable from Node.

**Option C — Seed via small Rust binary (recommended for this codebase).** Write `backend/src/bin/seed_e2e.rs` that takes a DB URL, opens the libsql connection, runs migrations (reusing `db::init_db`), inserts users with argon2 hashes (reusing `auth::service::hash_password`), and exits. `globalSetup` runs `cargo run --bin seed_e2e -- --db /tmp/...` BEFORE starting the main backend. Reuses production code, no test-only re-implementations. **This is the path of least surprise** and matches how the codebase already structures its work.

D-12 reset between describe blocks is best implemented via a tiny `POST /api/v1/__test_reset` admin endpoint guarded by `CRONOMETRIX_E2E=true` (mirror of license bypass gating). It executes:
```sql
DELETE FROM attendance_events;
DELETE FROM leaves;
DELETE FROM daily_records;
DELETE FROM daily_record_overrides;
DELETE FROM audit_log;
```
Then re-runs deterministic time-calc seed if needed. The endpoint MUST 404 unless `CRONOMETRIX_E2E=true`. Same gating discipline as D-13 license bypass.

### Mock Hikvision Device Layer

Two distinct surfaces:

**Inbound (tests → backend webhook):**
- Backend exposes `/api/v1/webhooks/hikvision` (per Phase 2 D-15 / 02-CONTEXT.md). Devices push multipart `EventNotificationAlert` XML to it.
- Tests construct canned XML matching the schema captured in Phase 2 RESEARCH (DS-K1T341/DS-K1T342 sample traffic) and POST it via Playwright's `request` fixture with digest auth headers.
- For Phase 9: the planner needs a fixture file `frontend/e2e/fixtures/hikvision-events/{employee_id}-entrada.xml` per scenario, plus a helper `postHikvisionEvent(request, employeeId, direction, capturedAtIso)` that sets correct headers + multipart body.

**Outbound (backend → mock Hikvision device):**
- The backend's device manager maintains alertStream connections AND issues commands like `PUT /ISAPI/AccessControl/UserInfo/SetUp` (enrollment), `PUT /ISAPI/RemoteControl/door/0` (door open), `GET /ISAPI/System/status` (health). [CITED: CLAUDE.md "ISAPI Integration Patterns"]
- For E2E, "registered devices" must point at a test-only HTTP server on `localhost:${MOCK_DEVICE_PORT}` that responds to those endpoints with canned success/failure JSON.

Two implementation choices for the mock outbound server:

| Option | Pros | Cons |
|--------|------|------|
| `wiremock-rs` 0.6.5 [VERIFIED: cargo search] | Battle-tested, declarative matchers, randomized port handling, async-runtime-agnostic | Adds dev-dependency; macros / DSL learning curve |
| Hand-rolled tiny Axum router | No new dep, full control, native Rust | Per-spec setup boilerplate; you re-implement matchers |

**Recommendation: hand-rolled Axum router.** Axum is already a project dependency, the entire mock surface is ~80 lines (one router with `PUT/GET/POST` routes returning fixed JSON/XML), and a dedicated mock binary `backend/src/bin/mock_hikvision.rs` keeps the test infrastructure inspectable. `wiremock-rs` shines for unit tests where each test creates an isolated mock; for E2E where the mock is a single long-running process, the unidirectional flow doesn't justify the extra abstraction.

The mock binary is started by `globalSetup` via `child_process.spawn` and pinned to a known port (e.g., 19090). Devices seeded into the test DB use `device_ip = '127.0.0.1:19090'`. `globalTeardown` SIGTERMs it.

### License-Bypass Safety (D-13 — load-bearing)

The threat: a test-only env flag leaks into a production startup and turns off the license gate, defeating LIC-01 / LIC-05.

The mitigation pattern (recommended for `backend/src/license/service.rs` extension):

```rust
// At startup, BEFORE calling load_and_validate_license:
let e2e = std::env::var("CRONOMETRIX_E2E").as_deref() == Ok("true");
let bypass = std::env::var("CRONOMETRIX_LICENSE_BYPASS").as_deref() == Ok("true");

match (e2e, bypass) {
    (true, true)   => { license_valid.store(true, Ordering::Relaxed); /* test mode */ }
    (false, true)  => {
        eprintln!("FATAL: CRONOMETRIX_LICENSE_BYPASS set without CRONOMETRIX_E2E. Refusing to start.");
        std::process::exit(2);
    }
    (true, false)  => { /* test runner ran without bypass — proceed normally */ }
    (false, false) => { /* production normal path */ }
}
```

**The unit test that locks this in:** in `backend/tests/license_bypass_safety.rs`:
```rust
#[test]
fn bypass_without_e2e_aborts() {
    // Use std::process::Command to spawn the binary with the bad combo
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_cronometrix"))
        .env("CRONOMETRIX_LICENSE_BYPASS", "true")
        .env_remove("CRONOMETRIX_E2E")
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "binary should refuse to start");
    assert_eq!(out.status.code(), Some(2));
}
```

This test is mandatory per CONTEXT.md `<specifics>`: "the planner MUST add a unit/integration test in the backend asserting that `CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true` aborts startup."

**Documentation requirement:** CLAUDE.md gets a new "Phase 9 E2E Test Mode" subsection documenting that `CRONOMETRIX_E2E=true` is a TEST-ONLY flag, must NEVER be set in production .env, and any deployment script that sets it should be considered compromised.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@playwright/test` | 1.59.1 | E2E test runner + browser automation | De-facto standard; project-dependencies + storageState + webServer all stable since 1.32; latest stable verified 2026-04-28 [VERIFIED: npm view] |
| Node | 20 LTS | Test runner runtime | Matches Phase 8 Frontend Coverage job (.github/workflows/ci.yml line 73) |
| Chromium (bundled) | latest | Browser engine | D-16 single-project; bundled with Playwright |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `xlsx` (SheetJS community) | 0.18.5 | Read downloaded Excel files | Reports E2E (D-03 reports.spec.ts) verifies downloaded XLSX content [VERIFIED: npm view xlsx] |
| `pdf-parse` | 2.4.5 | Extract text from PDF | Reports E2E verifies downloaded PDF content [VERIFIED: npm view pdf-parse] |
| `dotenv` (dev) | latest | Load test-specific env into Node | Optional convenience for local dev — pass env via shell or `webServer.env` instead |

`xlsx` and `pdf-parse` are added as **devDependencies** in `frontend/package.json`. Both have vibrant ecosystems, deterministic parsing, and stable APIs.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Playwright project-dependencies | `globalSetup` only | globalSetup lacks HTML report + tracing for setup failures — debug nightmare on CI red [CITED: playwright.dev/docs/test-global-setup-teardown] |
| `xlsx` (SheetJS) | `exceljs` | exceljs is heavier, slower, but generates Excel; we only READ here, so xlsx is leaner [VERIFIED: npm size compare] |
| `pdf-parse` | `pdf2json`, `pdfjs-dist` | pdf-parse extracts plaintext (what we need); pdf2json gives structured but verbose JSON; pdfjs-dist is huge [CITED: medium.com/bosphorusiss verify-pdf-contents] |
| Hand-rolled Axum mock | `wiremock-rs` 0.6.5 | wiremock-rs is excellent for per-test isolated mocking; we want a single long-running mock — Axum is simpler [VERIFIED: cargo search wiremock] |
| `cargo run --release` in webServer | Pre-built binary | Pre-built keeps webServer fast (skip 30-180s compile race window); recommended even though CONTEXT.md D-10 says `cargo run --release` |

**Installation:**
```bash
cd frontend
npm install --save-dev @playwright/test xlsx pdf-parse
npx playwright install --with-deps chromium
```

**Verified versions (2026-04-28):**
- `@playwright/test`: 1.59.1 (latest stable; 1.60.0 in alpha) [VERIFIED: npm view]
- `xlsx`: 0.18.5 [VERIFIED: npm view]
- `pdf-parse`: 2.4.5 [VERIFIED: npm view]

## Architecture Patterns

### System Architecture Diagram (data flow at test time)

```
┌────────────────────────────────────────────────────────────────────────┐
│  Playwright test runner (Node 20, TZ=America/Caracas)                  │
│                                                                        │
│  ┌──────────────┐   ┌──────────────────────────────────────────────┐  │
│  │  setup/      │──▶│  spawn mock_hikvision binary (port 19090)    │  │
│  │  *.setup.ts  │   │  spawn seed_e2e binary (writes ephemeral DB) │  │
│  │              │   │  POST /auth/login × 3 → save storageState/{role}.json
│  └──────────────┘   └──────────────────────────────────────────────┘  │
│         │                                                              │
│         ▼ dependencies: ['setup']                                      │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │  chromium project (workers=1 in CI)                              │ │
│  │  ─────────────────────────────────────────────────────────────── │ │
│  │  login.spec.ts            (UI-driven, no storageState)           │ │
│  │  dashboard.spec.ts        (use admin storageState)               │ │
│  │  timesheet.spec.ts        (use supervisor or admin)              │ │
│  │  employees.spec.ts        (use admin)                            │ │
│  │  devices.spec.ts          (use admin)                            │ │
│  │  reports.spec.ts          (use admin/supervisor; download XLSX/PDF)
│  │  audit.spec.ts            (use admin)                            │ │
│  │  rbac.spec.ts             (use viewer; assert 403/redirect)      │ │
│  └──────────────────────────────────────────────────────────────────┘ │
│         │                                                              │
│         ▼  HTTP                       ▼ HTTP                           │
│  ┌──────────────────────┐    ┌──────────────────────────────────┐     │
│  │  Next.js dev server  │    │  ./target/release/cronometrix    │     │
│  │  port 3000           │───▶│  port 4001                       │     │
│  │  (or next start)     │    │  TZ=America/Caracas              │     │
│  │                      │    │  CRONOMETRIX_E2E=true            │     │
│  │                      │    │  CRONOMETRIX_LICENSE_BYPASS=true │     │
│  │                      │    │  CRONOMETRIX_DB_URL=file:/tmp/.. │     │
│  │                      │    │  paths_root=/tmp/cm-e2e-paths    │     │
│  │                      │    └──────────┬───────────────────────┘     │
│  └──────────────────────┘               │                              │
│                                         │ outbound ISAPI                │
│                                         ▼                              │
│                            ┌────────────────────────────┐              │
│                            │  mock_hikvision binary     │              │
│                            │  port 19090                │              │
│                            │  serves PUT /ISAPI/...     │              │
│                            └────────────────────────────┘              │
└────────────────────────────────────────────────────────────────────────┘
                            │
                            ▼ globalTeardown
                  rm /tmp/cronometrix-e2e-${RUN_ID}.db
                  SIGTERM mock_hikvision + backend (Playwright handles)
```

### Recommended Project Structure

```
frontend/
├── playwright.config.ts                # webServer × N, projects, timezone, reporters
├── e2e/
│   ├── .auth/                          # gitignored; storageState files
│   │   ├── admin.json                  # generated by *.setup.ts
│   │   ├── supervisor.json
│   │   └── viewer.json
│   ├── fixtures/
│   │   ├── api.ts                      # API helper (typed request wrapper)
│   │   ├── hikvision-events/           # canned EventNotificationAlert XML files
│   │   │   ├── ana-entrada.xml
│   │   │   └── ana-salida.xml
│   │   ├── time.ts                     # frozen Date helpers (anchored to Caracas)
│   │   └── selectors.ts                # central data-testid catalog (typed)
│   ├── setup/
│   │   ├── 00-build-and-seed.setup.ts  # cargo build + seed DB + generate storageStates
│   │   └── 99-teardown.setup.ts        # delete tmp files
│   ├── login.spec.ts                   # D-01 UI-driven (no storageState)
│   ├── dashboard.spec.ts               # D-02
│   ├── timesheet.spec.ts               # D-03
│   ├── employees.spec.ts               # D-03
│   ├── devices.spec.ts                 # D-03
│   ├── reports.spec.ts                 # D-03 (download XLSX + PDF + parse)
│   ├── audit.spec.ts                   # D-04
│   └── rbac.spec.ts                    # D-01 cross-role (Viewer denied /devices)
├── package.json                        # +@playwright/test, +xlsx, +pdf-parse, +e2e scripts
└── ...
backend/
├── src/
│   ├── bin/
│   │   ├── seed_e2e.rs                 # NEW — runs migrations + seeds users/depts/employees
│   │   └── mock_hikvision.rs           # NEW — tiny Axum mock device
│   └── license/
│       └── service.rs                  # MODIFIED — bypass flag w/ E2E gating
├── tests/
│   └── license_bypass_safety.rs        # NEW — locks D-13 safety in
└── Cargo.toml                          # +[bin] entries for seed_e2e, mock_hikvision
.github/
└── workflows/
    └── ci.yml                          # MODIFIED — append "E2E Tests" job
```

### Pattern 1: Project-dependencies for setup

```typescript
// playwright.config.ts (skeleton — values are illustrative; planner finalizes ports/paths)
import { defineConfig, devices } from '@playwright/test'

const RUN_ID = process.env.GITHUB_RUN_ID ?? `local-${Date.now()}`
const DB_PATH = `/tmp/cronometrix-e2e-${RUN_ID}.db`
const PATHS_ROOT = `/tmp/cronometrix-e2e-paths-${RUN_ID}`

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,                    // D-12 determinism — start serial; planner can lift
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 1 : undefined, // single worker in CI for shared DB safety
  reporter: [['html', { outputFolder: 'playwright-report' }], ['github']],
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: 'http://localhost:3000',
    timezoneId: 'America/Caracas',
    locale: 'es-VE',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'setup',
      testMatch: /.*\.setup\.ts/,
    },
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } },
      dependencies: ['setup'],
    },
  ],
  webServer: [
    {
      // Run pre-built binary; the "build" runs in a CI step before this.
      command: process.env.CI
        ? '../backend/target/release/cronometrix'
        : '../backend/target/debug/cronometrix',
      url: 'http://localhost:4001/api/v1/health',
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
      env: {
        SERVER_HOST: '127.0.0.1',
        SERVER_PORT: '4001',
        TURSO_DATABASE_URL: `file:${DB_PATH}`,
        TZ: 'America/Caracas',
        CRONOMETRIX_E2E: 'true',
        CRONOMETRIX_LICENSE_BYPASS: 'true',
        CRONOMETRIX_LEAVES_ROOT: `${PATHS_ROOT}/leaves`,
        CRONOMETRIX_EVENTS_ROOT: `${PATHS_ROOT}/events`,
        ENROLLMENTS_DIR: `${PATHS_ROOT}/enrollments`,
        CRONOMETRIX_CAPTURES_TMP: `${PATHS_ROOT}/captures-tmp`,
        DATA_DIR: PATHS_ROOT,
        JWT_SECRET: 'e2e-test-secret-must-be-32-bytes-long-1234',
        DEVICE_CREDS_KEY: 'e2e-test-device-creds-key-32-bytes-base64-padded=',
      },
    },
    {
      command: 'next start --port 3000',
      url: 'http://localhost:3000',
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
      env: {
        NEXT_PUBLIC_API_URL: 'http://localhost:4001',
      },
    },
  ],
})
```

### Pattern 2: Setup project that generates storageStates

```typescript
// e2e/setup/00-build-and-seed.setup.ts
import { test as setup, expect } from '@playwright/test'
import { execSync, spawn } from 'node:child_process'
import path from 'node:path'

const BACKEND_DIR = path.resolve(__dirname, '../../../backend')

setup('seed DB and start mock device', async ({ }) => {
  // 1) seed DB via small Rust bin (reuses real migration runner)
  execSync(
    `cargo run --quiet --bin seed_e2e -- --db ${process.env.CRONOMETRIX_DB_URL ?? 'file:/tmp/cronometrix-e2e.db'}`,
    { cwd: BACKEND_DIR, stdio: 'inherit' }
  )

  // 2) Start the mock Hikvision device server (long-running, killed by Playwright on test end)
  const mock = spawn('cargo', ['run', '--quiet', '--bin', 'mock_hikvision'], {
    cwd: BACKEND_DIR,
    env: { ...process.env, MOCK_DEVICE_PORT: '19090' },
    stdio: 'inherit',
    detached: false,
  })
  // optional: wait for /health on the mock
})

setup('authenticate as admin', async ({ request }) => {
  const r = await request.post('http://localhost:4001/api/v1/auth/login', {
    data: { username: 'e2e_admin', password: 'e2e-admin-pass' },
  })
  expect(r.ok()).toBeTruthy()
  await request.storageState({ path: 'e2e/.auth/admin.json' })
})

setup('authenticate as supervisor', async ({ request }) => {
  const r = await request.post('http://localhost:4001/api/v1/auth/login', {
    data: { username: 'e2e_supervisor', password: 'e2e-supervisor-pass' },
  })
  expect(r.ok()).toBeTruthy()
  await request.storageState({ path: 'e2e/.auth/supervisor.json' })
})

setup('authenticate as viewer', async ({ request }) => {
  const r = await request.post('http://localhost:4001/api/v1/auth/login', {
    data: { username: 'e2e_viewer', password: 'e2e-viewer-pass' },
  })
  expect(r.ok()).toBeTruthy()
  await request.storageState({ path: 'e2e/.auth/viewer.json' })
})
```

### Pattern 3: Per-spec storageState

```typescript
// e2e/employees.spec.ts
import { test, expect } from '@playwright/test'

test.use({ storageState: 'e2e/.auth/admin.json' })

test('admin can create new employee with mandatory fields', async ({ page, request }) => {
  await page.goto('/employees')
  await page.getByRole('button', { name: 'Nuevo Empleado' }).click()
  // ... fill form, submit ...
  await expect(page.getByText('Empleado creado')).toBeVisible()

  // Audit assertion (D-03): verify audit_log entry was created
  const audit = await request.get('http://localhost:4001/api/v1/audit', {
    params: { table: 'employees', operation: 'INSERT', limit: 1 },
  })
  expect(audit.ok()).toBeTruthy()
  const entry = await audit.json()
  expect(entry.data[0].actor_id).toBe('e2e_admin')
})
```

### Pattern 4: Excel download verification

```typescript
// e2e/reports.spec.ts (excerpt)
import { test, expect } from '@playwright/test'
import * as XLSX from 'xlsx'

test('admin downloads pre-payroll Excel report and verifies content', async ({ page }) => {
  await page.goto('/reports')
  await page.getByLabel('Período').selectOption('weekly')
  const downloadPromise = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Exportar Excel' }).click()
  const download = await downloadPromise

  const tmpPath = await download.path()
  expect(tmpPath).toBeTruthy()

  const wb = XLSX.readFile(tmpPath!)
  const sheet = wb.Sheets[wb.SheetNames[0]]
  const rows = XLSX.utils.sheet_to_json<{ Empleado: string; 'Min Trabajados': number }>(sheet)
  expect(rows.length).toBeGreaterThan(0)
  expect(rows.find(r => r.Empleado === 'Ana Pérez')).toBeDefined()
})
```

### Pattern 5: PDF download verification

```typescript
import pdf from 'pdf-parse'
import * as fs from 'node:fs'

test('admin downloads pre-payroll PDF and verifies header', async ({ page }) => {
  await page.goto('/reports')
  const downloadPromise = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Exportar PDF' }).click()
  const download = await downloadPromise
  const buf = fs.readFileSync(await download.path() as string)
  const parsed = await pdf(buf)
  expect(parsed.text).toContain('Pre-Nómina')
  expect(parsed.text).toMatch(/Empleados:\s*\d+/)
})
```

### Pattern 6: Mock Hikvision device (hand-rolled Axum)

```rust
// backend/src/bin/mock_hikvision.rs
use axum::{Router, routing::{get, put, post}, Json};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port: u16 = std::env::var("MOCK_DEVICE_PORT").unwrap_or("19090".into()).parse()?;
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ISAPI/System/status", get(|| async { Json(serde_json::json!({
            "status": "OK", "deviceModel": "DS-K1T341", "firmwareVersion": "1.0.0"
        })) }))
        .route("/ISAPI/RemoteControl/door/0", put(|| async { "<ResponseStatus><statusCode>1</statusCode></ResponseStatus>" }))
        .route("/ISAPI/AccessControl/UserInfo/SetUp", put(|| async { "<ResponseStatus><statusCode>1</statusCode></ResponseStatus>" }));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("mock_hikvision listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Logging into the UI in every spec.** Defeats D-06 hybrid-auth strategy. Specs reuse storageState; only `login.spec.ts` exercises the form.
- **Caching Playwright browsers between CI runs.** Official docs explicitly say no — restore time ≈ download time. [CITED: playwright.dev/docs/ci]
- **Using `page.waitForTimeout(N)` to wait for DB updates.** Replace with `expect(locator).toHaveText(...)` polling, or query the API directly via `request`. Sleeps are flake.
- **Asserting on Spanish strings only.** Mixes copy-change risk with logic-change risk. Prefer `getByRole('button', { name: 'Nuevo Empleado' })` (Spanish only when load-bearing; otherwise prefer `data-testid`). [CITED: playwright.dev/docs/locators]
- **Visual snapshot diffs** (D-17 explicit). `toMatchSnapshot()` flakes across font/OS combos. Per-test screenshot-on-failure is fine for triage; not for assertion.
- **Sharing one DB across parallel workers without coordination.** Either pin `workers: 1` in CI (default in skeleton above) OR shard the test DB per worker. Phase 9 starts with workers=1 per D-12 determinism.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Auth state generation | Manual cookie hacking + JSON construction | `request.post('/auth/login')` + `request.storageState({ path })` | Playwright's serializer matches what its loader expects exactly |
| Process orchestration (start/stop backend) | Custom child_process scripts | Playwright `webServer` (with `gracefulShutdown`) | Built-in handles SIGTERM, port readiness, log capture |
| Browser binary management | Manual download + extraction | `npx playwright install --with-deps` | Track-correct browser version per Playwright release |
| Excel parsing | Manual ZIP + XML parse | `xlsx` (SheetJS) | Sheet structure, cell types, dates — minefield without a library |
| PDF text extraction | Manual binary read | `pdf-parse` | Binary format with content streams + encryption variants |
| Mock outbound HTTP server matching | Hand-rolled HTTP server with manual routing | Either `wiremock-rs` or a 50-line Axum router (we chose Axum because already a dep) | Reinventing well-trodden patterns; tiny scope is fine, anything bigger isn't |
| Argon2 password hashing in Node setup | Calling argon2 from a Node lib | Run a Rust seed binary that reuses `auth::service::hash_password` | Single hashing path; no chance of "test passwords don't match prod hash params" |
| Time-zone freezing for Date in Node | `mockdate` / `sinon.useFakeTimers()` | Just set `TZ=America/Caracas` env var on test runner + backend AND `timezoneId` on browser | Three-line fix vs runtime monkey-patching that can't reach the browser |
| Database isolation between tests | Hand-rolled per-test DB clone | One DB + targeted DELETE-then-reseed via test reset endpoint (D-12) | DB clone is slow; selective truncation matches CONTEXT.md decision |
| HTML report generation | Custom dashboard | Playwright's HTML reporter | Free, complete, supports traces; D-18 uploads it as artifact |

**Key insight:** Almost every component in this phase has an industry-standard solution. The temptation will be to write our own test reset endpoint or our own auth fixture. Resist except for the mock_hikvision binary, where a 50-line Axum router IS less rope than a wiremock-rs declarative DSL because we control both sides of the contract.

## Runtime State Inventory

This is a green-field test-infrastructure phase, not a rename/refactor. The Runtime State Inventory section is **not applicable** because:

- No existing strings are being renamed.
- No databases or stored data are being migrated.
- No live services have configurations to update.

The closest concern is the **license bypass flag**, which IS a new piece of "stored state" in the sense that the running backend process holds it in env. The backend startup check (D-13 mitigation) is the only meaningful "state" being added, and it's covered explicitly by the `license_bypass_safety.rs` integration test.

## Common Pitfalls

### Pitfall 1: Backend startup race ("connection refused" intermittent)

**What goes wrong:** `webServer.url` returns 200 (health endpoint) but a parallel webServer (Next.js) hits the API before the database has finished warming up, returning 500 on the first request.

**Why it happens:** Health endpoint does `SELECT 1` which proves the DB connection works, but it doesn't prove all background tasks (recompute worker, supervisor, watchdog) have spun up. A test that POSTs an attendance event before the recompute_tx is ready hangs.

**How to avoid:**
- Make `/api/v1/health` optionally check sub-systems via `?deep=true`. The webServer probe uses `?deep=true`.
- OR have `globalSetup` poll `/api/v1/health` AND `/api/v1/health/deep` after webServer reports ready, and only then proceed with seeding/login.

**Warning signs:** Sporadic test failures on the FIRST test in a fresh run; never reproducible locally with `reuseExistingServer: true`.

### Pitfall 2: License-bypass flag leaks into production

**What goes wrong:** A developer runs `make e2e` locally with `.env.test` sourced into their shell, then forgets to unset, then deploys to staging — the binary boots without license validation.

**Why it happens:** Env vars are process-globally racy. `dotenvy::dotenv()` reads `.env` regardless.

**How to avoid:**
- D-13 implementation: bypass flag aborts startup if `CRONOMETRIX_E2E` is missing.
- Document in CLAUDE.md: "`CRONOMETRIX_E2E=true` is a TEST-ONLY flag. Production deployments MUST refuse to start with it set" — and add a startup-time check that logs `tracing::error!` when CRONOMETRIX_E2E is true (a deploy log alert anyone watching will see).
- The integration test `license_bypass_safety.rs` locks the abort-on-missing-E2E behavior.

**Warning signs:** A staging environment where `/setup/activate` returns 200 without doing anything (because `license_valid` was already true at boot).

### Pitfall 3: Port collisions with dev environment

**What goes wrong:** Developer has `cargo run` running on port 3001 in another terminal; Playwright `webServer` config uses 3001 too; tests confusingly hit the dev backend with a different DB.

**Why it happens:** No port lock; Playwright `reuseExistingServer: true` happily attaches to whatever's there.

**How to avoid:**
- Use a NON-DEFAULT port for E2E. Recommend 4001 (CONTEXT.md D-10 says "test port 4001").
- `webServer.url` includes a query param like `?cronometrix-e2e=1` and `/api/v1/health?cronometrix-e2e=1` returns 200 only when the backend was started with `CRONOMETRIX_E2E=true`. This makes "wrong server attached" fail closed.

**Warning signs:** Tests fail with weird audit-log entries linking a real dev user.

### Pitfall 4: Spanish locale mismatches (Node ICU)

**What goes wrong:** Some Node distributions ship with a small ICU set; `Intl.DateTimeFormat('es-VE')` falls back to English, breaking date assertions.

**Why it happens:** Old Node base images don't include full-icu.

**How to avoid:**
- Node 20 LTS includes full ICU by default. [VERIFIED: Node 20 release notes ship with `--with-intl=full-icu`]
- If the planner ever needs to run on Alpine/slim base, install `icu-libs` or use `node:20-slim`.

**Warning signs:** Date strings like "Apr 28" in tests when expecting "abr 28" or "28 abr".

### Pitfall 5: SSE flakes (D-04 banner test)

**What goes wrong:** The dashboard SSE disconnect banner test passes locally but flakes in CI because the timing windows (1s/2s/4s/8s exponential backoff) don't match the test's polling.

**Why it happens:** SSE EventSource auto-reconnect is browser-driven; tests can't easily intercept the reconnect cadence.

**How to avoid:**
- Don't assert on the EXACT delay. Instead: trigger the disconnect (kill the SSE route, return 503), assert the banner appears within 5s, restore the route, assert the banner disappears within 10s.
- Mock the Phase 4 `useSSE` hook in unit tests; in E2E rely on natural state changes.

**Warning signs:** Test passes once, fails three times in a row, then passes.

### Pitfall 6: Cargo target/ size in CI cache

**What goes wrong:** Caching `backend/target/` between runs to skip recompile causes 2-3 GB cache writes that take longer than recompiling.

**Why it happens:** Rust incremental compilation generates lots of intermediate artifacts.

**How to avoid:**
- Use `Swatinem/rust-cache@v2` (already used in Phase 8 backend-coverage job line 32-34). It caches `~/.cargo/registry`, `~/.cargo/git`, `target/` smartly.
- Or accept full release rebuild on every CI run (~3-5 min) — usually faster than cache restore.

**Warning signs:** CI run > 15 min; "cache restore took 4 min" log.

### Pitfall 7: Audit endpoint doesn't exist yet

**What goes wrong:** D-04 says audit screen tests are required, and D-03 says every CRUD test asserts an audit_log entry was created. But there is currently NO `/api/v1/audit` endpoint and the `/(dashboard)/audit/page.tsx` is a placeholder ("Próximamente"). [VERIFIED: backend/src/main.rs has no /audit route; frontend/src/app/(dashboard)/audit/page.tsx is a 10-line placeholder]

**Why it happens:** Audit Trail UI was scoped as v2 (REQUIREMENTS.md AUDIT-01..03) and not delivered in any prior phase.

**How to avoid:** This is an OPEN QUESTION for the planner — see §Open Questions. Options:
- **A.** Phase 9 ALSO builds a minimal `GET /api/v1/audit` endpoint + minimal audit list UI as Wave 0 work (test enabler). Justifiable per CLAUDE.md "audit-everything" non-negotiable + D-04 in CONTEXT.
- **B.** D-03 mutation tests assert audit by querying `audit_log` table directly via a test-only `/api/v1/__test_audit_query` endpoint (CRONOMETRIX_E2E gated). D-04 audit-screen tests use the placeholder UI assertions only.
- **C.** Defer D-04 audit screen tests to a future phase; have D-03 use a Rust integration test (already covered) for audit_log assertions and the E2E suite asserts the API only.

**Recommended:** Option A. The audit endpoint is small (read-only, paginated, RBAC: admin), the UI is a vanilla TanStack Table, and it satisfies a long-standing CLAUDE.md commitment. This expands Phase 9 scope but the planner should size for it explicitly rather than discover it mid-execution.

## Code Examples

(See §Architecture Patterns for fully-worked examples 1-6. Below are smaller specific examples.)

### Posting a fake Hikvision event from a test (inbound webhook)

```typescript
// e2e/fixtures/hikvision.ts
import type { APIRequestContext } from '@playwright/test'
import * as fs from 'node:fs'
import * as path from 'node:path'

export async function postHikvisionEvent(
  request: APIRequestContext,
  fixtureFile: string,
  digestAuth: { user: string; pass: string }
) {
  const xml = fs.readFileSync(path.join(__dirname, 'hikvision-events', fixtureFile), 'utf8')
  // First request returns 401 with WWW-Authenticate; reqwest+diqwest handle this in the
  // backend's outbound; here we only need the inbound, which the backend treats as
  // already-authenticated when CRONOMETRIX_E2E=true (planner adds a test-mode bypass).
  return request.post('http://localhost:4001/api/v1/webhooks/hikvision', {
    headers: { 'Content-Type': 'multipart/form-data; boundary=MIME_boundary' },
    data: xml,
  })
}
```

### RBAC test (Viewer denied /devices)

```typescript
// e2e/rbac.spec.ts
import { test, expect } from '@playwright/test'
test.use({ storageState: 'e2e/.auth/viewer.json' })

test('viewer cannot access /devices', async ({ page }) => {
  await page.goto('/devices')
  // D-14 in 04-CONTEXT: ISAPI command buttons hidden for viewer
  // Plus backend 403 on POST commands.
  await expect(page.getByRole('button', { name: 'Abrir puerta' })).toHaveCount(0)
})

test('viewer gets 403 on direct command POST', async ({ request }) => {
  const r = await request.post('http://localhost:4001/api/v1/devices/dev-1/commands', {
    data: { command: 'door_open' },
  })
  expect(r.status()).toBe(403)
})
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Cypress for E2E | Playwright | 2022+ | Playwright has multi-tab, multi-context, better CI debugging (traces), no iframe issues. Project chose Playwright because of D-19 multi-tab login behavior test. |
| `globalSetup` config option | Project-dependencies (`setup` project + `dependencies: ['setup']`) | Playwright 1.31+ | Setup gets traces + HTML report visibility; debug-friendly when CI red. [CITED: playwright.dev/docs/test-global-setup-teardown] |
| Caching Playwright browsers in CI | Don't cache; just `npx playwright install --with-deps` | 2024+ | Restore time ≈ download time per official docs. [CITED: playwright.dev/docs/ci] |
| `data-testid` everywhere | `getByRole` first, `getByTestId` only when no accessible role | Playwright 1.27+ | Tests double as accessibility audit. [CITED: playwright.dev/docs/locators] |
| Visual snapshot diffs as default | Behavioral assertions (D-17 in CONTEXT) | 2024+ industry shift | Visual diffs flake across OS/font; behavior is what matters. |

**Deprecated/outdated:**
- `playwright-cli` (separate package) — replaced by `@playwright/test` since 1.10.
- `playwright.config.js` (CommonJS) — TypeScript config preferred for type-checking `defineConfig`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 9 will need a minimal `/api/v1/audit` endpoint + audit UI to satisfy D-04 | §Pitfall 7, §Open Questions | If wrong, planner scopes too small; D-04 tests can't run. Planner MUST resolve in PLAN.md or `/gsd-discuss-phase` follow-up. |
| A2 | A test-only `__test_reset` endpoint (gated by `CRONOMETRIX_E2E=true`) is the cleanest implementation of D-12 | §Domain Research §Ephemeral SQLite Fixture | Alternative: write to SQLite directly from globalSetup via `better-sqlite3`. Riskier — bypasses the migration-tracker; SQL evolves. |
| A3 | A small Rust seed binary is preferred over Node-side seeding | §Domain Research §Ephemeral SQLite Fixture | Seeding via Node would require porting argon2id hashing (different parameters could produce non-prod-equivalent hashes). |
| A4 | Pre-built Rust binary launched by `webServer` is preferred over `cargo run` | §Domain Research §Rust/Axum Boot | CONTEXT.md D-10 explicitly says `cargo run --release` — but the rationale ("speed") is better served by a separate build step. Worth confirming with user; not a locked decision. |
| A5 | Phase 9 will pin `@playwright/test@1.59.1` (latest stable as of 2026-04-28) | §Standard Stack | If 1.60.0 stable lands before Phase 9 ships, planner should bump. No breaking changes expected based on alpha tags. |
| A6 | Hand-rolled Axum mock_hikvision is simpler than wiremock-rs for this use case | §Domain Research §Mock Hikvision Device Layer | If the mock surface grows past ~5 routes or needs rich matching, wiremock-rs becomes the better choice. |
| A7 | `next start` against pre-built frontend is preferable to `next dev` | §Architecture Patterns Pattern 1 | `next dev` has hot-reload overhead and slower cold start. CONTEXT.md leaves this to Claude's discretion. |
| A8 | `workers: 1` in CI is the right starting point | §Architecture Patterns Pattern 1 | Sharded workers possible later (Playwright supports `--shard 1/3`), but D-12 reset semantics need re-thinking before sharding. |
| A9 | Spanish UI strings ARE localized in components (NOT just login form) | §Domain Research selector strategy | Verified: `frontend/src/app/(dashboard)/dashboard/page.tsx` uses "Empleados Presentes", "Distribución por Depto." [VERIFIED via Read] — but `login/page.tsx` uses ENGLISH ("Log in to Cronometrix"). Login spec must use ENGLISH strings or wait for localization. |
| A10 | The `/api/v1/webhooks/hikvision` inbound endpoint exists | §Code Examples | The backend has alertStream listener tasks (Phase 2 02-02-PLAN) but the WEBHOOK pattern (push from device to backend) may not exist as a route — needs verification by planner. CONTEXT.md D-14 says "tests POST canned XML to /api/v1/webhooks/hikvision" so the planner may need to ADD this route. |

**A9 finding is non-trivial:** The login page is in English. Either (a) Phase 9 includes a Spanish-localization sub-plan for the login form (per D-19 user-visible Spanish), or (b) login.spec.ts asserts on English strings until a future i18n phase. Recommend confirming with user.

## Open Questions

1. **Audit endpoint + UI scope (A1, Pitfall 7).**
   - What we know: D-04 requires audit-screen E2E tests; D-03 requires every CRUD test to assert audit_log entry. Backend has no `/api/v1/audit` route. Frontend audit page is a placeholder.
   - What's unclear: Does Phase 9 build the audit endpoint + minimal UI as Wave 0, or does it use a test-only DB query endpoint?
   - Recommendation: **Build it as Wave 0 of Phase 9**. The audit endpoint is small (paginated read of `audit_log`, RBAC: admin), the UI is a TanStack Table with date/user filter. Estimated: 1-2 days. Without it, D-04 cannot be satisfied as user wrote it.

2. **Login page localization (A9).**
   - What we know: D-19 says Spanish UI; login page is currently English ("Log in to Cronometrix", "Username", "Password", "Invalid username or password.").
   - What's unclear: Should Phase 9 localize login as part of this work, or test against English copy and treat localization as a separate phase?
   - Recommendation: **Test against current English copy** (locks behavior); add a TODO note in login.spec.ts that strings will need updating when login is localized. Avoids scope creep; documents the gap.

3. **`/api/v1/webhooks/hikvision` route existence (A10).**
   - What we know: CONTEXT.md D-14 references this route; backend has alertStream LISTENER (outbound consumer) but the inverse — a webhook the device POSTs to — may not exist.
   - What's unclear: Is the inbound webhook already a route? If not, who adds it?
   - Recommendation: Planner confirms during planning by reading Phase 2 RESEARCH/PLAN. If missing, the route must be added (CONTEXT scope) — small 30-line handler that re-uses existing event ingestion service.

4. **Backend run mode in CI (A4).**
   - What we know: CONTEXT.md D-10 says `cargo run --release --bin cronometrix`. The build is ~3-5 min cold; `webServer.timeout` defaults to 60s, so first run will time out.
   - What's unclear: Does the planner add a separate "Build backend" CI step before Playwright (recommended), or rely on `webServer.timeout: 600_000`?
   - Recommendation: **Separate build step.** Cleaner failure modes — Cargo errors are surfaced as build failure, not test timeout.

5. **Cargo seed_e2e binary location.**
   - What we know: Recommend writing `backend/src/bin/seed_e2e.rs`.
   - What's unclear: Should this binary be excluded from production Docker images? `[[bin]]` entries in Cargo.toml all build by default.
   - Recommendation: Use a `[features]` flag — `seed-e2e = []` — and gate the binary with `#[cfg(feature = "seed-e2e")]`. Local dev + CI run with `--features seed-e2e`; production Docker build doesn't enable it. (Same for `mock_hikvision`.)

6. **Phase 9 expansion to fully-localized Spanish UI.**
   - Out of scope per current CONTEXT, but raised by A9. Possible deferred item for a future i18n phase.

7. **TZ freeze for backend wall-clock (D-20).**
   - What we know: Setting `TZ=America/Caracas` makes `chrono::Local` resolve to Caracas. But "fix the system clock" goes further — making the test reproducible across days.
   - What's unclear: Is faking the clock necessary, or is it sufficient that backend uses Caracas TZ + tests use a known seed date for daily_records?
   - Recommendation: **Don't fake the clock.** Seeded daily_records carry explicit `anchor_date` values. Only `daily_records.created_at`/`updated_at` shift with wall clock, and tests don't assert on those. Faking the clock (libfaketime, frozen-time) is heavy machinery for a problem we can sidestep with seed-data discipline.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node | Test runner, frontend dev/start | ✓ (assumed) | 20 LTS (matches Phase 8 CI) | — |
| Rust toolchain | Backend build, mock_hikvision, seed_e2e | ✓ | nightly + stable per `rust-toolchain.toml` | — |
| `cargo-llvm-cov` | Phase 8 — not needed for Phase 9 | ✓ | 0.8.5 | — |
| `cargo-nextest` | Phase 8 — not needed for Phase 9 | ✓ | latest | — |
| `@playwright/test` | E2E test runner | ✗ NOT INSTALLED | — | None — must `npm install --save-dev` (E2E-TOOLING) |
| Playwright Chromium binary | Browser automation | ✗ NOT INSTALLED | — | None — `npx playwright install --with-deps chromium` (one-time setup) |
| `xlsx` (SheetJS) | Reports E2E | ✗ NOT INSTALLED | — | None — `npm install --save-dev xlsx` |
| `pdf-parse` | Reports E2E | ✗ NOT INSTALLED | — | None — `npm install --save-dev pdf-parse` |
| SQLite | Backend storage | ✓ (libSQL bundled) | — | — |
| `/proc`, `/sys` (license fingerprint) | License module on Linux | ✓ in CI (Ubuntu) | — | Bypass via D-13 flag |
| TZ data (`/usr/share/zoneinfo/America/Caracas`) | Backend TZ resolution | ✓ in CI (Ubuntu has tzdata) | — | If missing: install `tzdata` package |

**Missing dependencies with no fallback:** None — all four are install-time additions to existing infrastructure.

**Missing dependencies with fallback:** None.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `@playwright/test` 1.59.1 |
| Config file | `frontend/playwright.config.ts` (NEW) |
| Quick run command | `cd frontend && npx playwright test --project=chromium --grep <pattern>` |
| Full suite command | `cd frontend && npx playwright test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| E2E-TOOLING | `@playwright/test` installed; config valid; `setup` project runs | smoke | `npx playwright test --project=setup` | ❌ Wave 0 |
| E2E-FIXTURES | All three storageState files generated; admin/supervisor/viewer can each authenticate | setup | `npx playwright test --project=setup` | ❌ Wave 0 |
| E2E-BACKEND | `webServer` boots backend; `/api/v1/health` returns 200; ephemeral DB created | infrastructure | covered by setup project + first-test boot | ❌ Wave 0 |
| E2E-MOCK | Mock Hikvision binary boots; outbound ISAPI command from backend reaches mock | infrastructure | covered by `devices.spec.ts` "admin opens door" test | ❌ Wave 0 |
| E2E-LICENSE-BYPASS | `CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true` aborts startup | unit (Rust) | `cargo nextest run --test license_bypass_safety` | ❌ Wave 0 |
| E2E-LOGIN | Happy login, invalid creds, password validation, session expiry, refresh-token rotation, multi-tab, RBAC redirect (D-01, ~8 tests) | e2e | `npx playwright test login.spec.ts` | ❌ Wave 1 |
| E2E-DASHBOARD | KPI calculations, donut chart, ring buffer, photo fallback, SSE banner, empty states (D-02, ~6 tests) | e2e | `npx playwright test dashboard.spec.ts` | ❌ Wave 1 |
| E2E-CRUD-TS | Timesheet list + filters + Registrar Novedad CRUD + validation + audit (D-03) | e2e + audit assertion | `npx playwright test timesheet.spec.ts` | ❌ Wave 2 |
| E2E-CRUD-EMP | Employees CRUD + search/filter + validation (D-03) | e2e + audit assertion | `npx playwright test employees.spec.ts` | ❌ Wave 2 |
| E2E-CRUD-DEV | Devices list + ISAPI dispatch + connection status + error states (D-03) | e2e + audit assertion | `npx playwright test devices.spec.ts` | ❌ Wave 2 |
| E2E-CRUD-REP | Reports generate + Excel + PDF export verification + filter combos (D-03) | e2e + file parsing | `npx playwright test reports.spec.ts` | ❌ Wave 2 |
| E2E-AUDIT | Audit log lists immutable entries; filter by user/date works (D-04) | e2e | `npx playwright test audit.spec.ts` | ❌ Wave 2 (depends on Wave 0 audit endpoint per Open Q1) |
| E2E-RBAC | Viewer denied `/devices` UI controls + 403 on POST (D-01 cross-cut) | e2e | `npx playwright test rbac.spec.ts` | ❌ Wave 2 |
| E2E-CI | New `E2E Tests` job in `.github/workflows/ci.yml`; pinned actions; artifacts uploaded | CI workflow | manual: open PR, see job pass | ❌ Wave 3 |
| E2E-DOCS | CLAUDE.md "Phase 9 E2E" subsection; `make e2e` target | docs | manual: read file | ❌ Wave 3 |

### Sampling Rate

- **Per task commit:** Run touched spec only — `npx playwright test <file>` (~10-30s for one spec).
- **Per wave merge:** Run full E2E suite — `npx playwright test` (estimated 5-15 min for 50+ tests at workers=1).
- **Phase gate:** Full suite green via `make e2e` AND on the live `E2E Tests` CI job before `/gsd-verify-work`.

### Eight Validation Dimensions for E2E (Nyquist coverage map)

The phase requirements span 8 distinct validation dimensions; Phase 9 must hit each:

| # | Dimension | What's Validated | Where Covered |
|---|-----------|------------------|---------------|
| 1 | **Smoke** | App boots, login screen renders | `login.spec.ts` first test, infra in setup project |
| 2 | **Contract** | Request/response shapes match between frontend and backend | Implicit: every spec exercises real APIs; failures surface schema drift |
| 3 | **Journey** | End-to-end user workflows (login → CRUD → logout) | All specs |
| 4 | **RBAC** | Each role sees only what it should; backend enforces | `rbac.spec.ts`, role-specific specs |
| 5 | **Mutation→Audit** | Every mutating action produces audit_log entry | Every D-03 CRUD test asserts audit (per CLAUDE.md non-negotiable) |
| 6 | **Error states** | Validation errors, 4xx, 5xx surfaced correctly | login.spec.ts, CRUD validation tests, devices error states |
| 7 | **Time-calc determinism** | Tolerance, lunch, overtime calcs reproduce in tests | `timesheet.spec.ts` with seeded events of known timestamps |
| 8 | **Real-time/SSE** | Live activity feed, ring buffer, disconnect banner | `dashboard.spec.ts` |

### Wave 0 Gaps

Mandatory before Wave 1 implementation can start:

- [ ] `frontend/playwright.config.ts` — webServer × 2, projects (setup + chromium), env injection (E2E-TOOLING)
- [ ] `frontend/e2e/setup/00-build-and-seed.setup.ts` — DB seed + storageState generation (E2E-FIXTURES)
- [ ] `frontend/e2e/fixtures/api.ts`, `selectors.ts`, `time.ts`, `hikvision-events/*.xml` — shared fixtures
- [ ] `frontend/.gitignore` (or `.gitignore` at repo root) — add `frontend/e2e/.auth/`, `frontend/playwright-report/`, `frontend/test-results/`
- [ ] `frontend/package.json` — add `@playwright/test`, `xlsx`, `pdf-parse` devDeps + `e2e`, `e2e:install` scripts
- [ ] `backend/src/bin/seed_e2e.rs` — seed binary (E2E-FIXTURES)
- [ ] `backend/src/bin/mock_hikvision.rs` — mock outbound device (E2E-MOCK)
- [ ] `backend/Cargo.toml` — `[[bin]]` entries with `required-features = ["seed-e2e"]` / `["mock-hikvision"]`
- [ ] `backend/src/license/service.rs` (or `main.rs`) — bypass-flag check w/ E2E gating (E2E-LICENSE-BYPASS)
- [ ] `backend/tests/license_bypass_safety.rs` — locks D-13 in
- [ ] `backend/src/main.rs` — register `__test_reset` route (D-12 mutable-table reset, gated)
- [ ] **Open Q1 resolution:** `backend/src/audit/mod.rs` + `GET /api/v1/audit` route, `frontend/src/app/(dashboard)/audit/page.tsx` — minimal viable audit screen (E2E-AUDIT enabler)
- [ ] **Open Q3 resolution:** `backend/src/webhooks/hikvision.rs` if route doesn't already exist (E2E-MOCK enabler)
- [ ] `Makefile` — `make e2e`, `make e2e-install` targets (mirror Phase 8's coverage targets)

## Implementation Risks

### Race conditions

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Backend reports ready (200 on /health) but workers not yet spawned, first POST 500s | MEDIUM | First test in run flakes | Add `?deep=true` to health endpoint that checks supervisor + recompute_tx readiness; webServer probes deep |
| Mock Hikvision starts AFTER backend tries first command dispatch on startup | LOW | Backend logs benign error, devices show "offline" briefly | Start mock_hikvision in setup project BEFORE webServer starts (project-dep ordering) |
| Two parallel `npx playwright test` runs on same dev box clobber `/tmp/cronometrix-e2e.db` | LOW | Garbage data; both fail | RUN_ID derived from `process.pid` or `crypto.randomUUID()` for local; `GITHUB_RUN_ID` in CI |
| Backend SIGTERM during DELETE in __test_reset truncates a write mid-flight | LOW | Followed by next-run startup, migrations re-applied — non-issue | webServer's gracefulShutdown is opt-in; default SIGTERM is fine |

### Flake sources

| Source | Sign | Mitigation |
|--------|------|------------|
| `page.waitForTimeout(N)` instead of explicit waits | Random failures unrelated to logic | Code review; lint rule prohibiting waitForTimeout in spec files |
| SSE reconnect timing assertions | Test passes 4/5 runs | Assert eventual consistency, not exact delay (see §Pitfall 5) |
| Date-dependent assertions (NOW() in DB) | Tests pass at 23:59 local, fail at 00:01 | Use seeded fixed dates; never rely on `NOW()` in test fixtures |
| Network jitter on real backend boot | webServer timeout occasional | Pre-build binary; raise timeout to 60-120s |
| Argon2 cost differences between dev seed and prod hash params | "Login worked locally, fails in CI" | Single hashing path via `seed_e2e` Rust binary (reuses prod code) |

### License-flag leak (D-13 — most critical)

Already covered in §Domain Research §License-Bypass Safety. Re-summarized:
- Detect `CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true` → abort startup with exit code 2.
- Lock with `tests/license_bypass_safety.rs` integration test.
- Document in CLAUDE.md as test-only.

### CI cache hits

| Cache | Hit rate | Risk on miss |
|-------|----------|--------------|
| `~/.cargo/registry`, `~/.cargo/git`, `target/` (via Swatinem/rust-cache@v2) | High after first run | Cold build adds ~3-5 min |
| `~/.npm` (via setup-node@v4 cache) | High | Cold install adds ~30s |
| Playwright browsers (`~/.cache/ms-playwright`) | NOT CACHED per official guidance | Fresh install ~30s — comparable to cache restore |

### Required-status-check trap

If `E2E Tests` is added to GitHub branch protection BEFORE the workflow has run successfully on `main`, no PR can merge. Mitigation (matching Phase 8 Plan 05's pattern): defer the branch-protection step to a manual follow-up, validate the workflow runs green on a real PR first, THEN flip the protection. CONTEXT.md D-15 explicitly calls this out as the same "manual follow-up" model from Phase 8.

## Project Constraints (from CLAUDE.md)

| Directive | Source | How research honors it |
|-----------|--------|------------------------|
| Tech stack: Rust + Axum backend; Next.js + TypeScript frontend | CLAUDE.md "Constraints" | Phase 9 doesn't modify either; tests verify them |
| Audit compliance: every mutation generates immutable audit log entry | CLAUDE.md "Constraints" | D-03 mutation tests assert audit_log entries; D-04 audit-screen tests required |
| Filesystem-root injection convention | CLAUDE.md "Conventions §Filesystem-root injection (Phase 8)" | All paths (`leaves_root`, `events_root`, `enrollments_root`, `captures_tmp_root`, `overrides_root`) passed via `webServer.env` to test scope |
| Deployment: Docker Compose | CLAUDE.md "Constraints" | Out of Phase 9 scope; mock_hikvision binary doesn't change Docker images (gated by feature flag per Open Q5) |
| Pinned action policy: `actions/checkout@v4`, `actions/setup-node@v4`, `actions/upload-artifact@v4`, etc. | CLAUDE.md "Test Coverage §CI gate" + Phase 8 T-08-15 | New `E2E Tests` job MUST use the same pinned versions; `permissions: contents: read` |
| 14-day artifact retention | CLAUDE.md "Test Coverage §HTML reports" | `playwright-report/` and `test-results/` upload with `retention-days: 14` |
| Test Coverage gate (Phase 8 — must not regress) | CLAUDE.md "Test Coverage" | Phase 9 is additive; doesn't touch `vitest.config.ts` (D-10 in Phase 8 CONTEXT excludes `src/app/**`) |
| Hard-fail CI gate philosophy | CLAUDE.md "Test Coverage §CI gate" + D-15 in this CONTEXT | `E2E Tests` job exits non-zero on any test failure; no soft-warn |
| Spanish UI assumption | CONTEXT D-19 + memory:project_jurisdiction | Tests prefer roles/test-ids; Spanish strings only when load-bearing |
| TZ = America/Caracas, no DST | memory:project_jurisdiction + D-20 | TZ env var on backend, runner, and `timezoneId` on browser context |
| GSD workflow enforcement | CLAUDE.md "GSD Workflow Enforcement" | This research is itself a GSD artifact |

## Sources

### Primary (HIGH confidence)

- **Playwright official docs (verified 2026-04-28):**
  - [playwright.dev/docs/test-webserver](https://playwright.dev/docs/test-webserver) — webServer config (command, url, env, gracefulShutdown, reuseExistingServer)
  - [playwright.dev/docs/auth](https://playwright.dev/docs/auth) — storageState + 3 auth strategies + multi-role pattern
  - [playwright.dev/docs/test-global-setup-teardown](https://playwright.dev/docs/test-global-setup-teardown) — project-dependencies pattern (preferred over globalSetup)
  - [playwright.dev/docs/test-projects](https://playwright.dev/docs/test-projects) — projects, dependencies, per-project use config
  - [playwright.dev/docs/api-testing](https://playwright.dev/docs/api-testing) — APIRequestContext for direct backend calls
  - [playwright.dev/docs/api/class-download](https://playwright.dev/docs/api/class-download) — download.path/saveAs/suggestedFilename, race-safe with Promise.all
  - [playwright.dev/docs/ci](https://playwright.dev/docs/ci) — official GitHub Actions YAML; "do not cache browsers" guidance
  - [playwright.dev/docs/locators](https://playwright.dev/docs/locators) — getByRole-first selector strategy
  - [playwright.dev/docs/release-notes](https://playwright.dev/docs/release-notes) — version timeline (1.59 latest stable through 2026-04-28)
  - [playwright.dev/docs/emulation](https://playwright.dev/docs/emulation) — `timezoneId`, `locale` for browser context

- **Codebase artifacts (VERIFIED via Read):**
  - `backend/src/main.rs` — startup sequence, license gate flow, port binding
  - `backend/src/license/service.rs` — license validation pattern bypass extension hooks into
  - `backend/src/state/paths.rs` — `Paths::for_test` test pattern parallels what `webServer.env` does
  - `backend/src/state/mod.rs` — `AppState` shape (license_valid AtomicBool, paths Arc<Paths>)
  - `frontend/src/app/login/page.tsx` — current login UX (English copy — A9 finding)
  - `frontend/src/app/(dashboard)/dashboard/page.tsx` — Spanish dashboard copy
  - `frontend/src/app/(dashboard)/audit/page.tsx` — placeholder only (Open Q1)
  - `frontend/package.json` — installed deps; Playwright NOT present
  - `frontend/vitest.config.ts` — Phase 8 thresholds (must not regress)
  - `.github/workflows/ci.yml` — Phase 8 jobs the new E2E job extends
  - `.planning/phases/09-.../09-CONTEXT.md` — locked decisions D-01..D-20
  - `.planning/phases/04-frontend-ui/04-CONTEXT.md` — Phase 4 UI contracts tests assert against
  - `.planning/phases/02-device-integration/02-CONTEXT.md` — Hikvision ISAPI patterns + alertStream context
  - `.planning/phases/06-licensing-deployment/06-CONTEXT.md` — license fingerprint architecture for D-13
  - `.planning/REQUIREMENTS.md` — requirement IDs across the system
  - `.planning/ROADMAP.md` — phase scope summary
  - `.planning/codebase/STACK.md`, `STRUCTURE.md` — tech stack + repo layout

- **npm registry (verified 2026-04-28 via `npm view`):**
  - `@playwright/test` 1.59.1 stable (1.60.0-alpha-2026-04-28 in alpha)
  - `xlsx` 0.18.5
  - `pdf-parse` 2.4.5

- **cargo (verified via `cargo search`):**
  - `wiremock` 0.6.5

### Secondary (MEDIUM confidence)

- [Verifying PDF file data in Playwright (skptricks 2025-05)](https://www.skptricks.com/2025/05/verifying-pdf-file-data-in-playwright.html) — pdf-parse + Playwright integration pattern
- [How to Download & Validate Excel File in Playwright (Medium)](https://medium.com/@testerstalk/how-to-download-validate-excel-file-in-playwright-b8acbb19a4e8) — XLSX read pattern after download.saveAs
- [Set the default time zone in Node.js (Stefan Judis)](https://www.stefanjudis.com/today-i-learned/set-the-default-time-zone-in-node-js/) — TZ env var behavior in Node
- [Configure Timezone for Jest/Node.js (codewithhugo)](https://codewithhugo.com/jest-set-timezone-tz-env-var/) — TZ env var caveats (process startup, not runtime)
- [Playwright `webServer` Without Surprises (Steve Kinney)](https://stevekinney.com/courses/self-testing-ai-agents/playwright-web-server-without-surprises) — practical webServer pitfalls
- [WireMock Rust docs (wiremock.org)](https://wiremock.org/docs/solutions/rust/) — wiremock-rs general docs (rejected as overkill, but documented as alternative)

### Tertiary (LOW confidence — flagged for validation by planner)

- [Integrating Playwright with Next.js (DEV)](https://dev.to/mehakb7/integrating-playwright-with-nextjs-the-complete-guide-34io) — community guide; verify any specific claim against official docs
- [Optimizing Database Integration in Playwright (Medium)](https://medium.com/@thananjayan1988/optimizing-database-integration-in-playwright-e86a3408ece9) — patterns for DB-backed E2E

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified via npm registry + official docs
- Architecture (project-dep + storageState pattern): HIGH — official Playwright recommendation
- Backend orchestration (real binary via webServer): HIGH — verified by reading backend/main.rs end-to-end
- Mock Hikvision device approach: MEDIUM — hand-rolled Axum is straightforward, but the inbound webhook route MAY need to be added (Open Q3)
- License-bypass safety pattern: HIGH — pure deterministic env-check logic, locked with integration test
- Audit endpoint requirement: MEDIUM — D-04 + Pitfall 7 + Open Q1 indicate Phase 9 may need to ALSO build the audit screen + endpoint to satisfy E2E coverage
- CI integration: HIGH — Phase 8's workflow is the template; pinned actions + permissions already established
- Pitfalls: HIGH — most are well-known Playwright/CI patterns
- Test inventory size (~50+): HIGH (sized by user in D-01..D-05)

**Research date:** 2026-04-28
**Valid until:** 2026-05-28 (30 days for stable Playwright; sooner if 1.60.0 stable lands)
