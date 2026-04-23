# Phase 3: Time Calculation Engine - Research

**Researched:** 2026-04-23
**Domain:** Rust attendance engine — chrono-tz, LOTTT labor law, tokio worker patterns, SQLite schema for materialized records
**Confidence:** HIGH (stack, patterns, law); MEDIUM (proptest integration, tokio-cron-scheduler exact API)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Materialization & Recompute:**
- D-01: DailyRecords persisted in materialized `daily_records` table keyed on `(employee_id, anchor_date)`
- D-02: Recompute trigger = event-driven (debounced mpsc publish after attendance_event insert) + nightly reconcile at 02:00 local
- D-03: Late-arriving events auto-recompute and raise `RECOMPUTE_AFTER_EDIT` anomaly
- D-04: Manual edits live in `daily_record_overrides`; engine never clobbers them; GET endpoint joins at read time

**Overnight Shifts & Timezone:**
- D-05: Anchor-date = shift-start date
- D-06: Overnight opt-in per department via `is_overnight_shift BOOLEAN` — no inference
- D-07: Single TZ per installation via `TZ` env var; canonical value `America/Caracas` (UTC-4)
- D-08: All calendar math through `chrono-tz` even though Venezuela has no DST since 2016

**Overtime (LOTTT):**
- D-09: `ordinary_daily_minutes` per department (default 480/420/450). OT = minutes above. LOTTT Art. 178 caps surfaced as anomaly flags only
- D-10: Engine stores `overtime_minutes` only; monetary multipliers applied in Phase 5
- D-11: `shift_type` enum = `day | night | mixed` on department; night threshold = 420 min per LOTTT Art. 117
- D-12: Engine flags `is_rest_day_worked = 1` on anchor dates that fall on configured rest days; surcharge % in Phase 5

**Leave Management:**
- D-13: Leave types: `medical | vacation | unpaid | manual`
- D-14: Full-day leave only in v1
- D-15: Immediate approval on create; admin-only; mandatory justification
- D-16: Leave wins over events; `work_minutes=0`, `overtime_minutes=0`, `late_minutes=0`; events still persisted; raises `EVENTS_ON_LEAVE_DAY`

**Tolerance & Anomaly:**
- D-17: Effective late threshold = `shift_start + late_arrival_tolerance_min + bonus_minutes`
- D-18: Anomaly codes (v1): `MISSING_ENTRY`, `MISSING_EXIT`, `UNKNOWN_FACE_IN_WINDOW`, `LUNCH_PUNCH_MISSING`, `OT_CAP_EXCEEDED_DAILY`, `OT_CAP_EXCEEDED_WEEKLY`, `OT_CAP_EXCEEDED_ANNUAL`, `EVENTS_ON_LEAVE_DAY`, `RECOMPUTE_AFTER_EDIT`, `OVERNIGHT_INFERENCE_AMBIGUOUS`
- D-19: Lunch punch fallback = deduct `lunch_duration_min` when no pair found + raise `LUNCH_PUNCH_MISSING`
- D-20: Aggregation window = `[shift_start - late_tol - bonus, shift_end + early_tol + bonus]` in local TZ; direction-aware; unknown events excluded from anchoring

### Claude's Discretion
- Rust module layout (likely `calc/`, `daily_records/`, `leaves/`)
- Exact `daily_records` column list beyond entities in decisions
- Debouncing mechanism (tokio mpsc vs SQLite job queue vs Notify)
- Cron scheduler choice (`tokio-cron-scheduler`, `tokio::time::interval`, systemd-timer)
- `chrono-tz` integration details (zone lookup, caching, fallback if env var missing)
- Whether anomaly inserts are app-code or SQLite trigger
- Exact Rust error taxonomy additions for calc-specific failures

### Deferred Ideas (OUT OF SCOPE)
- Holiday calendar + surcharge (HOL-01..03)
- Partial-day leave
- Pending-approval leave workflow
- Per-department timezone
- Configurable leave types table
- Collective-bargaining OT multiplier overrides
- Vacation balance tracking
- Precise mixed-shift 7.5h threshold
- Per-minute day/night partition for night premium
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CALC-01 | First-entry/last-exit rule across all devices within shift window | Aggregation window math (Section: Architecture Patterns §Aggregation Algorithm) |
| CALC-02 | Work minutes with configurable tolerance margins | Tolerance window formula (D-17), chrono-tz DateTime arithmetic |
| CALC-03 | Detect and flag late arrivals and early departures | Same tolerance window; anomaly code set (D-18) |
| CALC-04 | Overtime based on department thresholds | LOTTT Art. 173/178 confirmed; D-09 ordinary_daily_minutes; anomaly cap flags |
| CALC-05 | Lunch deduction per department config (fixed or punch) | D-19 fallback logic; lunch_mode column already in departments |
| CALC-06 | Overnight shift with anchor-date logic | D-05/D-06 anchor-date model; chrono-tz NaiveDate + shift-window computation |
| LEAVE-01 | Admin registers medical leave with date range | D-13/D-14/D-15; leaves schema design |
| LEAVE-02 | Admin registers manual adjustments with justification | Same leaves schema; leave_type = manual; mandatory justification field |
| LEAVE-03 | System excludes approved leave days from calculations | D-16 overlay precedence; query join pattern at compute time |
| LEAVE-04 | Medical leave receives different salary treatment | medical flag on DailyRecord; in-engine = zero work_minutes + type flag; pay math in Phase 5 |
</phase_requirements>

---

## Summary

Phase 3 is a pure-Rust domain computation layer. The engine's core job is: given a slice of `attendance_events` for an employee-day, plus that employee's department config and global rules, emit a `DailyRecord` struct with no database access. Persistence wraps that pure function. This separation makes the engine independently unit-testable without any I/O.

The dominant technical risk is correctness of the aggregation window under overnight shifts. The `chrono-tz` crate provides everything needed — IANA zone lookup, UTC↔local conversion, and NaiveDate arithmetic — and `America/Caracas` is confirmed UTC-4 with no DST since May 1, 2016. Venezuela has never observed DST under the 2016 offset; IANA tzdata correctly records this. The engine still routes all calendar math through `chrono-tz` to keep future DST-capable markets affordable.

The recompute worker follows the exact Supervisor pattern already established in Phase 2: an `mpsc::unbounded_channel` with a long-running `tokio::spawn` task that drains the channel and debounces work keyed by `(employee_id, anchor_date)`. A plain `tokio::time::interval` is recommended for the nightly 02:00 reconcile rather than `tokio-cron-scheduler` — it eliminates a dependency and the single fixed-time schedule maps cleanly to a "next-tick-after-02:00" loop.

**Primary recommendation:** Build the engine as a pure function `fn compute_daily_record(events, dept, rules, tz) -> (DailyRecord, Vec<AnomalyCode>)` with no async, no I/O, and comprehensive unit tests. All async and DB work lives in the persistence wrapper.

