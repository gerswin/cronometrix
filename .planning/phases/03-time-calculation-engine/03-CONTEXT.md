# Phase 3: Time Calculation Engine - Context

**Gathered:** 2026-04-23
**Status:** Ready for planning

<domain>
## Phase Boundary

The Attendance Engine transforms raw `attendance_events` into payroll-ready `DailyRecord` rows: applies first-entry/last-exit across devices within the shift window, computes late/early flags under tolerance + bonus rules, deducts lunch per department mode, calculates overtime under Venezuelan LOTTT caps, attributes overnight shifts to the correct anchor date, and overlays leave records. Covers CALC-01..06 and LEAVE-01..04.

Engine is a pure domain layer (no I/O). Persistence wrapper writes DailyRecords + anomalies + leave overlay. Thin CRUD API exposed for leave management. No frontend work (Phase 4). No report/export (Phase 5).

**Legal frame:** Target jurisdiction = Venezuela. Labor law = LOTTT (Ley Orgánica del Trabajo, los Trabajadores y las Trabajadoras). Applicable articles: 117 (night shift), 118 (OT premium +50%), 120 (Sunday/rest-day premium), 173 (workday limits), 178 (OT caps).

Out of scope: holiday calendar + surcharge (v2 HOL-01..03), partial-day leave, pending-approval workflow, payroll-to-money conversion, multi-region / per-department TZ.

</domain>

<decisions>
## Implementation Decisions

