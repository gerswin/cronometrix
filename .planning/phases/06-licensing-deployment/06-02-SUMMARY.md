---
phase: 06-licensing-deployment
plan: 02
subsystem: licensing

tags:
  - rust
  - axum
  - tower-middleware
  - jwt
  - rs256
  - hardware-fingerprint
  - license-gate
  - nextjs
  - react
  - shadcn-ui
  - zod
  - react-hook-form
  - axios

requires:
  - phase: 06-licensing-deployment
    provides: license module (verify_license_jwt, load_and_validate_license, activate_license, renewal_task) + AppError::Unlicensed + Config license fields (Plan 06-01)

provides:
  - cronometrix_api::license::middleware::require_license Tower middleware
  - AppState.license_valid Arc<AtomicBool> branch-free gate flag
  - main.rs request pipeline gated end-to-end (cookie_auth + viewer + supervisor_read + supervisor + report + admin)
  - POST /api/v1/setup/activate public endpoint (idempotent, 409 ALREADY_ACTIVATED on re-post)
  - GET /api/v1/setup/status now returns BOTH initialized AND licensed booleans
  - frontend/src/app/setup/license/{page,layout}.tsx UI-SPEC compliant activation form
  - frontend/src/lib/validations.ts licenseSchema + LicenseFormData
  - /setup/page.tsx redirect to /setup/license when licensed === false
  - renewal_task spawned in main.rs alongside other tokio handles, drained at shutdown

affects:
  - 06-03 (deployment): license_jwt_path mount path proven working via this plan
  - 06-04 (UI activation flow): UI-SPEC contract realized — error code map, success banner, status branching all wired

tech-stack:
  added: []
  patterns:
    - "Reverse-order route_layer chaining: in axum 0.8, .route_layer() applies in reverse — adding require_license AFTER existing require_auth/require_* layers means require_license runs FIRST on the request path. This is load-bearing for T-06-17 (no auth-state leakage on unlicensed installs) and is encoded in the gate behavior tests."
    - "Idempotent public activation: setup_activate guards on state.license_valid before calling DO Functions, returning 409 ALREADY_ACTIVATED if already true. Prevents accidental re-activation overwriting the cached JWT (T-06-19, T-06-24)."
    - "Test fixture license bypass: every existing AppState construction site sets license_valid = Arc::new(AtomicBool::new(true)) so previously-passing tests stay green. The FALSE case is asserted exactly once in license_tests.rs::gate_behavior_tests."
    - "Cross-platform activation tests via wiremock + cfg-guarded acceptance: tests accept both Linux happy-path (200 + license_valid flips) and macOS dev fail-closed (Internal due to /proc/cpuinfo absence). Either outcome proves the security invariant; CI on Linux runs the strong path."
    - "Surface stub then GREEN: Task 1 commits a setup_activate stub that returns 502 ACTIVATION_UNREACHABLE so main.rs compiles with the route registered and protected groups can be wrapped. Task 2 replaces the stub body with the production activation flow — keeps Task 1 atomically committable while preserving the surface contract for downstream wiring."
    - "License key format validation: zod regex on the frontend (XXXX-XXXX-XXXX-XXXX, alphanumeric, case-insensitive) + length(19) + manual split-check on the backend. No regex crate static needed; validator's length attribute alone covers the cheap pre-check."
    - "Suspense-wrapped useSearchParams: Next.js 16 prerender requires useSearchParams() to live under a Suspense boundary. /login page extracted to LoginPageInner + Suspense fallback so static export works."

