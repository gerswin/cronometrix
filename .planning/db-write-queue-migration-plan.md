# Status: superseded/completed by Phase 12 Plan 12-03

This document is retained as historical planning input. Its executable
replacement is
`.planning/phases/12-v1-0-release-stabilization/12-03-PLAN.md`, which completed
the original intent with a bounded/drained queue, atomic domain transitions,
an enforced queue-only boundary, immutable audit rows, and load evidence.

| Historical sprint | Completed by Phase 12 Plan 12-03 |
|---|---|
| Sprint 1 — close existing queue gaps | Tasks 1, 3, and 5 |
| Sprint 2 — migrate core CRUD writes | Tasks 1 and 5 |
| Sprint 3 — leave, timesheet, and daily records | Tasks 2, 4, and 5 |
| Sprint 4 — system writers and background jobs | Tasks 1, 3, and 5 |
| Sprint 5 — eliminate direct helpers | Task 5 static queue-only gate |
| Sprint 6 — prove contention is gone | Task 7 four-profile load proof |

The historical Sprint 6 path `scripts/load_test.sh` was inaccurate. The real
and retained script is `backend/scripts/load_test.sh`; the Phase 12 orchestration
entry point is `backend/scripts/run_write_queue_load_profiles.sh`.

---

# Plan: DB Write Queue Migration

**Generated**: 2026-04-30
**Estimated Complexity**: High

## Overview
Migraremos todos los writes directos a SQLite/libSQL hacia `DbWriteQueue` para que la aplicación tenga un único canal serializado de mutaciones. El objetivo no es solo “usar la cola”, sino eliminar rutas paralelas de escritura que todavía compiten por locks, especialmente en handlers de alta frecuencia y en jobs en background.

La estrategia es incremental:
- primero cerrar la brecha entre los helpers `*_queued` existentes y sus variantes directas;
- luego mover los handlers de CRUD y los jobs de sistema que siguen escribiendo directo;
- finalmente eliminar helpers legacy, ajustar pruebas y validar con load test de lectura/escritura concurrente.

## Prerequisites
- `DbWriteQueue` debe seguir aceptando `execute`, `execute_batch` y `run`.
- El worker de escritura debe permanecer como el único consumidor de mutaciones.
- Las rutas de lectura no deben migrarse salvo que formen parte de una transacción de escritura.
- La migración debe conservar respuestas HTTP, códigos de error y semántica de concurrencia optimista.

## Sprint 1: Close the Existing Queue Gaps
**Goal**: Convert the paths that already have queued equivalents so the queue covers the obvious leftovers without changing API behavior.

**Demo/Validation**:
- grep shows no handler calling a direct write helper when a queued helper exists for the same operation.
- unit/integration tests for tenant-info, enrollments, purge, and command audit still pass.

### Task 1.1: Remove direct fallback paths in tenant-info and favor queued flow
- **Location**: `src/tenant_info/service.rs`, `src/tenant_info/handlers.rs`
- **Description**: Keep `update_tenant_info_queued` as the only mutation path and remove any remaining direct-write entry point from request handling. Preserve optimistic concurrency and 409 conflict behavior.
- **Dependencies**: None
- **Acceptance Criteria**:
  - PATCH `/tenant-info` only writes through `state.db_write`.
  - `get_tenant_info` remains read-only.
  - 409 behavior is unchanged.
- **Validation**:
  - Existing tenant-info tests pass.
  - Add or update one test asserting the handler does not call direct `conn.execute`.

### Task 1.2: Normalize enrollment mutation entry points to queued variants
- **Location**: `src/enrollments/service.rs`, `src/enrollments/handlers.rs`, `src/enrollments/pusher.rs`, `src/workers/purge.rs`
- **Description**: Ensure all request-path enrollment writes use the queued versions consistently: start enrollment, push status transitions, reset/retry, finalize status, mapping upsert, and purge delete/pending-delete transitions.
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
  - No request-path enrollment mutation reaches `conn.execute` directly when a queued counterpart exists.
  - Background push/purge jobs keep the same visible behavior.
