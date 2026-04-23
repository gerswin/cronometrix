//! Pure-engine tests for Phase 3 Plan 03-01. Fixture-driven scenarios +
//! hand-written edge cases + property-based OT monotonicity.

use chrono::{NaiveDate, TimeZone, Utc};
use cronometrix_api::calc::anomalies::AnomalyCode;
use cronometrix_api::calc::models::{
    AttendanceEventRow, DepartmentConfig, EngineInput, GlobalRulesRow,
};
use cronometrix_api::calc::{compute_daily_record, DailyRecordOutput};
use proptest::prelude::*;
use serde::Deserialize;

// -----------------------------------------------------------------------------
// Fixture-driven LOTTT scenarios
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ScenarioEvent {
    direction: String,
    captured_at_iso: String,
    is_unknown: bool,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    description: String,
    shift_type: String,
    is_overnight_shift: bool,
    ordinary_daily_minutes: i64,
    shift_start: String,
    shift_end: String,
    lunch_mode: String,
    lunch_duration_min: Option<i64>,
    late_tolerance: i64,
    early_tolerance: i64,
    bonus_minutes: i64,
    anchor_date: String,
    events: Vec<ScenarioEvent>,
    expected_work_minutes: i64,
    expected_overtime_minutes: i64,
    expected_late_minutes: i64,
    expected_early_departure_minutes: i64,
    expected_anomalies: Vec<String>,
}

fn load_scenarios() -> Vec<Scenario> {
    let raw = std::fs::read_to_string("tests/fixtures/lottt_scenarios.json")
        .expect("lottt_scenarios.json readable");
    serde_json::from_str(&raw).expect("lottt_scenarios.json is valid JSON")
}

fn build_input(s: &Scenario) -> EngineInput {
    let dept = DepartmentConfig {
        id: "dept-test".into(),
        shift_start_time: s.shift_start.clone(),
        shift_end_time: s.shift_end.clone(),
        shift_type: s.shift_type.clone(),
        is_overnight_shift: s.is_overnight_shift,
        ordinary_daily_minutes: s.ordinary_daily_minutes,
        lunch_mode: s.lunch_mode.clone(),
        lunch_duration_min: s.lunch_duration_min,
    };
    let rules = GlobalRulesRow {
        late_arrival_tolerance_min: s.late_tolerance,
        early_departure_tolerance_min: s.early_tolerance,
        bonus_minutes: s.bonus_minutes,
    };
    let events: Vec<AttendanceEventRow> = s
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let dt = chrono::DateTime::parse_from_rfc3339(&e.captured_at_iso)
                .expect("fixture has valid ISO timestamp");
            AttendanceEventRow {
                id: format!("evt-{}", i),
                employee_id: Some("emp-test".into()),
                device_id: "dev-1".into(),
                direction: e.direction.clone(),
                captured_at: dt.timestamp(),
                is_unknown: e.is_unknown,
            }
        })
        .collect();
    let anchor_date = NaiveDate::parse_from_str(&s.anchor_date, "%Y-%m-%d")
        .expect("anchor_date fixture is YYYY-MM-DD");
    EngineInput {
        events,
        dept,
        rules,
        leave: None,
        anchor_date,
        tz: "America/Caracas".parse().unwrap(),
        weekly_ot_minutes_so_far: 0,
        annual_ot_minutes_so_far: 0,
        prior_record_existed: false,
    }
}

