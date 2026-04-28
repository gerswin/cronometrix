---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 04B
type: execute
wave: 4
depends_on: [08-04A]
files_modified:
  # New backend integration / unit test files for the infrastructure subsystem (enrollments + workers + license + recompute + supervisor).
  # 11 source modules in this bucket; expected ~9-11 new test files (a few small modules can share a file).
  - backend/tests/enrollments_handlers_test.rs
  - backend/tests/enrollments_models_test.rs
  - backend/tests/enrollments_pusher_test.rs
  - backend/tests/enrollments_service_test.rs
  - backend/tests/license_fingerprint_test.rs
  - backend/tests/license_service_extra_test.rs
  - backend/tests/recompute_nightly_test.rs
  - backend/tests/recompute_worker_test.rs
  - backend/tests/supervisor_watchdog_test.rs
  - backend/tests/workers_backfill_test.rs
  - backend/tests/workers_purge_test.rs
  # Possibly modified ONLY if exclusions are accepted at Plan 04C checkpoint (≤3 backend total across 04A+04B):
  - Makefile
autonomous: false
requirements: [QUALITY-GATE]
must_haves:
  truths:
    - "Every backend infrastructure file in this bucket reaches ≥70% line coverage when measured by `make coverage-backend`"
    - "Backend infrastructure files in this bucket reach ≥60% branch coverage under nightly toolchain (CI in Plan 05 verifies branch numbers — same caveat as 04A)"
    - "Existing tests still pass: `cd backend && cargo nextest run` exits 0 after additions"
    - "After 04A + 04B land, the backend gate is GREEN: project ≥90% line + every counted file ≥70% line. Project branch verifies in CI under nightly. Backend portion of `make coverage-backend` exits 0."
    - "No file in this bucket is excluded from coverage without explicit justification surfaced at Plan 04C checkpoint"
  artifacts:
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md"
      provides: "Read-only INPUT — authoritative gap list. Every test file written corresponds to a row in the backend gap table."
      contains: "FAIL:"
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md"
      provides: "Read-only INPUT — patterns established by 04A (wiremock digest-auth, inline #[cfg(test)] for pure-data modules) carry into 04B"
      contains: "Patterns established"
    - path: "backend/tests/<one file per backend infrastructure bucket row, ≤11 total>"
      provides: "New backend integration / unit tests for enrollments + workers + license + recompute + supervisor"
      contains: "#[tokio::test]"
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04B-SUMMARY.md"
      provides: "Documents which backend infrastructure modules were closed, before% → after% per file, exclusions (if any), and confirms the backend gate is fully green"
      contains: "Files closed"
  key_links:
    - from: "make coverage-backend"
      to: "scripts/enforce-coverage-floor.sh + cargo llvm-cov nextest"
      via: "Makefile invocation"
      pattern: "make coverage-backend"
    - from: "backend/tests/<new test files>"
      to: "backend/src/<source modules>"
      via: "axum-test::TestServer + common::test_state_with_tmpdir + wiremock + tokio::time::pause/advance"
      pattern: "common::test_state_with_tmpdir|tokio::time::pause"
---

<objective>
Close the **backend infrastructure** subset of the Plan 03 baseline gap (11 files): enrollments subsystem (handlers + models + pusher + service), license subsystem (fingerprint + service), recompute subsystem (nightly scheduler + worker), supervisor watchdog, and the two background workers (backfill + purge). After this plan, every backend file in this bucket reaches ≥70% line / ≥60% branch and `make coverage-backend` exits 0 (project ≥90% line gate + per-file floor met across all backend code).

Purpose: This is the SECOND of three sub-plans splitting Plan 04 by subsystem. 04A closed the backend domain bucket (auth, calc, config, daily_records, departments, employees, devices/models, events, isapi/client, leaves, state/paths). 04B closes the infrastructure bucket — long-running async tasks, license validation, and the enrollment pipeline. 04C will close the frontend bucket and provide the single shared human verification checkpoint for the entire gap-fill set.

