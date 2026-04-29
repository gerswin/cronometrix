# Phase 9: E2E Playwright Test Suite - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 9 delivers a comprehensive end-to-end test suite using Playwright that exercises the full Next.js frontend (`frontend/src/app/`) against the real Rust/Axum backend with seeded SQLite. It locks user-visible behavior across all dashboard routes, formalizes auth fixtures (3 RBAC roles), and adds a required-to-merge CI gate so that frontend regressions on Next.js route pages — explicitly excluded from Vitest coverage in Phase 8 D-10 — are caught before merge.

**In scope:**
- Playwright tooling: install `@playwright/test`, configure `frontend/playwright.config.ts`, scaffold `frontend/e2e/` directory.
- Auth fixtures: programmatic + UI-driven hybrid login, three storageState files (Admin, Supervisor, Viewer) seeded by `globalSetup`.
- E2E specs at **Full UAT depth** for these route groups (~50+ tests total):
  - `/login` — happy path, all error states, password validation, session expiry, refresh token, multi-tab.
  - `/(dashboard)/dashboard` — all KPI calculations, donut chart breakdown, ring-buffer behavior, photo fallback, every empty state, SSE disconnect banner.
  - `/(dashboard)/timesheet` (marcaciones) — list + filters + Registrar Novedad CRUD + validation + audit-log entry creation.
  - `/(dashboard)/employees` (empleados) — CRUD + search/filter + validation.
  - `/(dashboard)/devices` (dispositivos) — list + ISAPI command dispatch + connection status + error/edge states.
  - `/(dashboard)/reports` (reportes) — generate report + Excel/PDF export verification + filter combinations.
  - `/(dashboard)/audit` — immutable audit-log listing + filter by user/date.
- Backend orchestration: Playwright `webServer` boots the real `cronometrix` binary on a test port against an ephemeral SQLite DB.
- Mock device layer: tests POST sample Hikvision `EventNotificationAlert` XML to the inbound webhook; outbound ISAPI is intercepted by a tiny test-only mock server.
- License-bypass test mode: a backend env flag, gated behind `CRONOMETRIX_E2E=true`, that skips the hardware fingerprint check.
- CI integration: a new `E2E Tests` job in `.github/workflows/ci.yml` that hard-fails on regression, marked as a required status check on PRs to `main`.

**Out of scope:**
- Cross-browser matrix beyond Chromium (Firefox, WebKit) — deferred.
- Visual regression / screenshot snapshot diffs — deferred.
- Mobile/responsive E2E (Phase 4 D-3 locked desktop-only ≥1280px).
- Real Hikvision device hardware tests — manual QA only.
- Turso cloud-sync E2E (offline replica behavior) — deferred.
- Frontend unit/integration tests — Phase 8 already covers those at ≥90% line / ≥85% branch.
- Mutating Phase 8 Vitest coverage gates — Phase 9 is additive.

</domain>

<decisions>
## Implementation Decisions

### Test Depth & Coverage

- **D-01 — Login: Full UAT depth.** Cover happy login, invalid credentials, password validation rules, session expiry, refresh-token rotation, multi-tab session behavior, RBAC redirect (Viewer cannot reach `/devices`). Approx. 8+ tests. `login.spec.ts` is the only file using UI-driven login; all other specs reuse storageState.
- **D-02 — Dashboard: Full UAT depth.** Cover all KPI tile calculations (Empleados Presentes, % Retraso Hoy, Dispositivos Activos, Alertas Diurnas), donut chart by department (Phase 4 D-5), 20-event ring buffer (Phase 4 D-6), photo fallback to initials (Phase 4 D-2), SSE disconnect banner with backoff (Phase 4 D-4), every empty state. Approx. 6+ tests.
- **D-03 — CRUD routes: Full UAT depth.** For each of `timesheet`, `employees`, `devices`, `reports`: full CRUD coverage, all filter combinations, all validation errors, and assertions that mutating actions produce the expected immutable audit-log entry (CLAUDE.md: every mutation to attendance records must generate an audit log entry). Reports tests must verify Excel + PDF export content, not just download success. Approx. 30+ tests across the four routes.
- **D-04 — Audit screen tests: required.** Auditability is non-negotiable per CLAUDE.md. Add 1–2 tests asserting (a) the audit log lists immutable entries and (b) filter by user/date works.
- **D-05 — Total target ≈ 50+ tests.** This is a deliberate scope decision; the planner should size waves and parallelism accordingly and not silently downscale to "smoke."

