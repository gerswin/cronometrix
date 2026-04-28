---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 04A
type: execute
wave: 4
depends_on: [08-03]
files_modified:
  # New backend integration / unit test files (one per gap module; some may extend existing files instead — see <action>).
  # 16 source modules in this bucket; expected ~12-16 new test files (some modules can share a file).
  - backend/tests/auth_handlers_extra_test.rs
  - backend/tests/auth_models_test.rs
  - backend/tests/anomalies_handlers_test.rs
  - backend/tests/calc_anomalies_test.rs
  - backend/tests/config_from_env_test.rs
  - backend/tests/daily_records_handlers_test.rs
  - backend/tests/daily_records_service_test.rs
  - backend/tests/db_mod_test.rs
  - backend/tests/departments_service_test.rs
  - backend/tests/devices_models_test.rs
  - backend/tests/employees_service_test.rs
  - backend/tests/events_handlers_extra_test.rs
  - backend/tests/isapi_client_test.rs
  - backend/tests/leaves_handlers_extra_test.rs
  - backend/tests/leaves_service_test.rs
  - backend/tests/state_paths_test.rs
  # Possibly modified ONLY if exclusions are accepted at Plan 04C checkpoint (≤3/side cap shared across 04A+04B):
  - Makefile
autonomous: false
requirements: [QUALITY-GATE]
must_haves:
  truths:
    - "Every backend domain file in this bucket reaches ≥70% line coverage when measured by `make coverage-backend`"
    - "Backend domain files in this bucket reach ≥60% branch coverage under nightly toolchain (deferred measurement; CI in Plan 05 verifies branch numbers — see <branch_coverage_note>)"
    - "Existing 319 backend tests still pass: `cd backend && cargo nextest run` exits 0 after additions"
    - "Closing this bucket lifts project-wide backend line% materially toward 90% (this bucket holds the largest line-deficit modules: anomalies/handlers 0%, config 0%, daily_records/handlers 0%)"
    - "No file in this bucket is excluded from coverage without explicit justification surfaced at Plan 04C checkpoint"
  artifacts:
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md"
      provides: "Read-only INPUT — authoritative gap list. Every test file written corresponds to a row in the backend gap table."
      contains: "FAIL:"
    - path: "backend/tests/<one file per backend domain bucket row, ≤16 total>"
      provides: "New backend integration / unit tests; each closes a row in the baseline backend gap table"
      contains: "#[tokio::test]"
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md"
      provides: "Documents which backend domain modules were closed, before% → after% per file, exclusions (if any), patterns used"
      contains: "Files closed"
  key_links:
    - from: "make coverage-backend"
      to: "scripts/enforce-coverage-floor.sh + cargo llvm-cov nextest"
      via: "Makefile invocation"
      pattern: "make coverage-backend"
    - from: "backend/tests/<new test files>"
      to: "backend/src/<source modules>"
      via: "axum-test::TestServer + common::test_state_with_tmpdir + wiremock"
      pattern: "common::test_state_with_tmpdir|TestServer::new"
---

<objective>
Close the **backend domain** subset of the Plan 03 baseline gap (16 files): authentication handlers/models, calc/anomalies, config + db + state, devices models, daily_records (handlers + service), departments/employees services, events handlers, isapi client, leaves (handlers + service). After this plan, every backend file in this bucket reaches ≥70% line / ≥60% branch (branch verified in CI under nightly per Plan 03 baseline caveat).

Purpose: This is the FIRST of three sub-plans splitting Plan 04 by subsystem. Plan 03's baseline measured 27 backend + 21 frontend = 48 files below the per-file floor (post the three D-09 pure-display exclusions already wired in `vitest.config.ts`). The original Plan 04 carried a 15-file scope cap; 48 > 15 mandated escalation. The user approved a 3-way split by subsystem: 04A (this plan, backend domain), 04B (backend infrastructure: enrollments + workers + license + recompute + supervisor), 04C (frontend). 04A→04B→04C run sequentially in wave 4; the 04C checkpoint is the single human verification gate for the whole gap-fill set.

Output: New tests under `backend/tests/` (≤16 files, mostly one-per-source-module; small modules with overlapping fixtures may share a test file — documented in 04A-SUMMARY). After 04A lands, only the backend infrastructure bucket (04B) and the frontend bucket (04C) remain to close before `make coverage` exits 0.
</objective>

<bucket_definition>
**Source files in this bucket (16 modules — drawn verbatim from `08-03-COVERAGE-BASELINE.md` backend FAIL list):**

