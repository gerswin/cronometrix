---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 02
subsystem: testing
tags: [tempfile, integration-tests, appstate, paths, fixture, parallel-tests, phase-8-wave-2]
requires:
  - phase: 08-01
    provides: "AppState carries Arc<Paths>; Paths::for_test(&Path) is the test constructor"
provides:
  - "common::test_state takes a third paths: Arc<Paths> argument"
  - "common::test_state_with_tmpdir(db, config) returns (AppState, TempDir) — the canonical wave-2+ test fixture"
  - "Every backend integration test owns a per-test TempDir; zero env-var mutation, zero parallel-test races"
  - "cargo build --tests + cargo test + cargo nextest run all green"
affects:
  - "Wave 3 (Plan 08-03) coverage tooling depends on parallel-clean test execution under cargo-llvm-cov"
  - "Wave 4+ (CI gate) depends on the same green baseline"
tech-stack:
  added: []
  patterns:
    - "test_state_with_tmpdir helper — returns (state, tmpdir) tuple; caller binds tmpdir to a local that outlives assertions"
    - "build_test_app helpers extended to return TempDir as the last tuple element when the helper builds an AppState internally"
    - "Inline AppState construction sites use Paths::for_test(_tmp.path()) with a sibling _tmp local"

key-files:
  created: []
  modified:
    - backend/tests/common/mod.rs
    - backend/tests/leave_tests.rs
    - backend/tests/event_tests.rs
    - backend/tests/listener_tests.rs
    - backend/tests/daily_record_tests.rs
    - backend/tests/multi_device_push_test.rs
    - backend/tests/reports_excel_test.rs
    - backend/tests/reports_test.rs
    - backend/tests/auth_tests.rs
    - backend/tests/department_tests.rs
    - backend/tests/employee_tests.rs
    - backend/tests/rules_tests.rs
    - backend/tests/tenant_info_test.rs
    - backend/tests/device_tests.rs
    - backend/tests/license_tests.rs
    - backend/tests/supervisor_tests.rs

key-decisions:
  - "test_state_with_tmpdir returns (AppState, TempDir) tuple — forces caller to acknowledge TempDir ownership and prevents Pitfall 1 (premature drop)"
  - "build_test_app helpers extended uniformly to return TempDir as the last tuple element, matching the test_state_with_tmpdir convention"
  - "leave_tests.rs / daily_record_tests / multi_device_push / reports / reports_excel: extend make_state-style helpers to return (AppState, TempDir) — uniform convention across the test suite"
  - "Inline AppState construction sites in supervisor_tests / device_tests / license_tests use Paths::for_test(_tmp.path()) with a sibling _tmp local rather than introducing a new helper — matches the inline shape these tests already had"
  - "Auto-fix Rule 3 applied: plan listed 4 sibling files but cargo build --tests revealed 8 additional test files with the same signature mismatch — all 12 fixed in Task 2b"

patterns-established:
  - "Per-test TempDir: every #[tokio::test] in backend/tests/ owns a TempDir bound to a local that outlives every assertion"
  - "Tuple-return fixture helpers: build_test_app / make_state / build_test_state / build_gated_app all return their TempDir alongside the constructed value(s)"
  - "Zero env-var mutation in tests: tempdir-rooted Paths replace the *RootGuard env::set_var idiom"

requirements-completed: [QUALITY-GATE]

# Metrics
duration: ~50min
completed: 2026-04-28
---

# Phase 8 Plan 02: Test fixture migration to AppState path injection Summary

**One-liner:** Migrate every backend integration-test fixture from `*RootGuard` env-var-mutation guards onto tempdir-backed `Paths::for_test` injection — `test_state_with_tmpdir` returns `(AppState, TempDir)`, every helper carries a per-test tempdir, and `cargo nextest run` (parallel by default) is now race-free.

## Performance

- **Duration:** ~50 min
- **Started:** 2026-04-28T17:09:00Z (approx)
- **Completed:** 2026-04-28T18:00:00Z (approx)
- **Tasks:** 3 (Task 1, Task 2a, Task 2b)
- **Files modified:** 16