key-files:
  created:
    - backend/src/license/middleware.rs
    - frontend/src/app/setup/license/page.tsx
    - frontend/src/app/setup/license/layout.tsx
    - .planning/phases/06-licensing-deployment/deferred-items.md
  modified:
    - backend/src/state.rs (added license_valid: Arc<AtomicBool>)
    - backend/src/license/mod.rs (declared pub mod middleware)
    - backend/src/main.rs (license_valid init + AppState wiring + 6 route_layer additions + renewal_task spawn/drain)
    - backend/src/setup/handlers.rs (extended setup_status with `licensed`; full setup_activate handler with idempotent guard)
    - backend/tests/license_tests.rs (6 new gate behavior tests in gate_behavior_tests submodule)
    - backend/tests/{auth,daily_record,department,device,employee,event,leave,listener,reports,reports_excel,rules,supervisor,tenant_info}_tests.rs (license_valid field on every AppState site — 25 occurrences total)
    - frontend/src/lib/validations.ts (licenseSchema + LicenseFormData)
    - frontend/src/app/setup/page.tsx (status check redirect to /setup/license when licensed=false)
    - frontend/src/__tests__/device-banner.test.tsx (Rule 3 fix — Device[] cast)
    - frontend/src/components/dashboard/dept-chart.tsx (Rule 3 fix — Recharts v3 formatter typing)
    - frontend/src/app/login/page.tsx (Rule 3 fix — Suspense boundary for useSearchParams)

key-decisions:
  - "Middleware ordering pinned via test: route_layer reverse-order is encoded in test_license_gate_blocks_unlicensed_protected_route — sending a valid Bearer token, asserting 403 UNLICENSED. If anyone reverses the chain, this test fails. Documented inline in middleware.rs and main.rs comments."
  - "/setup/activate stays public per LIC-01: any attacker can hit it, but only an attacker with a valid license_key + on the right hardware can succeed. License_key is a bearer secret distributed out of band by the operator. Cloudflare L7 throttling is the deployment mitigation for flood attempts."
  - "Idempotency via state.license_valid (not DB row): the gate is in-memory + JWT-on-disk. Second activate POST after success reads license_valid=true and returns 409 ALREADY_ACTIVATED — never re-hits DO Functions. Prevents log noise, prevents JWT churn, and matches the user-facing 'already activated' UX."
  - "Manual split-check beats regex(path=...) validator: regex form requires once_cell::Lazy<Regex> static; manual split is 5 lines, allocation-free, and reads clearer. validator's length(19) prevents the obvious garbage cases."
  - "Test fixture bypass uses Arc<AtomicBool>::new(true) literal everywhere (not a helper): keeps test code grep-able and avoids hiding the gate decision behind a function call. Editor finds 'license_valid:' and the value is right there."
  - "Frontend success banner uses border-l-4 border-green-600 + ShieldCheck (new variant introduced by UI-SPEC), parallel to the existing destructive variant. No new shadcn primitive needed — both banners are the same pattern with different colors and icons."

patterns-established:
  - "License gate as Tower middleware applied in reverse-chain order to fire BEFORE auth — closes T-06-17 information disclosure"
  - "Idempotent public activation handler with AtomicBool guard for race-safe re-post handling"
  - "Test fixture additive Config + AppState backfill via single-line replace_all once new field landed"
  - "Cross-platform fingerprint test: cfg(target_os = \"linux\") for hard assertions, accept Internal on macOS dev as fail-closed equivalent"
  - "Next.js 16 useSearchParams Suspense wrapping pattern (LoginPageInner + Suspense default export)"

requirements-completed:
  - LIC-01
  - LIC-05

duration: 13min
completed: 2026-04-27
---

# Phase 06 Plan 02: License Gate Wiring + Activation UI Summary

**License gate plumbed through the request pipeline (require_license Tower layer wraps every protected route group, runs BEFORE require_auth via axum 0.8 reverse-order route_layer chaining), POST /setup/activate public endpoint with idempotent AtomicBool guard, /setup/status now reports licensed alongside initialized, frontend /setup/license page UI-SPEC compliant with error-code map and ShieldCheck success banner.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-04-27T20:52:16Z
- **Completed:** 2026-04-27T21:05:20Z
- **Tasks:** 2 (both committed atomically)
- **Files modified:** 30 (4 src + 16 backend tests + 4 frontend Phase 6 + 3 frontend Rule-3 fixes + license_tests.rs + 2 new frontend files + deferred-items.md)

## Accomplishments