- **Validation**:
  - Enrollment integration tests pass.
  - Add focused tests for retry and finalization paths.

### Task 1.3: Keep command audit on the queue and verify it is the only write in the command handler
- **Location**: `src/devices/handlers.rs`, `src/devices/service.rs`
- **Description**: Confirm the command dispatch handler persists audit rows only via the queued audit helper and does not introduce any direct write path around it.
- **Dependencies**: None
- **Acceptance Criteria**:
  - Command dispatch still returns the same status codes and device errors.
  - Audit persistence stays queued.
- **Validation**:
  - Device command tests pass.
  - Add a regression test for audit-row persistence under command timeout/error cases.

## Sprint 2: Migrate Core CRUD Writes
**Goal**: Move the main high-traffic CRUD mutations to `DbWriteQueue`.

**Demo/Validation**:
- `employees`, `departments`, `devices`, `rules`, and `setup` writes no longer call `conn.execute` directly from request handlers/services.
- CRUD behavior, version conflicts, and unique-constraint errors remain intact.

### Task 2.1: Queue employee create/update/deactivate
- **Location**: `src/employees/service.rs`, `src/employees/handlers.rs`
- **Description**: Add queued variants for create, update, and deactivate; route handler mutations through them; preserve version checks, unique employee code handling, and soft-delete semantics.
- **Dependencies**: Sprint 1 complete
- **Acceptance Criteria**:
  - POST `/employees`, PATCH `/employees/{id}`, DELETE `/employees/{id}` all write through the queue.
  - `VERSION_CONFLICT` and `EMPLOYEE_CODE_EXISTS` responses are preserved.
- **Validation**:
  - Employee CRUD tests pass.
  - Add a test covering create/update/deactivate under concurrent request load.

### Task 2.2: Queue department create/update
- **Location**: `src/departments/service.rs`, `src/departments/handlers.rs`
- **Description**: Add queued variants for department create and update, then migrate the handlers to use them.
- **Dependencies**: Sprint 1 complete
- **Acceptance Criteria**:
  - POST/PATCH `/departments` writes only through the queue.
  - Unique-name conflict and version conflict behavior remain unchanged.
- **Validation**:
  - Department tests pass.
  - Add regression tests for unique-name conflict and stale-version update.

### Task 2.3: Queue device create/update/deactivate
- **Location**: `src/devices/service.rs`, `src/devices/handlers.rs`
- **Description**: Move device CRUD mutations onto the queue, including encrypted password rotation and soft delete, while preserving partial unique-index handling for active device IP:port collisions.
- **Dependencies**: Sprint 1 complete
- **Acceptance Criteria**:
  - POST/PATCH/DELETE `/devices/{id}` use queued writes.
  - `DEVICE_IP_EXISTS` and `VERSION_CONFLICT` behavior remains stable.
- **Validation**:
  - Device CRUD tests pass.
  - Add a concurrency test around overlapping IP:port updates.

### Task 2.4: Queue global rules update and setup bootstrap insert
- **Location**: `src/rules/handlers.rs`, `src/setup/handlers.rs`
- **Description**: Add queued write paths for the singleton global rules update and the initial admin insert used by setup bootstrap.
- **Dependencies**: Sprint 1 complete
- **Acceptance Criteria**:
  - PATCH `/rules` writes through the queue.
  - Setup bootstrap user creation writes through the queue.
  - Singleton/version semantics are preserved.
- **Validation**:
  - Rules tests pass.
  - Setup flow tests pass in a clean database.

## Sprint 3: Migrate Leave, Timesheet, and Daily-Record Writes
**Goal**: Move the calculation-adjacent write paths that are hit during normal attendance operations.

**Demo/Validation**:
- leave creation, timesheet overrides, and daily-record materialization all write through the queue.
- recompute and leave-overlay behavior remains identical from the API perspective.