## Accomplishments

- `common::test_state` extended to a 3-argument signature accepting `Arc<Paths>`; new `common::test_state_with_tmpdir(db, config) -> (AppState, TempDir)` is the canonical fixture for every wave-2+ integration test.
- All three high-churn central files (`leave_tests.rs`, `event_tests.rs`, `listener_tests.rs`) deleted their `LeavesRootGuard` / `EventsRootGuard` / `ENV_GUARD` structures verbatim — env-var mutation eliminated from the test surface.
- Every other integration test (12 files total — the 4 listed in the plan plus 8 discovered during cargo build) updated to the new signature with a per-test TempDir.
- `cargo build --tests` exits 0; `cargo test` reports 319 passed / 22 ignored; `cargo nextest run` reports 319 passed under parallel execution.

## Task Commits

1. **Task 1: Extend common::test_state + add test_state_with_tmpdir helper** — `7e1b679` (test)
2. **Task 2a: HIGH-CHURN migration — leave_tests, event_tests, listener_tests** — `4329392` (refactor)
3. **Task 2b: SIGNATURE-ONLY migration — 12 sibling test files** — `7bc26c4` (refactor)

## Files Created/Modified

### Modified

- `backend/tests/common/mod.rs` — `test_state` gains `paths: Arc<Paths>` parameter; new `test_state_with_tmpdir` helper returns `(AppState, TempDir)`.
- `backend/tests/leave_tests.rs` — `LeavesRootGuard` deleted (lines 45-70 in pre-fix); `make_state` now returns `(AppState, TempDir)`; 11 `let _guard = LeavesRootGuard::new();` call sites replaced with tuple destructuring; the `leaves::service::leaves_root()` assertion uses `state.paths.leaves_root` instead.
- `backend/tests/event_tests.rs` — `ENV_GUARD` static + `EventsRootGuard` struct deleted (lines 33-65 pre-fix); `build_test_app` returns `(Router, AppState, TempDir)`; `seed_event` takes `events_root: &Path`; 13 tests restructured to seed AFTER building the app so the per-test events_root is available.
- `backend/tests/listener_tests.rs` — `ENV_GUARD` + lifetime-variant `EventsRootGuard<'a>` deleted (lines 27-56 pre-fix); `make_state` returns `(AppState, TempDir)`; 5 tests destructure the tuple; the `cronometrix_api::events::service::events_root()` assertion replaced with `state.paths.events_root`.
- `backend/tests/daily_record_tests.rs` — `make_state` returns `(AppState, TempDir)`; 3 call sites destructure.
- `backend/tests/multi_device_push_test.rs` — Inline AppState struct literal at line 148 collapsed to call `build_test_state`; the helper itself rewritten to call `common::test_state_with_tmpdir`; 4 call sites destructure.
- `backend/tests/reports_excel_test.rs` — `make_state` returns `(AppState, TempDir)`; 12 call sites destructure (most via `let (state, _tmp) = make_state(db); let app = build_test_app(state);`).
- `backend/tests/reports_test.rs` — Same pattern; 27 call sites destructure.
- `backend/tests/auth_tests.rs` — `build_test_app` returns `(Router, TempDir)`; 4 call sites destructure.
- `backend/tests/department_tests.rs` — `build_test_app` returns `(Router, TempDir)`; 3 call sites destructure.
- `backend/tests/employee_tests.rs` — `build_test_app` returns `(Router, TempDir)`; 4 call sites destructure.
- `backend/tests/rules_tests.rs` — `build_test_app` returns `(Router, TempDir)`; 3 call sites destructure.
- `backend/tests/tenant_info_test.rs` — `build_test_app` returns `(Router, TempDir)`; 5 call sites destructure.
- `backend/tests/device_tests.rs` — `build_test_app` returns `(Router, TempDir)`; 14 helper call sites destructure; 5 inline `common::test_state(db_arc.clone(), config)` sites add a sibling `_tmp` local with `Paths::for_test(_tmp.path())`.
- `backend/tests/license_tests.rs` — `build_gated_app` returns `(Router, Arc<AtomicBool>, TempDir)`; 6 call sites destructure.
- `backend/tests/supervisor_tests.rs` — `build_test_app` returns 4-tuple `(Router, AppState, mpsc::UnboundedReceiver<DeviceLifecycleEvent>, TempDir)`; 5 call sites destructure; 9 inline `common::test_state(...)` sites add sibling `_tmp` locals.

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| `test_state_with_tmpdir` returns a tuple `(AppState, TempDir)` | Forces the caller to acknowledge TempDir ownership at the call site — the type system surfaces Pitfall 1 (premature drop) at compile time. A `Default` impl on `Paths` was rejected per the plan because production AppState uses `Arc<Paths>` unconditionally; tests must match production shape. |
| `build_test_app` helpers return their TempDir as the last tuple element | Uniform convention across all 8 files that have a `build_test_app` helper. The TempDir lives in router state via `state.paths`; without returning it, the caller would unknowingly drop the tempdir at function return. |
| Inline AppState construction sites use `Paths::for_test(_tmp.path())` with a sibling `_tmp` local | These are tests that already had inline `cronometrix_api::state::AppState { ... }` literals or inline `common::test_state(...)` calls — adding a one-line `_tmp` local matches their existing inline shape better than refactoring to call a new helper. |
| Plan scope expanded from 4 → 12 sibling files | The plan listed `daily_record_tests`, `multi_device_push_test`, `reports_excel_test`, `reports_test` as the four sibling files needing signature updates. Running `cargo build --tests` after Task 2a revealed 8 more files (`auth_tests`, `department_tests`, `employee_tests`, `rules_tests`, `tenant_info_test`, `device_tests`, `license_tests`, `supervisor_tests`) that also called the old 2-arg `common::test_state` signature. Per Rule 3 (auto-fix blocking issues), all 12 were migrated — coverage tooling in Wave 3 cannot run if any test fails to compile. |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Migrated 8 sibling test files not listed in the plan**

