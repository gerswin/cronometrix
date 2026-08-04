//! `calc::lunch::compute_lunch_deduction` — M-03 revised contract.
//!
//! Pre-M-03 this module built an `Aggregated` by hand and let
//! `compute_lunch_deduction` scan `entries_in_window`/`exits_in_window`
//! itself to find a mid-shift (exit, entry) pair. That scan moved to
//! `aggregation::pair_work_intervals` (called once by `engine.rs`, shared
//! with worked-minutes computation instead of duplicated), so the function
//! under test here now takes the already-computed `worked_minutes` and
//! `had_mid_shift_break` directly. See `calc::lunch` module docs for the
//! full M-03 rationale (avoiding double-deducting a punched lunch gap that
//! pairing already excluded from `worked_minutes`).

use cronometrix_api::calc::anomalies::AnomalyCode;
use cronometrix_api::calc::lunch::compute_lunch_deduction;
use cronometrix_api::calc::models::DepartmentConfig;

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

/// M-03 signature migration of the pre-M-03
/// `punch_mode_handles_missing_boundaries_and_missing_return` case that used
/// a hand-built `Aggregated` with no exit at all in the window: no closed
/// interval exists, so `had_mid_shift_break = false`. Value unchanged: falls
/// back to the configured 45 minutes and flags `LunchPunchMissing`.
#[test]
fn punch_mode_with_no_break_punched_falls_back_and_flags() {
    let punch = department("punch", Some(45));
    assert_eq!(
        compute_lunch_deduction(480, false, &punch),
        (45, Some(AnomalyCode::LunchPunchMissing))
    );
}

/// M-03: same 45-minute fallback department, but this time
/// `pair_work_intervals` (called upstream, in `engine.rs`) found a real
/// mid-shift break — its duration is already excluded from `worked_minutes`,
/// so `compute_lunch_deduction` must not subtract anything further, and must
/// not raise `LunchPunchMissing` (a break WAS found).
///
/// This is a NEW test: under the pre-M-03 contract, "a break was found" and
/// "how much to deduct" were the same computation living inside this module;
/// M-03 splits them, so the "already accounted for, deduct nothing" branch
/// did not previously exist to test.
#[test]
fn punch_mode_with_break_already_excluded_deducts_nothing() {
    let punch = department("punch", Some(45));
    assert_eq!(compute_lunch_deduction(420, true, &punch), (0, None));
}

/// M-03 signature migration of the pre-M-03
/// `unknown_mode_and_missing_fixed_duration_default_to_zero` case. Values
/// unchanged: a mode string that is neither `fixed` nor `punch` deducts the
/// configured fallback unconditionally (25), and a `fixed` department with
/// no configured duration deducts 0 (`unwrap_or(0)`).
#[test]
fn unknown_mode_and_missing_fixed_duration_default_to_zero() {
    assert_eq!(
        compute_lunch_deduction(480, false, &department("custom", Some(25))),
        (25, None)
    );
    assert_eq!(
        compute_lunch_deduction(480, false, &department("fixed", None)),
        (0, None)
    );
}

// -----------------------------------------------------------------------------
// M-03 decision 2: `fixed` mode is no longer unconditional.
// -----------------------------------------------------------------------------

/// Hand-derivation: worked 480min (8h), configured lunch 60min.
/// 480 > 60, so the full configured lunch is deducted: 480 - 60 = 420
/// work_minutes (verified at the `compute_daily_record` level in
/// `punch_pairing_test.rs`). Here we assert the deduction amount itself.
#[test]
fn fixed_mode_deducts_full_amount_for_a_normal_shift() {
    let fixed = department("fixed", Some(60));
    assert_eq!(compute_lunch_deduction(480, false, &fixed), (60, None));
}

/// Hand-derivation: worked 45min total, configured lunch 60min. A
/// 45-minute presence cannot have contained a 60-minute unpaid lunch without
/// leaving negative work — the pre-M-03 code deducted the full 60 anyway
/// (clamped to 0 work_minutes downstream). M-03 deducts 0 instead: a lunch
/// nobody could have taken is not subtracted from a shift that short.
#[test]
fn fixed_mode_shorter_than_the_lunch_itself_deducts_nothing() {
    let fixed = department("fixed", Some(60));
    assert_eq!(compute_lunch_deduction(45, false, &fixed), (0, None));
}

/// Boundary: worked_minutes exactly equal to the configured lunch (60 == 60)
/// also deducts nothing — treating the whole shift as lunch is as implausible
/// as treating a shorter one that way. The threshold is a strict `>`.
#[test]
fn fixed_mode_exactly_equal_to_the_lunch_itself_deducts_nothing() {
    let fixed = department("fixed", Some(60));
    assert_eq!(compute_lunch_deduction(60, false, &fixed), (0, None));
}
