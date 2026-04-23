# Phase 3: Time Calculation Engine - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-23
**Phase:** 03-time-calculation-engine
**Areas discussed:** Materialization strategy, Overnight + timezone, Overtime model (Venezuelan LOTTT), Leave semantics, Cross-cutting (bonus/anomaly/lunch/aggregation)

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Materialization strategy | When/how DailyRecords get computed | ✓ |
| Overnight + timezone | Anchor-date rule + TZ scope | ✓ |
| Overtime model | Threshold, multiplier, night, Sunday | ✓ |
| Leave semantics | Types, partial-day, approval, overlay | ✓ |

User selected all four areas.

---

## Materialization Strategy

### Q1: How should DailyRecords be stored?
| Option | Description | Selected |
|--------|-------------|----------|
| Materialized table (Recommended) | Persisted daily_records table, fast reads, recompute logic needed | ✓ |
| Pure view (compute each request) | No storage, always fresh, slower reports | |
| Hybrid: recent live, historical frozen | Current computed live, older materialized + frozen | |

### Q2: What triggers recompute?
| Option | Description | Selected |
|--------|-------------|----------|
| Event-driven + nightly reconcile (Recommended) | Each event debounced enqueue + nightly cron for yesterday | ✓ |
| Nightly batch only | Cron at 02:00 computes yesterday only | |
| On-demand only | Compute on API call, cache result | |

### Q3: Late-arriving events?
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-recompute affected day (Recommended) | Overwrite record, audit captures delta | ✓ |
| Mark stale, admin triggers | Staleness flag, admin clicks refresh | |
| Ignore events older than N days | Hard cutoff on finalized records | |

### Q4: Manual timesheet edits interaction?
| Option | Description | Selected |
|--------|-------------|----------|
| Override layer (Recommended) | Separate daily_record_overrides table | ✓ |
| Mutate DailyRecord directly | Flag is_manually_edited, block recompute | |
| Synthetic-event model | Edit inserts synthetic attendance_event | |

---

## Overnight + Timezone

### Q1: Overnight shift anchor?
| Option | Description | Selected |
|--------|-------------|----------|
| Shift-start date (Recommended) | Shift starting 22:00 Mon belongs to Monday | ✓ |
| Majority-hours date | Day with more hours wins | |
| Configurable per department | Dept picks anchor rule | |

### Q2: Overnight detection?
| Option | Description | Selected |
|--------|-------------|----------|
| Dept flag: is_overnight_shift (Recommended) | Explicit boolean opt-in | ✓ |
| Inferred: shift_end < shift_start | Auto-detect from time columns | |
| Per-event: cluster by gap threshold | Engine inspects actual events | |

### Q3: Timezone scope?
| Option | Description | Selected |
|--------|-------------|----------|
| Single TZ per installation (Recommended) | Install-level `TZ=America/Caracas` | ✓ |
| Per-department TZ | Dept has timezone column | |
| Per-employee TZ | Employee-level override | |

### Q4: DST handling?
| Option | Description | Selected |
|--------|-------------|----------|
| Trust chrono-tz, test edge days (Recommended) | chrono-tz + IANA, fixture tests for DST days | ✓ |
| Store + calc everything in UTC | Ignore DST entirely | |
| Defer to v2 | Document limitation | |

**Notes:** Venezuela does NOT observe DST (abolished 2016). chrono-tz still used for future-proofing (Colombia/Ecuador expansion) but v1 Venezuela fixtures don't need DST edge-day tests.

---

## Overtime Model (Venezuelan LOTTT)

**Jurisdiction correction mid-discussion:** User clarified target = Venezuela, not Mexico. Re-presented overtime options after researching LOTTT Arts. 117, 118, 120, 173, 178.

### Q1: OT threshold + cap?
| Option | Description | Selected |
|--------|-------------|----------|
| Per-dept threshold + LOTTT caps enforced (Recommended) | `ordinary_daily_minutes` column; caps flagged as anomalies | ✓ |
| Fixed 8h/40h no caps | Hardcode, ignore caps | |
| Full LOTTT rules in engine | Refuse to materialize breaches | |

### Q2: OT multiplier?
| Option | Description | Selected |
|--------|-------------|----------|
| Single 1.5x on calc, minutes only (Recommended) | Engine stores minutes, Phase 5 applies multiplier | ✓ |
| Store minutes + multiplier per record | Per-record override capability | |
| Split day vs Sunday/holiday OT | Multiple OT fields | |