### Authentication Fixtures

- **D-06 — Hybrid auth strategy.** Inside `login.spec.ts`, run UI-driven login through the real `/login` form. Every other spec reuses a pre-built `storageState` JSON to skip login. Avoids paying ~2–3s of UI overhead per test at 50+ tests while still exercising the real login UX in the dedicated spec.
- **D-07 — Three role fixtures: Admin, Supervisor, Viewer.** Matches the RBAC model in CLAUDE.md (`role` claim in JWT). Each role has its own storageState file. Tests pick the role they need (e.g., RBAC negative tests use Viewer to assert `/devices` returns 403 / redirects).
- **D-08 — Seed users via `globalSetup`.** A Playwright `globalSetup` script issues idempotent SQL INSERT … ON CONFLICT against the test DB to seed `e2e_admin`, `e2e_supervisor`, `e2e_viewer` users with argon2id password hashes. Single source of truth; no reliance on dev-DB state.
- **D-09 — storageState files at `frontend/e2e/.auth/{role}.json`, gitignored.** Add `frontend/e2e/.auth/` to `.gitignore`. `globalSetup` regenerates all three files on every run (local + CI). Files are never committed; they are working artifacts only.

### Backend Orchestration & Test Data

- **D-10 — Real backend via Playwright `webServer`.** `playwright.config.ts` spins up the actual Rust/Axum binary (`cargo run --release --bin cronometrix` in CI; `cargo run` locally) on test port `4001` (final port: planner discretion, must be config-only and avoid collision with dev port). Frontend dev server runs on its own port. No network mocks at the API layer — tests exercise real Axum handlers, real `libsql` queries, real time-calculation logic.
- **D-11 — Ephemeral SQLite per run.** `globalSetup` creates a fresh SQLite file at `/tmp/cronometrix-e2e-${RUN_ID}.db`, runs all migrations, seeds users + departments + employees + devices fixtures. The backend starts with `CRONOMETRIX_DB_URL` pointing to this file. `globalTeardown` deletes it. Path follows the **Filesystem-root injection convention** from CLAUDE.md (Phase 8) — all backend roots (`leaves_root`, `events_root`, `enrollments_root`, `captures_tmp_root`, `overrides_root`) are passed via env so the test process never collides with dev paths.
- **D-12 — Reset mutable tables between describe blocks.** Tables `attendance_events`, `leaves`, `audit_log`, and any time-calc derived tables are truncated and reseeded between describe blocks (use Playwright fixture scope `worker` or a custom `test.beforeAll`). `users`, `departments`, `employees`, `devices`, `holidays` stay intact across the run for performance. This makes each describe block deterministic without the cost of full DB recreate.
- **D-13 — License bypass via env flag, gated by `CRONOMETRIX_E2E=true`.** Backend reads `CRONOMETRIX_LICENSE_BYPASS=true` only when `CRONOMETRIX_E2E=true` is also set; in any other build/runtime configuration, the presence of `CRONOMETRIX_LICENSE_BYPASS` must abort startup with a clear error so the flag cannot leak to production. Both flags are set by Playwright's `webServer.env`. Documented as a test-only flag in CLAUDE.md.
- **D-14 — Mock Hikvision device layer.** Inbound: tests trigger marcaciones by `POST`ing canned `EventNotificationAlert` XML (and optional JPEG payload) to `/api/v1/webhooks/hikvision` with valid digest auth. Outbound: a tiny test-only Axum router (or `wiremock-rs`) listens on `localhost:${MOCK_DEVICE_PORT}` and impersonates a Hikvision unit so the backend's outbound ISAPI calls (door open, enroll face, status check) succeed without real hardware. The mock server is started by `globalSetup`.

