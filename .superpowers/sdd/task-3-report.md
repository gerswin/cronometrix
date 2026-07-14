# Task 3 implementer report

## Status

DONE

## Implemented

- Replaced the split/direct enrollment creation paths with the single public
  `start_enrollment` mutation. It writes the JPEG through `AtomicFileGuard`,
  then performs face enrollment, enrollment, stable employee face linkage,
  active-device fan-out, and typed response construction in one queued
  transaction. A worker-owned `after_commit` callback keeps the JPEG.
- Added canonical background transactions `complete_push_success`,
  `complete_push_failure`, and `finalize_enrollment`.
- Made successful device push + mapping one atomic transition. A mapping
  persistence failure rolls back success, records a terminal failed recovery
  state, and never repeats accepted ISAPI side effects.
- Kept worker database admission on the queue's Busy-only background policy.
  Purge now preserves `pending_delete` if its post-device DB delete fails;
  backfill tests prove a mapping failure does not replay device calls.
- Replaced detached capture tasks with a `CapturesMap` lifecycle owner containing
  the state map, tracked `JoinSet`, admission flag, and cancellation token.
- Added monotonic capture creation/terminal timestamps, 45-second active TTL,
  5-minute terminal TTL, <=30-second cleanup cadence, deterministic explicit
  `Instant` cleanup, captured JPEG-before-state deletion, recoverable delete
  failures, and a root-confined orphan sweep.
- Capture device lookup/decryption/client construction now completes before map
  admission. Capture JPEG publication uses `AtomicFileGuard`.
- Added startup/periodic cleanup and graceful shutdown ordering in `main`: stop
  HTTP admission, cancel/await capture tasks, compensate state/JPEGs, await the
  cleanup worker, then drain/close the database writer. SIGKILL recovery is
  documented as the next-start orphan sweep.
- Added an `EnrollmentTaskTracker` to application state. Enrollment fan-out and
  public retry work are admitted atomically, shutdown closes admission, and all
  accepted ISAPI work plus terminal persistence is awaited before writer drain.
- `start_enrollment` now returns a non-serializable exact dispatch snapshot
  (device IDs/credentials and employee name) created and decrypted inside the
  same transaction as its push rows. There is no post-commit active-device/name
  lookup divergence, and its manual `Debug` implementation omits credentials.
- Added additive migration `019_device_operation_checkpoints` and durable
  prepared/device-applied/manual checkpoints for enrollment, backfill, and
  purge operations. Confirmed device success followed by DB failure recovers
  DB-only on the next cycle; ambiguous prepared operations fail manual instead
  of replaying. Successful pushes cannot be retried.
- Finalization now leaves an enrollment open while any push is pending or
  in-progress. Setup/lookup/admission failures become explicit terminal/manual
  states rather than silent success gaps.
- Added Ctrl-C and SIGTERM shutdown support through a testable first-signal
  selector.
- Capture admission now inserts state and registers its task without a
  suspension gap. Reads clone state before filesystem work; cleanup snapshots,
  performs no-follow identity-safe deletion without a map lock, then uses
  compare-remove. Startup and periodic cleanup perform the wall-clock-aged
  orphan sweep.
- Added a post-commit `EnrollmentDispatcher`: start and retry transactions now
  commit each push row with its `prepared` checkpoint and synchronously enqueue
  an exact private-token command from `after_commit`. Request cancellation can
  no longer lose committed dispatch work, and retry handlers perform no
  fallible/post-commit work.
- Added fail-closed startup recovery for every enrollment/backfill/purge
  checkpoint before HTTP or workers start. Restarted `prepared` attempts become
  manual and are never replayed; `device_applied` attempts finish DB-only.
- Preserved checkpoints as manual for device errors, partial calls, and
  timeouts. Duration-injected focal tests prove a second cycle emits no new
  Hikvision request for enrollment, backfill, or purge.
- Capture cleanup now retains the original opaque file identity. Captured and
  orphan replacement races preserve the foreign file, orphan metadata and
  identity come from one no-follow descriptor, and periodic cleanup includes
  the orphan sweep.
- Tracker drain now aggregates task errors and JoinErrors. Authorized dispatch
  catches target panics, terminalizes exact push IDs, finalizes enrollment, and
  reports the error. Main shutdown executes every drain phase best-effort and
  returns the first recorded failure.
- Startup `device_applied` enrollment recovery now commits push success,
  mapping, aggregate enrollment finalization, and checkpoint deletion in one
  transaction. A finalization failure rolls back every change and preserves
  the checkpoint for fail-closed retry on the next startup.
- Capture reads now require the `FileIdentity` retained in `CaptureState` and
  compare it with `fstat` metadata from the same no-follow descriptor before
  reading. A regular-file pathname replacement returns a stable identity error
  and no foreign bytes.

## TDD evidence

### RED