| # | Source file | Baseline line% | Test target |
|---|---|---|---|
| 1 | `backend/src/anomalies/handlers.rs` | 0.00% | `tests/anomalies_handlers_test.rs` |
| 2 | `backend/src/auth/handlers.rs` | 67.39% | `tests/auth_handlers_extra_test.rs` (extend; existing `auth_tests.rs` covers happy paths) |
| 3 | `backend/src/auth/models.rs` | 30.77% | `tests/auth_models_test.rs` (likely inline `#[cfg(test)] mod tests`) |
| 4 | `backend/src/calc/anomalies.rs` | 61.54% | `tests/calc_anomalies_test.rs` (or inline; mirrors `calc_tests.rs` pattern) |
| 5 | `backend/src/config.rs` | 0.00% | `tests/config_from_env_test.rs` |
| 6 | `backend/src/daily_records/handlers.rs` | 0.00% | `tests/daily_records_handlers_test.rs` (extend `daily_record_tests.rs` if size grows) |
| 7 | `backend/src/daily_records/service.rs` | 53.10% | `tests/daily_records_service_test.rs` |
| 8 | `backend/src/db/mod.rs` | 46.67% | `tests/db_mod_test.rs` |
| 9 | `backend/src/departments/service.rs` | 66.95% | `tests/departments_service_test.rs` (extend `department_tests.rs` if simpler) |
| 10 | `backend/src/devices/models.rs` | 50.00% | `tests/devices_models_test.rs` (likely inline `#[cfg(test)] mod tests`) |
| 11 | `backend/src/employees/service.rs` | 61.29% | `tests/employees_service_test.rs` (extend `employee_tests.rs` if simpler) |
| 12 | `backend/src/events/handlers.rs` | 55.68% | `tests/events_handlers_extra_test.rs` (extend `event_tests.rs` if simpler) |
| 13 | `backend/src/isapi/client.rs` | 57.23% | `tests/isapi_client_test.rs` (uses `wiremock`) |
| 14 | `backend/src/leaves/handlers.rs` | 46.56% | `tests/leaves_handlers_extra_test.rs` (extend `leave_tests.rs`) |
| 15 | `backend/src/leaves/service.rs` | 69.87% | `tests/leaves_service_test.rs` (just below floor; one or two tests probably suffice) |
| 16 | `backend/src/state/paths.rs` | 33.33% | `tests/state_paths_test.rs` (likely inline `#[cfg(test)] mod tests` for `from_env`/`for_test` constructors) |

**File count: 16.** This exceeds the original Plan 04's 15-file scope cap by 1, justified by:
1. The cap existed for the *combined* Plan 04 (backend + frontend, ~50% context budget). Splitting into 04A/04B/04C distributes the work across three context windows; each child can carry up to ~15-20 files within its own ~50% budget.
2. The user approved the 3-way split with the explicit understanding that 04A/04C would each run close to the original cap.
3. Several rows in this bucket are *already* near the floor (auth/handlers 67.39%, calc/anomalies 61.54%, departments/service 66.95%, leaves/service 69.87%) — closing them needs 1-2 tests each, not a full suite.
4. Three rows can plausibly use **inline `#[cfg(test)] mod tests`** instead of new files (auth/models, devices/models, state/paths — pure data structures or pure constructors with no I/O), reducing the file count below 16 if the executor judges that cleaner.

**Files NOT in this bucket** (handled by 04B): enrollments/{handlers, models, pusher, service}, license/{fingerprint, service}, recompute/{nightly, worker}, supervisor/watchdog, workers/{backfill, purge}.

**Files NOT in this bucket** (handled by 04C): all 21 frontend post-D-09 FAIL files.
</bucket_definition>

<branch_coverage_note>
Per Plan 03 baseline (08-03-COVERAGE-BASELINE.md run-time provenance §): the local baseline run used **stable rustc 1.93.0** (Homebrew, no rustup) — `--branch` is nightly-only, so backend lcov reported `BRF=0` for every record and the post-processor reported branch% as 100% for every file (no data). The line% gap list is accurate; branch% is deferred to Plan 05's CI run under nightly.

**What this means for 04A:**
- Plan 04A targets line% ≥70% for every file in the bucket (the measurable signal locally today).
- Plan 04A also writes branch-exercising tests (e.g., handler error paths, validator-failure branches per CONTEXT D-12) because nightly CI in Plan 05 will measure branch% and re-failure of the gate then would force a return to gap-fill work — undesirable.
- The Plan 04C checkpoint (final reviewer) MAY require running `rustup install nightly-2026-04-01 && rustup component add llvm-tools-preview` locally and re-running `make coverage-backend` with the recipe's `--branch` flag to verify ≥60% branch on every bucket file before approving. The executor should attempt this; if the local box has no rustup, document the limitation in 04A-SUMMARY (same caveat as Plan 03 used).
</branch_coverage_note>

<scope_cap>
**Hard cap on Plan 04A scope:** at most **16 new test files** AND at most **5 hours** of estimated work.

The bucket list above is the authoritative gap. Before adding tests, the executor MUST:

1. Re-read `08-03-COVERAGE-BASELINE.md` and verify the 16 backend rows in the table above still match the baseline. If a row is no longer present (e.g., Plan 04A landed mid-stream and re-measurement shows it already passing), skip it.
2. If for any reason the file count grows beyond 16 (e.g., a single source module turns out to need two test files for organisation), stop and confirm with the user before exceeding.
3. If estimated work exceeds 5 hours (e.g., >20 min/file average due to complex `wiremock` setups for `isapi/client`), stop and confirm.

**Why this cap exists:** Each of 04A/04B/04C must complete within ~50% context. Padding any one of them past its cap pushes the sub-plan into the quality-degradation zone (>50% context).
</scope_cap>

<exclusions_policy>
**No exclusions are pre-approved at planning time** — same rule as the original Plan 04. If the executor encounters a file whose coverage genuinely cannot be raised above the floor, the file is NOT auto-excluded:

1. The executor surfaces the file at the **Plan 04C human checkpoint** (the single shared checkpoint for all three sub-plans) with:
   - The file's measured coverage and a specific reason it cannot be raised (e.g., "requires real ISAPI device traffic that wiremock cannot simulate", "covers a `Display` impl with no observable behavior").
   - The proposed exclusion regex (Makefile `--ignore-filename-regex`).
2. The exclusion is added to `Makefile`'s `--ignore-filename-regex` ONLY after reviewer sign-off at the 04C checkpoint.
3. The same exclusion is mirrored as a row in `CLAUDE.md`'s `## Test Coverage` exclusion table — Plan 06 Task 1 picks this up. The CLAUDE.md edit and the Makefile edit MUST be a single commit so a future audit can diff them and see no drift.
4. Hard cap: **≤3 exclusions on the backend side total across 04A + 04B combined**. Hitting the cap at 04A consumes the budget for 04B too.

**Specifically for files in this bucket:**
- No file in 04A's bucket has a planning-time pre-approved exclusion. `state/paths.rs` was created by Plan 01 and is well-defined surface (env-or-default + tempdir constructor); its 33.33% line% is a true gap, not a "hard to test" call.
</exclusions_policy>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-CONTEXT.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-RESEARCH.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-PATTERNS.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-UI-SPEC.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-01-SUMMARY.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-02-SUMMARY.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-SUMMARY.md

@backend/Cargo.toml
@backend/tests/common/mod.rs
@backend/tests/auth_tests.rs
@backend/tests/event_tests.rs
@backend/tests/leave_tests.rs
@backend/tests/department_tests.rs
@backend/tests/employee_tests.rs
@backend/tests/calc_tests.rs

<interfaces>
<!-- Per Plan 02 SUMMARY, the canonical backend test fixture is: -->
<!-- let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config); -->
<!-- _tmp MUST be bound to a local that outlives the assertions (Plan 01 Pitfall 1). -->
<!-- Helpers in backend/tests/common/mod.rs available to extend: test_db, test_state, test_state_with_tmpdir. -->

<!-- Per Plan 02 SUMMARY, helpers like build_test_app already exist in: -->
<!--   - auth_tests.rs (returns (Router, TempDir)) -->
<!--   - department_tests.rs (returns (Router, TempDir)) -->
<!--   - employee_tests.rs (returns (Router, TempDir)) -->
<!--   - event_tests.rs (returns (Router, AppState, TempDir)) -->
<!--   - leave_tests.rs (uses make_state which returns (AppState, TempDir)) -->
<!-- New extra tests SHOULD reuse these fixtures rather than re-rolling AppState construction. -->

<!-- dev-deps available (backend/Cargo.toml): -->
<!--   - axum-test 16 (TestServer for handler tests) -->
<!--   - wiremock 0.6.5 (HTTP mocking — required for isapi/client.rs digest auth retry) -->
<!--   - tempfile 3 (TempDir — used via test_state_with_tmpdir) -->
<!--   - proptest 1.11 (use ONLY where it provably closes a branch) -->

<!-- Per UI-SPEC: this bucket is backend-only — `ui_surface: none`. No frontend changes. -->