**IMPORTANT NOTE — STATE.md obsolete blocker:** STATE.md Phase 3 blocker currently reads "Mexico DST timezone boundaries". This is wrong. Target jurisdiction is Venezuela (`America/Caracas`, UTC-4, no DST since 2016). The blocker should be updated to: "Venezuela LOTTT articles 117/173/178 caps and America/Caracas offset confirmed — no blocking DST concern for v1." The planner MUST update STATE.md as part of planning.

---

## Standard Stack

### Core Additions to Cargo.toml

| Crate | Version | Purpose | Why This One |
|-------|---------|---------|--------------|
| `chrono-tz` | 0.10.4 | IANA timezone lookup for `America/Caracas` and future zones | Pairs with existing `chrono` 0.4; generates constants from IANA tzdata; `America::Caracas` accessible as typed constant |
| `tokio-cron-scheduler` | 0.15.1 | (Optional) Nightly 02:00 reconcile cron | Only needed if cron-expression scheduling is preferred over manual interval loop |

The project's existing `Cargo.toml` already contains:
- `chrono = { version = "0.4", features = ["serde"] }` — covers all time arithmetic [VERIFIED: Cargo.toml]
- `tokio = { version = "1", features = ["full"] }` — mpsc channels, intervals, spawn [VERIFIED: Cargo.toml]
- `libsql = "0.9.30"` — persistence [VERIFIED: Cargo.toml]
- `uuid = { version = "1", features = ["v4", "serde"] }` — record IDs [VERIFIED: Cargo.toml]
- `serde`, `serde_json`, `validator`, `thiserror`, `anyhow`, `tracing` — all reusable [VERIFIED: Cargo.toml]

**No new crate required for the engine itself** beyond `chrono-tz`. The optional `tokio-cron-scheduler` adds ~200KB binary overhead; weigh against a 15-line manual interval loop.

### Version Verification

```bash
# Confirmed via cargo search on 2026-04-23
chrono-tz = "0.10.4"
tokio-cron-scheduler = "0.15.1"
proptest = "1.11.0"       # for property-based tests (dev-dependency)
```

[VERIFIED: cargo search output, 2026-04-23]

### Installation

```toml
# Add to [dependencies] in Cargo.toml
chrono-tz = "0.10.4"

# Optional — nightly reconcile; only if cron expression preferred
tokio-cron-scheduler = "0.15.1"

# Add to [dev-dependencies]
proptest = "1.11.0"
```

---

## Architecture Patterns

### Recommended Module Layout

```
backend/src/
├── calc/
│   ├── mod.rs           # pub use; re-export engine entry point
│   ├── engine.rs        # pure fn compute_daily_record() — NO I/O, NO async
│   ├── aggregation.rs   # first-entry/last-exit window logic
│   ├── overtime.rs      # LOTTT Art. 173/178 cap checks
│   ├── lunch.rs         # fixed-deduction / punch-mode deduction
│   ├── overnight.rs     # anchor-date + window-start/end from shift config
│   └── anomalies.rs     # AnomalyCode enum + builder
├── daily_records/
│   ├── mod.rs
│   ├── models.rs        # DailyRecord, DailyRecordAnomaly, DailyRecordOverride structs
│   ├── service.rs       # async DB read/write, recompute_for_day(), upsert_daily_record()
│   └── handlers.rs      # GET /daily-records, GET /daily-records/:id, GET /anomalies
├── leaves/
│   ├── mod.rs
│   ├── models.rs        # Leave struct, CreateLeaveRequest, LeaveType enum
│   ├── service.rs       # async CRUD, leave_covers_date()
│   └── handlers.rs      # POST/GET/DELETE /leaves
└── recompute/
    ├── mod.rs
    ├── worker.rs        # RecomputeWorker — mpsc receiver, debounce, orchestrates service calls
    └── nightly.rs       # nightly_reconcile_task() — interval loop or cron
```

Following the Phase 1/2 established pattern: `{domain}/{mod.rs, models.rs, service.rs, handlers.rs}`. [VERIFIED: codebase inspection]

### Pattern 1: Pure Engine Function (no I/O)

The engine core takes all inputs as plain data structures. The async service layer fetches inputs and persists outputs.

```rust
// Source: project design pattern established Phase 1/2 + this research recommendation
// calc/engine.rs

use chrono::NaiveDate;
use chrono_tz::Tz;

pub struct EngineInput {
    pub events: Vec<AttendanceEventRow>,
    pub dept: DepartmentConfig,
    pub rules: GlobalRulesRow,
    pub leave: Option<LeaveRow>,   // Some() if date is covered by a leave
    pub anchor_date: NaiveDate,
    pub tz: Tz,
}

pub struct DailyRecordOutput {
    pub work_minutes: i64,
    pub overtime_minutes: i64,
    pub late_minutes: i64,
    pub early_departure_minutes: i64,
    pub is_rest_day_worked: bool,
    pub anomalies: Vec<AnomalyCode>,
}

/// Pure function — zero I/O, zero async. Deterministic given identical inputs.
/// Testable with cargo test, no database required.
pub fn compute_daily_record(input: &EngineInput) -> DailyRecordOutput {
    // 1. Leave overlay wins (D-16)
    if input.leave.is_some() {
        return leave_overlay_result(input);
    }
    // 2. Compute shift window in local TZ
    // 3. Aggregate events
    // 4. Apply tolerance, lunch deduction, overtime caps
    // 5. Return result + anomalies
    todo!()
}
```

### Pattern 2: chrono-tz Timezone Handling

```rust
// Source: docs.rs/chrono-tz/0.10.4 [VERIFIED via WebFetch]
use chrono::{NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;

/// Parse timezone from TZ env var with fallback to America/Caracas
pub fn parse_tz(raw: &str) -> anyhow::Result<Tz> {
    raw.parse::<Tz>()
        .map_err(|_| anyhow::anyhow!("Unknown IANA timezone: {}", raw))
}

/// Convert UTC epoch seconds to a NaiveDate in the installation's local TZ.
/// Used to determine which anchor_date an event belongs to.
pub fn epoch_to_local_date(epoch_secs: i64, tz: Tz) -> NaiveDate {
    chrono::DateTime::from_timestamp(epoch_secs, 0)
        .unwrap()
        .with_timezone(&tz)
        .date_naive()
}

/// Build the aggregation window [window_start, window_end] as UTC epoch seconds.
/// For overnight shifts (is_overnight_shift = true), window_end crosses midnight.
pub fn shift_window_utc(
    anchor_date: NaiveDate,
    shift_start: NaiveTime,   // e.g., NaiveTime::from_hms(22, 0, 0)
    shift_end: NaiveTime,     // e.g., NaiveTime::from_hms(06, 0, 0)
    is_overnight: bool,
    late_tol_min: i64,
    early_tol_min: i64,
    bonus_min: i64,
    tz: Tz,
) -> (i64, i64) {
    let tolerance_before = chrono::Duration::minutes(late_tol_min + bonus_min);
    let tolerance_after = chrono::Duration::minutes(early_tol_min + bonus_min);

    let window_start_naive = anchor_date
        .and_time(shift_start)
        - tolerance_before;

    let end_date = if is_overnight {
        anchor_date.succ_opt().unwrap()  // next calendar day
    } else {
        anchor_date
    };
    let window_end_naive = end_date.and_time(shift_end) + tolerance_after;

    // Convert local naive → UTC epoch (LocalResult::Single is guaranteed for America/Caracas)
    let start_utc = tz.from_local_datetime(&window_start_naive)
        .single()
        .unwrap()
        .timestamp();
    let end_utc = tz.from_local_datetime(&window_end_naive)
        .single()
        .unwrap()
        .timestamp();

    (start_utc, end_utc)
}
```

