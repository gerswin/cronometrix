---
phase: quick-260428-3qg
plan: 01
subsystem: backend-tests
tags: [refactor, tests, phase-7, regression-fix, dx]
requires:
  - "backend/src/state.rs::AppState (9 fields incl. Phase 7 purge_tx, backfill_tx, captures)"
  - "backend/src/enrollments/handlers.rs::new_captures_map"
provides:
  - "backend/tests/common/mod.rs::test_state — single source of truth for AppState construction in legacy tests"
  - "Green compile across all 14 previously broken integration test crates"
affects:
  - "All future plans that add fields to AppState — only common::test_state must be updated"
tech-stack:
  added: []
  patterns:
    - "Post-construction mutation for non-default AppState fields (lifecycle_tx, license_valid)"
    - "Helper function in tests/common/mod.rs for cross-crate fixture reuse"
key-files:
  created: []
  modified:
    - backend/tests/common/mod.rs
    - backend/tests/auth_tests.rs
    - backend/tests/daily_record_tests.rs
    - backend/tests/department_tests.rs
    - backend/tests/employee_tests.rs
    - backend/tests/event_tests.rs
    - backend/tests/leave_tests.rs
    - backend/tests/license_tests.rs
    - backend/tests/listener_tests.rs
    - backend/tests/reports_excel_test.rs
    - backend/tests/reports_test.rs
    - backend/tests/rules_tests.rs
    - backend/tests/tenant_info_test.rs
    - backend/tests/device_tests.rs
    - backend/tests/supervisor_tests.rs
decisions:
  - "Post-construction mutation (let mut state; state.field = ...) preferred over a parameterised builder — keeps test_state arity small (db, config) and matches the recipe used by Phase 7 multi_device_push_test"
  - "license_tests.rs gate_behavior_tests block migrated as an override case (license_valid: lv.clone()) — even though plan listed it as default-only inline-literal pattern"
  - "All 9 supervisor_tests sites migrated as overrides (plan listed 6 as default-only) — actual source had Some(lifecycle_tx) / Some(_lifecycle_tx) at every site"
metrics:
  duration: "~6 minutes"
  completed: 2026-04-28
---

# Phase quick-260428-3qg Plan 01: Backend Test Compile Fix Summary

Restored backend test compile (15 files, 14 broken crates) by extracting `common::test_state` and migrating every legacy `AppState { ... }` literal to call it.

## What Changed

Phase 7 (07-01) added three new fields to `AppState` — `purge_tx`, `backfill_tx`, `captures` — but did not update the 14 pre-Phase-7 integration-test crates that built `AppState { ... }` struct literals inline. `cargo test --no-run` failed with `error[E0063]: missing fields backfill_tx, captures and purge_tx in initializer of AppState` across every affected crate.

The fix is twofold:

1. **Helper added** — `backend/tests/common/mod.rs` now exposes `pub fn test_state(db: Arc<libsql::Database>, config: Arc<Config>) -> AppState` that constructs every field with sensible defaults (all optional channels `None`, `license_valid` true, `captures` fresh empty map).

2. **All 14 broken crates migrated** to call `common::test_state(...)` instead of inlining the literal. Tests that need a non-default channel (3 cases — license_tests overriding `license_valid`, supervisor_tests overriding `lifecycle_tx`) use post-construction mutation.

## Files Changed

15 files modified, 130 insertions / 267 deletions across 2 commits.

**Per-file site count:**

| File | Sites migrated | Pattern |
|---|---|---|
| backend/tests/common/mod.rs | +1 helper added | New |
| backend/tests/auth_tests.rs | 1 | inline → helper |
| backend/tests/daily_record_tests.rs | 1 | make_state body |
| backend/tests/department_tests.rs | 1 | inline → helper |
| backend/tests/employee_tests.rs | 1 | inline → helper |
| backend/tests/event_tests.rs | 1 | inline → helper |
| backend/tests/leave_tests.rs | 1 | make_state body |
| backend/tests/license_tests.rs | 1 | inline → helper + post-construction lv override |
| backend/tests/listener_tests.rs | 1 | make_state body |
| backend/tests/reports_excel_test.rs | 1 | make_state body |
| backend/tests/reports_test.rs | 1 | make_state body |
| backend/tests/rules_tests.rs | 1 | inline → helper |
| backend/tests/tenant_info_test.rs | 1 | inline → helper |
| backend/tests/device_tests.rs | 5 | inline → helper (4 with `db_arc.clone()`, 1 with `Arc::new(db)`) |
| backend/tests/supervisor_tests.rs | 9 | post-construction lifecycle_tx override (all 9 sites) |

**Total construction sites migrated:** 26.

## Test Results

```
$ cargo test --no-run
... (14 previously broken test executables now compile)
EXIT: 0

$ cargo nextest run
Summary [10.944s] 319 tests run: 319 passed, 22 skipped
```

Zero new failures. Zero pre-existing tests regressed. All 22 skipped tests are `#[ignore]`'d (Phase 7 wave-2 HTTP enrollment tests, performance benches) — same set as before the refactor.

## Commits

