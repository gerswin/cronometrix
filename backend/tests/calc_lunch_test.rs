use cronometrix_api::calc::aggregation::Aggregated;
use cronometrix_api::calc::anomalies::AnomalyCode;
use cronometrix_api::calc::lunch::compute_lunch_deduction;
use cronometrix_api::calc::models::{AttendanceEventRow, DepartmentConfig};

fn event(id: &str, direction: &str, captured_at: i64) -> AttendanceEventRow {
    AttendanceEventRow {
        id: id.to_string(),
        employee_id: Some("employee-1".to_string()),
        device_id: "device-1".to_string(),
        direction: direction.to_string(),
        captured_at,
        is_unknown: false,
    }
}

fn department(mode: &str, fallback: Option<i64>) -> DepartmentConfig {
    DepartmentConfig {
        id: "department-1".to_string(),
        shift_start_time: "08:00".to_string(),
        shift_end_time: "17:00".to_string(),
        shift_type: "day".to_string(),
        is_overnight_shift: false,
        ordinary_daily_minutes: 480,
        lunch_mode: mode.to_string(),
        lunch_duration_min: fallback,
    }
}

fn aggregate(
    entry: Option<i64>,
    exit: Option<i64>,
    entries: Vec<AttendanceEventRow>,
    exits: Vec<AttendanceEventRow>,
) -> Aggregated {
    Aggregated {
        canonical_entry: entry,
        canonical_exit: exit,
        unknown_in_window: false,
        entries_in_window: entries,
        exits_in_window: exits,
    }
}

#[test]
fn punch_mode_handles_missing_boundaries_and_missing_return() {
    let punch = department("punch", Some(45));

    assert_eq!(
        compute_lunch_deduction(&aggregate(None, Some(17_000), vec![], vec![]), &punch),
        (0, None)
    );
    assert_eq!(
        compute_lunch_deduction(&aggregate(Some(1_000), None, vec![], vec![]), &punch),
        (0, None)
    );

    let missing_return = aggregate(
        Some(1_000),
        Some(17_000),
        vec![event("entry", "entry", 1_000)],
        vec![
            event("lunch-out", "exit", 8_000),
            event("exit", "exit", 17_000),
        ],
    );
    assert_eq!(
        compute_lunch_deduction(&missing_return, &punch),
        (45, Some(AnomalyCode::LunchPunchMissing))
    );
}

#[test]
fn punch_mode_uses_the_first_complete_mid_shift_pair() {
    let agg = aggregate(
        Some(1_000),
        Some(17_000),
        vec![
            event("entry", "entry", 1_000),
            event("lunch-in", "entry", 9_800),
            event("late-entry", "entry", 12_000),
        ],
        vec![
            event("early-exit", "exit", 500),
            event("lunch-out", "exit", 8_000),
            event("exit", "exit", 17_000),
        ],
    );

    assert_eq!(
        compute_lunch_deduction(&agg, &department("punch", Some(60))),
        (30, None)
    );
}

#[test]
fn unknown_mode_and_missing_fixed_duration_default_to_zero() {
    let agg = aggregate(Some(1_000), Some(17_000), vec![], vec![]);
    assert_eq!(
        compute_lunch_deduction(&agg, &department("custom", Some(25))),
        (25, None)
    );
    assert_eq!(
        compute_lunch_deduction(&agg, &department("fixed", None)),
        (0, None)
    );
}