<!-- Per security threat model (CONTEXT.md):
     New tests for auth/handlers, leaves/handlers, daily_records/handlers should cover BOTH the
     fail-closed (4xx) and pass paths. Negative-path coverage of security controls is itself
     a security control (RESEARCH § "Validating the Gate Itself"). -->
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Backend domain bucket — handlers + services + models gap-fill (Phase A)</name>
  <files>
    backend/tests/anomalies_handlers_test.rs (new — 0% → ≥70%),
    backend/tests/auth_handlers_extra_test.rs (new — 67.39% → ≥70%),
    backend/tests/auth_models_test.rs (new OR inline #[cfg(test)] mod tests in src/auth/models.rs — 30.77% → ≥70%),
    backend/tests/calc_anomalies_test.rs (new OR inline in src/calc/anomalies.rs — 61.54% → ≥70%),
    backend/tests/config_from_env_test.rs (new — 0% → ≥70%),
    backend/tests/daily_records_handlers_test.rs (new — 0% → ≥70%),
    backend/tests/daily_records_service_test.rs (new — 53.10% → ≥70%),
    backend/tests/db_mod_test.rs (new — 46.67% → ≥70%)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 8 module rows still appear; baseline drives the test list)
    - backend/tests/common/mod.rs §test_state_with_tmpdir (canonical fixture per Plan 02)
    - backend/tests/auth_tests.rs (existing build_test_app pattern; auth_handlers_extra_test extends this)
    - backend/tests/daily_record_tests.rs (existing make_state pattern; daily_records_handlers_test extends this)
    - backend/src/anomalies/handlers.rs (read whole file — currently 0% line; identify every public route handler and its branches)
    - backend/src/auth/handlers.rs (find the 32.61% uncovered region — likely refresh-token, logout, or password-change error branches)
    - backend/src/auth/models.rs (read whole — likely Display/Default/From impls or argon2 hash error branches)
    - backend/src/calc/anomalies.rs (find the 38.46% uncovered branches — likely tolerance edge cases or shift-detection rejection cases)
    - backend/src/config.rs (read whole — Config::from_env covers 5+ env vars; baseline shows 0% so neither branch is exercised today)
    - backend/src/daily_records/handlers.rs (read whole — currently 0%, including the override-write path that uses state.paths.overrides_root from Plan 01)
    - backend/src/daily_records/service.rs (find the 46.90% uncovered region — likely shift-rule application branches or error variants)
    - backend/src/db/mod.rs (read whole — find the 53.33% uncovered branches; likely libsql connection-error or migration-error variants)
  </read_first>
  <coverage_discipline>
    Same rules as the original Plan 04 (NOT TDD — gap-fill against measured numbers):
    - `#[tokio::test]` for async tests (every handler test).
    - `axum-test::TestServer` for handler integration tests; reuse the `build_test_app` helper from `auth_tests.rs` / `event_tests.rs` / `leave_tests.rs` / `daily_record_tests.rs`.
    - `wiremock` ONLY in this Phase if a handler under test calls outbound HTTP (none in this bucket — wiremock is needed in Task 2 for isapi/client). Do not add new dev-deps.
    - `tempfile::TempDir` via `common::test_state_with_tmpdir(...)` for any test that touches the filesystem.
    - `proptest` only if the bucket forces it (extremely unlikely for this Phase — none of the 8 modules in Phase A do branchy math; calc/anomalies.rs is closest, and a handful of explicit cases will close it faster than a property).

    Test what is uncovered, not what is convenient. Use the HTML coverage report (`backend/target/llvm-cov/html/index.html`) regenerated after each batch to drive the next batch.

    **Per-source-file test design:**

    1. `backend/src/anomalies/handlers.rs` (0% line). Identify every public handler (likely `list_anomalies`, `acknowledge_anomaly`, plus a `delete_anomaly`-style endpoint). For each:
       - Happy path with seeded state (200/204 + correct body shape).
       - Auth-fail branch (401 with no JWT, 403 with insufficient role per `auth/middleware.rs`/`auth/rbac.rs`).
       - Validator-failure branch (400 on bad payload — e.g., bogus `anomaly_id` UUID or out-of-range page).
       - DB-error branch where applicable (use a fake `AppState` whose db is dropped or read-only — at minimum exercise the "not found" 404 path).
       Write as `#[tokio::test]` integration tests using `TestServer` against a `build_test_app(db)` helper.

    2. `backend/src/auth/handlers.rs` (67.39% → ≥70%). Already partially covered by `auth_tests.rs`. Identify the uncovered region (use HTML report — likely `refresh_token`, `change_password`, or `logout` error branches). Add 2-3 targeted tests:
       - Refresh with expired/invalid token → 401.
       - Change-password with wrong current password → 401.
       - Change-password with valid old + new → 204 (verify hash changed in DB).
       Place in `auth_handlers_extra_test.rs` to keep `auth_tests.rs` semantically clean.

    3. `backend/src/auth/models.rs` (30.77% → ≥70%). Likely contains password-hash helpers / Claims / token types. Inline `#[cfg(test)] mod tests` is preferred (existing pattern — `events/service.rs` and `calc/*` use it). Tests:
       - `Claims::new` happy path + edge cases (zero-duration exp, max-duration exp).
       - Password hash + verify roundtrip; verify with wrong password → false.
       - Any `From`/`TryFrom` impls for token types — happy + error cases.
       If the file's logic is too thin for an inline mod (pure newtypes), bypass with a `tests/auth_models_test.rs` integration file that exercises observable shape via `auth/handlers.rs`.

    4. `backend/src/calc/anomalies.rs` (61.54% → ≥70%). Likely a pure-function module that classifies attendance events as `LATE`/`EARLY_LEAVE`/`MISSING_OUT`/etc. given a shift definition + tolerance. Inline `#[cfg(test)] mod tests` (matches `calc/lunch.rs`, `calc/overnight.rs` which are already at 82-95%):
       - Each anomaly variant has a "produces this anomaly" case AND a "does not produce" case.
       - Tolerance boundary: exactly-at-tolerance, just-inside, just-outside.
       - Multi-anomaly day (employee both LATE and EARLY_LEAVE).
       Reuse fixture builders from `backend/src/calc/engine.rs`'s existing tests if any.

    5. `backend/src/config.rs` (0% → ≥70%). Inline `#[cfg(test)] mod tests`:
       - `Config::from_env` happy path: set all required env vars, parse, assert each field.
       - Each unwrap_or_else default: unset the env var, assert the default value lands in the field.
       - `SERVER_PORT` parse-error: set to non-numeric, assert anyhow error.
       - `DEVICE_CREDS_KEY` parse-error: set to non-base64, assert anyhow error.
       - Manual `Debug` impl redaction: format a Config, assert `jwt_secret` and `device_creds_key` do NOT appear in plaintext.
       Use `temp_env::with_vars` if available (otherwise a small RAII guard local to the test mod).

    6. `backend/src/daily_records/handlers.rs` (0% → ≥70%). Identify every public handler. For each:
       - Happy path GET (list daily records for a date range) with seeded state.
       - Override POST/PATCH (uses `state.paths.overrides_root` from Plan 01 — verify the file lands in the tempdir).
       - Validator-failure branch (400 on bad date, bad employee_id).
       - 401 on missing JWT, 403 on insufficient role.
       Use `daily_record_tests.rs::make_state` helper (already returns `(AppState, TempDir)` per Plan 02).

    7. `backend/src/daily_records/service.rs` (53.10% → ≥70%). Find the uncovered service-layer functions (likely `apply_override`, `recalculate_for_date`, or similar). Inline `#[cfg(test)] mod tests` OR `daily_records_service_test.rs` (depends on whether the functions need a full DB or can take a `Connection`).
       - Each function's happy path + at least one error branch.
       - Boundary cases (empty input, single record, bulk recompute).

    8. `backend/src/db/mod.rs` (46.67% → ≥70%). Likely contains `init_db`, `connect`, `migrate` helpers. Tests:
       - `init_db` happy path against a fresh tempdir SQLite.
       - `connect` against a non-existent path → expect specific libsql error.
       - Migration application: run twice, assert idempotent.
       Use `common::test_db()` helper as the analog (it already constructs a libsql `Database` for tests).

    **After each batch (~3 modules), re-run `make coverage-backend`** (or the off-recipe equivalent if the local box lacks rustup) and inspect the HTML report. Stop adding tests once the per-file floor is met.
  </coverage_discipline>
  <action>
    Execute the per-source-file test design listed in <coverage_discipline> for the 8 Phase-A modules, in any order the executor finds natural (the modules in Phase A are mutually independent — no test depends on another's fixtures).

    **Step 1 — re-read the baseline.** Confirm the 8 rows in the Task 1 file list still appear in `08-03-COVERAGE-BASELINE.md`. If a row is missing (e.g., a previous orchestrator run already closed it), skip and note in 04A-SUMMARY.

    **Step 2 — for each source file, READ THE SOURCE FIRST** to identify the specific uncovered branches. A useful starter heuristic:
    ```bash
    grep -nE 'match|if let|\.is_ok\(\)|\.is_err\(\)|\.is_some\(\)|\.is_none\(\)' <source-file>
    ```
    Then write the minimum test set that lifts the file ≥70% line (and ≥60% branch under nightly — add branch-exercising tests preemptively).

    **Step 3 — run the suite after each batch.** Use:
    ```
    cd backend && cargo nextest run --test <new-test-file-name>
    ```
    to validate the file in isolation. Then run `make coverage-backend` (or the off-recipe stable-rustc command if rustup unavailable — see Plan 03 SUMMARY) to confirm the file moved above floor.

    **Step 4 — exclusion handling.** No file in this bucket has a planning-time pre-approved exclusion. If a file genuinely cannot be lifted above floor, surface at the Plan 04C checkpoint (the shared one) with a written rationale.

    Per UI-SPEC: backend-only — `ui_surface: none`. No frontend changes.

    Per security threat model: `auth/handlers.rs`, `daily_records/handlers.rs`, and `leaves/handlers.rs` (Task 2) tests MUST cover both fail-closed (401/403/4xx) and pass paths. Negative-path coverage is itself a security control.
  </action>
  <verify>
    <automated>cd backend && cargo nextest run > /tmp/cov-04a-task1.log 2>&1; ec=$?; tail -10 /tmp/cov-04a-task1.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-backend > /tmp/cov-04a-task1-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04a-task1-gate.log; awk '/^FAIL:.*(anomalies\/handlers|auth\/handlers|auth\/models|calc\/anomalies|config\.rs|daily_records\/(handlers|service)|db\/mod)/' /tmp/cov-04a-task1-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd backend && cargo nextest run` exits 0 (no regression from new tests; ~319 + new tests pass).
    - Every Phase-A source file (8 listed in <files>) is at or above 70% line in the Makefile gate output for `make coverage-backend`. If the local box lacks nightly, the executor runs the off-recipe stable-rustc command per Plan 03 SUMMARY and confirms line% gain in the lcov post-processor output.
    - HTML report renders: `backend/target/llvm-cov/html/index.html` exists after the run.
    - Every test added uses ONLY existing fixtures (`common::test_state_with_tmpdir`, existing `build_test_app` helpers); no new dev-deps; no env-var mutation in tests (per Plan 02's banned pattern).
    - No exclusions added to `Makefile`'s `--ignore-filename-regex` in this task. (Exclusions, if any, surface at Plan 04C checkpoint and are landed by Plan 06.)
    - No regression in already-passing files: re-run `bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60` and confirm no NEW `FAIL:` lines appear for files NOT in the 04A bucket. (Files in 04B's bucket — enrollments, license, recompute, supervisor, workers — will still appear as FAIL; that's expected and resolves in Plan 04B.)
  </acceptance_criteria>
  <done>
    8 Phase-A backend modules ≥70% line. Tests follow existing patterns (no new dev-deps, no env-var mutation, fixtures via `common::test_state_with_tmpdir` + existing `build_test_app` helpers). 04A-SUMMARY captures before% → after% per file.
  </done>
</task>

<task type="auto">
  <name>Task 2: Backend domain bucket — devices/models, departments/employees services, events/handlers, isapi/client, leaves/handlers, leaves/service, state/paths (Phase B)</name>
  <files>
    backend/tests/departments_service_test.rs (new — 66.95% → ≥70%; OR extend department_tests.rs),
    backend/tests/devices_models_test.rs (new OR inline #[cfg(test)] in src/devices/models.rs — 50% → ≥70%),
    backend/tests/employees_service_test.rs (new — 61.29% → ≥70%; OR extend employee_tests.rs),
    backend/tests/events_handlers_extra_test.rs (new — 55.68% → ≥70%; OR extend event_tests.rs),
    backend/tests/isapi_client_test.rs (new — 57.23% → ≥70%; uses wiremock for digest auth retry),
    backend/tests/leaves_handlers_extra_test.rs (new — 46.56% → ≥70%; extend leave_tests.rs),
    backend/tests/leaves_service_test.rs (new OR inline — 69.87% → ≥70%; one test likely sufficient),
    backend/tests/state_paths_test.rs (new OR inline #[cfg(test)] in src/state/paths.rs — 33.33% → ≥70%)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 8 module rows still appear)
    - backend/tests/common/mod.rs §test_state_with_tmpdir
    - backend/tests/department_tests.rs (existing build_test_app pattern)
    - backend/tests/employee_tests.rs (existing build_test_app pattern)
    - backend/tests/event_tests.rs (existing build_test_app pattern returning (Router, AppState, TempDir))
    - backend/tests/leave_tests.rs (existing make_state pattern returning (AppState, TempDir))
    - backend/tests/listener_tests.rs (existing wiremock-style ISAPI test pattern, if present)
    - backend/Cargo.toml (verify wiremock 0.6.5 in dev-deps)
    - backend/src/departments/service.rs (find the 33.05% uncovered — likely error variants in CRUD)
    - backend/src/devices/models.rs (likely ConnectionState transitions, Display impls, From conversions)
    - backend/src/employees/service.rs (find the 38.71% uncovered — likely bulk-insert, encoding edge cases)
    - backend/src/events/handlers.rs (find the 44.32% uncovered — likely the SSE stream branch + photo retrieval error paths)
    - backend/src/isapi/client.rs (find the 42.77% uncovered — digest auth challenge-response retry; door-open + sync endpoints)
    - backend/src/leaves/handlers.rs (find the 53.44% uncovered — POST create_leave validator branches + multipart file extension validation)
    - backend/src/leaves/service.rs (find the 30.13% uncovered — write_evidence error branches; persist_leave conflict)
    - backend/src/state/paths.rs (Plan 01 created this — Paths::from_env covers 5 env-or-default branches; Paths::for_test covers 1; baseline shows 33.33% so two of the three logical paths are unexercised)
  </read_first>
  <coverage_discipline>
    Same rules as Task 1. Notable additions for this Phase B batch:

    - `wiremock` is REQUIRED for `isapi/client.rs` testing — this is the canonical pattern per RESEARCH.md.
      - Mock the device endpoint to return `401` with a `WWW-Authenticate: Digest realm="..."` header on first request.
      - Assert the client computes the digest and re-issues with `Authorization: Digest username="..." response="..."`.
      - Mock returns `200` on the retry; assert the parsed response shape.
      - Cover at minimum: door-open (`PUT /ISAPI/RemoteControl/door/0`), status-check (`GET /ISAPI/System/status`), and a non-2xx error path (`PUT` returning `500` → `Result::Err`).

    **Per-source-file test design:**

    1. `backend/src/departments/service.rs` (66.95% → ≥70%). Read source. Likely 1-2 tests close the gap. Existing `department_tests.rs` already covers happy paths via handlers; add unit-level coverage for any pure helper or error variant exposed by the service layer (e.g., `Department::validate_name` length-bound branches, `delete_department` cascade-conflict error).

    2. `backend/src/devices/models.rs` (50% → ≥70%). Inline `#[cfg(test)] mod tests` in the source file. Likely covers:
       - `ConnectionState` enum variants (Display/Debug strings; `from_str` for any DB serialization).
       - `Device::is_online`-style helpers if present.
       - Any `Hostname::parse`/`Slug::new` validators.

    3. `backend/src/employees/service.rs` (61.29% → ≥70%). Likely uncovered: bulk operations, edge cases on department-FK resolution. 1-2 service-level tests using `common::test_db` + direct service calls (no full app needed).

    4. `backend/src/events/handlers.rs` (55.68% → ≥70%). Existing `event_tests.rs` covers `get_event_photo` happy path post-Plan-01. Find the uncovered: likely the SSE stream handler (`event_broadcast` → `Sse<...>`) and any pagination-error branch. Add to `events_handlers_extra_test.rs`:
       - SSE stream: subscribe, broadcast one event, assert it lands in the stream (use `tower::ServiceExt::ready` + reading from the response body).
       - Photo retrieval: 404 when path-traversal would escape `state.paths.events_root` (security control coverage — already mostly there from Plan 01, just verify).

    5. `backend/src/isapi/client.rs` (57.23% → ≥70%). Use `wiremock`:
       - Test 1: door-open success after digest challenge → assert request count 2 (challenge + retry), final response 200.
       - Test 2: status check returns 200 immediately → assert no challenge/retry needed.
       - Test 3: door-open returns 500 → assert `Err(...)` with the expected error variant.
       - Test 4: first response missing `WWW-Authenticate` header on 401 → assert client returns digest-auth error (does not loop).

    6. `backend/src/leaves/handlers.rs` (46.56% → ≥70%). Existing `leave_tests.rs` covers happy paths. Add `leaves_handlers_extra_test.rs`:
       - `create_leave` with bad date range (end < start) → 400.
       - `create_leave` with unsupported file extension → 400 (per the file-extension allowlist).
       - `create_leave` with file > size limit → 400.
       - `get_leave_evidence` with traversal payload (`../../../etc/passwd`) → 400 or 404 (canonicalize-fail branch — security control).
       - `delete_leave` 404 (id not in DB).

    7. `backend/src/leaves/service.rs` (69.87% — fractionally below floor → ≥70%). One test likely closes it. Find the single uncovered branch via HTML report; could be the `write_evidence` IO-error branch or the `persist_leave` conflict-error branch.

    8. `backend/src/state/paths.rs` (33.33% → ≥70%). Inline `#[cfg(test)] mod tests`:
       - `Paths::from_env` happy path: set all 5 env vars, assert each field equals the env value.
       - `Paths::from_env` defaults: unset all env vars, assert each field equals the documented default (`./data/leaves`, `./data/events`, `./data/enrollments`, `/tmp/enrollments-captures`, `./data`).
       - `Paths::for_test`: pass a `TempDir::new().unwrap().path()`, assert each field is a subdir of it.
       - `env_or_default` private helper: covered transitively by the from_env tests.
  </coverage_discipline>
  <action>
    Execute the per-source-file test design listed in <coverage_discipline> for the 8 Phase-B modules, in any order. wiremock-using tests (`isapi_client_test.rs`) are the most setup-heavy; the executor may save them for last.

    **Step 1 — re-read the baseline.** Same as Task 1.

    **Step 2 — for each source file, READ THE SOURCE FIRST** to identify the specific uncovered branches.

    **Step 3 — run the suite after each batch.**

    **Step 4 — exclusion handling.** Same as Task 1. If `isapi/client.rs` cannot be raised because wiremock cannot simulate a digest challenge accurately (extremely unlikely — RESEARCH cites wiremock as the canonical pattern), surface at the Plan 04C checkpoint.

    Per UI-SPEC: backend-only.

    Per security threat model (T-08-12 from original Plan 04): leaves/handlers and events/handlers traversal-rejection branch coverage is a positive security control; tests must include the negative path.
  </action>
  <verify>
    <automated>cd backend && cargo nextest run > /tmp/cov-04a-task2.log 2>&1; ec=$?; tail -10 /tmp/cov-04a-task2.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-backend > /tmp/cov-04a-task2-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04a-task2-gate.log; awk '/^FAIL:.*(departments\/service|devices\/models|employees\/service|events\/handlers|isapi\/client|leaves\/(handlers|service)|state\/paths)/' /tmp/cov-04a-task2-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd backend && cargo nextest run` exits 0.
    - Every Phase-B source file (8 listed in <files>) is at or above 70% line in the Makefile gate output. (Branch% is a CI-under-nightly verification per <branch_coverage_note>; locally executor verifies line% only if rustup unavailable.)
    - `wiremock` is used in `isapi_client_test.rs` per the canonical RESEARCH pattern; no manual HTTP server roll-up.
    - All Phase-A files (Task 1) remain ≥70% — no regression.
    - Cumulative for 04A: all 16 source files in this bucket are at or above 70% line. After 04A lands, the only remaining backend FAILs in `make coverage-backend` output should be the 11 modules in 04B's bucket.
    - No new flaky tests: `cd backend && cargo nextest run` exits 0 across 3 successive runs (validate by running 3× in a row before committing).
  </acceptance_criteria>
  <done>
    All 16 backend domain modules in the 04A bucket ≥70% line. Cumulative effect: 27 → 11 backend FAILs (the 11 remaining are 04B's bucket). 04A-SUMMARY captures before% → after% per file and any patterns established for downstream sub-plans.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| test → coverage report | Tests are local code; coverage report is local artifact; no untrusted data. |
| wiremock → backend test | wiremock spawns a local HTTP server bound to localhost only; used only in `isapi_client_test.rs`. |
| HTTP client (reqwest+diqwest) → wiremock mock | Outbound digest-auth flow under test; no real ISAPI device contacted. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-08-11A | Tampering | Coverage exclusion abuse (backend domain bucket) | mitigate | No exclusions are pre-approved at planning time. If a file cannot be raised, executor surfaces at Plan 04C shared checkpoint with written rationale; reviewer decides. Hard cap of 3 backend exclusions across 04A+04B combined. |
| T-08-12A | Repudiation | Audit-trail tests on negative paths in this bucket | mitigate | Tests for `auth/handlers.rs` (refresh/change-password failures), `daily_records/handlers.rs` (401/403), `leaves/handlers.rs` (path traversal rejection), `events/handlers.rs` (photo path traversal) MUST cover the negative path. Negative-path coverage is itself a security control per RESEARCH § "Validating the Gate Itself". |
| T-08-13A | Information Disclosure | Test fixtures containing sensitive data | accept | Existing repo fixtures (`tests/fixtures/test_license_*.pem`, `alertstream_*.bin`) are already committed; new fixtures introduced by 04A must contain only synthetic data (random UUIDs, fake employee names, no real PII). Existing repo policy applies; no new exposure introduced. |
| T-08-14A | Tampering | wiremock test reachability | accept | wiremock binds to a localhost ephemeral port and is dropped at end of test. No persistent network surface; no risk of cross-test leakage. |
</threat_model>

<verification>
1. `cd backend && cargo nextest run` exits 0 (no regression from 04A's new tests).
2. `make coverage-backend` (or off-recipe stable-rustc fallback) exits with EVERY 04A source file ≥70% line. The remaining FAILs are exactly the 11 files in 04B's bucket.
3. HTML report renders: `backend/target/llvm-cov/html/index.html` exists after the run.
4. No new flaky tests: `cargo nextest run` 3× in succession all green.
5. No exclusions added in this plan; if any are needed, surfaced at Plan 04C checkpoint.
6. Every test follows existing repo patterns (`#[tokio::test]`, `axum-test`, `wiremock`, `common::test_state_with_tmpdir`); no new dev-deps.
7. 04A-SUMMARY exists at `.planning/phases/08-.../08-04A-SUMMARY.md` documenting per-file before% → after% and any Phase-A patterns extended for 04B/04C.

This is the green-light signal for Plan 04B to begin.
</verification>

<success_criteria>
- All 16 backend domain modules in this bucket ≥70% line (and ≥60% branch under nightly when measurable).
- Cumulative effect on `make coverage-backend`: 27 backend FAILs → 11 backend FAILs (the 11 are 04B's bucket).
- Test additions follow existing patterns (no new test framework, no new dev-deps, no env-var mutation in tests).
- 04A-SUMMARY exists and documents per-file deltas + any patterns to carry into 04B.
</success_criteria>

<output>
After completion, create `.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md` with:
- Files closed (path + before% → after%)
- Test files added (path list — actual files, not the speculative list in <files>)
- Patterns established or extended (e.g., wiremock digest-auth pattern, inline `#[cfg(test)] mod tests` for pure-data modules)
- Any files surfaced for the Plan 04C checkpoint as exclusion candidates (with written rationale)
- Final per-file backend numbers for the 16 bucket files
- Note any local-vs-CI toolchain caveats encountered (e.g., re-confirm Plan 03's stable-rustc workaround if rustup absent)
</output>