### Task 3.1: Queue leave create/cancel mutations
- **Location**: `src/leaves/service.rs`, `src/leaves/handlers.rs`
- **Description**: Add queued variants for leave insert and soft-delete cancelation. Keep overlap checks and evidence handling intact.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - POST `/leaves` and DELETE `/leaves/{id}` mutate only via the queue.
  - Overlap, evidence, and version checks are preserved.
- **Validation**:
  - Leave tests pass.
  - Add regression coverage for overlap conflict plus queued cancelation.

### Task 3.2: Queue daily-record override insert
- **Location**: `src/daily_records/handlers.rs`, `src/daily_records/service.rs`
- **Description**: Move the override insert to `DbWriteQueue` while keeping evidence file writing outside the queue and recompute scheduling after commit.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - POST `/daily-records/{id}/overrides` uses queued DB write.
  - Recompute request still fires after the write succeeds.
- **Validation**:
  - Timesheet tests pass.
  - Add a test that asserts the override row exists before recompute triggers.

### Task 3.3: Queue daily-record upsert transaction
- **Location**: `src/daily_records/service.rs`
- **Description**: Keep the current transactional upsert logic, but ensure the entire mutation path is executed inside the queue, not on a freely acquired connection. This is the highest-value contention fix because it batches the insert/update plus anomaly refresh in one serialized job.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - The daily-record recompute path no longer uses a direct connection for write transactions.
  - Transaction behavior and rollback semantics remain unchanged.
- **Validation**:
  - Recompute worker tests pass.
  - Add a regression test for rollback on anomaly insertion failure.

## Sprint 4: Migrate System Writers and Background Jobs
**Goal**: Eliminate direct writes from supervisor, event ingestion, and background workers that can still collide with user traffic.

**Demo/Validation**:
- device online/offline updates, attendance event persistence, and enrollment push jobs all use the queue.
- background jobs continue to function after the request handlers are migrated.

### Task 4.1: Queue supervisor connection-state updates
- **Location**: `src/supervisor/status.rs`, `src/supervisor/watchdog.rs`
- **Description**: Replace direct device status updates with queued writes for both the online/offline state machine and the stale-device watchdog.
- **Dependencies**: Sprint 2 complete
- **Acceptance Criteria**:
  - Connection-state updates never call direct `conn.execute`.
  - Watchdog semantics remain unchanged.
- **Validation**:
  - Supervisor tests pass.
  - Add a test for repeated state transitions under queued execution.

### Task 4.2: Queue attendance-event persistence
- **Location**: `src/events/service.rs`, `src/isapi/stream.rs`
- **Description**: Ensure attendance-event persistence and related photo-path side effects are serialized through the write queue.
- **Dependencies**: Sprint 3 complete
- **Acceptance Criteria**:
  - Event ingestion no longer writes directly to the DB from the stream path.
  - Deduplication behavior remains unchanged.
- **Validation**:
  - Event processor tests pass.
  - Add a regression test for duplicate suppression under load.

### Task 4.3: Queue remaining enrollment worker writes
- **Location**: `src/enrollments/service.rs`, `src/enrollments/pusher.rs`, `src/workers/purge.rs`, `src/workers/backfill.rs`
- **Description**: Remove any remaining direct writes in enrollment push, purge, and backfill jobs. All device-face-mapping and push-state mutations should go through queued variants.
- **Dependencies**: Sprint 1 complete
- **Acceptance Criteria**:
  - No background enrollment worker performs direct writes.
  - Queue variants are the only write entry points for mapping and push state.
- **Validation**:
  - Enrollment worker tests pass.
  - Add a smoke test for backfill + purge interaction.

## Sprint 5: Eliminate Legacy Direct Write Helpers
**Goal**: Remove the old non-queued mutation helpers so new code cannot bypass the queue accidentally.

**Demo/Validation**:
- `rg` over `src` shows no live mutation path calling `conn.execute` directly for supported v1 writes except the queue worker itself and intentional bootstrap/migration code.

