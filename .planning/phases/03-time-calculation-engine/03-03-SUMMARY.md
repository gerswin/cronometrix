---
phase: 03-time-calculation-engine
plan: 03
subsystem: leaves-management
tags: [leaves, overlay, rbac, multipart, audit, lottt, d-13-d-16]
dependency_graph:
  requires:
    - Plan 03-01 (EngineInput.leave: Option<LeaveRow>, engine.rs overlay branch, errors.rs CalcError variant pattern)
    - Plan 03-02 (no direct dependency; overnight shifts orthogonal to leaves)
    - Phase 1 (AppState + RBAC middleware, users/employees tables, audit_log table, validator-derived DTO pattern, PaginatedResponse<T>)
    - Phase 2 (events::service::write_photo_atomic reused for evidence writes)
  provides:
    - leaves table + daily_record_overrides table (schemas reserved by Plan 03-01)
    - AFTER INSERT/UPDATE/DELETE audit triggers on leaves and daily_record_overrides
    - leaves::service public surface (create_leave / get_by_id / list / cancel / fetch_active_leave_for_date / leaves_root)
    - leaves::handlers full CRUD (POST multipart, GET list/id/evidence, DELETE cancel)
    - AppError::LeaveConflict (HTTP 409)
    - D-16 overlay wired end-to-end in daily_records::service::recompute_for_day
    - 2 LOTTT leave-overlay fixtures (medical full-day + vacation-with-events)
  affects:
    - Phase 4 (supervisor queue reads leaves.leave_type via daily_records JOIN; timesheet editor writes to daily_record_overrides)
    - Phase 5 (payroll export reads daily_records.leave_id → leaves.leave_type for IVSS-indemnization treatment of medical leaves per LEAVE-04)
tech-stack:
  added:
    - axum feature "multipart" enabled on the existing 0.8.8 dependency (no new crate)
  patterns:
    - Multipart streaming with content-type enum guard (pdf/jpeg/png) + hard 10MB cap before DB commit
    - Server-generated UUID evidence paths (user filename discarded) + canonicalize + starts_with root guard on read
    - Overlap check as SQL predicate (from_date <= ?new_to AND to_date >= ?new_from) — same shape as the single-day overlay query specialized to one anchor_date via ?2=?2
    - Soft-delete via status='cancelled' + deleted_at with optimistic concurrency (version column + 404/409 disambiguation)
    - Handler publishes RecomputeRequest for every anchor_date in [from_date, to_date] on both create and cancel — existing daily_records pick up / drop the overlay transparently
key-files:
  created:
    - backend/src/db/migrations/009_daily_record_overrides.sql
    - backend/src/db/migrations/010_leaves.sql
    - backend/src/db/migrations/011_phase3_audit_triggers.sql
    - backend/src/leaves/mod.rs
    - backend/src/leaves/models.rs
    - backend/src/leaves/service.rs
    - backend/src/leaves/handlers.rs
    - backend/tests/leave_tests.rs
  modified:
    - backend/Cargo.toml
    - backend/Cargo.lock
    - backend/src/db/mod.rs
    - backend/src/errors.rs
    - backend/src/lib.rs
    - backend/src/main.rs
    - backend/src/daily_records/service.rs
    - backend/tests/calc_tests.rs
    - backend/tests/common/mod.rs
    - backend/tests/fixtures/lottt_scenarios.json