Output: New tests under `backend/tests/` (≤11 files). After 04B lands, the backend gate is fully green; only the frontend bucket (04C) remains to close before `make coverage` (composite) exits 0.
</objective>

<bucket_definition>
**Source files in this bucket (11 modules — drawn verbatim from `08-03-COVERAGE-BASELINE.md` backend FAIL list):**

| # | Source file | Baseline line% | Test target |
|---|---|---|---|
| 1 | `backend/src/enrollments/handlers.rs` | 0.94% | `tests/enrollments_handlers_test.rs` (extend `enrollments_test.rs` if simpler) |
| 2 | `backend/src/enrollments/models.rs` | 0.00% | `tests/enrollments_models_test.rs` (likely inline `#[cfg(test)] mod tests`) |
| 3 | `backend/src/enrollments/pusher.rs` | 56.57% | `tests/enrollments_pusher_test.rs` (uses `wiremock` for ISAPI push mocks) |
| 4 | `backend/src/enrollments/service.rs` | 23.17% | `tests/enrollments_service_test.rs` |
| 5 | `backend/src/license/fingerprint.rs` | 13.33% | `tests/license_fingerprint_test.rs` (likely inline `#[cfg(test)] mod tests`) |
| 6 | `backend/src/license/service.rs` | 18.95% | `tests/license_service_extra_test.rs` (extend existing `license_tests.rs`) |
| 7 | `backend/src/recompute/nightly.rs` | 0.00% | `tests/recompute_nightly_test.rs` (uses `tokio::time::pause/advance` for scheduler) |
| 8 | `backend/src/recompute/worker.rs` | 0.00% | `tests/recompute_worker_test.rs` |
| 9 | `backend/src/supervisor/watchdog.rs` | 53.57% | `tests/supervisor_watchdog_test.rs` (extend existing `supervisor_tests.rs`) |
| 10 | `backend/src/workers/backfill.rs` | 0.00% | `tests/workers_backfill_test.rs` |
| 11 | `backend/src/workers/purge.rs` | 0.00% | `tests/workers_purge_test.rs` |

**File count: 11.** Within the per-sub-plan ceiling. The bucket is dominated by **async background tasks** (recompute + workers + supervisor) which are testably isolated using `tokio::time::pause()` + `tokio::time::advance()` to deterministically advance scheduler clocks (no real sleeping in tests).

**Files NOT in this bucket** (handled by 04A): auth/{handlers, models}, calc/anomalies, anomalies/handlers, config, daily_records/{handlers, service}, db/mod, departments/service, devices/models, employees/service, events/handlers, isapi/client, leaves/{handlers, service}, state/paths.

**Files NOT in this bucket** (handled by 04C): all 21 frontend post-D-09 FAIL files.
</bucket_definition>

<branch_coverage_note>
Same caveat as 04A: backend baseline run used stable rustc (no `--branch`), so branch% defers to Plan 05's CI run under nightly. 04B targets line% locally and writes branch-exercising tests preemptively (e.g., scheduler tick boundary cases, license JWT signature-fail vs fingerprint-fail vs expired branches).
</branch_coverage_note>

<scope_cap>
**Hard cap on Plan 04B scope:** at most **11 new test files** AND at most **4 hours** of estimated work.

The 11 modules in this bucket are well-scoped — tokio time-mocking patterns are well-documented; the most setup-heavy modules are `enrollments/handlers` (multipart + AI-validation + kiosk-capture timeout branches) and `license/service` (JWT validation against fixture PEMs).

If estimated work exceeds 4 hours OR file count exceeds 11, escalate before continuing.
</scope_cap>

<exclusions_policy>
**No exclusions are pre-approved at planning time.** Same rule as 04A. The 3-exclusion budget for the backend side is **shared across 04A + 04B**; 04B inherits whatever 04A left unused.

