---
phase: 03-time-calculation-engine
plan: 01
subsystem: attendance-engine-core
tags: [calc, daily-records, recompute, anomalies, lottt]
dependency_graph:
  requires:
    - attendance_events table (Phase 2)
    - employees / departments / global_rules (Phase 1)
    - AppState pattern + auth RBAC middleware (Phase 1)
  provides:
    - daily_records table (materialized, engine-owned)
    - daily_record_anomalies append-only table
    - pure calc::compute_daily_record function
    - async recompute worker + nightly reconcile
    - GET /api/v1/daily-records[/{id}]
    - GET /api/v1/anomalies (supervisor-or-above)
    - AppError::CalcError variant (reserved)
    - Config.timezone + AppState.recompute_tx
  affects:
    - Plan 03-02 (overnight shifts extend shift_window)
    - Plan 03-03 (leaves populate EngineInput.leave)
    - Phase 4 (supervisor queue reads /anomalies)
    - Phase 5 (payroll export reads daily_records)
tech-stack:
  added:
    - chrono-tz 0.10.4 (dependency)
    - proptest 1.11.0 (dev-dependency)
  patterns:
    - Pure domain engine (no I/O, no async) → persistence wrapper → async worker
    - mpsc::UnboundedSender publish + HashSet dedup + 500ms debounce (mirror of Phase 2 Supervisor)
    - tokio::time::sleep to next 02:00 local via chrono-tz (no cron crate)
    - ON CONFLICT DO UPDATE for engine-owned upsert (never INSERT OR REPLACE)
    - Single-connection txn for write path (libSQL shared-cache lock contention)
key-files:
  created:
    - backend/src/calc/mod.rs
    - backend/src/calc/anomalies.rs
    - backend/src/calc/models.rs
    - backend/src/calc/engine.rs
    - backend/src/calc/aggregation.rs
    - backend/src/calc/lunch.rs
    - backend/src/calc/overtime.rs
    - backend/src/daily_records/mod.rs
    - backend/src/daily_records/models.rs
    - backend/src/daily_records/service.rs
    - backend/src/daily_records/handlers.rs
    - backend/src/anomalies/mod.rs
    - backend/src/anomalies/handlers.rs
    - backend/src/recompute/mod.rs
    - backend/src/recompute/worker.rs
    - backend/src/recompute/nightly.rs
    - backend/src/db/migrations/007_daily_records.sql
    - backend/src/db/migrations/008_daily_record_anomalies.sql
    - backend/src/db/migrations/012_shift_type_to_departments.sql
    - backend/tests/calc_tests.rs
    - backend/tests/daily_record_tests.rs
    - backend/tests/fixtures/lottt_scenarios.json
  modified:
    - backend/Cargo.toml
    - backend/src/lib.rs
    - backend/src/state.rs
    - backend/src/config.rs
    - backend/src/errors.rs
    - backend/src/main.rs
    - backend/src/db/mod.rs
    - backend/src/events/service.rs
    - backend/src/isapi/stream.rs
    - backend/tests/common/mod.rs
    - backend/tests/auth_tests.rs
    - backend/tests/department_tests.rs
    - backend/tests/device_tests.rs
    - backend/tests/employee_tests.rs
    - backend/tests/event_tests.rs
    - backend/tests/listener_tests.rs
    - backend/tests/rules_tests.rs
    - backend/tests/supervisor_tests.rs
    - .planning/STATE.md
decisions:
  - "Single-connection txn for recompute_for_day: libSQL shared-cache lock contention between reader + separate writer connection produced 'database is locked' errors under test load. Fix: reuse the same `conn` for BEGIN/COMMIT after all read cursors are drained — safe because libSQL sequences async ops on one connection."
  - "Module layout follows {mod, models, service, handlers} pattern from Phase 1/2 (calc/ is the exception — engine logic decomposes into aggregation/lunch/overtime/engine submodules, no service because it's pure)."
  - "isapi/stream::ingest_pair clones a slim NewAttendanceEvent snapshot before moving the original into persist_attendance_event. The snapshot keeps the recompute publish side non-mutating on the existing persist contract."
  - "Test Config builders across 8 integration test files got timezone: America/Caracas and AppState.recompute_tx: None added uniformly via scripted patch."
metrics:
  duration_min: 26
  tasks_completed: 2
  tests_added: "13 (5 lottt fixtures + 3 aggregation + 2 lunch + 3 overtime unit + 2 dr integration + 1 proptest — 270k invocations)"
  completed_date: 2026-04-23
---

# Phase 3 Plan 01: Time Calculation Engine Core Summary

**One-liner:** Pure Rust attendance engine with LOTTT Art. 178 caps, materialized `daily_records` upserts via ON CONFLICT DO UPDATE, and event-driven recompute (mpsc + 500ms debounce) wired end-to-end — 156/156 workspace tests green.

## What Was Built

### Migrations (3 added, in registered order)