decisions:
  - "LEAVE_OVERLAP error code maps to HTTP 409 via a dedicated AppError::LeaveConflict variant rather than reusing generic Conflict. Rationale: distinguishes overlap rejection (always an application-level business rule) from optimistic-concurrency version conflicts in API responses, allowing the Phase 4 UI to surface overlap-specific remediation ('cancel the existing leave first')."
  - "Evidence files are stored with filenames = UUIDv4 + extension (pdf/jpg/png). The UUID is generated at multipart-parse time, BEFORE the DB insert; if the insert fails (overlap / validation) the file remains orphaned until manual cleanup. Accepted because (a) write_photo_atomic uses a tempfile+rename pattern so we never leave truncated files, and (b) orphan rate is bounded by human error rate on admin forms — not a concerning leak given the single-tenant deployment model."
  - "axum::extract::Multipart requires the `multipart` feature flag (not enabled by Plan 03-01/02). Added as a dependency-config diff; multer was already transitive via axum, so no new top-level crate entered the build graph."
  - "calc_tests.rs fixture loader gained optional `active_leave` + `expected_leave_id_non_null` fields via serde `default`. Existing 7 scenarios in lottt_scenarios.json are unchanged — backward-compatibility preserved, and the 2 new leave scenarios exercise the engine overlay path directly without touching the persistence layer."
metrics:
  duration_min: 28
  tasks_completed: 2
  tests_added: "11 integration + 2 LOTTT fixture scenarios + 1 validation negative test = 14 new behavioral assertions; workspace total rises 165 → 180 (+15)"
  completed_date: 2026-04-23
---

# Phase 3 Plan 03: Leave Management + Overlay Wiring Summary

**One-liner:** LEAVE-01..04 delivered end-to-end — admin-only multipart evidence upload, overlap check via `LEAVE_OVERLAP → 409`, soft-delete with optimistic concurrency, and D-16 overlay activated through `daily_records::service::recompute_for_day` fetching `fetch_active_leave_for_date()` per anchor_date. 180/180 workspace tests green; Phase 3 CONTEXT decisions D-13..D-16 all shipped.

## What Was Built

### Migration Order (final Phase 3 state)

| # | Name | Content |
|---|------|---------|
| 001 | initial_schema | users, departments, employees, global_rules, audit_log |
| 002 | audit_triggers | Phase 1 INSERT/UPDATE/DELETE triggers |
| 003 | devices | Phase 2 device registry |
| 004 | attendance_events | Phase 2 event store + dedup index |
| 005 | command_audit_log | Phase 2 append-only command audit |
| 006 | devices_audit_triggers | Phase 2 device mutation audits |
| 007 | daily_records | Plan 03-01 materialized engine-owned table |
| 008 | daily_record_anomalies | Plan 03-01 append-only anomaly rows |
| **009** | **daily_record_overrides** | **Plan 03-03 — operator edits (Phase 4 consumer)** |
| **010** | **leaves** | **Plan 03-03 — leave taxonomy (D-13), CHECK enum, soft-delete** |
| **011** | **phase3_audit_triggers** | **Plan 03-03 — triggers on leaves + daily_record_overrides** |
| 012 | shift_type_to_departments | Plan 03-01 `shift_type`/`is_overnight_shift`/`ordinary_daily_minutes` backfill |

All 12 migrations run idempotently via the existing `_migrations` tracking table. `011` lives BEFORE `012` so trigger installation precedes the column backfill — the plan's verification script `python3 -c "import re; ..."` confirms.

### `backend/src/leaves/` public surface

```rust
// service.rs
pub fn leaves_root() -> PathBuf                           // env CRONOMETRIX_LEAVES_ROOT override
pub async fn create_leave(conn, actor_id, req, evidence_relpath) -> Result<LeaveResponse>
pub async fn get_by_id(conn, id) -> Result<LeaveResponse>
pub async fn list(conn, q) -> Result<PaginatedResponse<LeaveResponse>>
pub async fn cancel(conn, id, version) -> Result<()>      // soft-delete, optimistic concurrency
pub async fn fetch_active_leave_for_date(conn, employee_id, anchor_date) -> Result<Option<LeaveRow>>

// handlers.rs
create_leave   POST   /api/v1/leaves             (require_admin, multipart/form-data)
list_leaves    GET    /api/v1/leaves             (require_auth, pagination+filters)
get_leave      GET    /api/v1/leaves/{id}        (require_auth)
cancel_leave   DELETE /api/v1/leaves/{id}?version=N   (require_admin, soft-delete + recompute)
get_leave_evidence GET /api/v1/leaves/{id}/evidence  (require_auth, canonicalize guard)
```