**Specifically for files in this bucket:**
- `backend/src/license/fingerprint.rs` — Reads `/proc/cpuinfo` and similar OS-specific data sources to compute a hardware fingerprint. RESEARCH does NOT pre-approve an exclusion. Test by abstracting the OS reads behind a small mockable trait (already done in the codebase if Plan 1 pattern was followed) OR by testing the deterministic-given-input parts (the hash function, the canonicalization) and surfacing the OS-specific read at the 04C checkpoint as an exclusion candidate.
- `backend/src/recompute/{nightly, worker}.rs` and `backend/src/workers/{backfill, purge}.rs` — Long-running async tasks. RESEARCH cites `tokio::time::pause/advance` as the canonical pattern; do not exclude.
- `backend/src/supervisor/watchdog.rs` — Has existing tests in `supervisor_tests.rs` covering happy paths; the 46.43% gap is likely the timeout/restart branch. Do not exclude.

If the executor genuinely cannot lift `license/fingerprint.rs` because OS reads cannot be abstracted post-hoc without a code change beyond test scope, that is the most likely 04B exclusion candidate to surface at Plan 04C checkpoint.
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
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-PLAN.md

@backend/Cargo.toml
@backend/tests/common/mod.rs
@backend/tests/enrollments_test.rs
@backend/tests/enrollment_lifecycle_test.rs
@backend/tests/license_tests.rs
@backend/tests/supervisor_tests.rs

