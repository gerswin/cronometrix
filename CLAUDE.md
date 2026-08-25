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

Pinned versions live in `backend/Cargo.toml` and `frontend/package.json` — read
those, not a copy. What follows is only the rationale the manifests can't carry.

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

Research provenance (source URLs + confidence ratings) is in
`.planning/research/STACK.md`.
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

Phase 8 established hard-failing coverage jobs on every PR to `main`. Install
steps, `make` targets, thresholds, CI job wiring, HTML reports, and how to read
a failing run are in the **`coverage-gate` skill** (`.claude/skills/coverage-gate/`).
The policy below stays here because adding an exclusion is a decision, not a
procedure.

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

The exclusion regex `(main\.rs|tests/common/.*)` is identical between
`Makefile` and `.github/workflows/ci.yml` — DO NOT change one without the
other; drift between local and CI scope makes the gate untrustworthy.

Backend note (macOS dev): `backend/src/license/fingerprint.rs` and
`backend/src/license/service.rs` cannot reach the per-file floor on macOS
because they read `/proc/cpuinfo` and `/sys/{class/net,block}` — pseudo-fs
that do not exist on Darwin. Linux CI under nightly measures both at full
coverage, and the gate passes there. macOS local runs are informational
when these two files FAIL the per-file floor; CI is authoritative.

## End-to-End Tests (Phase 9)

Playwright suite running against the real Rust/Axum backend and real Next.js
frontend; a hard-fail gate via the `E2E Tests` job. Install, `make` targets,
ports, CI wiring, and failure triage are in the **`e2e-tests` skill**
(`.claude/skills/e2e-tests/`). The three contracts below stay here because they
must be known before touching config, not only when running the suite.

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

### Time-zone freeze (D-20)

`TZ=America/Caracas` is set in THREE places — all required:
1. Backend webServer process (via `webServer.env` in playwright.config.ts)
2. Test runner Node process (via `e2e-tests` job-level `env:` and local shell)
3. Browser context (`use: { timezoneId: 'America/Caracas' }` in playwright.config.ts)

Setting only one is a known flake source. If a test asserts dates and fails
intermittently, check all three places.

### Login language contract (Phase 12)

As of the **2026-07-13 Phase 12 supersession** of Phase 9 Addendum D-19,
`/login` is Spanish-authoritative. Its E2E contract locks `Iniciar Sesión`,
`Usuario`, `Contraseña`, `Mostrar contraseña` / `Ocultar contraseña`, both
Spanish error messages, the Spanish required-field message, and root
`<html lang="es-VE">`. Phase 9's English assertions remain historical
evidence only; current tests and operator guidance must use this contract.

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->