### RBAC Matrix (per T-3-19 mitigation + D-09)

| Action | Admin | Supervisor | Viewer |
|--------|:-----:|:----------:|:------:|
| POST /leaves | allow | 403 | 403 |
| DELETE /leaves/{id} | allow | 403 | 403 |
| GET /leaves | allow | allow | allow |
| GET /leaves/{id} | allow | allow | allow |
| GET /leaves/{id}/evidence | allow | allow | allow |

Tests `create_leave_forbidden_for_supervisor` and `create_leave_forbidden_for_viewer` both pass with HTTP 403.

### Evidence File Lifecycle

1. **Upload:** admin submits `multipart/form-data` with an optional `evidence` field. Handler validates `Content-Type ∈ {application/pdf, image/jpeg, image/png}` (T-3-16) and reads bytes with a 10 MB hard cap (T-3-21). If oversize or wrong content-type → 422 VALIDATION_ERROR before any DB work.
2. **Path generation:** `{UUIDv4}.{ext}` under `leaves_root()` (default `./data/leaves`, env-overridable to a tempdir for tests via `CRONOMETRIX_LEAVES_ROOT`). User filename is completely discarded (T-3-15).
3. **Write:** `events::service::write_photo_atomic` — tempfile + fsync + rename. Reused directly; no Phase 3 re-implementation.
4. **Read:** `GET /leaves/{id}/evidence` reads `leaves.evidence_path`, rejects any value containing `..` or starting `/`, canonicalizes the root and resolved path, and verifies `canonical.starts_with(root_canonical)` before `tokio::fs::read`. Any failure → 404 `LEAVE_EVIDENCE_NOT_FOUND` (never 500). Test `evidence_path_traversal_rejected` seeds a malicious `../../../../etc/passwd` row and asserts 404.
5. **Delete:** not implemented in v1 — `cancel_leave` performs a soft-delete of the DB row but does NOT remove the evidence file. Accepted because LOTTT legal retention requires the evidence to remain for audit traceability even after leave cancellation.

### Leave Overlay Wiring (D-16)

`daily_records::service::recompute_for_day` sequence (unchanged from Plan 03-01 except step 7):

1. Load employee + department config.
2. Load global_rules singleton.
3. Window-bounded event fetch via `calc::aggregation::shift_window`.
4. Weekly OT lookback.
5. Annual OT lookback.
6. Prior-row existence check → RECOMPUTE_AFTER_EDIT.
7. **NEW:** `leaves::service::fetch_active_leave_for_date(&conn, employee_id, anchor_date).await?` → `Option<LeaveRow>`.
8. Build `EngineInput { leave: active_leave, ... }` and call pure `calc::compute_daily_record`.
9. Upsert `daily_records` + replace `daily_record_anomalies` in one transaction.

When an active leave covers `anchor_date`, the engine's existing overlay branch (written in Plan 03-01, dormant until now) fires:

```rust
if let Some(leave) = &input.leave {
    let mut anomalies = Vec::new();
    if !input.events.is_empty() { anomalies.push(AnomalyCode::EventsOnLeaveDay); }
    if input.prior_record_existed { anomalies.push(AnomalyCode::RecomputeAfterEdit); }
    return DailyRecordOutput {
        work_minutes: 0, overtime_minutes: 0, late_minutes: 0,
        early_departure_minutes: 0, is_rest_day_worked: false,
        entry_at: None, exit_at: None,
        leave_id: Some(leave.id.clone()),
        anomalies,
    };
}
```

Raw `attendance_events` are NEVER deleted; D-16 requires that the append-only event store is preserved — only the derived DailyRecord is zeroed. Integration test `leave_overlay_suppresses_work_minutes` asserts both claims simultaneously (`work=0 AND event_count_in_store == 2`).