<interfaces>
<!-- 04A established (or extended) these patterns; 04B reuses them: -->
<!--   - common::test_state_with_tmpdir(db, config) -> (AppState, TempDir) — canonical fixture (Plan 02) -->
<!--   - wiremock::Mock::given(method("PUT")).respond_with(...) — digest-auth pattern (extended by 04A's isapi_client_test.rs) -->
<!--   - Inline #[cfg(test)] mod tests for pure-data modules (auth/models, devices/models, state/paths in 04A) -->

<!-- New for 04B: tokio time mocking. Canonical pattern for testing async schedulers/workers: -->
<!--   tokio::time::pause();                              // freeze the clock -->
<!--   let handle = tokio::spawn(my_worker(...));         // spawn the worker -->
<!--   tokio::time::advance(Duration::from_secs(60)).await; // jump 60s; runs all due timers -->
<!--   tokio::time::resume();                             // (or drop the handle) -->
<!--   assert!(handle.is_finished() == false);            // worker still running -->
<!--   // assert observable side effects (DB writes, mpsc messages, etc.) -->
<!-- This requires the test to be #[tokio::test(start_paused = true)] OR call pause() at the top. -->

<!-- Per Plan 02 SUMMARY, supervisor_tests.rs already returns: -->
<!--   build_test_app(db) -> (Router, AppState, mpsc::UnboundedReceiver<DeviceLifecycleEvent>, TempDir) -->
<!-- The watchdog tests can extend this; receiver is the channel for asserting watchdog events. -->

<!-- Per Plan 02 SUMMARY, license_tests.rs already returns: -->
<!--   build_gated_app(db) -> (Router, Arc<AtomicBool>, TempDir) -->
<!-- The AtomicBool is license_valid; flipping it tests middleware vs service paths. -->

<!-- dev-deps available (backend/Cargo.toml): -->
<!--   - axum-test 16, wiremock 0.6.5, tempfile 3, proptest 1.11 -->
<!--   - tokio 1.51 with features = ["full"] (provides tokio::time::pause/advance — feature "test-util" required) -->
<!--   - VERIFY: backend/Cargo.toml dev-dependencies for tokio includes "test-util". If not, this plan must add it as the FIRST step (single-line dev-dep change is in scope per CONTEXT.md "no new dev-deps" rule — features on existing deps are not new deps). -->

<!-- Per UI-SPEC: backend-only — `ui_surface: none`. -->

<!-- Per security threat model:
     - License middleware/service negative-path coverage is itself a security control (T-08-12).
     - License fingerprint is a hardware-binding control (CONTEXT D-21 backwards compatibility); tests must cover the fail-closed branch (mismatched fingerprint → license_valid = false). -->
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Enrollments subsystem — handlers + models + pusher + service gap-fill</name>
  <files>
    backend/tests/enrollments_handlers_test.rs (new — 0.94% → ≥70%; OR extend enrollments_test.rs / enrollment_lifecycle_test.rs),
    backend/tests/enrollments_models_test.rs (new OR inline #[cfg(test)] in src/enrollments/models.rs — 0% → ≥70%),
    backend/tests/enrollments_pusher_test.rs (new — 56.57% → ≥70%; uses wiremock for ISAPI face-push),
    backend/tests/enrollments_service_test.rs (new — 23.17% → ≥70%)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 4 module rows still appear)
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md (patterns extended by 04A — wiremock digest auth, inline mod tests)
    - backend/tests/common/mod.rs §test_state_with_tmpdir
    - backend/tests/enrollments_test.rs (existing 2.2K — extend pattern; small file, easier to extend than fork)
    - backend/tests/enrollment_lifecycle_test.rs (existing 8.8K — covers happy lifecycle; new tests cover error branches)
    - backend/Cargo.toml (verify wiremock 0.6.5 in dev-deps)
    - backend/src/enrollments/handlers.rs (read whole — 0.94% line; identify start_enrollment, capture_from_device, retry_push, finish_enrollment + their multipart parsing branches per RESEARCH lines 432-455)
    - backend/src/enrollments/models.rs (read whole — 0% line; likely Display/From/Default impls + EnrollmentState enum)
    - backend/src/enrollments/pusher.rs (find the 43.43% uncovered — face-push HTTP error retry logic against ISAPI; uses reqwest+diqwest stack)
    - backend/src/enrollments/service.rs (find the 76.83% uncovered — start_enrollment + finish_enrollment + the AI-validation branch)
    - backend/src/enrollments/image_pipeline.rs (informational — already at 87.67%; do NOT touch beyond reading for context)
  </read_first>
  <coverage_discipline>
    Same rules as 04A. Specific patterns:

    1. `enrollments/handlers.rs` (0.94% → ≥70%). The most uncovered module in 04B; multipart parsing + error branches dominate. Tests:
       - `start_enrollment` happy path: POST multipart with valid employee_id → 201 + EnrollmentState::Pending row in DB.
       - `start_enrollment` validator-fail: missing employee_id, bogus department_id, employee already enrolled → 400.
       - `capture_from_device` (kiosk-capture path): POST multipart with image → image lands in `state.paths.captures_tmp_root` (Plan 01 wiring), AI-validation invoked, state transition to AwaitingReview.
       - `capture_from_device` AI-validation failure: mock the AI validator to return `BlurDetected` → 400.
       - `capture_from_device` kiosk-capture timeout: spawn handler with a delayed device response → 504.
       - `retry_push` happy path + already-pushed branch + mid-flight failure branch (uses `state.paths.enrollments_root`).
       - `finish_enrollment`: state must be AwaitingReview; from any other state → 409.
       Reuse `enrollments_test.rs::build_test_app`-style fixture.

    2. `enrollments/models.rs` (0% → ≥70%). Inline `#[cfg(test)] mod tests`:
       - `EnrollmentState` enum: each variant's Display/Debug/serde roundtrip.
       - Any builder/From impls for EnrollmentRecord, FacedataMetadata.
       - State-transition validity check (if present): `is_valid_transition(from, to)` table-test.

    3. `enrollments/pusher.rs` (56.57% → ≥70%). Uses wiremock — extend the digest-auth pattern from 04A's `isapi_client_test.rs`:
       - Push face-data succeeds: mock `PUT /ISAPI/AccessControl/UserInfo/SetUp` returns 200 → `Result::Ok(PushedAt(now))`.
       - Push fails on auth: mock returns 401 with no `WWW-Authenticate` → `Err(DigestAuthMissing)`.
       - Push fails on device 5xx: mock returns 500 → retry logic engages, eventually `Err(MaxRetriesExceeded)`.
       - Push to invalid endpoint: connection refused → `Err(ConnectionFailed)`.

    4. `enrollments/service.rs` (23.17% → ≥70%). Find the uncovered service-layer:
       - `start_enrollment` success path with AppState fixture.
       - `start_enrollment` employee-not-found, employee-already-enrolled error branches.
       - `finish_enrollment` happy path + invalid-state-transition branch.
       - The AI-validation invocation branch (mock the validator if it's a trait; otherwise exercise via the handler test in #1).
  </coverage_discipline>
  <action>
    Execute the per-source-file test design listed in <coverage_discipline> for the 4 enrollments modules.

    **Step 0 — verify tokio test-util feature.** Check `backend/Cargo.toml` `[dev-dependencies]` for `tokio = { ... features = [..., "test-util"] }`. If absent, add `"test-util"` to the existing tokio dev-dep features array. This is required for Task 2's `tokio::time::pause/advance`. (Adding a feature to an existing dep is not a new dep — well within the no-new-dev-deps rule.)

    **Step 1 — re-read the baseline.**

    **Step 2 — for each source file, READ THE SOURCE FIRST.**

    **Step 3 — run the suite after each module.**

    **Step 4 — exclusion handling.** None expected for this bucket; if a multipart parsing branch in `enrollments/handlers.rs` cannot be exercised because of an inaccessible private state machine, surface at Plan 04C checkpoint.
  </action>
  <verify>
    <automated>cd backend && cargo nextest run > /tmp/cov-04b-task1.log 2>&1; ec=$?; tail -10 /tmp/cov-04b-task1.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-backend > /tmp/cov-04b-task1-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04b-task1-gate.log; awk '/^FAIL:.*enrollments\/(handlers|models|pusher|service)/' /tmp/cov-04b-task1-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd backend && cargo nextest run` exits 0.
    - Every enrollments module (4 listed in <files>) is at or above 70% line.
    - `wiremock` reused per 04A pattern in `enrollments_pusher_test.rs`; no manual HTTP server roll-up.
    - All 04A bucket files (16 modules) remain ≥70% — no regression.
    - tokio dev-dep features includes `test-util` (verify `cargo build --tests` exits 0 after the feature edit).
    - No new dev-deps added (only feature additions to existing tokio dep).
  </acceptance_criteria>
  <done>
    All 4 enrollments backend infrastructure modules ≥70% line. tokio "test-util" feature confirmed available for Task 2's time-mocking. 04B-SUMMARY captures Task 1 deltas.
  </done>
</task>

<task type="auto">
  <name>Task 2: License + recompute + supervisor + workers — async/scheduled subsystem gap-fill</name>
  <files>
    backend/tests/license_fingerprint_test.rs (new OR inline #[cfg(test)] in src/license/fingerprint.rs — 13.33% → ≥70%),
    backend/tests/license_service_extra_test.rs (new — 18.95% → ≥70%; extend existing license_tests.rs),
    backend/tests/recompute_nightly_test.rs (new — 0% → ≥70%; uses tokio::time::pause/advance),
    backend/tests/recompute_worker_test.rs (new — 0% → ≥70%),
    backend/tests/supervisor_watchdog_test.rs (new — 53.57% → ≥70%; extend supervisor_tests.rs),
    backend/tests/workers_backfill_test.rs (new — 0% → ≥70%),
    backend/tests/workers_purge_test.rs (new — 0% → ≥70%)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 7 module rows still appear)
    - backend/tests/license_tests.rs (existing 23.7K — extend; build_gated_app fixture available per Plan 02)
    - backend/tests/supervisor_tests.rs (existing 30.5K — extend; build_test_app returns mpsc receiver for asserting events)
    - backend/tests/fixtures/test_license_*.pem (existing fixtures from prior phases; reuse for license tests)
    - backend/src/license/fingerprint.rs (read whole — 13.33%; likely OS-read + canonicalize + hash + serde)
    - backend/src/license/service.rs (find the 81.05% uncovered — load_and_validate_license fail-closed branches: file missing, JWT signature invalid, fingerprint mismatch, expired)
    - backend/src/license/middleware.rs (informational — already 100%; do NOT touch)
    - backend/src/recompute/nightly.rs (read whole — 0%; likely a tokio::spawn'd cron-style scheduler that ticks at 02:00 local time)
    - backend/src/recompute/worker.rs (read whole — 0%; likely the per-tick recompute job over the previous day's daily_records)
    - backend/src/supervisor/watchdog.rs (find the 46.43% uncovered — likely the device-offline-timeout branch + restart-attempt branch)
    - backend/src/workers/backfill.rs (read whole — 0%; likely a one-shot or periodic backfill of historical daily_records)
    - backend/src/workers/purge.rs (read whole — 0%; likely deletes evidence files older than retention cutoff)
  </read_first>
  <coverage_discipline>
    Same rules as Task 1. Specific patterns for time-sensitive async code (RESEARCH covers this; canonical):

    **tokio time mocking pattern:**
    ```rust
    #[tokio::test(start_paused = true)]
    async fn worker_runs_at_scheduled_tick() {
        let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
        let handle = tokio::spawn(workers::purge::run(state.clone()));

        // Advance past the worker's first scheduled tick (e.g., 60s interval).
        tokio::time::advance(Duration::from_secs(61)).await;

        // Yield so the spawned task gets to run.
        tokio::task::yield_now().await;

        // Assert the worker's observable side effect (e.g., row deleted).
        let conn = state.db.connect().unwrap();
        // ... query DB ...

        handle.abort();  // clean shutdown
    }
    ```

    Use `start_paused = true` on the test attribute — clearer than calling `tokio::time::pause()` inside the test body, and ensures spawned tasks see the paused clock from t=0.

    **Per-source-file test design:**

    1. `backend/src/license/fingerprint.rs` (13.33% → ≥70%). Inline `#[cfg(test)] mod tests`:
       - The hash function: deterministic given fixed input — table-test.
       - The canonicalization function: handles whitespace + ordering — table-test.
       - If the OS-read function is behind a trait, mock it; if it isn't, test only the deterministic-given-input parts and surface the OS-specific read at the 04C checkpoint as the most likely backend exclusion candidate.

    2. `backend/src/license/service.rs` (18.95% → ≥70%). Extend `license_tests.rs`:
       - `load_and_validate_license` happy path: load `tests/fixtures/test_license_valid.pem`, assert returns `Ok(LicenseClaims { ... })`.
       - File-missing branch: pass nonexistent path → `Err(FileNotFound)`.
       - JWT signature invalid: load `test_license_bad_signature.pem` (or generate one from a different keypair) → `Err(SignatureInvalid)`.
       - Fingerprint mismatch: load a license bound to a different fingerprint → `Err(FingerprintMismatch)`.
       - Expired: load a license with `exp` in the past → `Err(Expired)`.
       Use `build_gated_app` from `license_tests.rs` for any test that exercises the middleware integration.

    3. `backend/src/recompute/nightly.rs` (0% → ≥70%). Tokio time mocking:
       - Scheduler starts; first tick fires at the configured time → recompute worker is invoked once.
       - Multiple ticks: advance through 25h → recompute invoked twice (at 02:00 each day).
       - Shutdown signal: send the lifecycle_tx shutdown, assert the spawn task exits cleanly.
       - Error branch: recompute returns Err → scheduler logs and continues (does not panic).

    4. `backend/src/recompute/worker.rs` (0% → ≥70%):
       - Recompute happy path: seed daily_records for yesterday, invoke worker, assert calc fields populated.
       - Empty-day branch: no events for the day → no-op (no rows updated, no error).
       - Error branch: DB unavailable mid-recompute → bubbles up as Err.
       - Boundary: spans midnight (overnight shift), DST (Venezuela has no DST per memory — but document the assumption).

    5. `backend/src/supervisor/watchdog.rs` (53.57% → ≥70%). Extend `supervisor_tests.rs`:
       - Device-offline-timeout: device's last heartbeat is older than threshold → watchdog emits DeviceOffline event on the mpsc receiver.
       - Restart-attempt branch: after N consecutive offline ticks, watchdog issues a restart attempt → assert the restart message lands.
       - Recovery: device heartbeats again after offline → DeviceOnline event.
       Use the existing 4-tuple fixture `(Router, AppState, mpsc::UnboundedReceiver<DeviceLifecycleEvent>, TempDir)`.

    6. `backend/src/workers/backfill.rs` (0% → ≥70%). Tokio time mocking:
       - Backfill runs once at startup: advance 0-1s, assert backfill iteration completed.
       - Backfill range covers yesterday + last 7 days (verify per RESEARCH).
       - Empty range: no events to backfill → no-op (no error, no rows touched).
       - Error branch: DB error mid-backfill → logged, retry on next tick.

    7. `backend/src/workers/purge.rs` (0% → ≥70%). Tokio time mocking + `tempfile::TempDir`:
       - Purge runs at scheduled tick: seed evidence files in `state.paths.events_root`/`state.paths.leaves_root` with old mtimes (use `filetime` crate if available, otherwise the test seeds files older than the cutoff via OS touch), advance the clock past one tick, assert old files deleted, assert recent files preserved.
       - FS unlink error path (best-effort: read-only file or permission-denied) → logged, continues.
       - Cutoff boundary: file exactly at retention boundary → preserved (not deleted).
  </coverage_discipline>
  <action>
    Execute the per-source-file test design listed in <coverage_discipline> for the 7 license + recompute + supervisor + workers modules.

    **Step 1 — re-read the baseline.**

    **Step 2 — verify tokio test-util feature is enabled** (Task 1 added it; sanity-check `cargo build --tests` here before writing time-mocked tests).

    **Step 3 — for each source file, READ THE SOURCE FIRST.** Identify the precise scheduler interval (60s? 5min? 24h?), the side-effect channel (mpsc, DB write, FS unlink), and the shutdown mechanism (lifecycle_tx, abort, drop).

    **Step 4 — run the suite after each module.**

    **Step 5 — exclusion handling.** Surface `license/fingerprint.rs` at Plan 04C checkpoint ONLY if the OS-read genuinely cannot be tested without abstracting it behind a trait (which would be a code change beyond test-only scope).

    Per security threat model: `license/service.rs` fail-closed branches (signature invalid, fingerprint mismatch, expired) are SECURITY CONTROLS — negative-path coverage is itself the mitigation. Tests must cover ALL fail-closed branches.
  </action>
  <verify>
    <automated>cd backend && cargo nextest run > /tmp/cov-04b-task2.log 2>&1; ec=$?; tail -10 /tmp/cov-04b-task2.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-backend > /tmp/cov-04b-task2-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04b-task2-gate.log; awk '/^FAIL:.*(license\/(fingerprint|service)|recompute\/(nightly|worker)|supervisor\/watchdog|workers\/(backfill|purge))/' /tmp/cov-04b-task2-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd backend && cargo nextest run` exits 0.
    - Every Task 2 source file (7 listed in <files>) is at or above 70% line.
    - tokio time-mocking pattern applied correctly (no real sleep in tests; `start_paused = true` or explicit `tokio::time::pause()`).
    - `wiremock` reused for any outbound HTTP in license/service or pusher (not expected here, but available).
    - All 04A + 04B-Task1 bucket files remain ≥70% — no regression.
    - **Cumulative milestone:** after this task, `make coverage-backend` should exit 0 (or document residual exclusions for the 04C checkpoint). Project-wide backend line% ≥90% AND every counted backend file ≥70% line.
    - No new flaky tests: `cd backend && cargo nextest run` exits 0 across 3 successive runs.
  </acceptance_criteria>
  <done>
    All 11 backend infrastructure modules in the 04B bucket ≥70% line. Cumulative effect: 0 backend FAILs in the lcov post-processor output. The backend half of the gate is GREEN. 04B-SUMMARY captures before% → after% per file and confirms the milestone.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| test → coverage report | Tests are local code; coverage report is local artifact; no untrusted data. |
| wiremock → backend test | wiremock spawns a local HTTP server bound to localhost only; used in `enrollments_pusher_test.rs`. |
| Test fixtures → license_service_extra_test | License PEM fixtures are pre-existing; new fixtures (if any) contain only synthetic identifiers. |
| tokio paused clock → spawned worker tasks | Time mocking is in-process; no real timers fire during tests. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-08-11B | Tampering | Coverage exclusion abuse (backend infrastructure bucket) | mitigate | No exclusions are pre-approved. Exclusion budget is shared with 04A (3 backend total). Most likely 04B exclusion candidate is `license/fingerprint.rs` OS-read; surface at Plan 04C checkpoint with written rationale. |
| T-08-12B | Repudiation | License negative-path coverage as security control | mitigate | `license/service.rs` fail-closed tests (signature invalid, fingerprint mismatch, expired) are themselves the security control. Tests MUST cover all fail-closed branches. |
| T-08-13B | Information Disclosure | License PEM fixtures | accept | Existing fixtures in `tests/fixtures/` are pre-committed and contain test keypairs only (not production keys). Any new fixture introduced by 04B must contain only synthetic data. |
| T-08-14B | Denial of Service | Worker / scheduler test isolation | mitigate | All time-sensitive tests use `tokio::time::pause/advance` — no test sleeps for real time. Worker tests `handle.abort()` after assertions. No risk of test suite hanging on real timers. |
| T-08-15B | Tampering | Purge worker file deletion under test | mitigate | `workers_purge_test` operates on a `TempDir` rooted in `state.paths.{events,leaves}_root` (Plan 01 pattern). No risk of touching real production paths; TempDir is per-test. |
</threat_model>

<verification>
1. `cd backend && cargo nextest run` exits 0 (no regression).
2. `make coverage-backend` (or off-recipe stable-rustc fallback) exits 0 — backend gate fully green: project ≥90% line + every counted file ≥70% line. (Branch% verification deferred to Plan 05 nightly CI.)
3. HTML report renders: `backend/target/llvm-cov/html/index.html` exists.
4. No new flaky tests: `cargo nextest run` 3× in succession all green.
5. tokio dev-dep `test-util` feature confirmed enabled (cargo build --tests exits 0).
6. Any exclusion candidate (e.g., `license/fingerprint.rs` OS-read) is surfaced at the Plan 04C checkpoint with written rationale.
7. 04B-SUMMARY exists at `.planning/phases/08-.../08-04B-SUMMARY.md` documenting per-file before% → after% and confirming backend gate is green.

This is the green-light signal for Plan 04C to begin.
</verification>

<success_criteria>
- All 11 backend infrastructure modules in this bucket ≥70% line (and ≥60% branch under nightly when measurable).
- Cumulative effect on `make coverage-backend`: 11 → 0 backend FAILs. The backend portion of the gate is fully green.
- Test additions follow existing patterns (no new test framework, no new dev-deps; tokio "test-util" feature added to existing dep).
- 04B-SUMMARY exists and confirms the backend gate milestone.
</success_criteria>

<output>
After completion, create `.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04B-SUMMARY.md` with:
- Files closed (path + before% → after%)
- Test files added (path list — actual files)
- Patterns established (tokio time mocking with `start_paused = true`, license fail-closed branch testing as security control coverage)
- Any files surfaced for the Plan 04C checkpoint as exclusion candidates (with written rationale — most likely `license/fingerprint.rs` OS-read)
- Final per-file backend numbers for the 11 bucket files
- Confirmation: backend portion of `make coverage-backend` exits 0 (cumulative 04A + 04B effect)
- Note any local-vs-CI toolchain caveats encountered
</output>
