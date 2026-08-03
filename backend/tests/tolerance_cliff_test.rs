//! H-01 regression tests: the configured tolerance (+ bonus) must gate
//! `late_minutes` / `early_departure_minutes` as a CLIFF, not a subtraction.
//!
//! Before the fix, `engine.rs` computed lateness from the nominal shift
//! start with no regard for `late_arrival_tolerance_min` / `bonus_minutes` —
//! that tolerance only ever widened the event-capture window
//! (`calc/overnight.rs`), a different concern. So an entry one minute past
//! an explicitly configured 10-minute tolerance reported `late_minutes = 1`
//! instead of the correct `late_minutes = 11`.
//!
//! Cases R1.1-R1.8 are taken verbatim from `docs/QA-GUIDE.md` §21.2 (lines
//! ~425-436). Inside the grace period (tolerance + bonus) lateness/earliness
//! is zero. Past the grace period it is the FULL amount measured from the
//! nominal shift boundary — not the excess over the grace. R1.2 expects 11,
//! not 1; R1.4 expects 16, not 1. This is intentional, not a typo: the rule
//! is a cliff.

use chrono::{NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;
use cronometrix_api::calc::compute_daily_record;
use cronometrix_api::calc::models::{
    AttendanceEventRow, DepartmentConfig, EngineInput, GlobalRulesRow,
};

fn tz() -> Tz {
    "America/Caracas".parse().unwrap()
}

fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn epoch_at(anchor: NaiveDate, time: NaiveTime, tz: Tz) -> i64 {
    tz.from_local_datetime(&anchor.and_time(time))
        .single()
        .expect("unambiguous local datetime for fixed Monday anchor in America/Caracas")
        .timestamp()
}

/// Builds an `EngineInput` for a single day-shift on a fixed Monday anchor
/// (2026-04-20, no DST ambiguity in America/Caracas), with one entry and one
/// exit event at the given local times.
#[allow(clippy::too_many_arguments)]
fn build_input(
    shift_start: &str,
    shift_end: &str,
    late_tol: i64,
    early_tol: i64,
    bonus: i64,
    entry_time: NaiveTime,
    exit_time: NaiveTime,
) -> EngineInput {
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    let tzv = tz();
    let dept = DepartmentConfig {
        id: "dept-tol-cliff".into(),
        shift_start_time: shift_start.into(),
        shift_end_time: shift_end.into(),
        shift_type: "day".into(),
        is_overnight_shift: false,
        ordinary_daily_minutes: 480,
        lunch_mode: "fixed".into(),
        lunch_duration_min: Some(0),
    };
    let rules = GlobalRulesRow {
        late_arrival_tolerance_min: late_tol,
        early_departure_tolerance_min: early_tol,
        bonus_minutes: bonus,
    };
    let entry_epoch = epoch_at(anchor, entry_time, tzv);
    let exit_epoch = epoch_at(anchor, exit_time, tzv);
    let events = vec![
        AttendanceEventRow {
            id: "e1".into(),
            employee_id: Some("emp-tol".into()),
            device_id: "dev-1".into(),
            direction: "entry".into(),
            captured_at: entry_epoch,
            is_unknown: false,
        },
        AttendanceEventRow {
            id: "e2".into(),
            employee_id: Some("emp-tol".into()),
            device_id: "dev-1".into(),
            direction: "exit".into(),
            captured_at: exit_epoch,
            is_unknown: false,
        },
    ];
    EngineInput {
        events,
        dept,
        rules,
        leave: None,
        anchor_date: anchor,
        tz: tzv,
        weekly_ot_minutes_so_far: 0,
        annual_ot_minutes_so_far: 0,
        prior_record_existed: false,
    }
}

// -----------------------------------------------------------------------------
// R1.1-R1.5: late arrival (shift 08:00-17:00)
// -----------------------------------------------------------------------------

/// R1.1 — turno 08:00, tol=10, bono=0; entrada 08:05 -> late=0 (dentro tol).
#[test]
fn r1_1_within_tolerance_no_bonus_is_zero() {
    let input = build_input("08:00", "17:00", 10, 10, 0, hm(8, 5), hm(17, 0));
    let out = compute_daily_record(&input);
    assert_eq!(out.late_minutes, 0);
}

/// R1.2 — turno 08:00, tol=10, bono=0; entrada 08:11 -> late=11 (NOT 1: the
/// cliff reports the full amount from nominal start once tolerance is
/// exceeded, not the excess over tolerance).
#[test]
fn r1_2_past_tolerance_no_bonus_is_full_amount() {
    let input = build_input("08:00", "17:00", 10, 10, 0, hm(8, 11), hm(17, 0));
    let out = compute_daily_record(&input);
    assert_eq!(out.late_minutes, 11);
}

/// R1.3 — turno 08:00, tol=10, bono=5; entrada 08:14 -> late=0 (10+5=15 min
/// grace period; bonus is additional to tolerance, not a replacement).
#[test]
fn r1_3_within_tolerance_plus_bonus_is_zero() {
    let input = build_input("08:00", "17:00", 10, 10, 5, hm(8, 14), hm(17, 0));
    let out = compute_daily_record(&input);
    assert_eq!(out.late_minutes, 0);
}

/// R1.4 — turno 08:00, tol=10, bono=5; entrada 08:16 -> late=16 (NOT 1: past
/// the 15-minute grace, full amount from nominal start).
#[test]
fn r1_4_past_tolerance_plus_bonus_is_full_amount() {
    let input = build_input("08:00", "17:00", 10, 10, 5, hm(8, 16), hm(17, 0));
    let out = compute_daily_record(&input);
    assert_eq!(out.late_minutes, 16);
}

/// R1.5 — turno 08:00, tol=10; entrada 07:55 (early arrival) -> late=0.
#[test]
fn r1_5_early_arrival_is_zero() {
    let input = build_input("08:00", "17:00", 10, 10, 0, hm(7, 55), hm(17, 0));
    let out = compute_daily_record(&input);
    assert_eq!(out.late_minutes, 0);
}

// -----------------------------------------------------------------------------
// R1.6-R1.8: early departure (shift 08:00-17:00)
// -----------------------------------------------------------------------------

/// R1.6 — salida 17:00, tol=10, bono=0; salida 16:55 -> early=0 (dentro tol).
#[test]
fn r1_6_within_tolerance_no_bonus_is_zero() {
    let input = build_input("08:00", "17:00", 10, 10, 0, hm(8, 0), hm(16, 55));
    let out = compute_daily_record(&input);
    assert_eq!(out.early_departure_minutes, 0);
}

/// R1.7 — salida 17:00, tol=10, bono=0; salida 16:45 -> early=15 (past
/// tolerance: full amount from nominal end, not the 5-minute excess).
#[test]
fn r1_7_past_tolerance_no_bonus_is_full_amount() {
    let input = build_input("08:00", "17:00", 10, 10, 0, hm(8, 0), hm(16, 45));
    let out = compute_daily_record(&input);
    assert_eq!(out.early_departure_minutes, 15);
}

/// R1.8 — salida 17:00, tol=10, bono=5; salida 16:46 -> early=0 (14 minutes
/// early, inside the 10+5=15-minute grace).
#[test]
fn r1_8_within_tolerance_plus_bonus_is_zero() {
    let input = build_input("08:00", "17:00", 10, 10, 5, hm(8, 0), hm(16, 46));
    let out = compute_daily_record(&input);
    assert_eq!(out.early_departure_minutes, 0);
}

// -----------------------------------------------------------------------------
// Explicit boundary-minute test: the exact edge of tolerance must still be
// zero (a `>=` where a `>` belongs loses the tolerance at its own boundary).
// -----------------------------------------------------------------------------

/// tol=10, bono=0: 08:10 (exactly at the tolerance boundary, 600s) must
/// still be 0; 08:11 (601-660s, one minute past) must be the full 11.
#[test]
fn late_boundary_exact_tolerance_is_zero_one_minute_past_is_full() {
    let at_boundary = build_input("08:00", "17:00", 10, 10, 0, hm(8, 10), hm(17, 0));
    let out_at_boundary = compute_daily_record(&at_boundary);
    assert_eq!(
        out_at_boundary.late_minutes, 0,
        "08:10 is exactly at the 10-minute tolerance boundary and must not be late"
    );

    let past_boundary = build_input("08:00", "17:00", 10, 10, 0, hm(8, 11), hm(17, 0));
    let out_past_boundary = compute_daily_record(&past_boundary);
    assert_eq!(
        out_past_boundary.late_minutes, 11,
        "08:11 is one minute past the tolerance boundary and must report the full 11 minutes"
    );
}

/// Mirrors the late-arrival boundary test for early departure: tol=10,
/// bono=0. Leaving exactly 10 minutes early (16:50) must be 0; leaving 11
/// minutes early (16:49) must be the full 11.
#[test]
fn early_departure_boundary_exact_tolerance_is_zero_one_minute_past_is_full() {
    let at_boundary = build_input("08:00", "17:00", 10, 10, 0, hm(8, 0), hm(16, 50));
    let out_at_boundary = compute_daily_record(&at_boundary);
    assert_eq!(
        out_at_boundary.early_departure_minutes, 0,
        "16:50 is exactly at the 10-minute tolerance boundary and must not be early"
    );

    let past_boundary = build_input("08:00", "17:00", 10, 10, 0, hm(8, 0), hm(16, 49));
    let out_past_boundary = compute_daily_record(&past_boundary);
    assert_eq!(
        out_past_boundary.early_departure_minutes, 11,
        "16:49 is one minute past the tolerance boundary and must report the full 11 minutes"
    );
}
