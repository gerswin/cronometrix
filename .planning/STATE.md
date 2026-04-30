---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: Documentation & Sign-off Hardening
status: executing
stopped_at: Completed 10-05-PLAN.md — v1.1 Backlog section added to REQUIREMENTS.md; 06-VERIFICATION.md deferred row 1 cross-references DEPL-03-AUTO; bidirectional deferral audit trail complete
last_updated: "2026-04-30T02:24:33.570Z"
last_activity: 2026-04-30 -- Phase --phase execution started
progress:
  total_phases: 10
  completed_phases: 10
  total_plans: 51
  completed_plans: 51
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-11)

**Core value:** Accurate, auditable time tracking that turns raw biometric events into payroll-ready data — with zero manual calculation and full legal traceability.
**Current focus:** Phase --phase — 10

## Current Position

Phase: --phase (10) — EXECUTING
Plan: 1 of --name
Status: Executing Phase --phase
Last activity: 2026-04-30 -- Phase --phase execution started

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 16
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 05 | 4 | - | - |
| 06 | 4 | - | - |
| 8 | 8 | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*
| Phase 01-foundation P00 | 25 | 2 tasks | 12 files |
| Phase 01-foundation P01 | 8 | 2 tasks | 13 files |
| Phase 01-foundation P02 | 6 | 2 tasks | 12 files |
| Phase 01-foundation P03 | 35 | 3 tasks | 15 files |
| Phase 01-foundation P04 | 8 | 2 tasks | 31 files |
| Phase 03-time-calculation-engine P01 | 26 | 2 tasks | 39 files |
| Phase 03-time-calculation-engine P02 | 9 | 2 tasks | 7 files |
| Phase 03 P03 | 28 | 2 tasks | 10 files |
| Phase 07 P01 | 180 | 6 tasks | 23 files |
| Phase 07 P02 | 18 minutes | 4 tasks | 30 files |
| Phase 08 P01 | 25 | 2 tasks | 11 files |
| Phase 08 P02 | 50 | 3 tasks | 16 files |
| Phase 08 P03 | 11 | 2 tasks | 5 files |
| Phase 08 P04A | 140 | 16 tasks | 16 files |
| Phase 08 P04B | 85min | 2 tasks tasks | 11 files files |
| Phase 08 P04C | 120min | 5 tasks tasks | 27 files files |
| Phase 08 P05 | 35 | 1 tasks | 1 files |
| Phase 08 P06 | 25 | 1 tasks | 1 files |
| Phase 09 P01 | 4 | 2 tasks | 6 files |
| Phase 09 P02 | 51 | 3 tasks | 3 files |
| Phase 09 P03 | 17 | 4 tasks | 8 files |
| Phase 09 P04 | 39 | 3 tasks | 7 files |
| Phase 09 P05 | 7 | 3 tasks | 7 files |
| Phase 09 P06 | 3 | 4 tasks | 10 files |
| Phase 09 P07 | 2 | 1 tasks | 1 files |
| Phase 09 P08 | 18 | 2 tasks | 7 files |
| Phase 09 P09 | 11 | 3 tasks | 13 files |
| Phase 09 P10 | 9 | 3 tasks | 5 files |
| Phase 09 P11 | 4 | 2 tasks | 2 files |
| Phase 09 P12 | 1 | 1 tasks | 1 files |
| Phase 09 P13 | 2 | 1 tasks | 1 files |
| Phase 10-v1-0-documentation-and-sign-off-hardening P03 | 10 | 1 tasks | 1 files |
| Phase 10-v1-0-documentation-and-sign-off-hardening P05 | 6 | 1 tasks | 2 files |

## Accumulated Context

### Roadmap Evolution