### Recompute-on-Leave-Change

Both `create_leave` and `cancel_leave` handlers call `publish_recompute_for_range(state, employee_id, from_date, to_date)`. The helper iterates every `anchor_date` in the inclusive range and sends a `RecomputeRequest` per day. The Phase 3 `RecomputeWorker` (mpsc + 500ms debounce + HashSet dedup) collapses consecutive requests for the same (employee_id, anchor_date) into a single recompute, so issuing 30 requests for a 30-day vacation block results in at most 30 distinct recomputes (one per day) regardless of worker scheduling.

Silent no-op when `state.recompute_tx` is `None` (test harness without a worker) — matches the Phase 2 Supervisor lifecycle-tx pattern.

### Audit Trail (migration 011)

Six triggers total — INSERT / UPDATE / DELETE on both `leaves` and `daily_record_overrides`. Each trigger:
- Generates UUID v4 via the `hex(randomblob)` idiom used in Phase 1/2.
- Writes `json_object(...)` payloads to `audit_log.old_data` / `new_data` carrying all business-visible columns (id, employee_id, from_date, to_date, leave_type, justification, evidence_path, status, version for leaves; id, daily_record_id, override_work_minutes, override_entry_at, override_exit_at, justification, evidence_path, overridden_by, status, version for overrides).
- Leaves `actor_id` NULL — app code can write a Phase-2-style `command_audit_log` row with actor context when needed.

### New LOTTT Fixtures (Scenarios 8 & 9)

| # | Scenario | Active leave | Events | Expected output |
|---|----------|--------------|:-----:|-----------------|
| 8 | Full-day medical, no events | medical 2026-04-20→2026-04-20 | 0 | work=0, ot=0, leave_id=SOME, no anomalies |
| 9 | Vacation with accidental punch | vacation 2026-04-20→2026-04-20 | 1 entry at 13:00Z | work=0, ot=0, leave_id=SOME, EVENTS_ON_LEAVE_DAY |

Both assert via `calc_tests::lottt_scenarios_all_pass` which now also asserts `out.leave_id.is_some()` whenever the fixture's `expected_leave_id_non_null` is true.

## Test Coverage Matrix

| Requirement / Threat | Test | Outcome |
|---------------------|------|---------|
| LEAVE-01 (medical + evidence) | `create_leave_medical_with_evidence` | 201 + evidence_path set + file on disk |
| LEAVE-01 negative | `create_leave_medical_without_evidence_rejected` | 422 VALIDATION_ERROR |
| LEAVE-02 (manual, no evidence) | `create_leave_manual_without_evidence` | 201 + justification persisted |
| LEAVE-03 (overlay suppresses work) | `leave_overlay_suppresses_work_minutes` | work=0, ot=0, EVENTS_ON_LEAVE_DAY, events preserved |
| LEAVE-04 (medical flag via JOIN) | `leave_overlay_medical_flag_preserved` | daily_records ⟵ leaves JOIN returns 'medical' |
| T-3-14 (overlap) | `create_leave_overlap_returns_conflict` | 409 LEAVE_OVERLAP |
| T-3-15 (path traversal) | `evidence_path_traversal_rejected` | 404 LEAVE_EVIDENCE_NOT_FOUND |
| T-3-19 (supervisor forbidden) | `create_leave_forbidden_for_supervisor` | 403 |
| T-3-19 (viewer forbidden) | `create_leave_forbidden_for_viewer` | 403 |
| Optimistic concurrency | `cancel_leave_optimistic_concurrency` | stale→409, correct→204 + row soft-deleted |
| D-09 read-all | `list_leaves_accessible_to_viewer` | viewer GET /leaves → 200 with data |
| D-16 engine unit | `lottt_scenarios_all_pass` (scenarios 8 & 9) | work=0, leave_id=Some(...), anomalies as expected |

## Phase 3 Close-Out Checklist