#[test]
fn lottt_scenarios_all_pass() {
    let scenarios = load_scenarios();
    assert!(
        !scenarios.is_empty(),
        "fixture must contain at least one scenario"
    );
    for s in &scenarios {
        let input = build_input(s);
        let out = compute_daily_record(&input);
        assert_eq!(
            out.work_minutes, s.expected_work_minutes,
            "work_minutes mismatch for scenario: {}",
            s.description
        );
        assert_eq!(
            out.overtime_minutes, s.expected_overtime_minutes,
            "overtime_minutes mismatch for scenario: {}",
            s.description
        );
        assert_eq!(
            out.late_minutes, s.expected_late_minutes,
            "late_minutes mismatch for scenario: {}",
            s.description
        );
        assert_eq!(
            out.early_departure_minutes, s.expected_early_departure_minutes,
            "early_departure_minutes mismatch for scenario: {}",
            s.description
        );
        let got_codes: Vec<String> =
            out.anomalies.iter().map(|a| a.as_str().to_string()).collect();
        for expected in &s.expected_anomalies {
            assert!(
                got_codes.contains(expected),
                "missing anomaly {} for scenario: {} (got {:?})",
                expected,
                s.description,
                got_codes
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Property-based overtime monotonicity
// -----------------------------------------------------------------------------

fn engine_with_synthetic_events(
    work_minutes: i64,
    ordinary_daily_minutes: i64,
) -> DailyRecordOutput {
    // Use a Monday anchor (2026-04-20) so is_rest_day_worked is false.
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    // Build two events around a shift starting at 09:00 America/Caracas.
    // Use UTC epoch for shift_start via chrono_tz.
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let start_local = anchor.and_time(chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    let shift_start_epoch = tz
        .from_local_datetime(&start_local)
        .single()
        .unwrap()
        .timestamp();
    let entry_epoch = shift_start_epoch;
    let exit_epoch = shift_start_epoch + work_minutes * 60;
    let events = vec![
        AttendanceEventRow {
            id: "e1".into(),
            employee_id: Some("emp".into()),
            device_id: "d".into(),
            direction: "entry".into(),
            captured_at: entry_epoch,
            is_unknown: false,
        },
        AttendanceEventRow {
            id: "e2".into(),
            employee_id: Some("emp".into()),
            device_id: "d".into(),
            direction: "exit".into(),
            captured_at: exit_epoch,
            is_unknown: false,
        },
    ];
    // Wide tolerance windows so the events stay inside; end set late enough
    // that 1200-min shifts also fit. bonus=0.
    let dept = DepartmentConfig {
        id: "d".into(),
        shift_start_time: "09:00".into(),
        // shift_end must cover exit ts; 09:00 + 1200min is over midnight, but
        // Plan 03-01 assumes day-only. Set shift_end = 23:59 and rely on the
        // large early_tolerance to include long shifts.
        shift_end_time: "23:59".into(),
        shift_type: "day".into(),
        is_overnight_shift: false,
        ordinary_daily_minutes,
        lunch_mode: "fixed".into(),
        lunch_duration_min: Some(0),
    };
    let rules = GlobalRulesRow {
        late_arrival_tolerance_min: 120,
        early_departure_tolerance_min: 1440,
        bonus_minutes: 0,
    };
    let input = EngineInput {
        events,
        dept,
        rules,
        leave: None,
        anchor_date: anchor,
        tz,
        weekly_ot_minutes_so_far: 0,
        annual_ot_minutes_so_far: 0,
        prior_record_existed: false,
    };
    compute_daily_record(&input)
}

proptest! {
    #[test]
    fn overtime_monotonicity(
        work_minutes in 60i64..=900i64,
        ordinary in 60i64..=600i64,
    ) {
        let out = engine_with_synthetic_events(work_minutes, ordinary);
        // work_minutes should match since no lunch deduction, events within window.
        prop_assert_eq!(out.work_minutes, work_minutes);
        prop_assert_eq!(out.overtime_minutes, (work_minutes - ordinary).max(0));
    }
}

// -----------------------------------------------------------------------------
// Plan 03-02: Overnight anchor-date correctness
// -----------------------------------------------------------------------------
// For any random (anchor_date, overnight shift_start ∈ 18:00–23:45,
// overnight shift_end ∈ 03:00–08:00) in America/Caracas, the computed
// nominal_shift_start converted back to local date must equal anchor_date
// (D-05 anchor = shift-start date rule). Venezuela has no DST so ambiguous
// must always be false.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn overnight_anchor_date_correctness(
        start_hour in 18u32..24u32,
        start_min_raw in 0u32..4u32,
        end_hour in 3u32..9u32,
        end_min_raw in 0u32..4u32,
        year in 2024i32..2030i32,
        month in 1u32..13u32,
        day in 1u32..29u32,
    ) {
        use chrono::NaiveDate;
        use chrono_tz::America::Caracas;
        use cronometrix_api::calc::models::{DepartmentConfig, GlobalRulesRow};
        use cronometrix_api::calc::overnight::shift_window_overnight_aware;

        let start_min = start_min_raw * 15;   // 0, 15, 30, 45
        let end_min = end_min_raw * 15;

        let anchor = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        let dept = DepartmentConfig {
            id: "d".into(),
            shift_start_time: format!("{:02}:{:02}", start_hour, start_min),
            shift_end_time: format!("{:02}:{:02}", end_hour, end_min),
            shift_type: "night".into(),
            is_overnight_shift: true,
            ordinary_daily_minutes: 420,
            lunch_mode: "fixed".into(),
            lunch_duration_min: Some(60),
        };
        let rules = GlobalRulesRow {
            late_arrival_tolerance_min: 10,
            early_departure_tolerance_min: 10,
            bonus_minutes: 0,
        };

        let (_ws, _we, nominal_start, _ne, amb) =
            shift_window_overnight_aware(anchor, &dept, &rules, Caracas);

        // Venezuela has no DST — ambiguous must always be false.
        prop_assert!(!amb, "America/Caracas should never produce ambiguous LocalResult");

        // Converting nominal_start back to local date must give anchor.
        let local_start_date = chrono::DateTime::from_timestamp(nominal_start, 0)
            .unwrap()
            .with_timezone(&Caracas)
            .date_naive();
        prop_assert_eq!(
            local_start_date,
            anchor,
            "nominal_start converted back to local date must equal anchor_date"
        );
    }
}

// Overtime monotonicity for overnight shifts — mirrors the day-shift test but
// with `is_overnight_shift=true`. Asserts that adding more worked minutes to
// an overnight shift never decreases `overtime_minutes`, and that OT matches
// `max(0, work - ordinary)`.
fn overnight_engine_with_synthetic_events(
    work_minutes: i64,
    ordinary_daily_minutes: i64,
) -> DailyRecordOutput {
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    // Start at 22:00 Mon local.
    let start_local = anchor.and_time(chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    let shift_start_epoch = tz.from_local_datetime(&start_local).single().unwrap().timestamp();
    let entry_epoch = shift_start_epoch;
    let exit_epoch = shift_start_epoch + work_minutes * 60;
    let events = vec![
        AttendanceEventRow {
            id: "e1".into(),
            employee_id: Some("emp".into()),
            device_id: "d".into(),
            direction: "entry".into(),
            captured_at: entry_epoch,
            is_unknown: false,
        },
        AttendanceEventRow {
            id: "e2".into(),
            employee_id: Some("emp".into()),
            device_id: "d".into(),
            direction: "exit".into(),
            captured_at: exit_epoch,
            is_unknown: false,
        },
    ];
    // shift_end=06:00 next day (crosses midnight). Wide early tolerance so
    // exits after 06:00 Tue still fall inside the window for long work spans.
    let dept = DepartmentConfig {
        id: "d".into(),
        shift_start_time: "22:00".into(),
        shift_end_time: "06:00".into(),
        shift_type: "night".into(),
        is_overnight_shift: true,
        ordinary_daily_minutes,
        lunch_mode: "fixed".into(),
        lunch_duration_min: Some(0),
    };
    let rules = GlobalRulesRow {
        late_arrival_tolerance_min: 120,
        early_departure_tolerance_min: 1440,
        bonus_minutes: 0,
    };
    let input = EngineInput {
        events,
        dept,
        rules,
        leave: None,
        anchor_date: anchor,
        tz,
        weekly_ot_minutes_so_far: 0,
        annual_ot_minutes_so_far: 0,
        prior_record_existed: false,
    };
    compute_daily_record(&input)
}

proptest! {
    #[test]
    fn overnight_overtime_monotonicity(
        work_minutes in 60i64..=900i64,
        ordinary in 60i64..=600i64,
    ) {
        let out = overnight_engine_with_synthetic_events(work_minutes, ordinary);
        prop_assert_eq!(out.work_minutes, work_minutes);
        prop_assert_eq!(out.overtime_minutes, (work_minutes - ordinary).max(0));
    }
}

// -----------------------------------------------------------------------------
// Keep a tiny sanity-check that the Wave 0 scaffold would have seen.
// -----------------------------------------------------------------------------
#[test]
fn wave_zero_marker() {
    // Engine is pure: two identical calls return identical output.
    let out1 = engine_with_synthetic_events(420, 480);
    let out2 = engine_with_synthetic_events(420, 480);
    assert_eq!(out1, out2);
    // Use `Utc` so the type is retained (silences unused import warnings).
    let _ = Utc::now();
    // AnomalyCode variants are stable string mappings.
    assert_eq!(AnomalyCode::MissingExit.as_str(), "MISSING_EXIT");
}