1. `007_daily_records.sql` — engine-owned materialized table keyed on `(employee_id, anchor_date)` with UNIQUE INDEX. No version column (D-04 — engine replaces via upsert, not optimistic concurrency). No audit triggers (recomputes are too frequent; manual-edit audit lives on `daily_record_overrides` in Plan 03-03).
2. `008_daily_record_anomalies.sql` — append-only anomaly rows with `FOREIGN KEY daily_record_id REFERENCES daily_records(id) ON DELETE CASCADE` and a CHECK constraint enumerating all 10 `AnomalyCode` variants.
3. `012_shift_type_to_departments.sql` — adds `shift_type`, `is_overnight_shift`, `ordinary_daily_minutes` to `departments`. Defaults (`day`, `0`, `480`) preserve existing Phase 1 rows.

Slots 009/010/011 are **reserved** for Plan 03-03 (leaves + overrides + phase 3 audit triggers) — intentionally skipped here so the merge order stays clean.

### Engine Module Layout (calc/)

```
backend/src/calc/
├── mod.rs           — re-exports compute_daily_record + EngineInput/Output + AnomalyCode
├── anomalies.rs     — AnomalyCode enum (all 10 D-18 variants) + as_str() mapping
├── models.rs        — EngineInput / DailyRecordOutput / AttendanceEventRow / DepartmentConfig / GlobalRulesRow / LeaveRow
├── aggregation.rs   — shift_window() + aggregate_events() — first-entry/last-exit + unknown-face flag
├── lunch.rs         — compute_lunch_deduction() — fixed + punch fallback with LUNCH_PUNCH_MISSING
├── overtime.rs      — check_overtime_caps() — LOTTT Art. 178 daily/weekly/annual
└── engine.rs        — compute_daily_record() — pure orchestrator, no I/O, no async
```

The engine is deterministic: given identical `EngineInput`, always returns identical `DailyRecordOutput`. Validated by proptest with 270,000 random invocations (900 work_minutes × 300 ordinary_daily_minutes range).

### RecomputeWorker Pattern (mpsc + HashSet + debounce)

```rust
loop {
    select! {
        biased;
        _ = shutdown.cancelled() => break;
        Some(req) = rx.recv() => {
            let mut pending = HashSet::new();
            pending.insert((req.employee_id, req.anchor_date));
            while let Ok(extra) = rx.try_recv() { pending.insert(...); }
            tokio::time::sleep(500ms).await;
            while let Ok(extra) = rx.try_recv() { pending.insert(...); }
            for (emp, date) in pending.drain() {
                dr_service::recompute_for_day(&state, &emp, date).await;
            }
        }
    }
}
```

Burst collapse: a multi-device punch-in producing 4 events within a few milliseconds for the same employee yields exactly ONE recompute call. Worker mirrors the Phase 2 Supervisor structure (biased select, CancellationToken, single task).

### Nightly Reconcile Mechanism

`recompute::nightly::nightly_reconcile_task` computes `seconds_until_next_2am` in the configured TZ (America/Caracas), sleeps that long via `tokio::time::sleep`, then calls `reconcile_prior_day` which iterates active employees and recomputes *yesterday's* anchor date for each. Per-employee errors are logged and swallowed so one bad row cannot wedge the whole pass. No `tokio-cron-scheduler` dependency — just stdlib `tokio::time`.

### LOTTT Scenario Fixtures (5 green)

| # | Scenario | Work | OT | Late | Early | Anomalies |
|---|----------|-----:|---:|-----:|------:|-----------|
| 1 | Normal day 9-17, 60m lunch | 420 | 0 | 0 | 0 | — |
| 2 | +15m late arrival | 405 | 0 | 15 | 0 | — |
| 3 | 11h workday → OT cap breach | 660 | 180 | 0 | 0 | `OT_CAP_EXCEEDED_DAILY` |
| 4 | Missing exit punch | 0 | 0 | 0 | 0 | `MISSING_EXIT` |
| 5 | Punch-mode lunch missing | 420 | 0 | 0 | 0 | `LUNCH_PUNCH_MISSING` |

All assertions pass; Plan 03-02 will extend the fixture set with overnight shift variants.

### Event-Ingestion Hook

`events::service::publish_recompute_if_employee(state, &event)` is the single entry point for publishing a `RecomputeRequest`. Guards:

1. **Pitfall 7** — skip if `event.employee_id.is_none()` (unknown-face events would flood the worker with NULL ids).
2. **recompute_tx is Option** — silently skip if None (test setups without a worker still compile and run).
3. **Pitfall 2** — `captured_at` is UTC epoch; we translate to local anchor date via `state.config.timezone` before publishing.

`isapi::stream::ingest_pair` calls this hook AFTER `PersistOutcome::Inserted` (never on `Deduplicated` — dedup hits already had a recompute triggered by the first insert).

### API Endpoints