| SHA       | Subject                                                                |
| --------- | ---------------------------------------------------------------------- |
| `aec66dd` | refactor(tests): add test_state helper + migrate 12 test files to use it |
| `022a76a` | fix(tests): migrate device_tests + supervisor_tests to common::test_state |

## Deviations from Plan

### [Rule 1 — Plan recipe correction] supervisor_tests.rs site classification

**Found during:** Task 2 — site-by-site inspection
**Issue:** The plan listed sites 369, 452, 500, 544, 589, 657 as "all default-only (all-None)" in supervisor_tests.rs, claiming only sites 144, 258, 326 needed `lifecycle_tx: Some(...)` override handling. Inspection of the source showed every one of the 9 sites set `lifecycle_tx: Some(...)` — 3 with `Some(lifecycle_tx.clone())`, 4 with `Some(_lifecycle_tx)` (graceful_shutdown + 3 watchdog tests), 2 with `Some(lifecycle_tx)`.
**Fix:** Applied post-construction mutation (`let mut state = ...; state.lifecycle_tx = Some(...);`) to ALL 9 sites, preserving each site's exact RHS expression. Test semantics are identical — every test that previously held a tx clone still holds one.
**Files modified:** backend/tests/supervisor_tests.rs
**Commit:** `022a76a`

### [Rule 1 — Plan recipe correction] license_tests.rs site classification

**Found during:** Task 1 — reading license_tests.rs:353 site
**Issue:** The plan listed `license_tests.rs:353` as an inline-literal default-only site. Inspection showed the literal sets `license_valid: lv.clone()` (a non-default `Arc<AtomicBool>` constructed from a test parameter). The default `Arc::new(AtomicBool::new(true))` from the helper would have changed test semantics — `gate_behavior_tests` deliberately constructs `lv` with `license_valid` parameterised so callers can flip the gate from outside.
**Fix:** Used post-construction mutation pattern (`let mut state = common::test_state(...); state.license_valid = lv.clone();`) to preserve the parameterised `Arc<AtomicBool>` semantics.
**Files modified:** backend/tests/license_tests.rs
**Commit:** `aec66dd`

### [Cleanup — non-functional] Removed unused `use cronometrix_api::state::AppState` imports

After migration, several files no longer reference `AppState` as a type. Removed the import from auth_tests.rs, department_tests.rs, employee_tests.rs, event_tests.rs, rules_tests.rs, tenant_info_test.rs, device_tests.rs to avoid `unused_imports` warnings. Files where `AppState` is still used as a function parameter / return type (daily_record_tests, leave_tests, listener_tests, reports_test, reports_excel_test, license_tests, supervisor_tests) keep the import. Test semantics unchanged.

## Future-Proofing

```
$ grep -rF "    AppState {" backend/tests/ | grep -v common/mod.rs | grep -v multi_device_push_test.rs
(no output — every legacy test file now goes through common::test_state)
```

When `AppState` next gains a field, only `backend/tests/common/mod.rs::test_state` needs an update. The 14 migrated crates compile automatically.

## Followups

- **Optional:** `multi_device_push_test.rs::build_test_state` was intentionally left untouched (it is Phase-7 code that already supports the new fields). A future cleanup PR could migrate it to `common::test_state` for full consistency. Tracked as cosmetic; no functional benefit.

## Self-Check: PASSED

**Commits verified:**
- `aec66dd` — found in `git log` (refactor(tests): add test_state helper + migrate 12 test files to use it)
- `022a76a` — found in `git log` (fix(tests): migrate device_tests + supervisor_tests to common::test_state)

**Files verified (all 15 modified files exist):**
- backend/tests/common/mod.rs — contains `pub fn test_state`
- backend/tests/auth_tests.rs — `let state = common::test_state(...)`
- backend/tests/daily_record_tests.rs — `make_state` body migrated
- backend/tests/department_tests.rs — `let state = common::test_state(...)`
- backend/tests/employee_tests.rs — `let state = common::test_state(...)`
- backend/tests/event_tests.rs — `let state = common::test_state(...)`
- backend/tests/leave_tests.rs — `make_state` body migrated
- backend/tests/license_tests.rs — `let mut state = common::test_state(...); state.license_valid = lv.clone();`
- backend/tests/listener_tests.rs — `make_state` body migrated
- backend/tests/reports_excel_test.rs — `make_state` body migrated
- backend/tests/reports_test.rs — `make_state` body migrated
- backend/tests/rules_tests.rs — `let state = common::test_state(...)`
- backend/tests/tenant_info_test.rs — `let state = common::test_state(...)`
- backend/tests/device_tests.rs — 5 sites all `let state = common::test_state(...)`
- backend/tests/supervisor_tests.rs — 9 sites all `let mut state = ...; state.lifecycle_tx = Some(...);`

**Build verification:**
- `cargo test --no-run` exits 0 (was failing on 14 crates with E0063 before this work)
- `cargo nextest run` — Summary: 319 tests run: 319 passed, 22 skipped, 0 failed
- Future-proofing grep returns 0 struct-literal sites outside common/mod.rs and multi_device_push_test.rs