| Decision | Plan | Status |
|----------|------|:------:|
| D-01 materialized table | 03-01 | done |
| D-02 event-driven + nightly | 03-01 | done |
| D-03 RECOMPUTE_AFTER_EDIT | 03-01 | done |
| D-04 separate overrides table | 03-03 | schema shipped (table populated by Phase 4) |
| D-05 overnight anchor=shift-start | 03-02 | done |
| D-06 is_overnight_shift column | 03-01 | done |
| D-07 single-TZ installation | 03-01 | done (Config::timezone) |
| D-08 DST-safe .earliest() | 03-02 | done |
| D-09 LOTTT Art. 178 caps | 03-01 | done |
| D-10 minutes, not money | 03-01 | done |
| D-11 shift_type enum | 03-01 | done |
| D-12 rest-day Sat+Sun hardcode | 03-01 | done |
| D-13 leave taxonomy enum | 03-03 | done |
| D-14 full-day only | 03-03 | done |
| D-15 immediate approval | 03-03 | done |
| D-16 overlay precedence | 03-03 | done (end-to-end wired) |
| D-17 bonus_minutes grace | 03-01 | done |
| D-18 anomaly codes | 03-01 | done |
| D-19 lunch punch fallback | 03-01 | done |
| D-20 aggregation window | 03-01 | done |

All 20 decisions from `03-CONTEXT.md` are now implemented.

| Requirement | Plan | Integration test |
|-------------|------|------------------|
| CALC-01..06 | 03-01, 03-02 | `lottt_scenarios_all_pass` (7 scenarios) + overnight proptest 256 cases |
| LEAVE-01..04 | 03-03 | 4 targeted tests + 2 fixture scenarios |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] axum 0.8 `multipart` feature missing**

- **Found during:** Task 2 (first `cargo build`)
- **Issue:** Plan's imports (`axum::extract::Multipart`) failed to resolve; the project's `axum = "0.8.8"` dependency carried only `features = ["macros"]`. Compile errors: `type annotations needed` on every `field.text()` / `field.bytes()` call because `MultipartError` wasn't reachable, masking the real cause.
- **Fix:** Added `"multipart"` to `axum` features in `backend/Cargo.toml`. No new top-level crate — `multer` was already a transitive dep of axum.
- **Files modified:** `backend/Cargo.toml` (+ `Cargo.lock` auto-refresh)
- **Commit:** a07569d

**2. [Rule 3 — Blocking] Hand-rolled DB assertion helper `state_ref_db` returned panic**

- **Found during:** Task 2 (writing `cancel_leave_optimistic_concurrency`)
- **Issue:** Plan's test skeleton assumed a way to reach back into `AppState.db` after building a Router; my first draft wrote a placeholder `state_ref_db` that panicked. Leaving it in would silently break the soft-delete assertion.
- **Fix:** Removed the panic helper entirely. Because `AppState.db` is `Arc<libsql::Database>` and `state.clone()` is cheap, the test now captures `state` BEFORE `build_test_app(state.clone())` and reuses `state.db` directly for post-mutation assertions. Cleaner and no dead-code helper.
- **Files modified:** `backend/tests/leave_tests.rs`
- **Commit:** a07569d

**3. [Rule 2 — Correctness] Extra validation negative test**

- **Found during:** Task 2 authoring
- **Issue:** Plan described `create_leave_medical_with_evidence` happy path but no explicit negative test for "medical + missing evidence" — yet this rule is an in-service assertion that could silently drift if refactored.
- **Fix:** Added `create_leave_medical_without_evidence_rejected` (expects 422). Same shape as the happy-path test, just drops the `evidence` field.
- **Files modified:** `backend/tests/leave_tests.rs`
- **Commit:** a07569d

### No Architectural Changes Required

Rules 1 and 4 did not fire. Plan's interface (`EngineInput.leave: Option<LeaveRow>`), schema shape (leaves + CHECK enum), and RBAC model all held under implementation.