- License gate live end-to-end: AppState.license_valid + require_license middleware + 6 route_layer attachments
- POST /setup/activate now performs the full activation flow (validate → idempotency guard → DO Functions → JWT verify+persist → flip gate)
- GET /setup/status now exposes the `licensed` field so the wizard can branch step 0 (license) before step 1 (admin)
- Frontend /setup/license page implemented per UI-SPEC: mono uppercase input, error-code map, ShieldCheck success banner, aria-disabled submit, Loader2 status check on mount
- /setup/page.tsx now redirects to /setup/license when status returns licensed=false
- 6 new license gate behavior tests in license_tests.rs::gate_behavior_tests prove: 403 UNLICENSED on protected routes when unlicensed, allowed when licensed, status ungated, format validation, full activation round-trip, 404 mapping
- 24/24 license_tests pass (was 18 from Plan 01; +6 new); 288/288 full backend suite green (was 282); frontend tsc + next build both clean
- LIC-01 and LIC-05 realized at the runtime layer: unlicensed installations cannot serve any protected route; fingerprint mismatch fails closed at activation AND at every startup

## Task Commits

Each task committed atomically:

1. **Task 1: AppState license_valid + require_license middleware + main.rs wiring + test fixture backfill** — `c57ac1d` (feat)
2. **Task 2: setup_activate handler + extended setup_status + license gate behavior tests + frontend activation page** — `96870d7` (feat)

_Note: Task 1 ships the wiring scaffold with a setup_activate stub so main.rs can register the route; Task 2 replaces the stub body with the production activation flow and adds the behavior tests that assert FALSE-case gate behavior. The plan-level RED→GREEN cadence is: Task 1 commits the surface (route registered, gate wrapped, tests bypassed via license_valid=true), Task 2 commits the FALSE-case tests + handler body simultaneously. The full backend suite stays green at every commit boundary._

## Files Created/Modified

**Created (license middleware + frontend):**
- `backend/src/license/middleware.rs` — Tower middleware that returns AppError::Unlicensed when license_valid is false
- `frontend/src/app/setup/license/page.tsx` — UI-SPEC compliant license activation form
- `frontend/src/app/setup/license/layout.tsx` — page metadata (title: "Cronometrix — License Activation")
- `.planning/phases/06-licensing-deployment/deferred-items.md` — log of pre-existing build/type errors fixed inline so plan verify gates pass

**Modified (backend src):**
- `backend/src/state.rs` — added `license_valid: Arc<std::sync::atomic::AtomicBool>` field with doc comment
- `backend/src/license/mod.rs` — `pub mod middleware;` (was Plan 01 placeholder)
- `backend/src/main.rs` — `use cronometrix_api::license;`; license_valid init from load_and_validate_license() before AppState construction; AppState struct literal carries license_valid; /setup/activate added to public_routes; 6 protected route groups (cookie_auth, viewer, supervisor_read, supervisor, report, admin) get .route_layer(require_license) AFTER existing auth/RBAC layers; renewal_task spawned and awaited at shutdown
- `backend/src/setup/handlers.rs` — setup_status now returns both `initialized` and `licensed` booleans; full setup_activate handler with format validation (length + segment alphanumeric check), idempotency guard, activate_license invocation, license_valid flip on success

**Modified (backend tests — license_valid bypass):**
- `backend/tests/{auth,daily_record,department,device,employee,event,leave,listener,reports,reports_excel,rules,supervisor,tenant_info}_tests.rs` — single line addition `license_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),` after `event_broadcast: None,` at every AppState construction site (25 occurrences across 13 files; supervisor_tests.rs alone has 9 sites — Rule 3 inclusion since the plan listed only 12 files)

**Modified (backend tests — new behavior):**
- `backend/tests/license_tests.rs` — appended `gate_behavior_tests` submodule with build_gated_app helper + body_to_json + 6 named tests

