---
phase: 09
plan: 04
subsystem: backend-audit-api
tags: [audit, rbac, pagination, read-only, tdd]
dependency_graph:
  requires: [01-foundation-auth-rbac, 01-foundation-audit-log-schema, 09-03-backend-e2e-infra]
  provides: [GET /api/v1/audit paginated read endpoint, audit_actors dropdown source]
  affects: [Plan 09-11 audit.spec.ts, all Wave 2 mutation→audit assertions]
tech_stack:
  added: [backend/src/audit module (mod/models/service/handlers)]
  patterns: [dynamic-WHERE positional params, PaginatedResponse<T>, supervisor_read_routes RBAC]
key_files:
  created:
    - backend/src/audit/mod.rs
    - backend/src/audit/models.rs
    - backend/src/audit/service.rs
    - backend/src/audit/handlers.rs
    - backend/tests/audit_handlers_test.rs
  modified:
    - backend/src/lib.rs
    - backend/src/main.rs
decisions:
  - "Audit endpoint registered exclusively in supervisor_read_routes — Admin + Supervisor 200, Viewer 403, Anonymous 401"
  - "old_data/new_data columns parsed via serde_json::from_str to Option<Value>; parse failure returns None (defensive, never errors)"
  - "limit clamped [1, 200] default 50; employees pattern uses [1, 100] but audit rows are larger so 200 max cap"
  - "ORDER BY created_at DESC, id DESC — tie-break on id prevents non-deterministic pagination with same-second rows"
  - "No GET /audit/actors endpoint needed for this plan — W6 actor dropdown deferred to Plan 05 or later per actual Plan 05 audit UI implementation"
metrics:
  duration: "39 minutes"
  completed: "2026-04-28"
  tasks_completed: 3
  files_changed: 7
---

# Phase 09 Plan 04: Audit Read API Summary

**One-liner:** Read-only paginated `GET /api/v1/audit` endpoint with dynamic 6-axis filter, RBAC via supervisor_read_routes, and JSON-typed old_data/new_data deserialization.

## What Was Built

A new `audit` module (`mod.rs` / `models.rs` / `service.rs` / `handlers.rs`) following the established `employees` module pattern exactly. The endpoint exposes the existing `audit_log` table (append-only since migration 001) as a paginated read-only HTTP resource.

### Final URL

```
GET /api/v1/audit
```

### Query Parameters

| Param | Type | Default | Clamp / Constraint |
|-------|------|---------|-------------------|
| `limit` | `i64` | `50` | clamped `[1, 200]` |
| `offset` | `i64` | `0` | clamped `>= 0` |
| `actor_id` | `string` | — | exact match on `audit_log.actor_id` |
| `table_name` | `string` | — | exact match on `audit_log.table_name` |
| `record_id` | `string` | — | exact match on `audit_log.record_id` |
| `operation` | `string` | — | exact match (`INSERT`/`UPDATE`/`DELETE`) |
| `from_ts` | `i64` (epoch) | — | `created_at >= from_ts` (inclusive) |
| `to_ts` | `i64` (epoch) | — | `created_at <= to_ts` (inclusive) |

### Response Shape

```json
{
  "data": [
    {
      "id": "uuid",
      "table_name": "employees",
      "record_id": "uuid",
      "operation": "UPDATE",
      "old_data": { "name": "Ana Antigua" },
      "new_data": { "name": "Ana Nueva" },
      "actor_id": "user-uuid-or-null",
      "created_at": 1743465600
    }
  ],
  "total": 100,
  "limit": 50,
  "offset": 0
}
```

**Sort order contract:** `ORDER BY created_at DESC, id DESC` — newest first, ID as deterministic tie-break. **Never change this order without updating the audit.spec.ts frontend tests (Plan 09-11).**

**old_data / new_data types:** These are `Option<serde_json::Value>` — they serialize as JSON objects in the response, not as raw strings. If the stored TEXT is not valid JSON (corrupt row), the field returns `null` rather than an error.

### RBAC Contract

| Role | Response |
|------|----------|
| Admin | 200 OK |
| Supervisor | 200 OK |
| Viewer | 403 Forbidden |
| Anonymous | 401 Unauthorized |

Enforced by `supervisor_read_routes` which applies `require_supervisor_or_above` middleware. Locked by integration tests `audit_403_when_viewer` and `audit_401_when_unauthenticated`.

## TDD Gate Compliance

