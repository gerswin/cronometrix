<!-- GSD:project-start source:PROJECT.md -->
## Project

**Cronometrix**

Cronometrix is a biometric time & attendance product for businesses using Hikvision facial recognition devices. It runs on-premise at each client site, connects to up to 4 biometric readers, calculates work hours with configurable tolerance rules, and syncs data to Turso cloud for remote access and backup. Built as a commercial product — each installation is independent.

**Core Value:** Accurate, auditable time tracking that turns raw biometric events into payroll-ready data — with zero manual calculation and full legal traceability.

### Constraints

- **Tech stack (backend):** Rust with Axum — performance-critical for real-time webhook processing and time calculations
- **Tech stack (frontend):** React/Next.js with TypeScript — mature ecosystem for data-heavy admin screens
- **Tech stack (database):** SQLite (local) + Turso (cloud sync) via libSQL — local-first architecture
- **Hardware dependency:** Must support Hikvision ISAPI protocol — this is non-negotiable
- **Audit compliance:** Every mutation to attendance records must generate an immutable audit log entry with justification
- **Desktop option (future):** Architecture should allow wrapping in Tauri later for desktop deployment
- **Deployment:** Docker Compose on Linux servers, one-command install via shell script
- **Licensing:** Hardware-bound via DO Functions — prevents unauthorized cloning across servers
- **Network access:** Cloudflare tunnel per client → `{client-slug}.cronometrix.com`
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
### Core Technologies
| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.77+ (stable) | Backend runtime | Memory-safe, zero-cost async, ideal for high-throughput webhook processing and time calculation logic |
| Axum | 0.8.x (0.8.8 current) | HTTP server framework | Tokio-native, Tower middleware ecosystem, ergonomic extractors; announced 0.8.0 in Jan 2025 with breaking changes from 0.7 |
| Tokio | 1.51.x | Async runtime | The de-facto standard, used by Axum/libSQL/reqwest; no realistic alternative for this stack |
| Next.js | 15.x | Frontend framework | App Router stable, SSR/SSG for admin dashboard, TypeScript-first; React 19 compatible |
| React | 19.x | UI library | Peer of Next.js 15; concurrent features (Suspense, Server Components) beneficial for real-time dashboard |
| TypeScript | 5.x | Frontend type safety | Required for TanStack Table, shadcn/ui, and Zod schema sharing |
### Backend — Rust Crate Ecosystem
| Crate | Version | Purpose | Why This One |
|-------|---------|---------|--------------|
| `axum` | 0.8.8 | HTTP server, routing, extractors | Tokio-native, best-in-class ergonomics; Tower integration gives CORS, tracing, compression for free |
| `tokio` | 1.51.1 | Async runtime | Required by axum; `features = ["full"]` for dev, trim for production |
| `tower-http` | 0.6.x | Middleware (CORS, tracing, compression, timeout) | Standard Tower middleware collection; use `features = ["cors", "trace", "compression-gzip", "timeout"]` |
| `libsql` | latest | SQLite local + Turso cloud sync | Official SDK from Turso; `Builder::new_remote_replica()` for embedded replica mode |
| `reqwest` | 0.13.2 | ISAPI HTTP client (outbound) | Most popular Rust HTTP client; async, TLS, connection pooling |
| `diqwest` | latest | Digest auth for ISAPI | Extends reqwest with RFC 2617 digest auth flow — Hikvision devices require digest auth |
| `quick-xml` | 0.39.x | XML parsing for ISAPI events | 10x faster than serde-xml-rs; serde integration via `features = ["serialize"]` |
| `serde` | 1.0.x | Serialization/deserialization | Universal; `features = ["derive"]` |
| `serde_json` | 1.0.x | JSON API request/response bodies | Required by axum's Json extractor |
| `jsonwebtoken` | 10.3.0 | JWT creation and validation | Standard Rust JWT library; supports HS256/RS256; use for auth tokens |
| `argon2` (RustCrypto) | 0.5.x | Password hashing | OWASP-recommended over bcrypt; `argon2id` variant; from `RustCrypto/password-hashes` |
| `chrono` | 0.4.42 | Date/time arithmetic | Attendance time calculations (tolerance windows, shift detection, lunch deduction) |
| `uuid` | 1.x | ID generation | UUID v4 for records; audit log IDs |
| `tracing` | 0.1.x | Structured logging | Tokio ecosystem standard; pairs with `tracing-subscriber` |
| `tracing-subscriber` | 0.3.x | Log output formatting | JSON output for production, pretty for dev |
| `anyhow` | 1.x | Error handling in application code | Ergonomic error propagation; use `thiserror` for library boundaries |
| `thiserror` | 2.x | Typed errors for axum handlers | Derive `IntoResponse` on custom error types |
| `validator` | 0.19.x | Request payload validation | Derive-based validation macros; combine with serde |
| `dotenv` / `dotenvy` | latest | Environment config | `dotenvy` is the maintained fork; for TURSO_DATABASE_URL, TURSO_AUTH_TOKEN |
### Frontend — React/Next.js Ecosystem
| Library | Version | Purpose | Why This One |
|---------|---------|---------|--------------|
| `@tanstack/react-table` | v8.x | Data tables (employee list, timesheet, audit log) | Headless — no UI lock-in; virtualizes large datasets; server-side sort/filter/paginate built-in |
| `@tanstack/react-query` | v5.x | Server state management, cache, background refetch | Industry standard; SSR hydration support for Next.js App Router; replaces ad-hoc fetch + useState |
| `shadcn/ui` | latest (copy-paste) | Component library (forms, dialogs, dropdowns) | You own the code — no upgrade breaking; built on Radix UI + Tailwind; pairs perfectly with TanStack Table |
| `tailwindcss` | 4.x | Utility CSS | Required by shadcn/ui; v4 released early 2025 with Vite-native engine |
| `react-hook-form` | 7.x | Form state management | Zero re-renders on input; uncontrolled components; integrates with Zod via `@hookform/resolvers` |
| `zod` | 3.x | Schema validation + TypeScript types | Single schema definition for form validation AND type inference; share with backend via contract |
| `react-big-calendar` | 1.x | Holiday/shift calendar UI | Free, MIT, gcal-style views; month/week/day/agenda; drag-and-drop for holiday config |
| `recharts` | 3.x | Dashboard charts (KPIs, attendance trends) | v3 released mid-2025 with improved accessibility and TypeScript; built on D3; MIT |
| `@tanstack/react-virtual` | v3.x | Virtualized lists (large employee/event tables) | Pairs with TanStack Table; renders only visible rows for 10k+ record tables |
| `date-fns` | 3.x | Date formatting/arithmetic | Tree-shakeable; use as react-big-calendar localizer; consistent with backend chrono semantics |
| `axios` | latest | HTTP client to Rust backend | OR use native fetch — for CRUD calls, TanStack Query manages the cache layer |
| `next-auth` / `jose` | latest | Session management | `jose` for JWT verification in Next.js middleware; or `next-auth` v5 if SSO is needed later |
| `lucide-react` | latest | Icon set | Default for shadcn/ui; consistent, tree-shakeable |
| `xlsx` | latest | Excel export (pre-payroll reports) | Client-side Excel generation; use `SheetJS/xlsx` community edition |
| `jspdf` + `jspdf-autotable` | latest | PDF export (reports, audit trails) | Client-side PDF; autotable for tabular data |
### Development Tools
| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo-watch` | Auto-recompile Rust on save | `cargo watch -x run` |
| `sqlx-cli` | Migration management | Use even with libSQL — migration files stay compatible |
| `cargo-nextest` | Faster test runner for Rust | Drop-in replacement for `cargo test` |
| `Biome` | Linter + formatter for TypeScript | Replaces ESLint + Prettier in one tool; much faster |
| `Vitest` | Frontend unit/integration tests | Native ESM, Vite-powered; pairs with React Testing Library |
| `Bruno` / `Postman` | ISAPI endpoint testing | Bruno preferred (local, git-friendly) |
## Installation
### Rust Backend
# Cargo.toml
### Frontend
# Scaffold
# Core
# shadcn/ui (interactive installer)
# Dev dependencies
## Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Axum version | 0.8.x | 0.7.x | 0.7.x is EOL; 0.8.0 released Jan 2025 with breaking changes — migrate forward, not backward |
| ORM | raw libSQL queries | SeaORM, sqlx | libSQL crate IS the right abstraction layer for Turso sync; adding SeaORM creates an unnecessary wrapper that fights the embedded replica API |
| XML parsing | `quick-xml` | `serde-xml-rs` | `quick-xml` is 10x faster AND supports serde derives; `serde-xml-rs` is slower with no advantage |
| Password hashing | `argon2` (RustCrypto) | `bcrypt` | OWASP recommends argon2id over bcrypt; bcrypt has 72-byte password limit; argon2 is PHC winner |
| Form library | `react-hook-form` | Formik | RHF is uncontrolled (zero re-renders); Formik is controlled and slow on large forms like timesheet editor |
| Calendar | `react-big-calendar` | FullCalendar premium | react-big-calendar is fully MIT; FullCalendar premium features require paid license (overkill for holiday config use case) |
| Charts | Recharts | Tremor | Tremor is built on Recharts anyway; direct Recharts gives more control for custom attendance trend visualizations |
| State management | TanStack Query | Redux Toolkit | RTK is for client-side state; attendance data is server state — TanStack Query is the correct tool |
| Auth JWT | `jsonwebtoken` crate | `axum-jwt-auth` | `axum-jwt-auth` wraps `jsonwebtoken`; building your own extractor from `jsonwebtoken` gives RBAC flexibility without opinionated wrapper constraints |
| HTTP client | `reqwest` | `hyper` directly | Reqwest wraps hyper with ergonomic API; direct hyper only needed if you need extreme raw control |
| Digest auth | `diqwest` | manual implementation | `diqwest` correctly handles the challenge-response flow for digest auth (401 → parse WWW-Authenticate → retry with computed MD5) |
## What NOT to Use
| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `diesel` ORM | Synchronous only; fights Tokio's async model; migration from diesel to async-compatible layer is painful | Raw `libsql` queries with typed structs |
| `actix-web` | Not wrong, but Axum is the tokio-team's official framework and has better Tower integration for this stack | `axum` 0.8 |
| `warp` | Unmaintained/stagnant; the trait-based approach is harder to extend | `axum` 0.8 |
| `moment.js` (frontend) | 300KB bloated, deprecated by maintainers | `date-fns` 3.x (tree-shakeable) |
| `react-query` v3/v4 | Older API; v5 has improved TypeScript inference and streaming/suspense for App Router | `@tanstack/react-query` v5 |
| `axios` for background fetching | TanStack Query manages caching/refetch — don't bypass it with raw axios calls outside query functions | TanStack Query `queryFn` wrapping fetch/axios |
| `react-table` v7 | v7 is deprecated; completely different API from TanStack Table v8 | `@tanstack/react-table` v8 |
| `emotion` / `styled-components` | Runtime CSS-in-JS conflicts with Next.js App Router RSC (React Server Components); Tailwind has no runtime | `tailwindcss` 4.x |
| `next-auth` v4 | v4 was designed for Pages Router; v5 is the App Router-compatible version | `next-auth` v5 (Auth.js) OR custom JWT middleware |
| Global Rust state with `Mutex<HashMap>` | Race conditions under concurrent webhook bursts; device state should live in DB | SQLite as single source of truth for device state |
## ISAPI Integration Patterns
### Inbound: Device pushes events to Cronometrix (attendance webhooks)
- `EventNotificationAlert` XML block (attendance event, employee ID, face capture time)
- Optional binary JPEG (face photo at the moment of capture)
### Outbound: Cronometrix sends commands to devices (door open, enrollment, sync profiles)
- `PUT /ISAPI/AccessControl/UserInfo/SetUp` — enroll employee face profile
- `PUT /ISAPI/RemoteControl/door/0` — remote door open
- `GET /ISAPI/System/status` — device health check
- `POST /ISAPI/Event/notification/httpHosts` — configure webhook listener URL
## Authentication & RBAC Architecture
### Backend (Rust/Axum)
- Issue JWT on login (`jsonwebtoken` crate, HS256, secret from env)
- Claims include: `sub` (user_id), `role` (Admin | Supervisor | Viewer), `exp`, `iat`
- Axum extractor validates JWT from `Authorization: Bearer <token>` header
- Role enforcement via Tower middleware layer applied per router group:
- Password stored as `argon2id` hash; verify on login
### Frontend (Next.js)
- Store JWT in `httpOnly` cookie (XSS-safe) or memory (for SPA)
- Next.js middleware reads cookie, redirects unauthenticated requests
- TanStack Query attaches Bearer token via `defaultOptions.queries.queryFn` wrapper
- Role-based UI gating via React context (derived from decoded JWT claims)
- No server-side RBAC enforcement in Next.js — backend is authoritative
## Stack Patterns by Variant
- Keep Axum running as a sidecar process, or migrate to Tauri's Rust backend commands
- `tauri-plugin-libsql` exists for direct libSQL in Tauri (see DEV.to article, MEDIUM confidence)
- Avoid embedding business logic in Next.js Server Actions — keep it in Rust so Tauri migration is smooth
- Add Server-Sent Events (SSE) endpoint in Axum — simpler than WebSockets for one-directional push
- TanStack Query `refetchInterval` is adequate for polling (every 5s) as a starting point
- Upgrade to SSE when polling feels laggy in production
- Tokio handles concurrent async tasks natively — no additional work needed
- Ensure `libsql` connection is shared via `Arc<Database>` in Axum state
- SQLite WAL mode enabled by default in libSQL embedded replicas
## Version Compatibility
| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `axum` 0.8.x | `tower-http` 0.6.x | tower-http 0.5.x is for axum 0.7; must use 0.6 with 0.8 |
| `axum` 0.8.x | `tokio` 1.x | Any tokio 1.x works |
| `libsql` | `tokio` 1.x | async-first, requires tokio runtime |
| `reqwest` 0.13.x | `tokio` 1.x | Use `rustls-tls` feature to avoid OpenSSL system dependency |
| Next.js 15 | React 19 | Next.js 15 requires React 19; `@tanstack/react-query` v5 is compatible |
| `@tanstack/react-table` v8 | `@tanstack/react-virtual` v3 | Must use matching major versions |
| `tailwindcss` 4.x | Next.js 15 | Tailwind 4 uses a different config format; shadcn/ui supports it |
| `react-big-calendar` | `date-fns` 3.x | Use date-fns as localizer; moment.js localizer is deprecated |
| `jsonwebtoken` 10.x | N/A | Breaking change from 8.x: encoding/decoding API changed |
## Sources
- [Axum 0.8.8 docs.rs](https://docs.rs/axum/latest/axum/) — version confirmed HIGH confidence
- [Tokio 1.51.1 docs.rs](https://docs.rs/tokio/latest/tokio/) — version confirmed HIGH confidence
- [reqwest 0.13.2 docs.rs](https://docs.rs/reqwest/latest/reqwest/) — version confirmed HIGH confidence
- [jsonwebtoken 10.3.0 docs.rs](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/) — version confirmed HIGH confidence
- [Turso Rust Quickstart](https://docs.turso.tech/sdk/rust/quickstart) — embedded replica API verified HIGH confidence
- [Turso Offline Sync Beta](https://turso.tech/blog/turso-offline-sync-public-beta) — Rust support confirmed MEDIUM confidence
- [Axum 0.8.0 Announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) — release date verified HIGH confidence
- [Hikvision ISAPI Event Listening](https://www.hikvisioneurope.com/eu/portal/portal/Technology%20Partner%20Program/03-How%20to/How%20to%20get%20real-time%20event%20in%20listening%20mode.pdf) — multipart event format MEDIUM confidence
- [Hikvision TPP Integration Center](https://tpp.hikvision.com/) — digest auth requirement MEDIUM confidence
- [diqwest crate](https://docs.rs/diqwest) — digest auth reqwest extension MEDIUM confidence
- [quick-xml performance comparison](https://capnfabs.net/posts/parsing-huge-xml-quickxml-rust-serde/) — 10x perf advantage MEDIUM confidence
- [TanStack Table v8](https://tanstack.com/table/latest) — current version HIGH confidence
- [TanStack Query v5](https://tanstack.com/query/latest) — SSR/App Router support HIGH confidence
- [shadcn/ui Data Table docs](https://ui.shadcn.com/docs/components/radix/data-table) — TanStack Table integration HIGH confidence
- [Recharts v3 changelog](https://blog.logrocket.com/best-react-chart-libraries-2025/) — v3 release mid-2025 MEDIUM confidence
- [RustCrypto password-hashes](https://github.com/RustCrypto/password-hashes) — argon2 crate HIGH confidence
- [chrono 0.4.42](https://crates.io/crates/chrono) — version MEDIUM confidence (from WebSearch)
- [react-hook-form + Zod shadcn guide](https://ui.shadcn.com/docs/forms/react-hook-form) — canonical pattern HIGH confidence
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

<!-- Phase 8 D-23 — DO NOT remove on conventions sync; this rule is a binding code convention, not a placeholder. -->
### Filesystem-root injection (Phase 8)

Code that needs a filesystem root (evidence dir, photo dir, override dir, kiosk
capture tmp) MUST read it from `state.paths.<field>` — never via
`std::env::var(...)` at use-site, and never via `PathBuf::from("./data/…")`.

The `Paths` substruct on `AppState` (`backend/src/state/paths.rs`) is populated
once at startup by `Paths::from_env()` and overridden in tests via
`Paths::for_test(tempdir)`. This eliminates cwd-dependence (tests failing
because they run from a different directory) and the env-var process-global
race (parallel tests clobbering each other's env vars).

| Path field | Env var | Default |
|-----------|---------|---------|
| `leaves_root` | `CRONOMETRIX_LEAVES_ROOT` | `./data/leaves` |
| `events_root` | `CRONOMETRIX_EVENTS_ROOT` | `./data/events` |
| `enrollments_root` | `ENROLLMENTS_DIR` | `./data/enrollments` |
| `captures_tmp_root` | `CRONOMETRIX_CAPTURES_TMP` | `/tmp/enrollments-captures` |
| `overrides_root` | `DATA_DIR` (joined with `overrides`) | `./data/overrides` |

Tests must use `common::test_state_with_tmpdir(db, config)` (returns
`(AppState, TempDir)`) and bind the returned `TempDir` to a local variable that
outlives the test's assertions — see `backend/tests/common/mod.rs`.
<!-- GSD:conventions-end -->

## Test Coverage

Phase 8 established hard-failing coverage jobs. Every PR to `main` runs the
same checks documented below; Phase 13 separately proves branch protection
blocks merge when a required threshold is missed.

### Install (one-time per developer)

```bash
# Backend coverage tooling (cargo-llvm-cov is a tool, NOT a Cargo dependency)
cargo install cargo-llvm-cov --locked --version 0.8.5

