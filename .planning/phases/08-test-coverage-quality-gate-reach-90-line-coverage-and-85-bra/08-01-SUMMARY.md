---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 01
subsystem: backend/state
tags: [refactor, appstate, paths, di, phase-8-wave-1]
requires: []
provides:
  - "AppState now carries an Arc<Paths> field populated by Paths::from_env() at startup"
  - "Paths::for_test(tempdir) lets tests own a TempDir-rooted hierarchy without env mutation"
  - "All backend/src/ call sites read filesystem roots from state.paths.*"
affects:
  - "Wave 2 (Plan 08-02) test-helper migration depends on this refactor compiling cleanly"
  - "Wave 3+ coverage measurement and CI gate depend on Wave 1+2 landing first"
tech_stack:
  added: []
  patterns:
    - "AppState dependency injection (peer of Arc<Config>)"
    - "Substruct constructor pair: from_env() for prod, for_test(&Path) for tests"
key_files:
  created:
    - backend/src/state/paths.rs
    - backend/src/state/mod.rs
  modified:
    - backend/src/main.rs
    - backend/src/leaves/service.rs
    - backend/src/leaves/handlers.rs
    - backend/src/events/service.rs
    - backend/src/events/handlers.rs
    - backend/src/enrollments/service.rs
    - backend/src/enrollments/handlers.rs
    - backend/src/daily_records/handlers.rs
    - backend/src/isapi/stream.rs
  removed:
    - backend/src/state.rs
decisions:
  - "Paths is its own struct (not fields on Config) — Config holds redacted secrets, Paths holds open filesystem roots; semantic separation keeps Config's Debug-redaction story clean"
  - "persist_attendance_event takes events_root: &Path as a parameter rather than reading from state — the function only takes &Connection so threading the path is the lightest change consistent with the no-env-at-use-site rule"
  - "Inline events/service.rs tests own a TempDir directly via fresh_events_root() helper — no need for an AppState fixture for unit tests of write_photo_atomic"
metrics:
  duration_minutes: ~25
  tasks: 2
  files_changed: 11
  completed_date: "2026-04-28"
requirements_completed: []
---

# Phase 8 Plan 01: AppState Paths Injection Summary

**One-liner:** Promote five filesystem roots (leaves, events, enrollments, captures-tmp, overrides) from cwd-dependent free-function env reads to a `Paths` substruct on `AppState`, populated once at startup via `Paths::from_env()` and overridable in tests via `Paths::for_test(tempdir)`.

## What Got Built

A two-task structural refactor that eliminates the cwd-dependent + env-var-race anti-pattern that caused `leave_tests` to fail under `cargo-llvm-cov` and any test runner that changes cwd or runs tests in parallel.

### Task 1 — Paths substruct + state module split (commit `3656927`)

- Converted `backend/src/state.rs` (single file) into `backend/src/state/{mod.rs, paths.rs}` (directory module).
- New `state/paths.rs` defines `pub struct Paths` with five `PathBuf` fields and two constructors:
  - `Paths::from_env()` — reads `CRONOMETRIX_LEAVES_ROOT` / `CRONOMETRIX_EVENTS_ROOT` / `ENROLLMENTS_DIR` / `CRONOMETRIX_CAPTURES_TMP` / `DATA_DIR` with the same string defaults the deleted helpers used (D-21 backwards compatibility).
  - `Paths::for_test(tmp: &Path)` — every field is a subdirectory of the supplied tempdir. Caller owns the `TempDir` for the test's duration.
- `AppState` gains a `pub paths: Arc<Paths>` field, positioned directly after `pub config: Arc<Config>` per the field-with-comment rhythm of the existing struct. Doc-comment cites D-18/D-19/Phase-8.
- One private helper: `fn env_or_default(key: &str, default: &str) -> PathBuf` — five call sites, mirrors the rhythm of `Config::from_env`.

### Task 2 — Wire Paths end-to-end (commit `44696b0`)