### Q3: Night shift + premium?
| Option | Description | Selected |
|--------|-------------|----------|
| Dept flag shift_type day/night/mixed (Recommended) | 420/480/450 min threshold; +30% applied in Phase 5 | ✓ |
| Per-minute night partition | Engine partitions actual clock time | |
| Defer to v2 | Treat all as day shift | |

### Q4: Sunday/rest-day?
| Option | Description | Selected |
|--------|-------------|----------|
| Detect + flag in engine, calc in Phase 5 (Recommended) | `is_rest_day_worked` flag, surcharge in report | ✓ |
| Full surcharge calc in engine | Monetary math inline | |
| Out of Phase 3 scope | Defer | |

---

## Leave Semantics

### Q1: Leave type taxonomy?
| Option | Description | Selected |
|--------|-------------|----------|
| Typed enum: medical/vacation/unpaid/manual (Recommended) | Fixed enum, salary_treatment baked in | ✓ |
| Medical only + generic manual | Two categories, justification freeform | |
| Fully configurable leave_types table | Admin defines types | |

### Q2: Partial-day leave?
| Option | Description | Selected |
|--------|-------------|----------|
| Full-day only in v1 (Recommended) | Whole days; partial via timesheet edit | ✓ |
| Start/end timestamps | Partial-day overlap math | |

### Q3: Approval workflow?
| Option | Description | Selected |
|--------|-------------|----------|
| Immediate effect on create (Recommended) | Admin-only, justification+evidence mandatory | ✓ |
| Pending → approved workflow | Two-actor state machine | |
| Immediate for admin, pending for supervisor | Role-based compromise | |

### Q4: Engine overlay precedence?
| Option | Description | Selected |
|--------|-------------|----------|
| Leave wins: zero work minutes, log ignored events (Recommended) | work=0, events persist + flag EVENTS_ON_LEAVE_DAY | ✓ |
| Events override leave | Events counted, leave flagged erroneous | |
| Configurable per leave type | medical vs manual differ | |

---

## Cross-cutting Threading Questions

### Q1: bonus_minutes + tolerance interaction?
| Option | Description | Selected |
|--------|-------------|----------|
| Bonus extends tolerance (Recommended) | late_threshold = shift_start + tolerance + bonus | ✓ |
| Bonus reduces late minutes | Subtract bonus from computed late | |
| Bonus is separate additive credit | Adds to work_minutes unconditionally | |

### Q2: Anomaly flagging model?
| Option | Description | Selected |
|--------|-------------|----------|
| Enum codes, multi-flag, best-effort calc (Recommended) | `daily_record_anomalies` table, codes, no block | ✓ |
| Single anomaly_flag boolean + notes text | Simple bit + freeform | |
| Block finalization on any anomaly | pending/finalized state machine | |

### Q3: Lunch punch-mode fallback?
| Option | Description | Selected |
|--------|-------------|----------|
| Apply fixed fallback + raise LUNCH_PUNCH_MISSING (Recommended) | Conservative deduction, visible anomaly | ✓ |
| Zero lunch deduction + flag | Maximize worked minutes, risk OT inflation | |
| Block finalization | Forces resolution, blocks dashboard | |

### Q4: Aggregation window + direction?
| Option | Description | Selected |
|--------|-------------|----------|
| Window = shift ± tolerance + direction-aware (Recommended) | Shift bounds + tolerance + bonus; direction-filtered; unknowns flagged | ✓ |
| Full-calendar-day window | 00:00–23:59 events | |
| Window configurable per department | Dept stores window margins | |

---

## Claude's Discretion

User deferred the following to planning:
- Rust module layout within `calc/`, `daily_records/`, `leaves/`
- DailyRecord column exhaustive list
- Event-driven recompute debouncing mechanism
- Cron scheduler choice
- chrono-tz integration details
- Anomaly insert location (app-code vs trigger)

## Deferred Ideas

- Holiday calendar + surcharge (v2 HOL-01..03)
- Partial-day leave
- Pending-approval workflow for leave
- Per-department timezone
- Configurable leave types table
- Collective-bargaining OT multiplier overrides
- Vacation balance tracking
- Precise mixed-shift threshold (7.5h LOTTT Art. 173)
- Per-minute day/night partition for night premium

## Jurisdiction / Legal Notes

Target market = Venezuela. All overtime, night, Sunday, and workday decisions pinned to LOTTT articles (117, 118, 120, 173, 178). STATE.md has an obsolete "Mexico DST" blocker entry — must be corrected to reference Venezuela / `America/Caracas` / no DST in a subsequent workflow.