- **Found during:** Task 2b (after Task 2a removed the *RootGuard centrals)
- **Issue:** Plan 02 Task 2b listed 4 sibling files (`daily_record_tests.rs`, `multi_device_push_test.rs`, `reports_excel_test.rs`, `reports_test.rs`) that needed signature updates. Running `cargo build --tests` after Task 2a's central refactor revealed 8 additional test files (`auth_tests.rs`, `department_tests.rs`, `employee_tests.rs`, `rules_tests.rs`, `tenant_info_test.rs`, `device_tests.rs`, `license_tests.rs`, `supervisor_tests.rs`) that all call `common::test_state(Arc::new(db), config)` with the old 2-arg signature. Without updating these, `cargo build --tests` fails — blocking the plan's success criteria (`cd backend && cargo build --tests` exits 0) and blocking Wave 3's coverage tooling entirely.
- **Fix:** Applied the same migration pattern to all 8 files: extend `build_test_app` / `build_gated_app` helpers to return their TempDir, or add inline `_tmp` + `Paths::for_test` for inline construction sites. Used the test fixture pattern most natural to each file (4-tuple return for supervisor_tests since it already returns multiple values; 3-tuple for license_tests; 2-tuple for the remaining 6).
- **Files modified:** All 8 listed above.
- **Verification:** `cargo build --tests` exits 0; `cargo test` 319 passed / 22 ignored; `cargo nextest run` 319 passed parallel.
- **Committed in:** `7bc26c4` (Task 2b commit).

**2. [Rule 3 - Blocking] Restructured event_tests.rs seed-then-build flow**