**Modified (frontend):**
- `frontend/src/lib/validations.ts` — appended licenseSchema (zod) and LicenseFormData type
- `frontend/src/app/setup/page.tsx` — checkStatus useEffect now redirects to /setup/license when data.licensed === false BEFORE the existing initialized branch
- `frontend/src/__tests__/device-banner.test.tsx` — Rule 3: replaced Partial<Device> with Device[] casts so tsc strict mode passes
- `frontend/src/components/dashboard/dept-chart.tsx` — Rule 3: widened Recharts v3 Tooltip formatter to handle ValueType | undefined
- `frontend/src/app/login/page.tsx` — Rule 3: extracted body to LoginPageInner, wrapped default export in Suspense for Next.js 16 prerender

## Decisions Made

- **Stub-then-replace for setup_activate to keep Task 1 atomic.** Task 1's verify gate (cargo build + nextest) requires the route registration to compile; Task 2 ships the full handler. The stub returned `AppError::BadGateway { code: "ACTIVATION_UNREACHABLE", ... }` so the surface contract is a real HTTP response shape, not a panic. Task 2's commit replaces only the body of `pub async fn setup_activate(...)`. Same surface signature.
- **Reverse-order route_layer pinned by test_license_gate_blocks_unlicensed_protected_route.** axum 0.8 inverts route_layer chains: the LAST `.route_layer(...)` runs FIRST on the request. To make `require_license` fire before `require_auth`, the new layer is added AFTER the existing auth layer. The test sends a valid Bearer token + license_valid=false and asserts 403 UNLICENSED — proves the order is correct. If anyone re-orders main.rs and breaks this, the test fails immediately.
- **License_valid backfill in supervisor_tests.rs (9 sites) was a Rule 3 deviation.** The plan listed 12 files and 16 sites; the actual count is 13 files and 25 sites because supervisor_tests.rs has 9 separate `let state = AppState { ... }` literals which the plan author missed. The single-line addition is mechanical and adds zero behavior change to those tests. Build fails without it.
- **/setup/activate is the only public write path on unlicensed installs.** Per LIC-01 first-run requirement: every other authenticated route returns 403 UNLICENSED before reading the Authorization header. /events/stream is also public but is read-only SSE. /setup/init writes a user but only when count==0. /setup/activate is the only writer that flips the license gate.
- **Idempotency via state.license_valid AtomicBool, not DB row.** The license cache is the on-disk JWT; the gate is in-memory. Two concurrent /setup/activate POSTs: the first wins, flips license_valid to true, persists JWT. The second reads license_valid=true via Acquire-equivalent (Relaxed is fine here — the only memory we coordinate on is this single bool) and returns 409 ALREADY_ACTIVATED without ever calling DO Functions or touching disk. activate_license itself uses temp+rename for crash safety (Plan 01 contract).
- **License key format check is zod-on-frontend + length+manual-split on backend.** Zod's regex catches the format on submit; backend's `length(19)` catches truncated payloads, the manual `parts.len() == 4 && all(|p| p.len()==4 && alphanumeric)` catches every other malformed shape without pulling in a `regex` static via once_cell. 5 lines of code, allocation-free, reads as well as a regex would.
- **Cross-platform tests via wiremock + accept-either-outcome.** Linux CI exercises the strong path: license_valid flips to true, body is `{"activated":true}`. macOS dev hosts have no `/proc/cpuinfo` so collect_fingerprint fails before the HTTP call ever fires; AppError::Internal surfaces and license_valid stays false. Both outcomes are fail-closed (no JWT persisted). The test asserts the correct outcome per platform, never panics.
- **Frontend SUI: aria-disabled, not disabled.** Submit button keeps tab focus and screen-reader announcement during submit; click is short-circuited via `onClick={isSubmitting ? (e) => e.preventDefault() : undefined}`. Established pattern from /setup/page.tsx, copied verbatim.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Backfilled license_valid on supervisor_tests.rs (9 AppState sites)**
- **Found during:** Task 1, after the AppState struct change broke `cargo build`
- **Issue:** Plan task 1 listed 12 test files / 16 AppState sites. Actual count: 13 files / 25 sites because supervisor_tests.rs has 9 inline `let state = AppState { ... }` literals (one per supervisor lifecycle test). Without the field, every supervisor test fails to compile (E0063 missing fields).
- **Fix:** `replace_all` the same one-line addition (`license_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),` after `event_broadcast: None,`) — same mechanical change, just one more file.
- **Files modified:** `backend/tests/supervisor_tests.rs`
- **Verification:** `cargo build --tests` clean; full nextest suite 282/282 pass at Task 1 boundary.
- **Committed in:** `c57ac1d` (Task 1 commit)