- `cargo test --all-features --test enrollments_service_test ...`
  exited 101 because `complete_push_success`, `complete_push_failure`, and
  `finalize_enrollment` did not exist.
- `cargo test --all-features --test enrollments_handlers_test ...`
  exited 101 because `CaptureState` had no monotonic timestamps.
- `cargo test --all-features --test capture_cleanup_test`
  exited 101 because the cleanup module, lifecycle APIs, and task tracking did
  not exist.
- `cargo test --all-features --test workers_purge_test
  purge_mapping_delete_failure_keeps_pending_delete_recovery_state` exited 101:
  the row remained active after the device delete succeeded and DB delete failed.
- `cargo test --all-features --test enrollments_pusher_test
  mapping_persistence_failure_marks_push_failed_without_retrying_device` exited
  101: the push remained `in_progress` instead of the required terminal failed
  recovery state.

Raw logs:

- `/tmp/cronometrix-12-03-task3-service-red.txt`
- `/tmp/cronometrix-12-03-task3-handler-red.txt`
- `/tmp/cronometrix-12-03-task3-capture-red.txt`
- `/tmp/cronometrix-12-03-task3-purge-red.txt`
- `/tmp/cronometrix-12-03-task3-pusher-red.txt`
- `/tmp/cronometrix-12-03-task3-remediation-service-red.txt`
- `/tmp/cronometrix-12-03-task3-remediation-capture-red.txt`
- `/tmp/cronometrix-12-03-task3-signals-red.txt`
- `/tmp/cronometrix-12-03-task3-dispatch-abort-red.txt`
- `/tmp/cronometrix-t3-finalize-red.txt`
- `/tmp/cronometrix-t3-capture-identity-red.txt`

### GREEN

- Rollback, mapping, finalize, cancellation ownership, capture lifecycle,
  pusher recovery, and purge recovery focused tests all pass.
- Final exact eight-suite command (including `multi_device_push_test`): 156
  passed, 0 failed, 7 existing ignored stubs.
- Library unit suite: 96 passed, 0 failed, 0 ignored.
- Focused two-cycle recovery tests assert exactly one external operation for
  enrollment, backfill, and purge after a post-device DB failure.
- Focused shutdown-during-ISAPI test proves the tracker waits for the device
  response and terminal DB state.
- The capture replacement focal and the full enrollment-handler suite each
  passed 20 consecutive runs after one non-reproducible harness SIGABRT.
- Capture focal tests cover fresh/expired orphans, fail-closed startup,
  cancellation while map admission is blocked, slow filesystem reads without a
  map lock, deterministic delete-to-compare-remove replacement, and symlink
  no-follow read/delete.
- `cargo check --all-targets --all-features`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Files changed

- `backend/src/enrollments/service.rs`
- `backend/src/enrollments/handlers.rs`
- `backend/src/enrollments/pusher.rs`
- `backend/src/enrollments/dispatcher.rs`
- `backend/src/enrollments/mod.rs`
- `backend/src/state/mod.rs`
- `backend/src/storage/atomic_file.rs`
- `backend/src/db/mod.rs`
- `backend/src/db/migrations/019_device_operation_checkpoints.sql`
- `backend/src/workers/capture_cleanup.rs`
- `backend/src/workers/mod.rs`
- `backend/src/main.rs`
- `backend/src/workers/purge.rs`
- `backend/tests/enrollments_service_test.rs`
- `backend/tests/enrollments_handlers_test.rs`
- `backend/tests/capture_cleanup_test.rs`
- `backend/tests/enrollments_pusher_test.rs`
- `backend/tests/workers_backfill_test.rs`
- `backend/tests/workers_purge_test.rs`
- `backend/tests/common/mod.rs`
- `backend/tests/enrollment_lifecycle_test.rs`
- `backend/tests/multi_device_push_test.rs`
- `.planning/phases/12-v1-0-release-stabilization/12-03-PLAN.md`

## Self-review

- No dependency, coverage-exclusion, or test-threshold changes. The only schema
  change is additive migration 019 for non-sensitive operation checkpoints;
  its apply/idempotency test passes and Task 6 remains reserved for migration
  020.
- All repository edits are within the task's authorized manifest; the report is
  the required SDD artifact.
- The seven ignored tests reported by the exact suite are pre-existing wave-0
  stubs; this task added no ignores.
- Orphan cleanup is intentionally limited to direct `*.jpg` children of the
  injected `captures_tmp_root`; it never recursively traverses another path.
- `/tmp/cronometrix-12-03-base-sha` was absent at closing verification. It was
  not recreated or modified. Git history still verifies the plan base as
  `c3fe7935a8ccccc0b15826bcb82d413d22e83188`, and Task 3 began at
  `5313204981c0c71d2a7901f542e760d8e30c1cbe`. The missing external marker
  prevents only that one `/tmp` identity assertion; it does not alter the diff
  or its ancestry.
- No unresolved correctness concern found.