- Phase 8 added: Test Coverage & Quality Gate — reach >=90% line / >=85% branch coverage backend + frontend, add CI thresholds, fix leave_tests cwd-dependent failure, document coverage commands
- Phase 9 added: E2E Playwright test suite — login, dashboard, marcaciones, empleados, dispositivos, reportes; auth fixtures; CI integration; covers src/app/ (excluded from Vitest per D-10)

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Rust + Axum backend — alertStream connections and concurrent webhook processing performance
- [Init]: SQLite + Turso — local-first, treat SQLite as write primary, cloud as async replica (beta caveat)
- [Init]: Audit trail enforced via SQLite triggers, not application code only — legal defensibility
- [Init]: UTC epoch integer storage for all timestamps — overnight shift and DST correctness from migration zero
- [Phase 01-foundation]: Placeholder SQL approach: include_str! guard skips execution if file starts with '-- Placeholder', enabling Wave 0 compilation without a real schema
- [Phase 01-foundation]: tests/common/mod.rs as shared fixture module: test_db() returns isolated in-memory libSQL DB per test call; TEST_JWT_SECRET constant for test-only JWT generation
- [Phase 01-foundation]: lib.rs added to expose pub modules — binary crates cannot be referenced from integration test crates without a library target
- [Phase 01-foundation]: Test fixture uses unique temp file DB not :memory: — sqlite3_open_v2(':memory:') creates isolated DB per connection causing migrations to be invisible to subsequent connections
- [Phase 01-foundation]: tracing-subscriber env-filter feature must be explicitly enabled for with_env_filter() — not included in plan Cargo.toml spec
- [Phase 01-foundation]: SameSite=Lax (not Strict) on refresh cookie: allows third-party link navigation in on-premise deployments while still blocking CSRF POST attacks
- [Phase 01-foundation]: refresh/logout routes not behind require_auth Bearer middleware — they self-authenticate via refresh cookie; Bearer middleware would block legitimate refresh flows
- [Phase 01-foundation]: jsonwebtoken rust_crypto feature enabled to avoid rustls CryptoProvider panic in test environments without a full TLS stack
- [Phase 01-foundation]: Soft delete verification in tests uses REST API (GET by id) not direct DB connection — libsql::Database does not implement Clone
- [Phase 01-foundation]: Dynamic WHERE clause with positional param indexing for optional filters — avoids SQL injection without ORM
- [Phase 01-foundation]: effective_from always updated on any PATCH to global_rules — per RULE-03, any rule change resets the effective period
- [Phase 01-foundation]: proxy.ts (not middleware.ts): Next.js 16 renamed Middleware to Proxy — function export also renamed to `proxy`
- [Phase 01-foundation]: Metadata in layout.tsx not page.tsx: Next.js 16 forbids metadata export from client components ('use client')
- [Phase 01-foundation]: Providers component: QueryClientProvider must be a client component, isolated from server Root Layout
- [Phase 01-foundation]: frontend/.git removed: create-next-app creates its own git repo; removed to track files in monorepo
- [Phase 03-time-calculation-engine]: Single-connection txn for recompute_for_day — libSQL shared-cache lock contention between separate reader/writer connections produced "database is locked" under test load; reusing the same conn after draining all read cursors is safe and matches events/service pattern.
- [Phase 03-time-calculation-engine]: ON CONFLICT(employee_id, anchor_date) DO UPDATE (not INSERT OR REPLACE) for daily_records upsert — preserves the row id so daily_record_anomalies FK survives recomputes (Pitfall 1).
- [Phase 03-time-calculation-engine]: LOTTT Art. 178 daily cap = total workday > 600min (work + OT), not "OT > 120min" — the statute constrains total hours, not OT-hours specifically.
- [Phase 03-time-calculation-engine]: Engine is pure (no I/O, no async) — aggregation/lunch/overtime/engine submodules, decomposed from the {mod, models, service, handlers} Phase 1/2 layout. Proptest validates determinism across 270k random inputs.
- [Phase 03-time-calculation-engine]: RecomputeWorker mirrors Phase 2 Supervisor: biased select, HashSet dedup, 500ms debounce, tokio::time::sleep-driven nightly (no cron crate).
- [Phase 03-time-calculation-engine]: publish_recompute_if_employee guards on employee_id.is_some() AND recompute_tx.is_some() — Pitfall 7 (never flood worker with unknown-face NULL ids) + test-setups-without-worker compatibility.
- [Phase 03-time-calculation-engine]: Overnight shifts: .earliest() path on LocalResult (not .single().unwrap()) — Caracas always returns Single(dt), but the infrastructure exists so a future DST market cannot panic the calc thread; ambiguity surfaces via OvernightInferenceAmbiguous anomaly.
- [Phase 03-time-calculation-engine]: shift_window() kept as 4-tuple delegating to shift_window_overnight_aware(); new shift_window_with_ambiguity() exposes the 5-tuple for engine.rs — zero callsite changes in service.rs or other modules, Plan 03-01 day-only tests pass unchanged.
- [Phase 03-time-calculation-engine]: No SQL change in daily_records::service for overnight support — because shift_window() now returns an across-midnight (start, end) range, the existing captured_at BETWEEN query picks up post-midnight events automatically. Proven by recompute_overnight_captures_post_midnight_events integration test.
- [Phase 03]: LEAVE_OVERLAP uses dedicated LeaveConflict variant (HTTP 409), not generic Conflict — distinguishes business-rule overlap from optimistic-concurrency conflicts for Phase 4 UI remediation.
- [Phase 03]: Evidence files are UUIDv4-named (user filename discarded). cancel_leave soft-deletes DB row but preserves evidence file on disk for LOTTT audit retention.
- D-06: JoinSet fire-and-forget fan-out for enrollment push
- diqwest-multipart: manual 2-step digest auth for multipart upload (stream body not cloneable)
- D-15/D-16: PurgeWorker + BackfillWorker via mpsc channels, workers spawned in main.rs
- Button.asChild not in @base-ui/react — AccessRestricted uses plain Link with Tailwind classes
- Kiosk query enabled: !!captureId only — refetchInterval handles terminal stop (kioskState condition caused test race)
- In-progress list v1 session-scoped — no list endpoint in 07-01; future plan adds GET /enrollments?status=in_progress
- Phase 8 D-18/D-19 (Wave 1): Paths substruct on AppState — Paths::from_env at startup, Paths::for_test(tempdir) in tests; eliminates the cwd-dependent + env-var-race anti-pattern that broke leave_tests under cargo-llvm-cov
- Phase 8 D-21 (Wave 1): Backwards compat preserved verbatim — same env var names (CRONOMETRIX_LEAVES_ROOT/CRONOMETRIX_EVENTS_ROOT/ENROLLMENTS_DIR/CRONOMETRIX_CAPTURES_TMP/DATA_DIR) and same string defaults the deleted helpers used
- persist_attendance_event signature gains events_root: &Path rather than &AppState — function only takes &Connection so threading a single &Path matches write_photo_atomic's shape
- Phase 8 D-20 (Wave 2): test_state_with_tmpdir returns (AppState, TempDir) tuple — type system surfaces Pitfall 1 (premature drop) at compile time; uniform across 16 test files
- Phase 8 Wave 2: scope expanded from 4 → 12 sibling files mid-execution — cargo build --tests revealed 8 additional callers of common::test_state with the 2-arg signature; per Rule 3 (blocking) all 12 migrated together
- Phase 8 D-22 (Wave 3 / Plan 03): Coverage tooling shipped — Vitest config with two-level threshold via glob form (no perFile:true; RESEARCH § Pitfall 4); awk-based lcov post-processor for per-file floor enforcement (cargo-llvm-cov has no per-file flag); rust-toolchain.toml pins nightly-2026-04-01 + llvm-tools-preview; baseline reveals 51 files below floor (27 backend + 24 frontend) — Plan 04 must triage scope-cap
- Phase 8 Plan 03: backend baseline measured without --branch (stable rustc 1.93.0 on local box, no rustup); BRF=0 across all records; line% = 63.09% measured accurately; backend branch% deferred to Plan 05 CI run under nightly
- Phase 8 04A: AppError variant pattern-match assertions over Display strings — service-layer error tests must match { Variant { code, message } => ... } because Display only emits the variant tag
- Phase 8 04A: wiremock + Mock::given(method).and(path) is the canonical pattern for ISAPI client digest-auth coverage; happy + 5xx + 401-without-WWW-Authenticate exhaust the retry-loop branches without real hardware
- Phase 8 04A: process-Mutex (static ENV_LOCK) around Paths::from_env / Config::from_env tests — needed under cargo nextest parallel execution; tolerate poisoned mutex via .unwrap_or_else(|e| e.into_inner()) for chained tests
- Phase 8 04B: tokio::test(start_paused=true) + tokio::time::advance is the canonical pattern for testing scheduler/worker async loops; tokio test-util feature already enabled at backend/Cargo.toml line 51
- Phase 8 04B: license/{fingerprint, service} cannot reach 70% line on macOS dev (no /proc/cpuinfo); surfaced as Plan 04C exclusion candidate — Linux CI under Plan 05 will measure them at full coverage
- Phase 8 04B [Rule 1 bug]: workers/backfill.rs read photo_path without joining state.paths.enrollments_root — production bug discovered by test, fixed inline (matches retry_push handler shape)
- Phase 8 04B: detached-spawn-task tests use polling-with-explicit-drop pattern — drop(rows) + drop(conn) between iterations is required because libsql shared-cache locks would otherwise starve the spawn task
- Phase 8 04C: face-detection.ts is testable in jsdom via vi.mock at the dynamic-import boundary — the original 'WebAssembly cannot be mocked' hedge does NOT apply when import-level mocking is used. NO exclusion needed.
- Phase 8 04C: Pre-existing flaky enrollment-modal.test.tsx (from Phase 7-02) was a Rule 1 bug — the global api.get mock returned the paginated employee-list shape for the /enrollments/:id polling endpoint, crashing on device_pushes.map. Fixed by routing api.get based on URL prefix (test-only fix, production unchanged).
- Phase 8 04C: 6 branch-bump test files added beyond the 21 bucket files (under Rule 2) to clear the 85% project branch gate from 81.88% → 85.12%. Targeted drill-down-dialog, filters-bar, period-picker, tenant-info-form, validations, and command-modal — all bumps are existing-file branch coverage, no new bucket scope.
- Phase 8 04C: FakeEventSource shim (custom class on globalThis) is the canonical pattern for testing useSSE — msw's EventSource doesn't simulate auto-close-on-error and progressive backoff. Custom shim makes all 5 backoff levels (1/2/4/8/30s capped) deterministic.
- Phase 8 04C approved: 2 macOS-only backend exclusions accepted (license/fingerprint.rs + license/service.rs); 6 Rule-2 branch-bump tests accepted; Rule-1 enrollment-modal fix accepted; Plan 05 (CI gate) unblocked
- Phase 8 Plan 05 [User direction]: CI validation deferred to manual follow-up — workflow file (.github/workflows/ci.yml) verified statically (grep + YAML parse); positive run, negative regression PR, and branch protection setup tracked as unchecked checklist in 08-05-SUMMARY.md Manual Follow-up section
- Phase 8 Plan 05: GitHub Actions workflow pinned to actions/checkout@v4, actions/setup-node@v4, actions/upload-artifact@v4, taiki-e/install-action@v2 (cargo-llvm-cov@0.8.5 + cargo-nextest), Swatinem/rust-cache@v2; permissions: contents: read at workflow level (least privilege per T-08-15)
- Phase 8 Plan 05: CI exclusion regex parity enforced — '(main\.rs|tests/common/.*)' identical between Makefile and .github/workflows/ci.yml backend job; prevents drift between local make coverage and CI gate
- Phase 8 D-22: CLAUDE.md ## Test Coverage section landed — install/local commands/thresholds/exclusions/HTML reports/CI gate/triage/public-vs-private/pending-validation pointer; documents the gate's design and the deferred-validation tracking (08-05 Manual Follow-up)
- Phase 8 D-23: CLAUDE.md Conventions § Filesystem-root injection landed inside GSD-managed markers (lines 185-212) with protective HTML comment; documents state.paths.* env var contract and test_state_with_tmpdir helper
- Phase 8 close-out: 6 of 6 plans complete in code-and-docs; live CI validation (positive run + negative regression PR + branch protection) remains as 08-05 Manual Follow-up to be executed by a human on the live GitHub Actions runner before the gate is declared 'active in production CI'
- Phase 9 Plan 01: Playwright 1.59.1 exact-pinned; webServer×3 (backend:4001/mock:4400/next:3001); TZ frozen in 3 places; workers=1 fullyParallel=false for D-12 determinism
- Phase 9 D-13: exit code 2 for bypass-without-e2e is a locked contract — do not change without updating license_bypass_safety.rs integration test
- Phase 9 D-13: TCP port polling is the correct readiness probe for subprocess integration tests — BufReader::read_line() blocks indefinitely on live child processes
- Phase 9 D-13: DEVICE_CREDS_KEY in subprocess tests must be base64-encoded 32 bytes (QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=); TURSO_DATABASE_URL must be absent
- D-09-03-A: Tuple params (not libsql::params![]) for inserts mixing &str and owned String — params! silently returns rows_affected=0 in that case
- D-09-03-B: Device ports 4400/4401 to satisfy idx_devices_ip_port_active unique index; exit device contacts mock admin port
- D-09-03-C: CARGO_BIN_EXE_cronometrix (not cronometrix-api) — explicit [[bin]] changes the env var name from package name to declared binary name
- D-09-04-A: Audit endpoint registered exclusively in supervisor_read_routes — Admin+Supervisor 200, Viewer 403, Anonymous 401; locked by 2 integration tests
- D-09-04-B: old_data/new_data TEXT columns parsed to Option<serde_json::Value> in service layer; parse failure returns None (defensive, never errors)
- D-09-04-C: ORDER BY created_at DESC, id DESC — deterministic tie-break prevents non-deterministic pagination with same-second rows
- W6 actor dropdown OPTION A: derive distinct actor IDs from current page data — /audit/actors endpoint deferred; Plan 11 must use actor_id values in selectOption()
- storageState path uses path.resolve(__dirname) — avoids cwd-relative path bugs in setup project
- 00-build-and-seed uses fs.existsSync pre-built binary check — fails fast on cargo build errors instead of swallowing them in try/catch
- globalTeardown removes .db-wal and .db-shm sidecar files — libSQL WAL mode creates these alongside the main DB file
- loginSchema password min(1) not min(8): login spec empty-field validation tests adapted from plan template's assumed min-8 to actual Zod schema constraint
- D-06 hybrid auth enforced: login.spec.ts is the only spec using UI-driven login; all 12 tests run in fresh browser contexts with no test.use({ storageState })
- T-09-09 open-redirect CR-02: safeRedirect rejects //evil.com covered by T-11; English copy locked per D-19 Addendum (7 strings enumerated in SUMMARY for i18n handoff)
- SSE banner always rendered in DOM using hidden attr instead of conditional render — enables toBeAttached() in Playwright without triggering disconnect
- KPITile accepts optional testId prop; page passes kpi-{slug} per tile to decouple test id from display text
- Employee.version added to frontend type — aligns with backend UpdateEmployeeRequest optimistic concurrency; hire_date corrected to string|null
- Employee CRUD dialogs built inline in employees/page.tsx — create/edit/deactivate wired to backend REST; deactivate button only on active rows with onDeactivateClick prop
- mutation->audit pattern: getAudit + expect.poll with 15s timeout — canonical pattern for Plan 10 devices+reports specs
- PATH B (mock recv-log) chosen for devices.spec.ts door-open assertion: command_audit_log is separate from audit_log; getAudit() only queries audit_log; /admin/recv-log is the B6 contract
- ExportButtons conditionally rendered in reports/page.tsx: only shown when reportQ.data exists; UI tests must click Emitir Reporte before asserting export buttons
- W4 RBAC reconciliation: POST /devices/{id}/commands and POST /leaves are admin_routes (admin only), not supervisor_routes — supervisor gets 403 on both; locked by rbac.spec.ts T-03/T-04/T-08
- W6 OPTION A confirmed: audit actor dropdown uses actor_id string as option value; selectOption('e2e-admin-id') is correct; no /audit/actors endpoint needed
- E2E Tests CI job: Rust toolchain via inline rustup shell step (no third-party action), TZ=America/Caracas at job+step level (W5), branch protection is Manual Follow-up (mirrors Phase 8 Plan 05 pattern)
- Phase 9 D-23: CLAUDE.md ## End-to-End Tests (Phase 9) section landed — documents install/env-flags/abort-contract/ports/TZ-freeze/file-layout/CI-gate/reading-failing-run/pending-validation; CRONOMETRIX_E2E + CRONOMETRIX_LICENSE_BYPASS abuse prevention contract explicit
- DEPL-03 traceability row set to Partial with explicit D-13 reference and DEPL-03-AUTO v1.1 backlog pointer — no code change, documentation closure only
- ENRL-* checkboxes remain [x] (correct); traceability column now also says Complete — inconsistency D-07 resolved
- v1.1 Backlog section deliberately NOT added — owned by plan 10-05 to avoid merge conflict (per RESEARCH Area 9)
- DEPL-03 deferral locked as DEPL-03-AUTO in v1.1 Backlog — not a bug; scope boundary accepted at v1.0 with cloudflared CLI vs Go SDK evaluation question for v1.1 planning

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 2]: Hikvision ISAPI XML schema varies by device model (DS-K1T341, DS-K1T342) — capture real alertStream traffic before implementation; do not rely on documentation alone
- [Phase 3]: Venezuela / America/Caracas / LOTTT compliance — IANA timezone fixed at `America/Caracas` (UTC-4, no DST since May 2016); LOTTT Art. 117/173/178 caps confirmed via Phase 3 research. No blocking DST concern for v1.
- [Phase 7]: ISAPI batch face profile enrollment failure behavior on partial failure (3 of 4 devices) is undocumented — requires hands-on hardware testing before designing the enrollment modal

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260428-3qg | Fix backend test compile errors via shared common::test_state helper | 2026-04-28 | 022a76a | [260428-3qg-fix-backend-test-compile-errors-by-addin](./quick/260428-3qg-fix-backend-test-compile-errors-by-addin/) |

## Session Continuity

Last session: 2026-04-30T02:24:33.566Z
Stopped at: Completed 10-05-PLAN.md — v1.1 Backlog section added to REQUIREMENTS.md; 06-VERIFICATION.md deferred row 1 cross-references DEPL-03-AUTO; bidirectional deferral audit trail complete
Resume file: None

**Planned Phase:** 9 (E2E Playwright test suite) — 13 plans — 2026-04-29T01:09:39.584Z