### CI Gate

- **D-15 — Required to merge.** Branch protection on `main` adds `E2E Tests` to the required status checks list, alongside Phase 8's `Backend Coverage` and `Frontend Coverage` jobs. Hard-fail on any test failure — no soft-warn, no override label. Aligns with the audit-compliance ethos (Phase 8 D-13 in 08-CONTEXT.md / CLAUDE.md).
- **D-16 — Chromium only.** Single Playwright project. Cronometrix is an on-premise admin tool; clients run Chrome/Edge. Firefox/WebKit added later only on customer request. Documented in CONTEXT under Deferred.
- **D-17 — Behavioral assertions only, no visual snapshots.** Tests select via accessible role + name + test-id only; no `toMatchSnapshot` image diffs (flaky in CI across font/OS). Playwright's default per-test screenshot-on-failure stays enabled for debugging — that is not visual regression.
- **D-18 — Upload all artifacts always.** `actions/upload-artifact@v4` uploads the entire `playwright-report/` HTML report plus `test-results/` (videos, traces, screenshots, retry attempts) on every CI run, not just failures. Retention 14 days to match Phase 8 coverage-report retention. Action versions pinned per Phase 8 threat model T-08-15 (least privilege).
- **D-19 — Spanish locale assumption.** UI is Spanish (login form, "Marcaciones," "Empleados," "Dispositivos," "Reportes," "Registrar Novedad"). Tests use Spanish strings only when matching user-visible copy; rely on accessible roles + test-ids first to keep selectors localization-tolerant.
- **D-20 — Time zone freeze.** Tests run with `TZ=America/Caracas` (per project_jurisdiction: target market Venezuela, no DST). `globalSetup` fixes the system clock or backend clock so time-calc assertions (tolerance windows, lunch deductions) are deterministic.

### Claude's Discretion

The planner / executor decides the following without further user input:

- Test-runner organization style (page object model vs. fixture-based — fixture-based recommended for this codebase given the Playwright `test.extend` pattern).
- Final test port numbers, mock device port, backend release-vs-debug build mode in CI.
- Parallelism / sharding strategy and worker count (must keep determinism per D-12).
- Retry policy on flake (default 1 retry in CI, 0 locally is reasonable).
- Concrete test-ID convention added to React components (e.g., `data-testid="kpi-empleados-presentes"`) — list of new test-ids belongs in PLAN.md.
- Whether to write a small Rust seed binary or use SQL files for fixture seeding.
- Whether mock outbound ISAPI uses `wiremock-rs` or a hand-rolled Axum router (both acceptable).
- File structure under `frontend/e2e/` (subdirectories per route group, shared fixtures in `frontend/e2e/fixtures/`).
- Whether the `webServer` block boots only the backend, or also the frontend dev server, or uses `next start` against a pre-built frontend.

### Folded Todos

None — no todos matched Phase 9 scope.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase boundary & roadmap
- `.planning/ROADMAP.md` — Phase 9 section: scope, dependencies on Phase 8.
- `.planning/REQUIREMENTS.md` — Acceptance criteria (TBD per ROADMAP; planner should derive from this CONTEXT + Phase 4 routes).