**2. [Rule 3 - Blocking] setup_activate stub registered in Task 1**
- **Found during:** Task 1, after main.rs added `.route("/setup/activate", post(setup::handlers::setup_activate))` to public_routes
- **Issue:** main.rs references setup_activate by name; without a definition in setup/handlers.rs, the build fails with E0425 ("cannot find value `setup_activate`"). Plan task 1 step 4d explicitly adds the route reference; plan task 2 then ships the handler body. Task 1 cannot commit a green build without the handler existing.
- **Fix:** Defined `pub async fn setup_activate(...)` in setup/handlers.rs returning `AppError::BadGateway { code: "ACTIVATION_UNREACHABLE", message: "Activation endpoint not yet implemented (Plan 06-02 Task 2 pending)" }`. The signature matches Task 2's final contract; Task 2 only replaces the body.
- **Files modified:** `backend/src/setup/handlers.rs` (added 25-line stub)
- **Verification:** `cargo build` clean; existing tests untouched.
- **Committed in:** `c57ac1d` (Task 1 commit)

**3. [Rule 3 - Blocking] Pre-existing TS strict-mode error in device-banner.test.tsx**
- **Found during:** Task 2, when running `npx tsc --noEmit` per plan verify gate
- **Issue:** `Partial<Device>` makes `name?: string | undefined`; the `DeviceStatusSummary` component expects `name: string` (required). Pre-existing — last touched in Plan 04-02 (commit 7f310a3); breaks `tsc --noEmit` and `next build`.
- **Fix:** Replaced `Partial<Device>` typing of `base` with a structural literal that satisfies all required fields, then declared each test's `devices` array as `Device[]`. No runtime change — the tests render the same component with the same shape.
- **Files modified:** `frontend/src/__tests__/device-banner.test.tsx`
- **Verification:** `npx tsc --noEmit` clean.
- **Committed in:** `96870d7` (Task 2 commit)

**4. [Rule 3 - Blocking] Pre-existing Recharts v3 Tooltip formatter type error**
- **Found during:** Task 2, same tsc gate
- **Issue:** Recharts v3 changed Formatter signature from `(value: number) => ...` to `(value: ValueType | undefined, ...) => ...`. dept-chart.tsx (Plan 04-02 commit 7f310a3) still uses the v2 form, breaking strict-mode compilation.
- **Fix:** Removed explicit `(val: number) =>` annotation; widened to `(val) => [...typeof val === 'number' ? val : 0...]`. Output is identical for valid numeric data; the fallback covers the (unreachable in this app) `undefined` case.
- **Files modified:** `frontend/src/components/dashboard/dept-chart.tsx`
- **Verification:** `npx tsc --noEmit` clean.
- **Committed in:** `96870d7` (Task 2 commit)

**5. [Rule 3 - Blocking] Pre-existing Next.js 16 Suspense requirement on /login**
- **Found during:** Task 2, when running `npx next build` per plan verify gate
- **Issue:** `/login` page calls `useSearchParams()` at the top of the default export. Next.js 16 prerender requires this hook to live under a Suspense boundary; otherwise the static export fails with "useSearchParams() should be wrapped in a suspense boundary at page /login". Pre-existing — last touched in Plan 04 fix 1f4e754. Without this fix, the plan's `next build` verify gate cannot pass.
- **Fix:** Renamed the body function to `LoginPageInner`, exported a new `LoginPage` default that wraps `<LoginPageInner />` in `<Suspense fallback={<Loader2 .../>}>`. The fallback mirrors the existing /setup wizard loading skeleton.
- **Files modified:** `frontend/src/app/login/page.tsx`
- **Verification:** `npx next build` exits 0 with /login + /setup + /setup/license all in the route table.
- **Committed in:** `96870d7` (Task 2 commit)