### Materialization & Recompute Strategy
- **D-01:** DailyRecords are persisted in a materialized `daily_records` table (not a pure view). Keyed on `(employee_id, anchor_date)`. Turso-sync friendly. Phase 5 reports read directly.
- **D-02:** Recompute trigger = event-driven + nightly reconcile. Each `attendance_events` insert enqueues an affected-day recompute (debounced). A nightly cron (02:00 local) re-reconciles the prior day to catch late arrivals.
- **D-03:** Late events (yesterday's punch surfacing today after device reconnect) auto-recompute the affected day and overwrite the existing DailyRecord. The prior record state is captured via audit trigger; a `RECOMPUTE_AFTER_EDIT` anomaly is raised on the new record so operators can audit drift.
- **D-04:** Manual timesheet edits (Phase 4) live in a separate `daily_record_overrides` table with mandatory justification + evidence per TS-03/TS-04. Engine computes the base DailyRecord; `GET /daily-records/:id` applies overrides at read time. Recompute never clobbers operator edits.

### Overnight Shifts & Timezone
- **D-05:** Anchor-date rule = shift-start date. A shift starting 22:00 Monday and ending 06:00 Tuesday belongs to Monday. Matches Venezuelan payroll convention and simplifies first-entry/last-exit aggregation.
- **D-06:** Overnight shifts are opt-in per department via `departments.is_overnight_shift BOOLEAN`. Explicit flag — no inference from `shift_end_time < shift_start_time` (too ambiguous for edge cases like 23:00–23:30 shifts).
- **D-07:** Single timezone per installation. Set via `TZ` env var (installer writes it during deployment). Canonical value = `America/Caracas` (UTC-4). No per-department or per-employee TZ.
- **D-08:** Calendar math uses `chrono-tz` with the configured IANA zone. Venezuela does NOT observe DST (abolished 2016), so 23h/25h calendar days do not occur in the v1 target market. The engine still routes day arithmetic through `chrono-tz` so a future DST-observing market can be supported without rework.

### Overtime Model (Venezuelan LOTTT)
- **D-09:** Each department stores `ordinary_daily_minutes` (default 480 day / 420 night / 450 mixed per LOTTT Art. 173). Minutes above = `overtime_minutes`. Engine enforces LOTTT Art. 178 caps as anomaly flags (not rejections): `OT_CAP_EXCEEDED_DAILY` (>120 min OT/day), `OT_CAP_EXCEEDED_WEEKLY` (>600 min OT/week), `OT_CAP_EXCEEDED_ANNUAL` (>6000 min OT/year per employee). Minutes are still attributed; operator reviews via Phase 4.
- **D-10:** Engine stores `overtime_minutes` only (no monetary calc). LOTTT Art. 118 multiplier (+50% = 1.5x) applied in Phase 5 report. Keeps engine about time, not money, and isolates compensation policy from calc logic.
- **D-11:** Department carries `shift_type` enum = `day | night | mixed`. `night` uses 420min ordinary threshold (LOTTT Art. 117). Night-shift +30% premium is applied in Phase 5 report from this flag. Engine does not partition each DailyRecord's minutes into day/night buckets in v1.
- **D-12:** Sunday/rest-day work detection lives in the engine: if the anchor date falls on a configured rest day for the employee, `DailyRecord.is_rest_day_worked = 1`. LOTTT Art. 120 surcharge math (%) happens in Phase 5 report. v1 default rest days = Saturday + Sunday (configurable later).

### Leave Management (LEAVE-01..04)
- **D-13:** Leave taxonomy = typed enum: `medical | vacation | unpaid | manual`.
  - `medical` — requires evidence (cert upload), paid treatment differs (IVSS indemnizes externally; in-engine, work_minutes=0, flagged as medical for report).
  - `vacation` — fully paid, counts against vacation balance (balance tracking deferred).
  - `unpaid` — work_minutes=0, no salary treatment.
  - `manual` — catch-all with mandatory justification (permissions, special leave, admin overrides).
  Schema: `leaves (id, employee_id, from_date, to_date, leave_type, justification, evidence_path, created_by, created_at)`.
- **D-14:** Full-day leave only in v1. Partial-day leave (half-day sick, early departure with permission) is handled via Phase 4 timesheet edit, not the leave system. Avoids overlap math with overnight shifts and tolerance windows.
- **D-15:** Approval workflow = immediate on create. Admin-only endpoint. Justification + evidence mandatory (evidence optional for `manual` type at admin discretion). No pending/approval state machine in v1. Audit trigger on `leaves` captures actor + justification for legal forensics.
- **D-16:** Overlay precedence = leave wins. If a date is inside any active leave row for the employee, `work_minutes=0`, `overtime_minutes=0`, `late_minutes=0`. Any `attendance_events` that fired on that day still persist in the event store (Phase 2 append-only invariant) but are flagged on the DailyRecord as `EVENTS_ON_LEAVE_DAY` for operator review.

### Tolerance, Aggregation & Anomaly Surface
- **D-17:** `global_rules.bonus_minutes` extends tolerance. Effective late threshold = `shift_start + late_arrival_tolerance_min + bonus_minutes`. Effective early-departure threshold = `shift_end - early_departure_tolerance_min - bonus_minutes`. Bonus is a grace window added on top of tolerance, matching RULE-02's "grace period" phrasing.
- **D-18:** Anomaly flagging = enum codes, multi-flag per DailyRecord, best-effort calc. Dedicated `daily_record_anomalies (daily_record_id, code, detail, created_at)` table. Enum codes (v1 set): `MISSING_ENTRY`, `MISSING_EXIT`, `UNKNOWN_FACE_IN_WINDOW`, `LUNCH_PUNCH_MISSING`, `OT_CAP_EXCEEDED_DAILY`, `OT_CAP_EXCEEDED_WEEKLY`, `OT_CAP_EXCEEDED_ANNUAL`, `EVENTS_ON_LEAVE_DAY`, `RECOMPUTE_AFTER_EDIT`, `OVERNIGHT_INFERENCE_AMBIGUOUS`. Engine never blocks finalization; supervisor resolves via Phase 4 timesheet edit. Phase 4 dashboard surfaces anomalies as a queue.
- **D-19:** Lunch punch-mode fallback: when `departments.lunch_mode = 'punch'` but no lunch-out/lunch-in pair is found in the shift window, engine deducts `departments.lunch_duration_min` as fallback AND raises `LUNCH_PUNCH_MISSING`. Conservative (doesn't inflate OT) + visible to operator.
- **D-20:** First-entry/last-exit aggregation window = `[shift_start - late_tolerance - bonus, shift_end + early_tolerance + bonus]` in the installation's TZ, anchored on the dept's shift config and overnight flag. Direction-aware: first `direction='entry'` event inside the window = arrival; last `direction='exit'` event inside the window = departure. Events with `is_unknown=1` are excluded from anchoring but raise `UNKNOWN_FACE_IN_WINDOW` if they fall inside the window. Multi-device duplicates within the 30s dedup bucket were already filtered upstream (Phase 2 D-05/D-06); if both survive, the engine picks the earliest-timestamp one as canonical.

### Claude's Discretion
- Rust module layout — likely `calc/` (pure domain, no I/O), `daily_records/` (persistence + REST), `leaves/` (CRUD + audit), following the Phase 1/2 `{mod, models, service, handlers}` pattern.
- Exact `daily_records` column list (beyond the entities captured in decisions) — planner finalizes.
- Debouncing mechanism for event-driven recompute (tokio mpsc vs SQLite job queue vs `tokio::sync::Notify`) — planner decides based on concurrency model.
- Cron scheduler (`tokio-cron-scheduler`, a plain `tokio::time::interval`, or systemd-timer external) — planner decides.
- `chrono-tz` integration details (zone lookup, caching, fallback if env var missing).
- Whether anomaly inserts are app-code (likely, since they trigger during calc, not on row mutation) vs SQLite trigger.
- Exact Rust error taxonomy additions to `AppError` for calc-specific failures.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/REQUIREMENTS.md` — CALC-01..06 + LEAVE-01..04 are Phase 3 scope; all other IDs are out of scope for this phase
- `.planning/PROJECT.md` — constraints (on-prem, audit-everything, Venezuela market implied)
- `.planning/STATE.md` — accumulated decisions. **NOTE:** the Phase 3 blocker row mentions "Mexico DST" — this is obsolete. Target is Venezuela (no DST since 2016). Update STATE.md blocker to reference `America/Caracas` / LOTTT compliance instead.
- `.planning/phases/01-foundation/01-CONTEXT.md` — carry-forward conventions (UUID PKs, UTC epoch INTEGER, version column, audit triggers, error envelope, offset pagination, 3-role RBAC, `/api/v1`)
- `.planning/phases/02-device-integration/02-CONTEXT.md` — upstream invariants for the event store (dedup at DB level, `is_unknown` handling, multi-device duplicates both persist)

### Backend code
- `backend/src/db/migrations/001_initial_schema.sql` — `employees`, `departments`, `global_rules` schemas this phase reads
- `backend/src/db/migrations/004_attendance_events.sql` — event store the engine consumes
- `backend/src/main.rs` — router layout; new `daily_records_routes` + `leaves_routes` nest under `/api/v1`
- `backend/src/errors.rs`, `backend/src/common.rs` — reuse `AppError`, `PaginatedResponse<T>`, `epoch_to_iso()`
- `backend/src/auth/middleware.rs`, `backend/src/auth/rbac.rs` — compose `require_admin` (leave writes), `require_supervisor_or_above` (timesheet edits in Phase 4), `require_auth` (reads)

### Stack reference
- `CLAUDE.md` — locked stack; add `chrono-tz` to the Rust crate list during planning (pairs with existing `chrono` 0.4.42)

### Venezuelan labor law (LOTTT)
- **Art. 117** — Jornada nocturna: 7h/day, 35h/week, +30% premium on night hours (7pm–5am)
- **Art. 118** — Horas extraordinarias: +50% premium on OT hours
- **Art. 120** — Prima dominical: surcharge on Sunday work
- **Art. 173** — Jornada ordinaria: 8h/day, 40h/week (day); 7.5h (mixed); 7h (night)
- **Art. 178** — Límites horas extras: max 2h/day, 10h/week, 100h/year; total workday incl. OT ≤ 10h
- Reference PDF: [LOTTT INCES official copy](https://www.inces.gob.ve/wp-content/uploads/2017/10/lot.pdf)

### External (research phase)
- `chrono-tz` docs — IANA zone handling, `America/Caracas` (UTC-4, no DST)
- LOTTT interpretation sources — cross-reference any LOTTT ambiguity against `accesoalajusticia.org` glossary and TSJ decisions during research

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppState { db: Arc<Database>, config: Arc<Config> }` — extend `Config` with `timezone` field (reads `TZ` env var, default `America/Caracas`).
- `AppError` enum + `IntoResponse` impl — reuse variants; add `CalcError` for domain failures, `LeaveConflict` for overlapping leave ranges.
- `PaginatedResponse<T>` — directly usable for `GET /daily-records`, `GET /leaves`, `GET /anomalies`.
- `epoch_to_iso()` helper — use for all DailyRecord timestamp serialization.
- RBAC middleware — compose leave writes behind `require_admin`, reads behind `require_auth` (3-role read-all from Phase 1 D-09).
- Validator-derive DTO pattern — reuse for `CreateLeaveRequest`, `UpdateLeaveRequest`, recompute trigger requests.
- Phase 2's `attendance_events` schema + dedup invariant — engine trusts (employee_id, device_id, direction, 30s-bucket) uniqueness; no app-level dedup on read.

### Established Patterns
- Module layout `{domain}/{mod.rs, models.rs, service.rs, handlers.rs}` — new modules `calc/`, `daily_records/`, `leaves/` follow this.
- SQLite audit triggers (Phase 1 `002_audit_triggers.sql` + Phase 2 `006_devices_audit_triggers.sql`) — extend to cover `leaves` and `daily_record_overrides`. `daily_records` is engine-written (not user-written) so triggers on it produce noisy audit rows — use app-code audit inserts instead, like `command_audit_log` (Phase 2 D-11). `daily_record_anomalies` is append-only; no trigger needed.
- Version column + optimistic concurrency — applies to `leaves` and `daily_record_overrides`; PATCH requires `version`. `daily_records` is engine-owned (not user-editable), no version column — recompute replaces via `INSERT OR REPLACE` keyed on `(employee_id, anchor_date)`.
- Soft-delete via `status` + `deleted_at` — applies to `leaves` (cancelling leave = soft-delete + audit). `daily_records` is not soft-deleted; recomputes replace in place.
- `/api/v1` router composition in `main.rs` — add `daily_records_routes`, `leaves_routes`, and `anomalies_routes`, merge under existing auth groups.

### Integration Points
- `main.rs` bootstrap — after `init_db`, spawn the recompute worker task (tokio mpsc receiver) and the nightly reconcile cron. Supervisor from Phase 2 already demonstrates the spawn pattern.
- `Config::from_env()` — add `timezone: chrono_tz::Tz` (parsed from `TZ` env var, required).
- Migration runner already picks up new `00X_*.sql` files. Phase 3 migrations: `007_daily_records.sql`, `008_daily_record_anomalies.sql`, `009_daily_record_overrides.sql`, `010_leaves.sql`, `011_phase3_audit_triggers.sql`, plus a `012_shift_type_to_departments.sql` altering `departments` with `shift_type`, `is_overnight_shift`, `ordinary_daily_minutes` columns.
- Phase 2's event insertion path — needs a lightweight hook that fires "affected day recompute" after successful dedup-insert. The hook publishes to the tokio mpsc channel owned by the recompute worker.

</code_context>

<specifics>
## Specific Ideas

- Target jurisdiction is **Venezuela**. The Phase 2 research and STATE.md mention "Mexican timezones / DST" — obsolete. Research phase MUST update fixtures and STATE.md to reference `America/Caracas` and LOTTT.
- LOTTT Art. 178 caps are *legal maxima*, not business-rule minimums. Engine raises anomalies when breached but still attributes the worked minutes — operator decides whether to roll the excess forward (unlikely) or accept it. Supervisor's Phase 4 review queue is the control point.
- Medical leave `salary_treatment` here = a flag only. Venezuelan IVSS (social security) indemnifies medical leave externally. The in-engine rule is: medical days = zero work_minutes, flagged medical for Phase 5 report. No medical pay math here.
- Override layer (D-04) means `daily_records` row is the engine's truth and `daily_record_overrides` is the operator's truth. Phase 5 reports MUST join both; Phase 4 timesheet edit writes to overrides; Phase 2's event-driven recompute only touches `daily_records`. This separation is the key reason recompute and manual edit can coexist safely.
- The engine's first pass can ignore the `shift_type = mixed` 7.5h threshold nuance and approximate as 480 minutes, with a Deferred Ideas note for the precise mixed threshold — low priority unless a v1 client actually runs mixed shifts.
- Venezuela does NOT observe DST. Engine uses `chrono-tz` for future-proofing (Colombia/Ecuador expansion), but the v1 test matrix does not need spring-forward/fall-back fixtures.

</specifics>

<deferred>
## Deferred Ideas

- **Holiday calendar + surcharge (HOL-01..03)** — v2 per REQUIREMENTS.md. Need calendar table, `salary_surcharge_pct` per holiday, engine overlay. Significant UI + engine work.
- **Partial-day leave** — start/end timestamps on leave record, overlap math with shift window. Start as Phase 4 timesheet-edit workflow; revisit when a client requests half-day sick leave.
- **Pending-approval leave workflow** — supervisor requests, admin approves. Add a `status = pending|approved|rejected` state machine + approval endpoint. Defer until a client asks for dual-actor accountability.
- **Per-department timezone** — multi-region / multi-site clients. Out of scope since product is per-site on-prem.
- **Configurable leave types table** — admin defines custom leave types with salary_pct and evidence requirements. Revisit if enum proves too rigid.
- **Collective-bargaining OT multiplier overrides** — some CBAs bump +50% to 100%. Add `overtime_multiplier_override` column on `departments` if a client's CBA requires it.
- **Vacation balance tracking** — accrual + consumption + carryover. Currently `vacation` leave just marks the day paid-without-work; no balance ledger. Build balance system when reporting demands it.
- **Precise mixed-shift threshold (7.5h per LOTTT Art. 173)** — v1 approximates to 480min for `shift_type = mixed`. Revisit once a mixed-shift client exists.
- **Per-minute day/night partition for night premium** — LOTTT Art. 117's +30% is currently applied on the whole shift when `shift_type = night`. A precise client may need per-minute partition for shifts that straddle 7pm/5am. Phase 5 report can refine.

</deferred>

---

*Phase: 03-time-calculation-engine*
*Context gathered: 2026-04-23*