**`from_local_datetime` returns `LocalResult<DateTime<Tz>>`:**
- `Single(dt)` — normal case (always for Venezuela/UTC-4 no DST)
- `Ambiguous(early, late)` — during fall-back (not possible in Venezuela)
- `None` — during spring-forward gap (not possible in Venezuela)

Use `.single().unwrap()` or `.earliest().unwrap()` — safe for `America/Caracas`. For future DST-capable markets the planner should choose `.earliest()` with a `OVERNIGHT_INFERENCE_AMBIGUOUS` anomaly. [ASSUMED: safe unwrap on America/Caracas because no DST since May 2016 per IANA tzdata]

### Pattern 3: Recompute Worker (matches Phase 2 Supervisor pattern)

```rust
// Source: modeled on backend/src/supervisor/mod.rs pattern [VERIFIED: codebase]
// recompute/worker.rs

use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct RecomputeRequest {
    pub employee_id: String,
    pub anchor_date: chrono::NaiveDate,
}

pub struct RecomputeWorker {
    state: AppState,
    shutdown: CancellationToken,
}

impl RecomputeWorker {
    pub async fn run(self, mut rx: mpsc::UnboundedReceiver<RecomputeRequest>) {
        let mut pending: HashSet<(String, chrono::NaiveDate)> = HashSet::new();
        let debounce = tokio::time::Duration::from_millis(500);

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => break,

                Some(req) = rx.recv() => {
                    // Drain all queued requests into pending set (dedup by key)
                    pending.insert((req.employee_id, req.anchor_date));
                    // Drain any additional messages already in the channel
                    while let Ok(extra) = rx.try_recv() {
                        pending.insert((extra.employee_id, extra.anchor_date));
                    }
                    // Short sleep to let burst settle
                    tokio::time::sleep(debounce).await;
                    // Drain again after sleep
                    while let Ok(extra) = rx.try_recv() {
                        pending.insert((extra.employee_id, extra.anchor_date));
                    }
                    // Process all pending
                    for (emp_id, date) in pending.drain() {
                        // call service::recompute_for_day(...)
                        let _ = daily_records_service::recompute_for_day(
                            &self.state, &emp_id, date
                        ).await;
                    }
                }
            }
        }
    }
}
```

**Key decision:** Use `mpsc::UnboundedSender<RecomputeRequest>` stored in `AppState` (same pattern as `lifecycle_tx`). The event processor (Phase 2 insertion path) calls `state.recompute_tx.send(...)` after a successful `INSERT OR IGNORE`. [VERIFIED: codebase state.rs pattern]

### Pattern 4: Nightly Reconcile — Plain interval loop (recommended)

```rust
// recompute/nightly.rs
// Preferred over tokio-cron-scheduler for single-installation on-prem use case.
// No additional crate; handles "02:00 local" by computing next target from chrono-tz.

pub async fn nightly_reconcile_task(state: AppState, tz: Tz, shutdown: CancellationToken) {
    loop {
        let next_run = next_2am(tz);  // chrono-tz NaiveDate + NaiveTime → UTC epoch
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep_until(
                    tokio::time::Instant::now()
                    + std::time::Duration::from_secs(
                        (next_run - chrono::Utc::now().timestamp()) as u64
                    )
                ) => {
                // Reconcile yesterday's records for all employees
                tracing::info!("nightly reconcile starting");
                let _ = daily_records_service::reconcile_prior_day(&state, tz).await;
            }
        }
    }
}
```

### Pattern 5: Override Read-Path (D-04)

```sql
-- GET /daily-records/:id — applies overrides at read time
-- Source: D-04 decision; LEFT JOIN is correct (override is optional)
SELECT
    dr.*,
    dro.override_work_minutes,
    dro.override_entry_at,
    dro.override_exit_at,
    dro.justification,
    dro.evidence_path,
    dro.overridden_by,
    dro.overridden_at
FROM daily_records dr
LEFT JOIN daily_record_overrides dro
    ON dro.daily_record_id = dr.id
    AND dro.deleted_at IS NULL
WHERE dr.id = ?1
```

Merge in application code: if override row exists, replace `work_minutes`, `entry_at`, `exit_at` with override values. Original engine values remain in `daily_records` for recompute safety.

### Anti-Patterns to Avoid

- **Inferring overnight from `shift_end < shift_start`:** D-06 mandates explicit flag. Edge case: `shift_end = 23:30 → shift_start = 23:00` would false-positive.
- **Using `DateTime::naive_local()`:** Without explicit TZ, produces wrong results for UTC-stored epochs. Always go `UTC epoch → with_timezone(&tz) → date_naive()`.
- **Storing `LocalResult::Ambiguous` without anomaly flag:** Future DST-capable market requirement. Build the unwrap behind a helper that emits `OVERNIGHT_INFERENCE_AMBIGUOUS` if not `Single`.
- **Global `Mutex<Tz>` for timezone caching:** `chrono_tz::Tz` is a Copy enum — no caching needed. Parse once in `Config::from_env()`, store as `config.timezone: chrono_tz::Tz`.
- **SQLite trigger on `daily_records`:** Engine-owned rows recompute frequently; triggers produce noisy audit rows. Use app-code audit inserts for manual leave/override mutations only (per project pattern established in Phase 2 D-11).
- **`INSERT OR REPLACE` with a new UUID:** Breaks FK references from `daily_record_anomalies`. Use `INSERT INTO ... ON CONFLICT(employee_id, anchor_date) DO UPDATE SET ...` to preserve the existing row `id`. [ASSUMED: SQLite upsert syntax; verify against libsql dialect]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| IANA timezone conversion | Manual UTC-4 offset arithmetic | `chrono-tz` 0.10.4 | Future DST zones; `LocalResult` handles edge cases; typed Tz enum vs string parsing in hot path |
| Cron scheduling | Custom clock-watcher loop | `tokio::time::sleep_until` + `chrono-tz` next-tick calc | Simpler than a cron library for a single fixed daily time |
| Debouncing event bursts | Custom `HashMap` + sleep | `HashSet` drain pattern (Pattern 3 above) + `try_recv` burst drain | Already in tokio stdlib; no additional crate |
| Optimistic concurrency on leaves | Manual SELECT-then-UPDATE | Version column + `WHERE version = ?` update (Phase 1 D-04 pattern) | Already established; prevents lost-update on concurrent leave edits |
| Anomaly persistence | Custom trigger or event bus | Append-only app-code insert during `compute_daily_record` | Anomalies are generated by the engine, not row mutations; trigger won't fire at compute time |