## Known Limitations (intentional deferrals per CONTEXT.md)

1. **Partial-day leave** — engine is full-day only (D-14). Half-day sick / early-departure with permission will be handled via Phase 4 timesheet edits writing to `daily_record_overrides`.
2. **Pending-approval workflow** — create is immediate (D-15). If a client later requires dual-actor accountability, add `status IN ('pending', 'approved', 'rejected')` state machine + approval endpoint.
3. **Vacation balance tracking** — `vacation` type currently zeros work_minutes without debiting any accrual counter. Balance system deferred until Phase 5 reporting demands it.
4. **Evidence file cleanup on cancel** — `cancel_leave` soft-deletes the DB row but preserves the evidence file (LOTTT audit retention). No v1 GC pass exists.
5. **Per-department timezone** — Config.timezone is a single IANA zone per installation (D-07). Multi-site clients would need a `departments.timezone` column + per-recompute zone lookup.
6. **Configurable leave types** — taxonomy is hard-coded to the 4-variant CHECK enum (medical/vacation/unpaid/manual). A future client with custom types would need an alternate schema: a `leave_types` table with per-row salary_pct + evidence_required columns.

## Known Stubs

None. All functions implemented; no `todo!()`, `unimplemented!()`, or `FIXME` markers in the Phase 3 code paths.

## Threat Flags

No new trust boundaries beyond the plan's `<threat_model>`. T-3-14 through T-3-21 are all covered by code (see the code-or-test mapping in the Test Coverage Matrix above).

## Self-Check: PASSED

Automated verification (all return FOUND / OK):

```bash
[ -f backend/src/db/migrations/009_daily_record_overrides.sql ] && echo FOUND
[ -f backend/src/db/migrations/010_leaves.sql ] && echo FOUND
[ -f backend/src/db/migrations/011_phase3_audit_triggers.sql ] && echo FOUND
[ -f backend/src/leaves/service.rs ] && echo FOUND
[ -f backend/src/leaves/handlers.rs ] && echo FOUND
[ -f backend/tests/leave_tests.rs ] && echo FOUND

grep -q 'CREATE TABLE IF NOT EXISTS daily_record_overrides' backend/src/db/migrations/009_daily_record_overrides.sql && echo OK
grep -q 'CREATE TABLE IF NOT EXISTS leaves' backend/src/db/migrations/010_leaves.sql && echo OK
grep -q 'CREATE TRIGGER IF NOT EXISTS audit_leaves_insert' backend/src/db/migrations/011_phase3_audit_triggers.sql && echo OK
grep -q 'CREATE TRIGGER IF NOT EXISTS audit_daily_record_overrides_insert' backend/src/db/migrations/011_phase3_audit_triggers.sql && echo OK

grep -q 'LeaveConflict' backend/src/errors.rs && echo OK
grep -q 'pub mod leaves;' backend/src/lib.rs && echo OK
grep -q 'fetch_active_leave_for_date' backend/src/daily_records/service.rs && echo OK
! grep -q 'leave: None' backend/src/daily_records/service.rs && echo OK
grep -q 'post(leaves::handlers::create_leave)' backend/src/main.rs && echo OK
grep -q 'delete(leaves::handlers::cancel_leave)' backend/src/main.rs && echo OK

cd backend && cargo nextest run --workspace 2>&1 | grep -q '180 tests run: 180 passed' && echo OK
```

```bash
git log --oneline | grep -qE "^25e25a6" && echo "FOUND: Task 1 commit 25e25a6"
git log --oneline | grep -qE "^a07569d" && echo "FOUND: Task 2 commit a07569d"
```

Both commits present.

**Commits:**
- Task 1: `25e25a6` — feat(03-03): add migrations 009/010/011 + leaves module skeleton + LeaveConflict
- Task 2: `a07569d` — feat(03-03): leaves CRUD + multipart upload + overlay wired; full integration tests