- **Found during:** Task 2a (event_tests.rs migration)
- **Issue:** `seed_event` in event_tests.rs calls `persist_attendance_event(conn, ev)` which Plan 08-01 changed to `persist_attendance_event(conn, events_root, ev)`. Pre-existing tests followed a "seed events → move db into build_test_app" pattern; once `seed_event` requires `events_root: &Path`, the path must come from the AppState, but the AppState is constructed inside `build_test_app` (which moves `db`).
- **Fix:** Added `events_root: &Path` parameter to `seed_event`. Restructured every test that seeds events to build the app FIRST (`let (app, state, _tmp) = build_test_app(db).await; let events_root = state.paths.events_root.clone();`), then seed using `state.db.connect()` and the cloned events_root.
- **Files modified:** `backend/tests/event_tests.rs` (helper + 9 tests restructured).
- **Verification:** `cargo nextest run --test event_tests` — 13 tests pass.
- **Committed in:** `4329392` (Task 2a commit).

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking issues required to land the plan's success criteria).
**Impact on plan:** Both auto-fixes were necessary for the plan to actually succeed (cargo build --tests must compile; tests must seed events using the new signature). No scope creep — the migration pattern is uniform across all 16 files.

## Issues Encountered

- **`cargo nextest` reports 1 leaky test** — pre-existing condition unrelated to this plan; will surface in coverage measurement (Plan 08-03) and be addressed there. Logged for visibility.

## Verification

```
$ cd backend && cargo build --tests 2>&1 | grep -cE "^error"
0

$ grep -rE "LeavesRootGuard|EventsRootGuard|ENV_GUARD" backend/src/ backend/tests/
(no matches)

$ grep -rnE "env::set_var\(.*(LEAVES_ROOT|EVENTS_ROOT|ENROLLMENTS_DIR|CAPTURES_TMP|DATA_DIR)" backend/src/ backend/tests/
(no matches)

$ grep -rn "test_state_with_tmpdir" backend/tests/ | wc -l
17

$ cd backend && cargo test
test result: ok. 319 passed; 0 failed; 22 ignored

$ cd backend && cargo nextest run
Summary [11.169s] 319 tests run: 319 passed (1 leaky), 22 skipped
```

## Self-Check: PASSED

- `backend/tests/common/mod.rs` — extended signature + new helper VERIFIED
- `backend/tests/leave_tests.rs` — LeavesRootGuard deleted VERIFIED
- `backend/tests/event_tests.rs` — EventsRootGuard + ENV_GUARD deleted VERIFIED
- `backend/tests/listener_tests.rs` — EventsRootGuard<'a> + ENV_GUARD deleted VERIFIED
- All 12 sibling test files migrated to 3-arg test_state — VERIFIED
- Commit `7e1b679` (Task 1) FOUND in git log
- Commit `4329392` (Task 2a) FOUND in git log
- Commit `7bc26c4` (Task 2b) FOUND in git log
- Zero env-var-mutation references in backend/src/ + backend/tests/ — VERIFIED
- 17 `test_state_with_tmpdir` references across backend/tests/ — VERIFIED
- `cargo build --tests` zero errors — VERIFIED
- `cargo test` 319 passed / 22 ignored — VERIFIED
- `cargo nextest run` 319 passed parallel — VERIFIED

## Threat Flags

None — this refactor is a mechanical migration of test fixtures off env-var mutation onto explicit per-test TempDir injection. Per the plan's threat model:

- **T-08-05 (Information Disclosure via test-side env mutation):** mitigated — every `*RootGuard` env::set_var call is gone; tests own per-test TempDirs that drop at scope end.
- **T-08-06 (TempDir lifetime):** mitigated — `test_state_with_tmpdir` returns a tuple, forcing caller acknowledgement; doc comment cites Pitfall 1 explicitly.
- **T-08-07 (Audit-log integrity in tests):** unchanged — audit-log triggers and on-disk evidence behaviors are untouched; only the path source is swapped.

No new network endpoints, auth paths, or trust-boundary changes introduced.

## Next Phase Readiness

- Wave 3 (Plan 08-03) can now run coverage tooling cleanly. `cargo-llvm-cov nextest` will execute under parallel test execution without the env-var race that was the root cause of Phase 8's existence.
- The single-leaky-test note flagged for Plan 08-03 to investigate during the first coverage run.

---
*Phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra*
*Completed: 2026-04-28*