**Key insight:** The domain is pure arithmetic. Everything complex in this phase lives in correct timezone-aware window math and LOTTT rule implementation — not in infrastructure choices.

---

## Venezuelan LOTTT Legal Interpretation

**Authority:** LOTTT (Ley Orgánica del Trabajo, los Trabajadores y las Trabajadoras, 2012 decree with 2022 amendments). Official PDF: [INCES LOTTT PDF](https://www.inces.gob.ve/wp-content/uploads/2017/10/lot.pdf). Cross-reference: [Jibble Venezuela Labor Law 2024](https://www.jibble.io/es/legislacion-laboral/venezuela). [CITED: jibble.io/es/legislacion-laboral/venezuela]

### Art. 173 — Jornada Ordinaria (Ordinary Workday)

| Shift Type | Daily Hours | Weekly Hours | Minutes/Day (engine) |
|------------|-------------|--------------|----------------------|
| Day (diurna) | 8h | 40h | 480 |
| Night (nocturna) | 7h | 35h | 420 |
| Mixed (mixta) | 7.5h | 37.5h | 450 (v1 approx: 480, see Deferred) |

Day shift hours = 05:00–19:00. Night shift = 19:00–05:00. [CITED: jibble.io]

### Art. 117 — Jornada Nocturna

Night shift: 7 hours daily / 35 hours weekly. +30% premium on night hours. Night period defined as 19:00–05:00. [CITED: jibble.io]

**Engine implication:** `shift_type = 'night'` → `ordinary_daily_minutes = 420`. The +30% premium is NOT computed by the engine (D-11) — it is a Phase 5 report multiplication factor.

### Art. 118 — Horas Extraordinarias (Overtime Premium)

+50% (1.5×) surcharge over ordinary salary for each overtime hour. [CITED: jibble.io]

**Engine implication:** Engine stores `overtime_minutes` only. Art. 118 multiplier applied in Phase 5.

### Art. 120 — Prima Dominical (Sunday/Rest-Day Premium)

Work on rest days entitles the worker to an additional surcharge. [CITED: accesoalajusticia.org — verified via WebSearch]

**Engine implication:** Engine sets `is_rest_day_worked = 1` on the `DailyRecord`. Surcharge % computed in Phase 5. Default rest days = Saturday + Sunday (v1 hardcoded; configurable later per deferred ideas).

### Art. 178 — Límites de Horas Extraordinarias (OT Caps)

| Limit | Cap | Engine Anomaly Code |
|-------|-----|---------------------|
| Daily total work incl. OT | ≤ 10 hours (600 min) | `OT_CAP_EXCEEDED_DAILY` |
| Weekly OT hours | ≤ 10 hours (600 min OT/week) | `OT_CAP_EXCEEDED_WEEKLY` |
| Annual OT hours | ≤ 100 hours (6,000 min OT/year) | `OT_CAP_EXCEEDED_ANNUAL` |

[CITED: jibble.io/es/legislacion-laboral/venezuela]

**Important nuance:** The daily cap is on total worked time including OT (ordinary + OT ≤ 600 min), not on OT alone. The weekly cap is specifically on OT hours (≤ 10h OT/week). The annual cap is 100h OT/year = 6,000 min. [CITED: WebSearch confirmed via multiple Venezuelan labor law sources]

**Engine implication per D-09:** All three anomaly codes are raised as flags. Engine still attributes all worked minutes. Operator reviews via Phase 4 queue.

### LOTTT Summary: What Engine Computes vs What Phase 5 Computes

| Concept | Engine (Phase 3) | Phase 5 Report |
|---------|-----------------|----------------|
| Overtime minutes | `overtime_minutes` stored | Art. 118 × 1.5 multiplier |
| Night shift flag | `shift_type = night` on dept | Art. 117 +30% premium |
| Rest-day flag | `is_rest_day_worked` on record | Art. 120 surcharge % |
| OT cap anomalies | Flags raised | Operator action |
| Medical leave flag | `leave_type = medical` | IVSS treatment note |

---

## First-Entry / Last-Exit Aggregation Algorithm

**Inputs:** slice of `attendance_events`, window `[w_start, w_end]` as UTC epoch seconds, `direction` field on each event.

**Algorithm:**

```
1. Filter events where captured_at IN [w_start, w_end]
2. Partition into: entry_events (direction='entry'), exit_events (direction='exit')
3. Exclude events where is_unknown=1 from anchoring; if any exist → emit UNKNOWN_FACE_IN_WINDOW
4. Sort entry_events by captured_at ascending → canonical_entry = first element (or None)
5. Sort exit_events by captured_at descending → canonical_exit = first element (last in time) (or None)
6. If canonical_entry is None → emit MISSING_ENTRY; work_minutes = 0
7. If canonical_exit is None → emit MISSING_EXIT; work_minutes = 0
8. If both present:
   a. raw_minutes = (canonical_exit.captured_at - canonical_entry.captured_at) / 60
   b. Apply lunch deduction (see below)
   c. Compute late_minutes = max(0, canonical_entry.captured_at - nominal_shift_start) / 60
      where nominal_shift_start = shift_start on anchor_date in UTC (no tolerance)
   d. Compute early_dep_minutes = max(0, nominal_shift_end - canonical_exit.captured_at) / 60
   e. work_minutes = raw_minutes - lunch_deduction
   f. overtime_minutes = max(0, work_minutes - ordinary_daily_minutes)
```

**Multi-device duplicate handling (D-20):** Phase 2's dedup bucket is `(employee_id, device_id, direction, 30s-bucket)`. Two devices may each persist one event for the same employee within the same 30s window. Engine selects the earliest entry timestamp across all devices and latest exit timestamp across all devices — this is the correct first-entry/last-exit behavior for a multi-gate scenario.

**Lunch deduction logic:**

```
if dept.lunch_mode == 'fixed':
    deduct dept.lunch_duration_min
elif dept.lunch_mode == 'punch':
    find lunch_exit = first 'exit' event AFTER canonical_entry with direction='exit'
          that is NOT canonical_exit (i.e., a mid-shift exit)
    find lunch_entry = first 'entry' event AFTER lunch_exit
    if both found:
        deduct (lunch_entry.captured_at - lunch_exit.captured_at) / 60
    else:
        deduct dept.lunch_duration_min (fallback per D-19)
        emit LUNCH_PUNCH_MISSING
```

---

## Overnight Anchor-Date Model

**Rule (D-05):** Anchor date = date of `shift_start`.

**Window construction:**

```
anchor_date = Monday 2026-04-20
shift_start_time = 22:00
shift_end_time = 06:00
is_overnight_shift = true

window_start (local) = 2026-04-20 22:00 - tolerance
window_end (local)   = 2026-04-21 06:00 + tolerance
```

Both are converted to UTC epoch seconds via `chrono-tz`. Events are queried with `WHERE captured_at BETWEEN ? AND ?`. The engine attributes all results to anchor_date = 2026-04-20.

**Edge case: shift ending exactly at midnight:**

```
shift_end_time = 00:00, is_overnight_shift = true
end_date = anchor_date + 1 day
window_end = 2026-04-21 00:00 + tolerance
```

This is consistent with the anchor-date model. `anchor_date.succ_opt()` handles the day rollover. [ASSUMED: NaiveDate::succ_opt is correct API for adding 1 day in chrono 0.4]

**DST safety (Venezuela):** `from_local_datetime(...).single()` always returns `Single` for `America/Caracas` because no DST. For future-proofing: use `.earliest()` with an `OVERNIGHT_INFERENCE_AMBIGUOUS` anomaly flag when `LocalResult::Ambiguous` occurs.

---

## Schema Design

### Migration Ordering

```
007_daily_records.sql
008_daily_record_anomalies.sql
009_daily_record_overrides.sql
010_leaves.sql
011_phase3_audit_triggers.sql
012_shift_type_to_departments.sql
```

`012` must be last because `daily_records` stores `shift_type` denormalized from departments at compute time (for Phase 5 reporting without a department JOIN).

### daily_records Table

```sql
CREATE TABLE IF NOT EXISTS daily_records (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    department_id TEXT NOT NULL REFERENCES departments(id),
    anchor_date TEXT NOT NULL,           -- ISO date 'YYYY-MM-DD'
    shift_type TEXT NOT NULL CHECK(shift_type IN ('day', 'night', 'mixed')),
    work_minutes INTEGER NOT NULL DEFAULT 0,
    overtime_minutes INTEGER NOT NULL DEFAULT 0,
    late_minutes INTEGER NOT NULL DEFAULT 0,
    early_departure_minutes INTEGER NOT NULL DEFAULT 0,
    is_rest_day_worked INTEGER NOT NULL DEFAULT 0 CHECK(is_rest_day_worked IN (0,1)),
    entry_at INTEGER,                    -- UTC epoch, canonical first entry
    exit_at INTEGER,                     -- UTC epoch, canonical last exit
    leave_id TEXT REFERENCES leaves(id), -- non-NULL if covered by leave (D-16)
    computed_at INTEGER NOT NULL,        -- UTC epoch when engine ran
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
    -- NOTE: no version column — engine uses upsert, not optimistic concurrency (D-04)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_records_employee_date
    ON daily_records(employee_id, anchor_date);
CREATE INDEX IF NOT EXISTS idx_daily_records_anchor ON daily_records(anchor_date);
CREATE INDEX IF NOT EXISTS idx_daily_records_employee ON daily_records(employee_id);
```

**Upsert pattern (NOT INSERT OR REPLACE — preserves id):**

```sql
INSERT INTO daily_records (id, employee_id, ..., computed_at, created_at, updated_at)
VALUES (?1, ?2, ..., ?N, ?N, ?N)
ON CONFLICT(employee_id, anchor_date) DO UPDATE SET
    work_minutes = excluded.work_minutes,
    overtime_minutes = excluded.overtime_minutes,
    late_minutes = excluded.late_minutes,
    early_departure_minutes = excluded.early_departure_minutes,
    is_rest_day_worked = excluded.is_rest_day_worked,
    entry_at = excluded.entry_at,
    exit_at = excluded.exit_at,
    leave_id = excluded.leave_id,
    shift_type = excluded.shift_type,
    computed_at = excluded.computed_at,
    updated_at = excluded.updated_at;
```

[ASSUMED: `ON CONFLICT ... DO UPDATE` (upsert) is supported by libsql/SQLite 3.24+; Turso uses SQLite 3.44+ so this is safe]

### daily_record_anomalies Table

```sql
CREATE TABLE IF NOT EXISTS daily_record_anomalies (
    id TEXT PRIMARY KEY,
    daily_record_id TEXT NOT NULL REFERENCES daily_records(id) ON DELETE CASCADE,
    code TEXT NOT NULL,                  -- AnomalyCode enum as TEXT
    detail TEXT,                         -- JSON detail, e.g. {"excess_minutes": 30}
    created_at INTEGER NOT NULL
    -- No version, no update — append-only per D-18
);

CREATE INDEX IF NOT EXISTS idx_anomalies_record
    ON daily_record_anomalies(daily_record_id);
CREATE INDEX IF NOT EXISTS idx_anomalies_code
    ON daily_record_anomalies(code);
```

**Anomaly insert strategy:** Delete old anomalies for the record (by `daily_record_id`) then bulk-insert the new set in the same transaction as the upsert. Avoids stale anomaly accumulation across recomputes.

### daily_record_overrides Table

```sql
CREATE TABLE IF NOT EXISTS daily_record_overrides (
    id TEXT PRIMARY KEY,
    daily_record_id TEXT NOT NULL REFERENCES daily_records(id),
    override_work_minutes INTEGER,
    override_entry_at INTEGER,
    override_exit_at INTEGER,
    justification TEXT NOT NULL,
    evidence_path TEXT,
    overridden_by TEXT NOT NULL REFERENCES users(id),
    overridden_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'revoked')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_overrides_record
    ON daily_record_overrides(daily_record_id)
    WHERE deleted_at IS NULL;
```

### leaves Table

```sql
CREATE TABLE IF NOT EXISTS leaves (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    from_date TEXT NOT NULL,             -- 'YYYY-MM-DD' inclusive
    to_date TEXT NOT NULL,               -- 'YYYY-MM-DD' inclusive
    leave_type TEXT NOT NULL CHECK(leave_type IN ('medical', 'vacation', 'unpaid', 'manual')),
    justification TEXT NOT NULL,
    evidence_path TEXT,
    created_by TEXT NOT NULL REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'cancelled')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_leaves_employee ON leaves(employee_id);
CREATE INDEX IF NOT EXISTS idx_leaves_dates ON leaves(from_date, to_date);
```

**Leave overlap query (used at compute time, D-16):**

```sql
SELECT * FROM leaves
WHERE employee_id = ?1
  AND from_date <= ?2   -- ?2 = anchor_date as 'YYYY-MM-DD'
  AND to_date >= ?2
  AND status = 'active'
  AND deleted_at IS NULL
LIMIT 1;
```

### 012_shift_type_to_departments.sql

```sql
-- Add Phase 3 columns to departments table
ALTER TABLE departments ADD COLUMN shift_type TEXT NOT NULL DEFAULT 'day'
    CHECK(shift_type IN ('day', 'night', 'mixed'));
ALTER TABLE departments ADD COLUMN is_overnight_shift INTEGER NOT NULL DEFAULT 0
    CHECK(is_overnight_shift IN (0,1));
ALTER TABLE departments ADD COLUMN ordinary_daily_minutes INTEGER NOT NULL DEFAULT 480;
```

`ALTER TABLE ADD COLUMN` is safe in SQLite/libSQL for nullable or DEFAULT-bearing columns. Turso replication propagates DDL. [ASSUMED: Turso libsql handles ALTER TABLE ADD COLUMN in sync; standard SQLite behavior, Turso docs confirm DDL replication]

---

## AppState and Config Extension

### Config Extension

```rust
// config.rs — add to Config struct and from_env()
pub timezone: chrono_tz::Tz,   // parsed from TZ env var; default America/Caracas

// In from_env():
let tz_str = std::env::var("TZ").unwrap_or_else(|_| "America/Caracas".to_string());
let timezone = tz_str.parse::<chrono_tz::Tz>()
    .map_err(|_| anyhow::anyhow!("Unknown IANA timezone in TZ env var: {}", tz_str))?;
```

### AppState Extension

```rust
// state.rs — add recompute channel sender
pub recompute_tx: Option<mpsc::UnboundedSender<RecomputeRequest>>,
```

`Option<...>` follows the same `lifecycle_tx` pattern — tests that build the router without a recompute worker get `None` and the handler gracefully skips publishing.

---

## Event-Driven Recompute Integration Point

Phase 2's event insertion path is in `events/service.rs`. After a successful `INSERT OR IGNORE` (non-duplicate event persists), the service needs to publish a `RecomputeRequest`:

```rust
// events/service.rs — after successful event insert
if rows_affected > 0 {
    // Event was not a duplicate; trigger recompute for affected employee-day
    let anchor = epoch_to_local_date(event.captured_at, tz);
    if let Some(tx) = &state.recompute_tx {
        let _ = tx.send(RecomputeRequest {
            employee_id: event.employee_id.clone(),
            anchor_date: anchor,
        });
    }
}
```

This is a fire-and-forget publish — the `let _ =` discards send errors (channel closed = worker already shut down, which only happens on graceful shutdown when no new events are expected anyway).

---

## Common Pitfalls

### Pitfall 1: INSERT OR REPLACE Breaks Foreign Keys
**What goes wrong:** `INSERT OR REPLACE` on `daily_records` generates a new row ID, silently cascading delete and re-insert. `daily_record_anomalies` rows referencing the old `id` are deleted by `ON DELETE CASCADE`, then the anomalies are not re-created.
**Why it happens:** `INSERT OR REPLACE` is effectively `DELETE + INSERT` under the hood.
**How to avoid:** Use `INSERT INTO ... ON CONFLICT(employee_id, anchor_date) DO UPDATE SET ...` to preserve the existing row id. Delete old anomalies by `daily_record_id` explicitly before the upsert, then re-insert new ones.
**Warning signs:** Anomaly table stays empty after recompute; recompute silently discards prior anomalies.

### Pitfall 2: UTC Epoch → Local Date at Midnight Boundary
**What goes wrong:** An event at 23:50 UTC might be 19:50 local (UTC-4) — assigning it to the UTC date (next day) instead of the local date (today) corrupts the anchor-date assignment.
**Why it happens:** `DateTime<Utc>.naive_utc().date()` returns the UTC date, not the local date.
**How to avoid:** Always use `epoch_to_local_date(epoch_secs, config.timezone)` — convert to local TZ first, then call `.date_naive()`.
**Warning signs:** Overnight shift workers get their events split across two anchor dates.

### Pitfall 3: Anomaly Accumulation Across Recomputes
**What goes wrong:** Recomputing a day adds more anomalies without clearing the old ones. The anomaly table grows with stale entries from each recompute.
**Why it happens:** Append-only table + naive insert without delete-first.
**How to avoid:** Wrap the upsert + anomaly insert in a single transaction: `DELETE FROM daily_record_anomalies WHERE daily_record_id = ?` then bulk insert the new set.
**Warning signs:** The same anomaly code appears multiple times for the same `(daily_record_id, code)`.

### Pitfall 4: LocalResult Ambiguous Panic on Future DST Markets
**What goes wrong:** `.single().unwrap()` panics in spring-forward/fall-back scenarios when `from_local_datetime` returns `LocalResult::Ambiguous`.
**Why it happens:** Venezuela has no DST so this never triggers in v1 — but the code ships to a future Colombian client with DST.
**How to avoid:** Use `.earliest().unwrap_or_else(|| /* raise OVERNIGHT_INFERENCE_AMBIGUOUS */ )` everywhere. For `America/Caracas` this is a no-op correctness improvement.
**Warning signs:** Test passes in Venezuela market, panics in Colombia.

### Pitfall 5: Weekly/Annual OT Cap Requires Cross-Day Lookback
**What goes wrong:** Engine computes one day at a time. Weekly OT cap (Art. 178: ≤10h OT/week) requires knowing the sum of `overtime_minutes` for the current ISO week from other already-computed days.
**Why it happens:** The pure engine function only sees one day's events. Weekly and annual aggregates must be fetched from the DB.
**How to avoid:** Pass pre-fetched `weekly_ot_minutes_so_far` and `annual_ot_minutes_so_far` as inputs to the engine function. The service layer fetches these with a SUM query over `daily_records` before calling the engine.
**Warning signs:** `OT_CAP_EXCEEDED_WEEKLY` never fires, or fires on every OT day regardless of weekly sum.

### Pitfall 6: Leave Overlap on Edge Dates (from_date = to_date boundary)
**What goes wrong:** A leave with `from_date = to_date = '2026-04-20'` should cover exactly one day. An off-by-one in the overlap query returns no rows.
**Why it happens:** `WHERE from_date <= anchor AND to_date >= anchor` is correct; `<` or `>` breaks single-day leaves.
**How to avoid:** Use `from_date <= ?anchor AND to_date >= ?anchor` (inclusive on both ends).
**Warning signs:** Single-day leaves do not suppress attendance calculation.

### Pitfall 7: Recompute on Unknown-Face Events
**What goes wrong:** When an event with `employee_id = NULL` (is_unknown=1) triggers a recompute publish, `RecomputeRequest.employee_id` is NULL/empty and the service queries with a NULL employee_id, potentially touching all employees.
**Why it happens:** The event insertion path publishes recompute without checking `is_unknown`.
**How to avoid:** In the event insertion path, only publish `RecomputeRequest` when `employee_id IS NOT NULL`.
**Warning signs:** Recompute worker produces an error or no-op loop for every unknown-face event.

---

## Code Examples

### chrono-tz: America/Caracas constant lookup

```rust
// Source: docs.rs/chrono-tz/0.10.4 [VERIFIED via WebFetch]
use chrono_tz::Tz;
use chrono_tz::America::Caracas;  // typed constant — zero runtime parsing

// Or parse from env var string:
let tz: Tz = "America/Caracas".parse().unwrap();  // Tz::America__Caracas variant

// Convert UTC epoch to local date:
let local_dt = chrono::DateTime::from_timestamp(epoch, 0)
    .unwrap()
    .with_timezone(&Caracas);
let local_date = local_dt.date_naive();  // NaiveDate in Caracas local time
```

### Anomaly Code enum

```rust
// calc/anomalies.rs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnomalyCode {
    MissingEntry,
    MissingExit,
    UnknownFaceInWindow,
    LunchPunchMissing,
    OtCapExceededDaily,
    OtCapExceededWeekly,
    OtCapExceededAnnual,
    EventsOnLeaveDay,
    RecomputeAfterEdit,
    OvernightInferenceAmbiguous,
}

impl AnomalyCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingEntry => "MISSING_ENTRY",
            // ... etc
        }
    }
}
```

### Weekly OT aggregate query

```sql
-- Used by service layer before calling engine, to feed weekly_ot_so_far
SELECT COALESCE(SUM(overtime_minutes), 0) as weekly_ot
FROM daily_records
WHERE employee_id = ?1
  AND anchor_date >= ?2   -- ISO week Monday
  AND anchor_date < ?3    -- anchor_date (exclusive, not yet computed today)
  AND anchor_date != ?4;  -- exclude any existing row for today being recomputed
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `chrono` FixedOffset arithmetic | `chrono-tz` IANA zone lookup | N/A | Future DST-capable markets work without code change |
| Trigger-based audit on daily_records | App-code audit inserts | Phase 2 pattern | Avoids noisy audit rows on frequent engine recomputes |
| `INSERT OR REPLACE` for upsert | `ON CONFLICT ... DO UPDATE` | SQLite 3.24 (2018) | Preserves row id; FK integrity maintained |
| `tokio-cron-scheduler` | Plain `tokio::time::sleep_until` for single daily cron | Design choice | Eliminates dependency for a simple single-schedule case |

**Deprecated/outdated:**
- `DateTime::naive_local()` — deprecated pattern; use `with_timezone(&tz).date_naive()` explicitly
- Storing `NaiveDateTime` in DB — project uses UTC epoch integers per Phase 1 D-05; no change needed

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `.single().unwrap()` on `from_local_datetime` is safe for `America/Caracas` because Venezuela has had no DST since May 2016 | Architecture Patterns §2, Pitfall 4 | Panic in future DST market — mitigated by using `.earliest()` instead |
| A2 | SQLite `ON CONFLICT ... DO UPDATE` (upsert) works in libsql 0.9.30 / Turso | Schema Design | Must fall back to SELECT + conditional INSERT/UPDATE if unsupported |
| A3 | `NaiveDate::succ_opt()` is the correct chrono 0.4 API for adding 1 day to a NaiveDate | Overnight Model | Compile error or runtime panic — check chrono 0.4 docs for exact method name |
| A4 | Turso replication correctly propagates `ALTER TABLE ADD COLUMN` DDL in embedded replica mode | Schema Migration 012 | Migration may fail on first sync after deployment |
| A5 | `chrono_tz::America::Caracas` constant exists in chrono-tz 0.10.4 | Standard Stack | Compilation error — confirmed via cargo search output and IANA tzdata inclusion |

---

## Open Questions

1. **Lunch punch-mode with multiple mid-shift exits**
   - What we know: Engine takes first exit after canonical_entry as lunch_exit, first entry after that as lunch_entry.
   - What's unclear: What if an employee exits twice mid-shift (e.g., two coffee breaks)? Does the engine sum both or only the first?
   - Recommendation: Sum the longest mid-shift exit/re-entry pair (most conservative), or deduct only the first pair. Default to first-pair for simplicity; document as a known limitation.

2. **OT_CAP_EXCEEDED_ANNUAL cross-year boundary at recompute**
   - What we know: Annual cap = 100h = 6,000 min per Art. 178. Recompute touches one day.
   - What's unclear: What calendar defines "year" — ISO year, fiscal year, or rolling 12 months?
   - Recommendation: Use calendar year (Jan 1 – Dec 31). Rolling 12 months adds complexity with no clear legal mandate.

3. **Recompute for employees without a department config**
   - What we know: `department_id` is required on every employee (Phase 1 EMP-04).
   - What's unclear: If a department is deactivated after records exist, does recompute fail?
   - Recommendation: Log error + skip recompute for that employee-day; raise internal alert. Do not panic.

---

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified — Phase 3 is backend Rust code only; no new external services, databases, or CLI tools beyond what Phases 1-2 already require).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest (already referenced in CLAUDE.md dev tools) + standard `#[tokio::test]` |
| Config file | `backend/Cargo.toml` — `[dev-dependencies]` section |
| Quick run command | `cargo test -p cronometrix-api calc` |
| Full suite command | `cargo nextest run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CALC-01 | First-entry/last-exit across devices within shift window | Unit | `cargo test -p cronometrix-api calc::aggregation` | ❌ Wave 0 |
| CALC-02 | Work minutes with configurable tolerance margins | Unit | `cargo test -p cronometrix-api calc::engine::tolerance` | ❌ Wave 0 |
| CALC-03 | Late arrival and early departure flags | Unit | `cargo test -p cronometrix-api calc::engine::flags` | ❌ Wave 0 |
| CALC-04 | Overtime calculation + LOTTT Art. 178 cap anomalies | Unit + Property | `cargo test -p cronometrix-api calc::overtime` | ❌ Wave 0 |
| CALC-05 | Lunch deduction (fixed + punch fallback) | Unit | `cargo test -p cronometrix-api calc::lunch` | ❌ Wave 0 |
| CALC-06 | Overnight shift anchor-date model | Unit + Property | `cargo test -p cronometrix-api calc::overnight` | ❌ Wave 0 |
| LEAVE-01 | Admin registers medical leave with date range | Integration (API) | `cargo test -p cronometrix-api leave_tests::create_medical_leave` | ❌ Wave 0 |
| LEAVE-02 | Admin registers manual adjustments with justification | Integration (API) | `cargo test -p cronometrix-api leave_tests::create_manual_leave` | ❌ Wave 0 |
| LEAVE-03 | Leave overlay excludes day from calculation | Integration (engine) | `cargo test -p cronometrix-api leave_tests::leave_overlay_suppresses_work` | ❌ Wave 0 |
| LEAVE-04 | Medical leave salary flag set correctly | Unit | `cargo test -p cronometrix-api calc::engine::medical_leave_flag` | ❌ Wave 0 |

### Property-Based Tests (proptest)

These are especially valuable for the time-arithmetic domain:

| Test | Strategy | What It Proves |
|------|----------|----------------|
| Overnight anchor-date | Random `(anchor_date, shift_start, offset_hours)` | `epoch_to_local_date(window_start, tz).date == anchor_date` always |
| OT cap monotonicity | Random `(work_minutes, ordinary_daily_minutes)` | `overtime_minutes >= 0` and `overtime_minutes == max(0, work - ordinary)` |
| Tolerance window symmetry | Random `(late_tol, bonus)` | Window never shrinks below shift times |
| Leave overlap | Random `(from_date, to_date, anchor_date)` | Overlap query returns row iff `from <= anchor <= to` |

```toml
# dev-dependencies addition
proptest = "1.11.0"
```

### Test Fixture Strategy

Following the existing `tests/common/mod.rs` pattern [VERIFIED: codebase]:

```
tests/
├── common/
│   └── mod.rs        # add: create_test_department_with_shift(), create_test_leave()
├── calc_tests.rs     # pure engine unit tests + proptest (no DB)
├── daily_record_tests.rs  # integration: recompute_for_day() + DB
├── leave_tests.rs    # integration: CRUD API + leave_overlay behavior
└── fixtures/
    ├── shift_scenarios.json   # canned shift configs for table-driven tests
    └── lottt_scenarios.json   # LOTTT edge-case inputs + expected outputs
```

**Table-driven LOTTT scenario fixtures:**

Each JSON entry captures: `{description, shift_type, events, expected_work_minutes, expected_overtime_minutes, expected_anomalies}`. This gives the team a living spec they can audit against LOTTT interpretation.

### Sampling Rate

- **Per task commit:** `cargo test -p cronometrix-api calc` (pure unit tests, < 5s)
- **Per wave merge:** `cargo nextest run` (full suite including integration)
- **Phase gate:** Full suite green + all LOTTT scenario fixtures pass before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/calc_tests.rs` — covers CALC-01..06 unit tests + proptest cases
- [ ] `tests/daily_record_tests.rs` — covers recompute_for_day() integration, upsert idempotency, anomaly clear-and-reinstate
- [ ] `tests/leave_tests.rs` — covers LEAVE-01..04 API + overlay semantics
- [ ] `tests/common/mod.rs` additions — `create_test_department_with_shift()`, `create_test_leave()`
- [ ] `tests/fixtures/lottt_scenarios.json` — LOTTT scenario fixture table
- [ ] `proptest = "1.11.0"` added to `[dev-dependencies]`

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No (engine is internal; no new auth surface) | — |
| V3 Session Management | No | — |
| V4 Access Control | Yes | `require_admin` on leave write endpoints; `require_auth` on read endpoints |
| V5 Input Validation | Yes | `validator` derive on `CreateLeaveRequest` (date format, leave_type enum, justification non-empty) |
| V6 Cryptography | No (no new secrets introduced) | — |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Leave overlap injection (admin creates overlapping leaves to zero out an employee's records) | Tampering | Audit trigger on `leaves` captures actor + timestamp; admin-only write endpoint |
| Recompute race: concurrent recompute + override write | Tampering | `daily_record_overrides` uses version column; override write fails if daily_record was deleted by concurrent recompute (FK constraint) |
| Forged `anchor_date` in recompute API (if exposed) | Tampering | Recompute trigger is internal only (mpsc channel); no public endpoint exposes raw anchor_date manipulation |
| SQL injection via `employee_id` in leave overlap query | Tampering | Parameterized queries via libsql positional params; never string-interpolated |

---

## Sources

### Primary (HIGH confidence)
- [docs.rs/chrono-tz/0.10.4](https://docs.rs/chrono-tz/0.10.4/chrono_tz/) — API patterns (WebFetch verified)
- [crates.io/crates/chrono-tz](https://crates.io/crates/chrono-tz) — version 0.10.4 (cargo search verified)
- `backend/Cargo.toml` — existing crate versions (direct codebase read)
- `backend/src/supervisor/mod.rs` — Supervisor pattern (direct codebase read)
- `backend/src/db/migrations/001_initial_schema.sql` — existing schema (direct codebase read)
- `backend/src/db/migrations/004_attendance_events.sql` — event store (direct codebase read)
- `backend/tests/common/mod.rs` — test infrastructure pattern (direct codebase read)

### Secondary (MEDIUM confidence)
- [jibble.io/es/legislacion-laboral/venezuela](https://www.jibble.io/es/legislacion-laboral/venezuela) — LOTTT Art. 117/118/120/173/178 interpretation (WebFetch verified)
- [timeanddate.com — Venezuela timezone change 2016](https://www.timeanddate.com/news/time/venezuela-change-timezone.html) — DST abolition confirmed UTC-4 since May 1, 2016 (WebSearch)
- [inces.gob.ve LOTTT PDF](https://www.inces.gob.ve/wp-content/uploads/2017/10/lot.pdf) — official LOTTT text (linked, direct PDF fetch timed out but URL confirmed via multiple searches)
- cargo search `tokio-cron-scheduler = "0.15.1"` — version confirmed

### Tertiary (LOW confidence — marked for validation)
- LOTTT Art. 178 "2 hours/day OT limit" claim in CONTEXT.md: WebSearch returned "total work incl. OT ≤ 10h/day" not "≤ 2h OT/day". The effective OT-per-day limit is implied (10h total - 8h ordinary = 2h OT max for day shift) but the statute states it as a total cap, not an OT-only cap. The engine anomaly `OT_CAP_EXCEEDED_DAILY` should compare `work_minutes + overtime_minutes > 600` (total ≤ 10h), not `overtime_minutes > 120` (OT alone).

---

## Project Constraints (from CLAUDE.md)

- Backend: Rust + Axum 0.8.x — all new code must be Rust
- Database: libSQL 0.9.x + Turso; raw queries, no ORM
- All timestamps: UTC epoch INTEGER stored in DB; ISO 8601 in API
- UUID v4 string PKs on all tables
- Version column on all user-mutable tables (leaves, daily_record_overrides)
- Soft-delete via `status` + `deleted_at`; no hard deletes
- Audit trail: every admin mutation writes an audit entry
- `/api/v1` prefix on all routes
- RBAC: admin for leave writes; auth for reads
- No diesel, no actix-web, no runtime CSS-in-JS

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified via cargo search and Cargo.toml codebase read
- Architecture: HIGH — follows established codebase patterns (Supervisor, AppState, migrations)
- LOTTT legal rules: MEDIUM — confirmed via jibble.io (secondary source) + multiple WebSearch cross-checks; primary PDF inaccessible directly but URL confirmed canonical
- Pitfalls: HIGH for schema/SQLite (verified); MEDIUM for LocalResult edge case (Venezuela-specific, theoretical)

**Research date:** 2026-04-23
**Valid until:** 2026-07-23 (90 days; LOTTT stable law, chrono-tz IANA tzdata updates infrequent)
