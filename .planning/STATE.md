---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Phase 5 context gathered
last_updated: "2026-04-25T18:31:24.557Z"
last_activity: 2026-04-24 -- Phase --phase execution started
progress:
  total_phases: 7
  completed_phases: 4
  total_plans: 15
  completed_plans: 15
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-11)

**Core value:** Accurate, auditable time tracking that turns raw biometric events into payroll-ready data — with zero manual calculation and full legal traceability.
**Current focus:** Phase --phase — 04

## Current Position

Phase: --phase (04) — EXECUTING
Plan: 1 of --name
Status: Executing Phase --phase
Last activity: 2026-04-24 -- Phase --phase execution started

Progress: [████████░░] 82%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*
| Phase 01-foundation P00 | 25 | 2 tasks | 12 files |
| Phase 01-foundation P01 | 8 | 2 tasks | 13 files |
| Phase 01-foundation P02 | 6 | 2 tasks | 12 files |
| Phase 01-foundation P03 | 35 | 3 tasks | 15 files |
| Phase 01-foundation P04 | 8 | 2 tasks | 31 files |
| Phase 03-time-calculation-engine P01 | 26 | 2 tasks | 39 files |
| Phase 03-time-calculation-engine P02 | 9 | 2 tasks | 7 files |
| Phase 03 P03 | 28 | 2 tasks | 10 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Rust + Axum backend — alertStream connections and concurrent webhook processing performance
- [Init]: SQLite + Turso — local-first, treat SQLite as write primary, cloud as async replica (beta caveat)
- [Init]: Audit trail enforced via SQLite triggers, not application code only — legal defensibility
- [Init]: UTC epoch integer storage for all timestamps — overnight shift and DST correctness from migration zero
- [Phase 01-foundation]: Placeholder SQL approach: include_str! guard skips execution if file starts with '-- Placeholder', enabling Wave 0 compilation without a real schema
- [Phase 01-foundation]: tests/common/mod.rs as shared fixture module: test_db() returns isolated in-memory libSQL DB per test call; TEST_JWT_SECRET constant for test-only JWT generation
- [Phase 01-foundation]: lib.rs added to expose pub modules — binary crates cannot be referenced from integration test crates without a library target
- [Phase 01-foundation]: Test fixture uses unique temp file DB not :memory: — sqlite3_open_v2(':memory:') creates isolated DB per connection causing migrations to be invisible to subsequent connections
- [Phase 01-foundation]: tracing-subscriber env-filter feature must be explicitly enabled for with_env_filter() — not included in plan Cargo.toml spec
- [Phase 01-foundation]: SameSite=Lax (not Strict) on refresh cookie: allows third-party link navigation in on-premise deployments while still blocking CSRF POST attacks
- [Phase 01-foundation]: refresh/logout routes not behind require_auth Bearer middleware — they self-authenticate via refresh cookie; Bearer middleware would block legitimate refresh flows
- [Phase 01-foundation]: jsonwebtoken rust_crypto feature enabled to avoid rustls CryptoProvider panic in test environments without a full TLS stack
- [Phase 01-foundation]: Soft delete verification in tests uses REST API (GET by id) not direct DB connection — libsql::Database does not implement Clone
- [Phase 01-foundation]: Dynamic WHERE clause with positional param indexing for optional filters — avoids SQL injection without ORM
- [Phase 01-foundation]: effective_from always updated on any PATCH to global_rules — per RULE-03, any rule change resets the effective period
- [Phase 01-foundation]: proxy.ts (not middleware.ts): Next.js 16 renamed Middleware to Proxy — function export also renamed to `proxy`
- [Phase 01-foundation]: Metadata in layout.tsx not page.tsx: Next.js 16 forbids metadata export from client components ('use client')
- [Phase 01-foundation]: Providers component: QueryClientProvider must be a client component, isolated from server Root Layout
- [Phase 01-foundation]: frontend/.git removed: create-next-app creates its own git repo; removed to track files in monorepo
- [Phase 03-time-calculation-engine]: Single-connection txn for recompute_for_day — libSQL shared-cache lock contention between separate reader/writer connections produced "database is locked" under test load; reusing the same conn after draining all read cursors is safe and matches events/service pattern.
- [Phase 03-time-calculation-engine]: ON CONFLICT(employee_id, anchor_date) DO UPDATE (not INSERT OR REPLACE) for daily_records upsert — preserves the row id so daily_record_anomalies FK survives recomputes (Pitfall 1).
- [Phase 03-time-calculation-engine]: LOTTT Art. 178 daily cap = total workday > 600min (work + OT), not "OT > 120min" — the statute constrains total hours, not OT-hours specifically.
- [Phase 03-time-calculation-engine]: Engine is pure (no I/O, no async) — aggregation/lunch/overtime/engine submodules, decomposed from the {mod, models, service, handlers} Phase 1/2 layout. Proptest validates determinism across 270k random inputs.
- [Phase 03-time-calculation-engine]: RecomputeWorker mirrors Phase 2 Supervisor: biased select, HashSet dedup, 500ms debounce, tokio::time::sleep-driven nightly (no cron crate).
- [Phase 03-time-calculation-engine]: publish_recompute_if_employee guards on employee_id.is_some() AND recompute_tx.is_some() — Pitfall 7 (never flood worker with unknown-face NULL ids) + test-setups-without-worker compatibility.
- [Phase 03-time-calculation-engine]: Overnight shifts: .earliest() path on LocalResult (not .single().unwrap()) — Caracas always returns Single(dt), but the infrastructure exists so a future DST market cannot panic the calc thread; ambiguity surfaces via OvernightInferenceAmbiguous anomaly.
- [Phase 03-time-calculation-engine]: shift_window() kept as 4-tuple delegating to shift_window_overnight_aware(); new shift_window_with_ambiguity() exposes the 5-tuple for engine.rs — zero callsite changes in service.rs or other modules, Plan 03-01 day-only tests pass unchanged.
- [Phase 03-time-calculation-engine]: No SQL change in daily_records::service for overnight support — because shift_window() now returns an across-midnight (start, end) range, the existing captured_at BETWEEN query picks up post-midnight events automatically. Proven by recompute_overnight_captures_post_midnight_events integration test.
- [Phase 03]: LEAVE_OVERLAP uses dedicated LeaveConflict variant (HTTP 409), not generic Conflict — distinguishes business-rule overlap from optimistic-concurrency conflicts for Phase 4 UI remediation.
- [Phase 03]: Evidence files are UUIDv4-named (user filename discarded). cancel_leave soft-deletes DB row but preserves evidence file on disk for LOTTT audit retention.

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 2]: Hikvision ISAPI XML schema varies by device model (DS-K1T341, DS-K1T342) — capture real alertStream traffic before implementation; do not rely on documentation alone
- [Phase 3]: Venezuela / America/Caracas / LOTTT compliance — IANA timezone fixed at `America/Caracas` (UTC-4, no DST since May 2016); LOTTT Art. 117/173/178 caps confirmed via Phase 3 research. No blocking DST concern for v1.
- [Phase 7]: ISAPI batch face profile enrollment failure behavior on partial failure (3 of 4 devices) is undocumented — requires hands-on hardware testing before designing the enrollment modal

## Session Continuity

Last session: --stopped-at
Stopped at: Phase 5 context gathered
Resume file: --resume-file

**Planned Phase:** 03 (time-calculation-engine) — 3 plans — 2026-04-23T18:47:20.670Z