- `main.rs:86` constructs `let paths = Arc::new(cronometrix_api::state::Paths::from_env());` and adds `paths,` to the AppState struct literal.
- `leaves/service.rs:28-32` `pub fn leaves_root()` deleted; `use std::path::PathBuf` no longer needed (removed).
- `leaves/handlers.rs:167` (create_leave) and `:276` (get_leave_evidence) read `state.paths.leaves_root` instead of calling the deleted helper. Canonicalize + path-traversal guard preserved verbatim per security threat model.
- `events/service.rs:74-78` `pub fn events_root()` deleted. `persist_attendance_event` signature changed to take `events_root: &Path` as a parameter. The inline `#[cfg(test)] mod tests` lost its `static ENV_GUARD: Mutex<()>` and `struct EventsRootGuard<'a>` — replaced by an `fn fresh_events_root() -> TempDir` helper that each test calls directly. All eight inline tests rewritten to pass `tmp.path()` to `persist_attendance_event`.
- `events/handlers.rs:105` (get_event_photo) reads `state.paths.events_root.clone()`.
- `isapi/stream.rs:321` `ingest_pair` threads `&state.paths.events_root` into `persist_attendance_event`.
- `enrollments/service.rs:29-40` `pub fn enrollments_root()` and `pub fn captures_tmp_root()` deleted. `start_enrollment` already takes `&AppState` so the inline call to `enrollments_root()` becomes `&state.paths.enrollments_root` directly — no signature change.
- `enrollments/handlers.rs:266` (retry_push) and `:381` (capture_from_device) read `state.paths.enrollments_root` / `state.paths.captures_tmp_root` from the in-scope `AppState`.
- `daily_records/handlers.rs:201-204` inline `env::var("DATA_DIR")...join("overrides")` block collapses to `let overrides_root = state.paths.overrides_root.clone();`.

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Separate `Paths` struct (not fields on `Config`) | `Config` already carries secrets (jwt_secret, device_creds_key) and has a manual `Debug` impl that redacts them; `Paths` carries no secrets and uses `#[derive(Clone, Debug)]`. Mixing the two would muddy `Config`'s redaction story. |
| `persist_attendance_event` signature gains `events_root: &Path` rather than `&AppState` | The function only takes `&Connection` — threading a single `&Path` is mechanically simpler than introducing a second struct dep, and matches `write_photo_atomic`'s shape. |
| Inline events/service.rs tests use a local `fresh_events_root() -> TempDir` helper, not the future `common::test_state_with_tmpdir` | These are unit tests of the `write_photo_atomic` path — they don't need AppState. Keeping them lean avoids a Wave-2 dependency for a Wave-1 commit. |
| Doc comment on the new field cites CLAUDE.md Conventions | Forward-references the convention rule that Plan 08-05 will write to CLAUDE.md (Filesystem-root injection). The new field will then have a stable doc-link target. |

## Deviations from Plan

None — both tasks executed exactly as specified in `08-01-PLAN.md`. Pre-existing warnings (unused `build_facedata_metadata` import in `isapi/client.rs:145`, unused `push_id` in `enrollments/handlers.rs:249`, deprecated `TimeoutLayer::new` in `main.rs:237`) are out-of-scope per the SCOPE BOUNDARY rule and were left untouched.

## Verification

```
$ cd backend && cargo check 2>&1 | grep -E "error\[" | wc -l
0

$ grep -rE 'env::var\(.*(LEAVES_ROOT|EVENTS_ROOT|ENROLLMENTS_DIR|CAPTURES_TMP|DATA_DIR)' backend/src/ | grep -v 'src/state/paths.rs' | wc -l
0

$ grep -rnE 'fn (leaves_root|events_root|enrollments_root|captures_tmp_root)\(\)' backend/src/ | wc -l
0

$ grep -nE 'static ENV_GUARD|struct (Leaves|Events)RootGuard' backend/src/events/service.rs | wc -l
0

$ grep -n 'Paths::from_env' backend/src/main.rs
86:    let paths = Arc::new(cronometrix_api::state::Paths::from_env());

$ grep -rn 'state\.paths\.' backend/src/ | grep -v 'src/state/' | wc -l
14
```

`cargo build --tests` shows 4 expected errors in `backend/tests/{common/mod.rs, leave_tests.rs, event_tests.rs, listener_tests.rs}` — these are the Wave-2 dependency Plan 08-02 will resolve (test-helper migration). They are NOT in scope for this plan.

## Self-Check: PASSED

- backend/src/state/paths.rs — FOUND
- backend/src/state/mod.rs — FOUND
- backend/src/state.rs — REMOVED (verified absent)
- Commit 3656927 — FOUND in git log
- Commit 44696b0 — FOUND in git log
- cargo check (src/ only) — exit 0, zero errors
- All five env-or-default keys present in paths.rs — VERIFIED
- All four `*_root()` helpers deleted — VERIFIED
- Inline ENV_GUARD / EventsRootGuard removed — VERIFIED
- 14 `state.paths.*` call sites across leaves, events, enrollments, daily_records, isapi — VERIFIED

## Threat Flags

None — this refactor is mechanical (path source swap). Existing canonicalize + path-traversal guards in `get_leave_evidence` and `get_event_photo` were preserved verbatim. No new network endpoints, auth paths, or trust-boundary changes introduced.
