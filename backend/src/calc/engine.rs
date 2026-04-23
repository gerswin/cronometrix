//! Pure `compute_daily_record()` — no I/O, no async. Deterministic.
//!
//! Given identical [`EngineInput`] returns identical [`DailyRecordOutput`].
//! Wires together aggregation → lunch → tolerance arithmetic → overtime caps
//! → rest-day detection → RECOMPUTE_AFTER_EDIT flag.

use chrono::Datelike;

use super::aggregation::{aggregate_events, shift_window};
use super::anomalies::AnomalyCode;
use super::lunch::compute_lunch_deduction;
use super::models::{DailyRecordOutput, EngineInput};
use super::overtime::check_overtime_caps;

pub fn compute_daily_record(input: &EngineInput) -> DailyRecordOutput {
    // D-16: Leave overlay wins. Plan 03-01 input.leave is always None; Plan
    // 03-03 populates it from the `leaves` table.
    if let Some(leave) = &input.leave {
        let events_on_leave = !input.events.is_empty();
        let mut anomalies = Vec::new();
        if events_on_leave {
            anomalies.push(AnomalyCode::EventsOnLeaveDay);
        }
        if input.prior_record_existed {
            anomalies.push(AnomalyCode::RecomputeAfterEdit);
        }
        return DailyRecordOutput {
            work_minutes: 0,
            overtime_minutes: 0,
            late_minutes: 0,
            early_departure_minutes: 0,
            is_rest_day_worked: false,
            entry_at: None,
            exit_at: None,
            leave_id: Some(leave.id.clone()),
            anomalies,
        };
    }

    let (window_start, window_end, nominal_start, nominal_end) =
        shift_window(input.anchor_date, &input.dept, &input.rules, input.tz);

    let agg = aggregate_events(&input.events, window_start, window_end);

    let mut anomalies: Vec<AnomalyCode> = Vec::new();
    if agg.unknown_in_window {
        anomalies.push(AnomalyCode::UnknownFaceInWindow);
    }

    let entry_ts = agg.canonical_entry;
    let exit_ts = agg.canonical_exit;

    let (work_minutes, late_minutes, early_departure_minutes) = match (entry_ts, exit_ts) {
        (None, _) => {
            anomalies.push(AnomalyCode::MissingEntry);
            (0_i64, 0_i64, 0_i64)
        }
        (_, None) => {
            anomalies.push(AnomalyCode::MissingExit);
            (0_i64, 0_i64, 0_i64)
        }
        (Some(ent), Some(exi)) => {
            let raw_minutes = ((exi - ent) / 60).max(0);
            let (lunch_ded, lunch_anom) = compute_lunch_deduction(&agg, &input.dept);
            if let Some(a) = lunch_anom {
                anomalies.push(a);
            }
            let work = (raw_minutes - lunch_ded).max(0);
            let late = (((ent - nominal_start).max(0)) / 60).max(0);
            let early = (((nominal_end - exi).max(0)) / 60).max(0);
            (work, late, early)
        }
    };

    let overtime_minutes = (work_minutes - input.dept.ordinary_daily_minutes).max(0);

    if work_minutes > 0 {
        anomalies.extend(check_overtime_caps(
            work_minutes,
            overtime_minutes,
            input.weekly_ot_minutes_so_far,
            input.annual_ot_minutes_so_far,
        ));
    }

    // D-12: v1 hardcodes Sat/Sun as rest days. Future iteration may store a
    // per-employee rest-day set.
    let is_rest_day_worked = matches!(
        input.anchor_date.weekday(),
        chrono::Weekday::Sat | chrono::Weekday::Sun
    ) && work_minutes > 0;

    // D-03: if the prior daily_records row already existed, the service layer
    // indicates a recompute-after-edit situation (late event arrival, nightly
    // reconcile, manual event backfill). Surface for operator review.
    if input.prior_record_existed {
        anomalies.push(AnomalyCode::RecomputeAfterEdit);
    }

    DailyRecordOutput {
        work_minutes,
        overtime_minutes,
        late_minutes,
        early_departure_minutes,
        is_rest_day_worked,
        entry_at: entry_ts,
        exit_at: exit_ts,
        leave_id: None,
        anomalies,
    }
}
