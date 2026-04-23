//! Lunch deduction logic (CALC-05, D-19).
//!
//! Two modes:
//! - `fixed`: always deduct `lunch_duration_min`.
//! - `punch`: look for a mid-shift (exit, entry) pair inside the aggregation
//!   window. If found, deduct the elapsed minutes between them. If either side
//!   is missing, fall back to `lunch_duration_min` AND raise
//!   `LunchPunchMissing` so the supervisor can confirm.

use super::aggregation::Aggregated;
use super::anomalies::AnomalyCode;
use super::models::DepartmentConfig;

/// Returns `(minutes_deducted, optional_anomaly)`.
pub fn compute_lunch_deduction(
    agg: &Aggregated,
    dept: &DepartmentConfig,
) -> (i64, Option<AnomalyCode>) {
    let fallback = dept.lunch_duration_min.unwrap_or(0);
    match dept.lunch_mode.as_str() {
        "fixed" => (fallback, None),
        "punch" => {
            let entry_ts = match agg.canonical_entry {
                Some(t) => t,
                None => return (0, None),
            };
            let exit_ts = match agg.canonical_exit {
                Some(t) => t,
                None => return (0, None),
            };
            // Mid-shift exit: first exit strictly after canonical_entry
            // and strictly before canonical_exit.
            let lunch_out = agg
                .exits_in_window
                .iter()
                .find(|e| e.captured_at > entry_ts && e.captured_at < exit_ts);
            let lunch_out = match lunch_out {
                Some(e) => e,
                None => return (fallback, Some(AnomalyCode::LunchPunchMissing)),
            };
            let lunch_in = agg
                .entries_in_window
                .iter()
                .find(|e| e.captured_at > lunch_out.captured_at && e.captured_at < exit_ts);
            let lunch_in = match lunch_in {
                Some(e) => e,
                None => return (fallback, Some(AnomalyCode::LunchPunchMissing)),
            };
            let mins = (lunch_in.captured_at - lunch_out.captured_at) / 60;
            (mins.max(0), None)
        }
        _ => (fallback, None),
    }
}

#[cfg(test)]
mod tests {
    use super::super::aggregation::Aggregated;
    use super::super::models::{AttendanceEventRow, DepartmentConfig};
    use super::*;

    fn mk_dept(mode: &str, lunch_min: Option<i64>) -> DepartmentConfig {
        DepartmentConfig {
            id: "d1".into(),
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            shift_type: "day".into(),
            is_overnight_shift: false,
            ordinary_daily_minutes: 480,
            lunch_mode: mode.into(),
            lunch_duration_min: lunch_min,
        }
    }

    fn ev(id: &str, direction: &str, captured_at: i64) -> AttendanceEventRow {
        AttendanceEventRow {
            id: id.into(),
            employee_id: Some("e1".into()),
            device_id: "dev".into(),
            direction: direction.into(),
            captured_at,
            is_unknown: false,
        }
    }

    #[test]
    fn lunch_punch_missing_falls_back_and_flags() {
        let agg = Aggregated {
            canonical_entry: Some(0),
            canonical_exit: Some(28_800),
            unknown_in_window: false,
            entries_in_window: vec![ev("e1", "entry", 0)],
            exits_in_window: vec![ev("x1", "exit", 28_800)],
        };
        let dept = mk_dept("punch", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(&agg, &dept);
        assert_eq!(mins, 60);
        assert_eq!(anomaly, Some(AnomalyCode::LunchPunchMissing));
    }

    #[test]
    fn lunch_fixed_returns_configured_minutes() {
        let agg = Aggregated {
            canonical_entry: Some(0),
            canonical_exit: Some(28_800),
            unknown_in_window: false,
            entries_in_window: vec![ev("e1", "entry", 0)],
            exits_in_window: vec![ev("x1", "exit", 28_800)],
        };
        let dept = mk_dept("fixed", Some(45));
        let (mins, anomaly) = compute_lunch_deduction(&agg, &dept);
        assert_eq!(mins, 45);
        assert!(anomaly.is_none());
    }
}