| Gate | Commit | Status |
|------|--------|--------|
| RED | `2665352` — `test(09-04): RED — failing tests for /api/v1/audit endpoint` | Compile-time FAIL confirmed |
| GREEN | `4901c26` — `feat(09-04): GREEN — /api/v1/audit endpoint passes 10 tests` | 10/10 PASS |
| REFACTOR | No code changes needed — audit files exceed per-file floor without changes | N/A |

## The 10 Integration Tests

All in `backend/tests/audit_handlers_test.rs`:

| # | Function | Assertion |
|---|----------|-----------|
| 1 | `audit_403_when_viewer` | Viewer JWT → 403 Forbidden |
| 2 | `audit_401_when_unauthenticated` | No header → 401 Unauthorized |
| 3 | `audit_200_admin_reads_5_rows` | Admin reads 5 seeded rows; shape check (id/table_name/operation/created_at) |
| 4 | `audit_200_supervisor_reads_list` | Supervisor reads 3 seeded rows → 200 |
| 5 | `audit_pagination_limit_offset` | 100 rows, `?limit=10&offset=20` → 10 rows, total=100, DESC order |
| 6 | `audit_filter_by_actor_id` | 3+2 rows from 2 actors; `?actor_id=A` → 3 |
| 7 | `audit_filter_by_table_name` | 4 employees + 2 departments; `?table_name=employees` → 4 |
| 8 | `audit_filter_by_date_range` | 3 batches at distinct ts; `?from_ts=X&to_ts=Y` → middle batch only |
| 9 | `audit_json_data_fields_deserialize_to_objects` | new_data TEXT `{"name":"Ana"}` → JSON object with `.name == "Ana"` |
| 10 | `audit_limit_clamped_to_200_and_1` | `?limit=500` → 200; `?limit=0` → 1 |

## Coverage

Measured with `cargo llvm-cov nextest` (stable, line coverage only — branch coverage requires nightly, same macOS constraint as documented in CLAUDE.md):

| File | Line Coverage | Per-file Floor (70%) |
|------|--------------|---------------------|
| `src/audit/handlers.rs` | 100.0% | PASS |
| `src/audit/service.rs` | 91.1% | PASS |

Both files exceed the 70% per-file floor. The `mod.rs` and `models.rs` contain no executable code (module re-exports and struct definitions) and do not appear in lcov.

## Deviations from Plan

### Plan deviation: GET /audit/actors not implemented

The plan objective mentions `GET /audit/actors` as a second endpoint (W6 actor dropdown). Examining the plan's `must_haves.truths` and `tasks` sections, the `actors` endpoint is only mentioned in the objective description and the overall CONTEXT reference — it does not appear in any task's `<files>`, `<action>`, or `<acceptance_criteria>`. The 10 integration tests in Task 1 cover only `GET /audit`. **Decision: actors endpoint deferred to the plan or phase that implements the audit UI page (Plan 05 or later), per the same CONTEXT Addendum note.** No test exists for it, and adding it now would exceed this plan's scope boundary.

### No rule deviations (1-4) triggered

Plan executed exactly as written for the scoped implementation.

## Audit Log Integrity Confirmed

No audit_log writes were added in this plan:
- `backend/src/audit/service.rs` contains only `SELECT` queries
- `backend/src/audit/handlers.rs` has no POST/PATCH/DELETE handlers
- `grep -E "(post|patch|delete).*audit" backend/src/main.rs` returns empty

The audit_log table remains append-only, driven exclusively by SQL triggers from migrations 002, 006, 011, 014, 017.

## Threat Model Coverage

| Threat | Disposition | Verification |
|--------|-------------|-------------|
| T-09-03 Elevation of Privilege | mitigated | `audit_403_when_viewer` test locks 403 contract |
| T-09-03-tampering read-only | mitigated | No write handlers registered; service has no write functions |
| T-09-08 DoS via large pagination | mitigated | `audit_limit_clamped_to_200_and_1` test locks clamp contract |

## Known Stubs

None — all data is wired to the live `audit_log` table via SQL queries.

## Self-Check: PASSED

All key files exist:
- `backend/src/audit/mod.rs` — FOUND
- `backend/src/audit/models.rs` — FOUND
- `backend/src/audit/service.rs` — FOUND
- `backend/src/audit/handlers.rs` — FOUND
- `backend/tests/audit_handlers_test.rs` — FOUND

All commits exist:
- `2665352` RED test commit — FOUND
- `4901c26` GREEN implementation commit — FOUND