- `GET /api/v1/daily-records` — filters: `employee_id`, `department_id`, `from_date`, `to_date`, `limit`, `offset`. Viewer+ per D-09.
- `GET /api/v1/daily-records/{id}` — single record with anomalies. Viewer+ per D-09.
- `GET /api/v1/anomalies` — supervisor queue with filters: `code`, `employee_id`, `from_date`, `to_date`. **Supervisor-or-above only (T-3-04)** — viewers cannot see raw anomaly-level detail.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Single-connection transaction for recompute_for_day**
- **Found during:** Task 2 (integration test failure)
- **Issue:** Initial implementation used a second dedicated `state.db.connect()` for the `BEGIN`/`COMMIT` phase. libSQL's shared-cache file-locked backend returned `SQLite failure: database is locked` when both connections were live, even after dropping all read cursors.
- **Fix:** Reuse the existing `conn` (same connection) for the transaction. All read cursors are drained + dropped before `BEGIN`, so there's no contention. This matches how `events/service.rs` handles its own reads+writes on one connection.
- **Files modified:** `backend/src/daily_records/service.rs`
- **Commit:** 22c670c

**2. [Rule 3 — Blocking] NewAttendanceEvent move semantics in isapi/stream.rs**
- **Found during:** Task 2 (hook wiring)
- **Issue:** `persist_attendance_event(conn, event)` takes `event` by value. Adding a post-insert `publish_recompute_if_employee(state, &event)` call would require moving `event` first, making the borrow unavailable.
- **Fix:** Build a lightweight snapshot (`NewAttendanceEvent` clone with empty `raw_xml` + `photo_bytes: None`) before the persist call; pass the snapshot to the publish hook. Keeps `persist_attendance_event`'s signature and all existing unit tests intact.
- **Files modified:** `backend/src/isapi/stream.rs`
- **Commit:** 22c670c

### No Architectural Changes Required

Everything built stayed within the plan's scope; Rules 1 and 4 did not fire.

## Reference Pattern for Plans 03-02 and 03-03

Two intentional "slots" are wired to accept specialization without touching the core engine:

1. **`EngineInput.leave: Option<LeaveRow>`** — always `None` in Plan 03-01. Plan 03-03 populates it from a `daily_records::service::fetch_active_leave_for_date()` call BEFORE invoking the engine. The engine's leave-overlay branch (D-16) already handles `Some(leave)` — returns `{work_minutes: 0, ..., anomalies: [EventsOnLeaveDay?]}`. Plan 03-03 **adds the fetch**, does not touch `engine.rs`.

2. **`dept.is_overnight_shift: bool`** — always `false` in Plan 03-01. Plan 03-02 implements the `true` branch inside `aggregation::shift_window()` by setting `end_date = anchor_date.succ_opt()` and emitting `OvernightInferenceAmbiguous` when `tz.from_local_datetime(...).single()` returns `None` (DST boundary). Plan 03-02 also adds overnight-shift LOTTT scenario fixtures.

This layering means Plans 03-02 and 03-03 are purely additive — they do not modify the pure engine's core algorithm, only specialize the inputs and the aggregation window.

## Known Stubs

None. All stub placeholders created in Task 1 (`fn todo_in_task2()` markers in aggregation/lunch/overtime/engine) were replaced with real implementations in Task 2.

## Threat Flags

No new trust boundaries introduced beyond what the plan's `<threat_model>` covered. The supervisor-or-above RBAC on `/anomalies` is already registered via T-3-04.

## Self-Check: PASSED

Automated verification:
- `cd backend && cargo build --lib` → compiles, 0 errors
- `cd backend && cargo nextest run --workspace` → 156 passed, 1 skipped, 0 failed
- `grep -q 'ON CONFLICT(employee_id, anchor_date) DO UPDATE' backend/src/daily_records/service.rs` → matches
- `! grep -q 'INSERT OR REPLACE' backend/src/daily_records/service.rs` → no hits
- `grep -q 'work_minutes + overtime_minutes > 600' backend/src/calc/overtime.rs` → matches (LOTTT total-workday cap, not OT-hours cap)
- `grep -q 'employee_id\.as_ref\|employee_id\.is_some' backend/src/events/service.rs` → matches (Pitfall 7 guard)
- All 21 `<automated>` verify steps from the plan pass (see Task 1 + Task 2 verify blocks in 03-01-PLAN.md)
- Required files exist at documented paths (verified via git ls-tree)
- Task 1 commit 03fd04d and Task 2 commit 22c670c both present in `git log`

```bash
[ -f backend/src/calc/engine.rs ] && echo FOUND: backend/src/calc/engine.rs
[ -f backend/src/daily_records/service.rs ] && echo FOUND: backend/src/daily_records/service.rs
[ -f backend/src/recompute/worker.rs ] && echo FOUND: backend/src/recompute/worker.rs
[ -f backend/src/db/migrations/007_daily_records.sql ] && echo FOUND
[ -f backend/src/db/migrations/008_daily_record_anomalies.sql ] && echo FOUND
[ -f backend/src/db/migrations/012_shift_type_to_departments.sql ] && echo FOUND
[ -f backend/tests/fixtures/lottt_scenarios.json ] && echo FOUND
git log --oneline | grep -qE "^03fd04d" && echo FOUND: commit 03fd04d
git log --oneline | grep -qE "^22c670c" && echo FOUND: commit 22c670c
```

All commands above return `FOUND`.