### Prior phase context (locked decisions to honor)
- `.planning/phases/04-frontend-ui/04-CONTEXT.md` — All route inventory, dashboard layout (D-5), SSE behavior (D-4), photo fallback (D-2), 20-event ring buffer (D-6), desktop-only ≥1280px viewport (D-3). Tests must match these contracts.
- `.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-CONTEXT.md` — D-10 frontend coverage scope (`src/app/` excluded from Vitest because Phase 9 covers it via E2E). Phase 9 must NOT change that exclusion. Also D-13 (audit-compliance gate ethos) and the CI workflow file Phase 9 must extend.
- `.planning/phases/06-licensing-deployment/06-CONTEXT.md` — Hardware-bound license, fingerprint = SHA256(cpu+mac+disk), RS256 JWT. Drives D-13 license bypass design.
- `.planning/phases/02-device-integration/02-CONTEXT.md` — Hikvision ISAPI patterns (digest auth, `EventNotificationAlert` XML format, webhook flow). Drives D-14 mock device layer.
- `.planning/phases/03-time-calculation-engine/03-CONTEXT.md` — Time-calc rules (tolerance, lunch, shifts) the timesheet E2E tests assert against. Read for expected outputs.
- `.planning/phases/05-reports-payroll-export/05-CONTEXT.md` — Excel/PDF export contracts; reports E2E must match.
- `.planning/phases/07-facial-enrollment-sync/07-CONTEXT.md` — Enrollment flow; if test depth extends to enrollment route, follow these decisions.

### Project conventions
- `CLAUDE.md` (project root) — RBAC architecture (Admin / Supervisor / Viewer JWT roles), Filesystem-root injection convention (drives D-11 path strategy), Test Coverage section (Phase 8 gate Phase 9 must not regress), pinned action policy (`actions/checkout@v4`, `actions/upload-artifact@v4`, etc.), 14-day artifact retention.
- `.planning/codebase/STACK.md` — Frontend (Next.js 15, React 19, TanStack Query v5, shadcn/ui, Tailwind 4) and backend (Axum 0.8, Tokio, libSQL) versions. Playwright must run against these.
- `.planning/codebase/STRUCTURE.md` — Repo layout. New `frontend/e2e/` directory must follow project structure conventions.
- `.planning/codebase/INTEGRATIONS.md` — Existing integration patterns (Cloudflare tunnel, Turso sync). Tests must avoid hitting external services.

### CI infrastructure
- `.github/workflows/ci.yml` — Existing Phase 8 CI workflow (Backend Coverage + Frontend Coverage jobs). Phase 9 adds an `E2E Tests` job to this same file, preserving the pinned actions and `permissions: contents: read` (T-08-15 least privilege).

### Implementation surface
- `frontend/package.json` — Existing deps (vitest 4.1.5, msw 2.7.0, @testing-library/react). Playwright is **not** installed; planner adds it as a devDependency.
- `frontend/src/app/` — Concrete route inventory: `(dashboard)/{audit,dashboard,devices,employees,enrollment,reports,settings,timesheet}`, `login/`, `setup/`. E2E specs map 1:1.
- `backend/src/state/paths.rs` — Filesystem-root struct that backend reads at startup. Tests pass test-scoped paths via env per CLAUDE.md convention.
- `backend/src/license/` — License module (fingerprint + service). Planner adds the bypass env-flag check here, gated by `CRONOMETRIX_E2E=true`.

### External docs (read on demand)
- Playwright official docs (`https://playwright.dev/docs/intro`) — webServer config, globalSetup, storageState, fixture model. Up to the planner / researcher to pull specific sections.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Phase 8 CI workflow (`.github/workflows/ci.yml`)** — Established pattern: pinned actions, `permissions: contents: read`, `actions/upload-artifact@v4` with retention. Phase 9 E2E job extends this same file rather than adding a parallel workflow.
- **`frontend/vitest.config.ts`** — Already excludes `src/app/**` from coverage (Phase 8 D-10). E2E suite is the formal complement; do not modify the Vitest config.
- **MSW (already installed at `msw 2.7.0`)** — NOT used for Phase 9 E2E (D-10 mandates real backend). Reserved for unit/integration testing.
- **`backend/src/state/paths.rs` `Paths::for_test(tempdir)`** — Already supports test-scoped roots. E2E `globalSetup` mirrors this pattern with env vars.
- **Existing argon2id password-hash code in backend** — Reused by `globalSetup` seed script (or invoked via a tiny `cargo run --bin seed-e2e-users` helper).