# Nightly Rust is required for branch coverage (--branch is unstable on stable rustc).
# The repo's rust-toolchain.toml pins a specific nightly date; rustup honors it
# automatically. To install that exact toolchain explicitly:
NIGHTLY=$(grep '^channel' rust-toolchain.toml | sed 's/.*"\(.*\)".*/\1/')
rustup toolchain install "$NIGHTLY" --component llvm-tools-preview

# Frontend coverage tooling is already installed
# (vitest + @vitest/coverage-v8 in frontend/package.json)
nvm use && npm install --global npm@11.12.1 && cd frontend && npm ci
```

Frontend installs are pinned to Node `24.15.0` and npm `11.12.1` via
the root `.nvmrc`, `package.json` engines, and `packageManager`. CI and the web
image must use the same pair and `npm ci`; a lockfile mismatch is a hard failure.

The pinned nightly is currently `nightly-2026-04-01`. Bump cadence is quarterly
(or earlier if nightly introduces an ICE / strict lint that blocks CI). Bump =
update `rust-toolchain.toml` + verify `make coverage-backend` still green.

### Local commands

```bash
make coverage           # Backend + frontend; both must pass
make coverage-backend   # Backend only (cargo-llvm-cov + per-file enforcer)
make coverage-frontend  # Frontend only (Vitest --coverage)
make test-ci-config     # Verify every setup-node version-file exists
```

The same coverage commands run in CI (`.github/workflows/ci.yml`), so local
green is the required pre-push evidence. It does not prove the live GitHub run
or branch protection; Phase 13 records those external results.

### Thresholds

| Side | Scope | Lines | Branches | Functions | Statements |
|------|-------|-------|----------|-----------|------------|
| Backend | Project-wide | >=90% | >=85% | — | — |
| Backend | Per file | >=70% | >=60% | — | — |
| Frontend | Project-wide | >=90% | >=85% | >=90% | >=90% |
| Frontend | Per file | >=70% | >=60% | >=70% | >=70% |

Thresholds are fixed (no ratchet): the gate compares against the threshold,
not against a stored baseline. A PR that drops coverage from 95% to 91%
passes; from 91% to 89% fails.

Backend project-wide line gate is enforced by `cargo llvm-cov nextest
--fail-under-lines 90`; backend project-wide branch gate + per-file floor are
enforced by `scripts/enforce-coverage-floor.sh lcov.info 85 70 60` (project
branch min / per-file line min / per-file branch min). Frontend gates are
enforced natively by Vitest from `frontend/vitest.config.ts`.

### Exclusion policy

Exclusions are minimal — write tests, don't shrink the denominator. Adding a
new exclusion requires a written justification in this section. The current
exclusions are:

| Side | Path / regex | Justification |
|------|--------------|---------------|
| Backend | `main.rs` | Tokio runtime startup; not unit-testable in this phase |
| Backend | `tests/common/*` | Test infrastructure — covering test fixtures inflates the denominator without security value |
| Frontend | `src/components/ui/**` | Vendored shadcn copies; covered upstream (D-10) |
| Frontend | `src/components/providers.tsx` | D-09: pure QueryClientProvider wrapper, no logic |
| Frontend | `src/components/layout/top-bar.tsx` | D-09: pure display, no logic |
| Frontend | `src/components/common/access-restricted.tsx` | D-09: pure display, no logic |
| Frontend | `src/app/**` | Next.js route pages; not in the coverage `include` set — covered by E2E (out of scope for Phase 8 per CONTEXT D-10) |
| Frontend | `src/**/*.test.{ts,tsx}` and `*.spec.{ts,tsx}` | Test files |
| Frontend | `src/**/__tests__/**` | Test fixtures and helpers |
| Frontend | `src/**/*.d.ts` | Type-only files; no executable code |

The frontend coverage `include` array is whitelist-style (`src/components/**`,
`src/hooks/**`, `src/lib/**`) — anything outside these globs is implicitly
excluded. The three D-09 file-specific exclusions above were added during
Plan 04C because the modules are pure-display wrappers with no branchable
logic; the exclusions appear in `frontend/vitest.config.ts`.

See
`.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04C-SUMMARY.md`
for the case-by-case justifications. If you find yourself wanting to add a new
exclusion, write the test instead — exclusions cap at 3 per side without an
explicit re-discussion.

Backend note (macOS dev): `backend/src/license/fingerprint.rs` and
`backend/src/license/service.rs` cannot reach the per-file floor on macOS
because they read `/proc/cpuinfo` and `/sys/{class/net,block}` — pseudo-fs
that do not exist on Darwin. Linux CI under nightly measures both at full
coverage, and the gate passes there. macOS local runs are informational
when these two files FAIL the per-file floor; CI is authoritative.

### HTML reports

Local:
- Backend: `backend/target/llvm-cov/html/index.html`
- Frontend: `frontend/coverage/index.html`

CI: artifacts named `backend-coverage-html` and `frontend-coverage-html` are
attached to every workflow run (retention: 14 days). Download from the GitHub
Actions run page even when the gate is red — the report helps drill into the
failing file.

### CI gate

Workflow file: `.github/workflows/ci.yml`

Triggers: push to any branch, pull_request targeting `main`.

Coverage jobs (intended required checks; live protection is verified in Phase 13):
- `Backend Coverage` — installs nightly Rust + cargo-llvm-cov + cargo-nextest;
  runs `cargo llvm-cov nextest --branch --all-features --ignore-filename-regex
  '(main\.rs|tests/common/.*)' --fail-under-lines 90 --lcov --output-path
  lcov.info`, then `bash ../scripts/enforce-coverage-floor.sh lcov.info 85 70
  60`. Threshold miss makes the job red; verified Phase 13 protection then
  blocks merge.
- `Frontend Coverage` — reads Node `24.15.0` from the root `.nvmrc`, pins npm
  `11.12.1`, and runs `npx vitest run --coverage`.
  Vitest enforces both project-wide and per-file thresholds natively from
  `frontend/vitest.config.ts`.

Both jobs run with `permissions: contents: read` (least privilege per
threat model T-08-15) and pin actions (`actions/checkout@v4`,
`actions/setup-node@v4`, `actions/upload-artifact@v4`,
`taiki-e/install-action@v2`, `Swatinem/rust-cache@v2`,
`cargo-llvm-cov@0.8.5`).

The exclusion regex `(main\.rs|tests/common/.*)` is identical between
`Makefile` and `.github/workflows/ci.yml` — DO NOT change one without the
other; drift between local and CI scope makes the gate untrustworthy.

The hard-fail behavior is locked-in (no soft-warn, no override label).
Aligns with the audit-compliance ethos of the product (D-13).

### Reading a failing run

1. Open the failing job's logs in the Actions tab.
2. For backend: the post-processor prints `FAIL: <file> line coverage X% < floor 70%`
   (or branch). Click the file in the HTML artifact to see uncovered lines.
3. For frontend: Vitest prints a threshold table per file; uncovered lines are
   highlighted in the HTML report.
4. Add tests to bring the file above the floor. Don't add an exclusion unless
   the file is genuinely uncoverable in this phase.

### Note on private vs public repo

HTML reports include source code excerpts. The repo is currently private, so
artifacts are scoped to repo collaborators. If the repo ever goes public,
revisit the artifact retention policy and consider scrubbing sensitive
comment patterns from the HTML output.

### Pending live validation (Plan 05 deferred)

Plan 05 (CI gate) shipped the workflow file but the live runtime
validation was deferred per user direction. Three checklist items remain
in
`.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-05-SUMMARY.md`
under "Manual Follow-up":

1. **Positive verification** — push the branch, confirm both jobs pass green
   on GitHub Actions, confirm HTML artifacts are downloadable.
2. **Negative regression PR** — open a deliberate red PR (add an untested
   `dead_code.rs`), confirm `Backend Coverage` FAILS at the post-processor
   step with `FAIL: backend/src/dead_code.rs line coverage 0.00% < floor 70%`,
   then close the PR.
3. **Branch protection** — in GitHub UI (Settings → Branches), require
   `Backend Coverage` and `Frontend Coverage` as status checks before merge to
   `main`.

Phase 8 is NOT considered fully green until A, B, and C all pass on the live
GitHub Actions runner with branch protection active. Anyone resuming this work
should consult `08-05-SUMMARY.md` for the exact commands.

## End-to-End Tests (Phase 9)

Phase 9 added a Playwright-based end-to-end test suite that runs against the
real Rust/Axum backend (boot via Playwright `webServer`, ephemeral SQLite,
mock Hikvision device) and the real Next.js frontend. The suite is a hard-fail
gate on every PR via the `E2E Tests` job in `.github/workflows/ci.yml`.

### Install (one-time per developer)

```bash
cd frontend && npm ci
npx playwright install --with-deps chromium

# Build helper binaries (gated by Cargo features so prod Docker excludes them):
cd backend
cargo build --release --bin cronometrix
cargo build --release --bin mock_hikvision --features mock-hikvision
cargo build --release --bin seed_e2e --features seed-e2e
```

Or use the orchestrated path:
```bash
make e2e-install
make e2e-build
```

### Local commands

```bash
make e2e            # Build backend binaries + run full Playwright suite
make e2e-build      # Build only — no test execution
make e2e-install    # Install npm deps + chromium browser
cd frontend && npx playwright test --grep "<spec name>"  # Run a single spec
```

The same commands run in CI (`.github/workflows/ci.yml::e2e-tests`), so a
green `make e2e` locally implies a green `E2E Tests` job on PRs.

### Login language contract (Phase 12)

As of the **2026-07-13 Phase 12 supersession** of Phase 9 Addendum D-19,
`/login` is Spanish-authoritative. Its E2E contract locks `Iniciar Sesión`,
`Usuario`, `Contraseña`, `Mostrar contraseña` / `Ocultar contraseña`, both
Spanish error messages, the Spanish required-field message, and root
`<html lang="es-VE">`. Phase 9's English assertions remain historical
evidence only; current tests and operator guidance must use this contract.

### Test-only env flags (DEV/TEST ONLY — must NEVER appear in prod env)

| Flag | Purpose | Abort contract |
|------|---------|----------------|
| `CRONOMETRIX_E2E=true` | Gates the bypass flag, gates `__test_reset` route registration, gates `seed_e2e` + `mock_hikvision` binary execution | Must be `true` for any of the below to be honored |
| `CRONOMETRIX_LICENSE_BYPASS=true` | Skip hardware-fingerprint license validation | If set WITHOUT `CRONOMETRIX_E2E=true`, the binary aborts with exit code 2 BEFORE entering the license check path. Locked by `backend/tests/license_bypass_safety.rs::bypass_without_e2e_aborts_with_code_2`. |

The integration test means: a deploy script that sets `CRONOMETRIX_LICENSE_BYPASS`
in production env will FAIL FAST instead of silently disabling the license gate
(which would defeat LIC-05 anti-cloning). If you ever see `CRONOMETRIX_E2E` or
`CRONOMETRIX_LICENSE_BYPASS` in a production .env, treat it as a misconfiguration
and refuse to deploy.

### File layout

```
frontend/
├── playwright.config.ts                # webServer × 3, projects, TZ freeze
├── e2e/
│   ├── .auth/                          # gitignored — storageState files (regenerated per run)
│   │   ├── admin.json
│   │   ├── supervisor.json
│   │   └── viewer.json
│   ├── fixtures/
│   │   ├── api.ts                      # API helpers (getAudit, resetMutableTables, pushHikvisionEvent)
│   │   ├── selectors.ts                # data-testid catalog
│   │   ├── time.ts                     # Caracas-anchored Date helpers
│   │   └── hikvision-events/*.xml      # canned EventNotificationAlert fixtures
│   ├── setup/
│   │   ├── 00-build-and-seed.setup.ts  # health probe + seed_e2e + reset mutable tables
│   │   └── 01-authenticate.setup.ts    # login as 3 roles → write storageState files
│   ├── login.spec.ts                   # D-01 UAT (Spanish copy — Phase 12 supersedes D-19)
│   ├── dashboard.spec.ts               # D-02 UAT (KPIs, donut, ring buffer, photo, SSE)
│   ├── timesheet.spec.ts               # D-03 marcaciones CRUD + audit
│   ├── employees.spec.ts               # D-03 empleados CRUD + audit
│   ├── devices.spec.ts                 # D-03 dispositivos CRUD + ISAPI dispatch via mock + audit
│   ├── reports.spec.ts                 # D-03 reportes (XLSX + PDF content verification)
│   ├── audit.spec.ts                   # D-04 auditoría screen UAT
│   ├── rbac.spec.ts                    # cross-cut: viewer/supervisor/admin/anonymous
│   └── global-teardown.ts              # rm /tmp/cronometrix-e2e-${RUN_ID}*

backend/
├── src/
│   ├── audit/                          # NEW — paginated read endpoint (D-04 enabler)
│   ├── bin/
│   │   ├── seed_e2e.rs                 # Cargo feature: seed-e2e
│   │   └── mock_hikvision.rs           # Cargo feature: mock-hikvision
│   ├── license/service.rs              # extended with evaluate_bypass(e2e, bypass)
│   ├── main.rs                         # license-gate flow + __test_reset gating
│   └── test_reset/mod.rs               # POST /api/v1/__test_reset (gated by env)
├── tests/
│   ├── audit_handlers_test.rs          # 10 integration tests
│   ├── license_bypass_safety.rs        # locks the abort-on-misconfig contract
│   └── test_reset_gating.rs            # locks the 404-without-e2e contract
└── Cargo.toml                          # [features] seed-e2e, mock-hikvision

.github/workflows/ci.yml                # added "E2E Tests" job alongside Phase 8 jobs
```

### Default ports

| Process | Port | Notes |
|---------|------|-------|
| Backend (test) | 4001 | webServer probe at `/api/v1/health` |
| Frontend (test) | 3001 | next start (CI) or next dev (local) |
| Mock Hikvision (public) | 4400 | impersonates Hikvision unit; serves `/ISAPI/*` |
| Mock Hikvision (admin) | 4401 | test-only; specs push events into the alertStream queue |

Override via env vars: `SERVER_PORT`, `MOCK_HIKVISION_PORT`, `MOCK_HIKVISION_ADMIN_PORT`.

### Time-zone freeze (D-20)

`TZ=America/Caracas` is set in THREE places — all required:
1. Backend webServer process (via `webServer.env` in playwright.config.ts)
2. Test runner Node process (via `e2e-tests` job-level `env:` and local shell)
3. Browser context (`use: { timezoneId: 'America/Caracas' }` in playwright.config.ts)

Setting only one is a known flake source. If a test asserts dates and fails
intermittently, check all three places.

### CI gate

Workflow file: `.github/workflows/ci.yml`
Job name: **E2E Tests** (the intended required-status-check name; case-sensitive).

Job steps:
1. `actions/checkout@v4`
2. Validate CI file references
3. `actions/setup-node@v4` reads Node `24.15.0` from the root `.nvmrc`
4. Pin npm to `11.12.1`
5. Install the exact `rust-toolchain.toml` nightly + `Swatinem/rust-cache@v2` (target/ + cargo registry)
6. `npm ci`
7. `npm run build`
8. `npx playwright install --with-deps chromium`
9. `cargo build --release` for the 3 binaries (cronometrix, mock_hikvision, seed_e2e)
10. `npx playwright test`
11. `actions/upload-artifact@v4` × 2 — `playwright-html-report` + `playwright-test-results`, both `if: always()`, retention 14 days

Pinned actions: parity with Phase 8 T-08-15 (`actions/checkout@v4`,
`actions/setup-node@v4`, `actions/upload-artifact@v4`, `Swatinem/rust-cache@v2`).
`permissions: contents: read` at workflow scope (least privilege).

### Reading a failing CI run

1. Open the failing job's logs in the Actions tab.
2. Download the `playwright-html-report` artifact (always uploaded).
3. Open `index.html` locally — Playwright's HTML report shows traces, screenshots,
   videos for each failure.
4. Reproduce locally with `cd frontend && npx playwright test <spec>` against
   a fresh DB (`make e2e` rebuilds the binary + reseeds).
5. If the failure is in `setup/`: check that `backend/target/release/seed_e2e`
   and `mock_hikvision` exist (run `make e2e-build`).

### Pending live validation (carried forward to Phase 13)

Plan 12 (CI gate) shipped the workflow file, but the live runtime validation
was deferred per Phase 8 Plan 05 precedent. Three items remain:

1. **Positive verification** — push the branch, confirm `E2E Tests` runs green
   on GitHub Actions, confirm both artifacts are downloadable.
2. **Negative regression PR** — open a deliberate red PR (break a spec assertion);
   confirm `E2E Tests` FAILS and the artifacts include the failing trace.
3. **Branch protection** — Settings → Branches → branch protection rule for `main`
   → Require status checks → add `E2E Tests` to required list.

Phase 13 Plan 13-01 is the current executable owner of items 1-3. Phase 9's
historical code delivery is not live-gate proof. See
`.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-12-SUMMARY.md`
for exact commands.

### Note on private vs public repo

Playwright HTML reports include screenshots + DOM snapshots that may
contain seeded test names (Ana Pérez, Luis García, etc.). The repo is
currently private, so artifacts are scoped to repo collaborators. If the
repo ever goes public, revisit retention policy and consider scrubbing.
Since seed_e2e uses synthetic test data only (no real PII), the disclosure
risk is low — same disposition as Phase 8 coverage HTML.

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