### Task 5.1: Delete or deprecate legacy direct helpers
- **Location**: `src/employees/service.rs`, `src/departments/service.rs`, `src/devices/service.rs`, `src/leaves/service.rs`, `src/rules/handlers.rs`, `src/daily_records/service.rs`, `src/enrollments/service.rs`, `src/supervisor/status.rs`, `src/events/service.rs`, `src/setup/handlers.rs`
- **Description**: Remove the direct-write functions or convert them to internal queued-only helpers. Keep read functions untouched.
- **Dependencies**: Sprints 2-4 complete
- **Acceptance Criteria**:
  - Public mutation APIs no longer expose direct-write helpers.
  - Any remaining direct writes are limited to migrations, seed scripts, or the queue worker itself.
- **Validation**:
  - Full backend test suite passes.
  - Static search shows no remaining call sites to removed helpers.

### Task 5.2: Tighten the queue contract and detect accidental bypasses
- **Location**: `src/db/write_queue.rs`, `src/state/mod.rs`, `src/main.rs`
- **Description**: Add a stronger contract around queued writes so future code cannot quietly open a direct connection for mutations. Consider helper naming, lintable conventions, and any needed API restrictions.
- **Dependencies**: Task 5.1
- **Acceptance Criteria**:
  - Mutation code has a clear “queue-only” pattern.
  - Queue worker remains the sole centralized writer.
- **Validation**:
  - Compile passes.
  - Add a small invariant test or doc comment check if appropriate.

## Sprint 6: Prove the Lock Contention Is Gone
**Goal**: Validate that the migration actually solves the observed `database is locked` failures under concurrent mixed load.

**Demo/Validation**:
- load test with `concurrency > 1` and mixed read/write traffic no longer produces sustained 500s from SQLite locks.
- baseline single-thread behavior remains unchanged.

### Task 6.1: Expand load-test coverage for write serialization
- **Location**: `scripts/load_test.sh` and any supporting test docs
- **Description**: Add dedicated modes for writes-only, reads-only, and mixed traffic. Preserve the current JSON/CSV reporting so results can be compared across runs.
- **Dependencies**: Sprints 2-5 complete
- **Acceptance Criteria**:
  - The script can isolate pure write contention.
  - Reports make it obvious whether failures are read-side or write-side.
- **Validation**:
  - Run 1-thread baseline, writes-only, and mixed-load scenarios.

### Task 6.2: Run regression suite under concurrent load
- **Location**: Backend test command / CI workflow
- **Description**: Execute the full backend suite plus the load test against a seeded local database to verify no write path still bypasses the queue.
- **Dependencies**: Task 6.1
- **Acceptance Criteria**:
  - No sustained `SQLite failure: database is locked` appears under the target concurrency profile.
  - Functional tests still pass.
- **Validation**:
  - `cargo test` or repo-standard test command passes.
  - Load test summary is attached to the migration evidence.

## Testing Strategy
- Run targeted tests after each sprint, not just at the end.
- Use one load-test baseline before migration and one after migration with the same concurrency profile.
- Keep one regression test per migrated domain, focused on queued write behavior and preserving error semantics.
- Add a final grep-based audit:
  - live request-path mutation code should not call `conn.execute(...)` directly except inside the queue worker or explicitly exempt bootstrap/migration code.

## Potential Risks & Gotchas
- Some helpers are both read and write adjacent; converting them mechanically can accidentally move read-only checks into the queue and reduce throughput.
- A few operations are multi-step transactions. If they are split into separate queued jobs, rollback semantics will change. Those must remain one queued job.
- Background workers can still bypass the queue even if request handlers are fixed. The migration has to include them.
- The queue type currently returns `anyhow::Result<()>` for `run`, so any transaction needing a typed return value may require extending the queue contract, not just swapping call sites.
- Setup/bootstrap and migration code are legitimate exceptions. They should be kept explicit so they do not mask accidental bypasses.

## Rollback Plan
- Revert the queued call sites sprint by sprint if a regression appears.
- Keep the direct helper implementations until the queue-backed versions are fully validated, then remove them only after load testing passes.
- If contention or latency worsens, revert the newest migrated domain first and compare its direct-write path against the queued path.