### Established Patterns
- **Pinned-action CI policy (Phase 8 T-08-15)** — `actions/checkout@v4`, `actions/setup-node@v4`, `actions/upload-artifact@v4`, etc. Phase 9 E2E job follows the same pinning + least-privilege permissions.
- **Filesystem-root injection (Phase 8 convention)** — All filesystem roots flow through `state.paths.*` from env vars. E2E test process injects test-only roots; never touches dev/prod paths.
- **Spanish UI strings + accessible-role-first selectors** — Pencil mockups (memory: `reference_pen_designs.md`) define copy. Tests prefer roles/test-ids over text matchers to stay localization-tolerant.
- **Hard-fail CI gate (Phase 8 D-13)** — No soft-warn, no override label. E2E job inherits this.

### Integration Points
- New file: `frontend/playwright.config.ts` — `webServer`, `projects: [{ name: 'chromium' }]`, `globalSetup`, `globalTeardown`, `use.storageState` (per-project or per-spec).
- New directory: `frontend/e2e/{login,dashboard,timesheet,employees,devices,reports,audit}.spec.ts`, plus `frontend/e2e/fixtures/`, `frontend/e2e/global-setup.ts`, `frontend/e2e/global-teardown.ts`, `frontend/e2e/.auth/` (gitignored).
- Modified: `.github/workflows/ci.yml` — add `E2E Tests` job. After local validation, branch protection on `main` adds `E2E Tests` as required status check (manual GitHub UI step, like Phase 8 Plan 05's deferred "Manual Follow-up").
- Modified: `.gitignore` — add `frontend/e2e/.auth/` and `playwright-report/` and `test-results/`.
- Modified: `frontend/package.json` — add `@playwright/test` devDependency, `e2e` script, `e2e:install` script.
- Modified: `backend/src/license/service.rs` (or equivalent) — add `CRONOMETRIX_LICENSE_BYPASS` env-flag check, gated by `CRONOMETRIX_E2E=true`. Production startup aborts if the bypass flag is set without the e2e flag.
- Modified: `CLAUDE.md` — append a "Phase 9 E2E" subsection documenting the test-only env flags, the `frontend/e2e/` layout, and how to run `make e2e` locally.
- Possibly modified: select Next.js components — add `data-testid` attributes where they don't already exist (planner enumerates).

</code_context>

<specifics>
## Specific Ideas

- The pre-Phase-8 frontend already has full coverage gates; Phase 9 must not perturb them. Phase 9 is **additive**: new files, new CI job, no Vitest config changes.
- The audit-page test (D-04) directly enforces the project's "every mutation generates an immutable audit log entry" non-negotiable from CLAUDE.md. Treat it as load-bearing, not optional.
- The `globalSetup` seed script is the single source of truth for users / departments / sample employees / sample devices. Tests do NOT create users via API; they assume seeded fixtures.
- "Full UAT" is the user's deliberate choice (D-01..D-04). The planner sizes plans to deliver ~50+ tests — splitting into multiple PLAN.md files (e.g., `09-01-tooling`, `09-02-fixtures`, `09-03-login-spec`, `09-04-dashboard-spec`, `09-05-crud-specs`, `09-06-audit-spec`, `09-07-ci-gate`) is encouraged for parallelism.
- D-13 (license bypass): the planner MUST add a unit/integration test in the backend asserting that `CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true` aborts startup. This prevents the test-only flag from leaking to production.

</specifics>

<deferred>
## Deferred Ideas

- **Cross-browser matrix (Firefox, WebKit)** — Add as a follow-up phase only if a customer requests it. Cronometrix is on-premise admin tooling on Chrome/Edge.
- **Visual regression / screenshot diffs** — Defer to a future phase if and when the design system stabilizes. Today's Phase 4 mockups are still evolving.
- **Mobile / responsive E2E** — Phase 4 D-3 locked desktop-only ≥1280px. Mobile is a separate roadmap item.
- **Real Hikvision device tests** — Manual QA only. Automated E2E uses the mock device layer (D-14).
- **Turso cloud-sync E2E** — Offline replica behavior, conflict resolution, and remote dashboard access deserve a dedicated phase.
- **E2E for `/setup` and `/setup/license` flows** — Owned by Phase 6 (installer). Out of Phase 9 scope unless ROADMAP changes.
- **E2E for `/(dashboard)/settings` and `/(dashboard)/enrollment`** — Defer; settings is mostly read-only and enrollment was finalized in Phase 7. Add later if regressions appear.
- **Performance / load-testing E2E** — Separate concern (k6, Lighthouse CI) for a future phase.

### Reviewed Todos (not folded)
None.

</deferred>

---

## Addendum (2026-04-28) — Resolution of Research-Surfaced Conflicts

The researcher (09-RESEARCH.md) identified three CONTEXT.md decisions in conflict with current codebase state. User-confirmed resolutions:

### D-04 Audit Screen → Build in Wave 0 (LOCKED)
Backend has no `/api/v1/audit` endpoint and `frontend/src/app/(dashboard)/audit/page.tsx` is a 10-line "Próximamente" placeholder. Phase 9 Wave 0 MUST add:
- Backend: `GET /api/v1/audit` paginated read endpoint (filter by user, date range; RBAC: Admin + Supervisor read; Viewer 403). Reads from existing `audit_log` table.
- Frontend: replace placeholder `audit/page.tsx` with TanStack Table-based audit list (filter by user/date, sortable, paginated). data-testid attributes added.
- Then D-04 tests (audit log lists immutable entries; filter by user/date works) become viable.

### D-14 Hikvision Integration → Mock alertStream Source (LOCKED, supersedes original D-14)
Original D-14 wording mentioned `/api/v1/webhooks/hikvision`. That endpoint does NOT exist. The actual integration is **outbound alertStream polling** (`backend/src/isapi/stream.rs` connects to `/ISAPI/Event/notification/alertStream` on the device). Phase 9 mock device:
- Test-only Axum mock server impersonates a Hikvision unit on `localhost:${MOCK_DEVICE_PORT}`.
- Mock serves `GET /ISAPI/Event/notification/alertStream` as a multipart streaming response with canned `EventNotificationAlert` XML chunks (and optional JPEG payload), with digest auth handshake.
- Mock also serves outbound endpoints used by enrollments + door open + status check (`/ISAPI/AccessControl/UserInfo/Record`, `/ISAPI/Intelligent/FDLib/FaceDataRecord`, `/ISAPI/AccessControl/UserInfoDetail/Delete`, `/ISAPI/RemoteControl/door/0`, `/ISAPI/System/status`).
- Tests inject events by pushing XML chunks into the mock's stream queue (HTTP API on the mock or a shared file/channel).
- No new production webhook route is added.

### D-19 Login UI Language → Test English Copy Now (LOCKED, supersedes original D-19 for login screen only)
`frontend/src/app/login/page.tsx` currently uses English ("Username", "Password", "Log in", error strings). Phase 9 tests the **current English copy** of the login form. Spanish i18n for the login screen is deferred to a future phase. Other dashboard routes (Marcaciones, Empleados, Dispositivos, Reportes, Auditoría, KPI tile labels) ARE Spanish today and tests assert Spanish copy where load-bearing.

These three resolutions DO expand Phase 9 scope (audit endpoint + UI in Wave 0). Planner sizes additional plans accordingly.

---

*Phase: 09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea*
*Context gathered: 2026-04-28*