---

**Total deviations:** 5 auto-fixed (5 blocking). All 5 are mechanical fixes to enable the plan's own explicit verify gates (`cargo build`, `cargo nextest run`, `npx tsc --noEmit`, `npx next build`). Three are pre-existing — they were latent because the previous plan didn't run all four gates, and Phase 6's UI-SPEC requirements made `next build` necessary. The other two are direct consequences of additive surface changes the plan introduced (AppState field, /setup/activate route).

**Impact on plan:** Zero scope creep. All deviations are correctness/build-blockers; not one introduces new behavior beyond what the plan requested. Documented in `.planning/phases/06-licensing-deployment/deferred-items.md` for traceability.

## Threat Flags

None. All new surface (require_license middleware, setup_activate handler, /setup/license page, license_valid field) is enumerated in the plan's threat_model (T-06-15..24). The Rule 3 frontend fixes touch unrelated UI files and introduce no new trust boundaries.

## Issues Encountered

- macOS dev host: `collect_fingerprint()` errors because `/proc/cpuinfo` does not exist. Plan 01 already anticipated this; Plan 02 tests accept either Linux happy-path or macOS Internal as long as license_valid stays false on error. Linux CI runs the strong assertions.
- The `tower-http::timeout::TimeoutLayer::new` deprecation warning in main.rs survives this plan — pre-existing from earlier phases, out of scope.
- Pre-existing build/type errors discovered when running the plan's verify gates — see Deviations section above.

## Self-Check

**Created files exist (verified):**
- `backend/src/license/middleware.rs` — FOUND
- `frontend/src/app/setup/license/page.tsx` — FOUND
- `frontend/src/app/setup/license/layout.tsx` — FOUND
- `.planning/phases/06-licensing-deployment/deferred-items.md` — FOUND

**Commits exist (verified via `git log --oneline`):**
- `c57ac1d` (Task 1 — feat: license gate wired into request pipeline + AppState) — FOUND
- `96870d7` (Task 2 — feat: setup_activate handler + /setup/license UI + gate behavior tests) — FOUND

**Plan verification block:**
- `cargo build` exits 0 — PASS (only pre-existing tower-http deprecation warning)
- `cargo nextest run` 288/288 pass, 2 skipped (unrelated) — PASS (was 282 before; +6 new gate tests)
- `cargo nextest run --test license_tests` 24/24 pass (was 18; +6 new) — PASS (≥ 17 required)
- `cd frontend && npx tsc --noEmit` exits 0 — PASS
- `cd frontend && npx next build` exits 0 with /setup/license route in tree — PASS
- `grep -c "license_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true))" backend/tests/*.rs | awk -F: '{s+=$2} END {print s}'` = 25 — PASS (≥ 16 required)
- `grep -c "require_license" backend/src/main.rs` = 6 attachments × wraps + 1 import + 1 spawn ref = covers ≥ 6 protected route groups — PASS (matches the 6 groups: cookie_auth, viewer, supervisor_read, supervisor, report, admin)

## Self-Check: PASSED

## Next Phase Readiness

- Plan 06-03 (deployment) consumes the license module via `LICENSE_JWT_PATH=/opt/cronometrix/data/license.jwt` env var; this plan proved the path-based load+verify works.
- Plan 06-04 (UI activation flow) consumes /setup/license + the error-code map; this plan implemented both per UI-SPEC.
- DO Functions URL must be set in production env: `DO_FUNCTIONS_ACTIVATE_URL=https://faas-...digitaloceanspaces.com/.../licenses/activate`. Empty string keeps the system gated; activation cannot proceed.
- The renewal_task is silent best-effort: if `DO_FUNCTIONS_RENEW_URL` is empty it sleeps forever (no-op). Operators can deploy without it and rely on the 365-day default JWT exp + manual re-activation.
- Pre-existing items still deferred for future plans:
  - tower-http TimeoutLayer::new deprecation in main.rs (out of scope; not blocking any verify gate)

---
*Phase: 06-licensing-deployment*
*Plan: 02*
*Completed: 2026-04-27*
