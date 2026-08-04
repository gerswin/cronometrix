//! Lunch deduction logic (CALC-05, D-19, revised for M-03).
//!
//! Two modes:
//! - `fixed`: deduct `lunch_duration_min` from worked time — but see the two
//!   M-03 decisions below; deduction is no longer unconditional.
//! - `punch`: a mid-shift (exit, entry) pair is expected to bracket lunch. If
//!   no such break was punched at all, fall back to `lunch_duration_min` AND
//!   raise `LunchPunchMissing` so the supervisor can confirm.
//!
//! ## M-03: why `punch` mode no longer scans for the break itself
//!
//! Before M-03, `work_minutes` was `last_exit - first_entry` (the extremes
//! bug), so a punched lunch break was invisible to the raw total and this
//! module had to find the mid-shift (exit, entry) pair itself and subtract
//! its duration explicitly.
//!
//! Now `engine::compute_daily_record` derives worked minutes from
//! [`super::aggregation::pair_work_intervals`], which pairs every entry with
//! its exit and sums only the closed intervals — a punched lunch gap is
//! *already* excluded from `worked_minutes` before this function ever runs.
//! Deducting the fallback duration on top of that would subtract the same
//! minutes twice. So this function's job in `punch` mode reduces to: was a
//! break punched at all? If yes, nothing further to deduct — the pairing
//! already accounted for it. If no (`had_mid_shift_break = false`, a single
//! continuous interval spanning the whole presence), assume the employee
//! forgot to punch out for lunch, fall back to the configured duration, and
//! raise `LunchPunchMissing` for operator review — unchanged from the
//! pre-M-03 contract for that specific case.
//!
//! ## M-03: the two `fixed`-mode decisions
//!
//! `fixed` mode does not look at punches at all — by definition it assumes
//! an untracked lunch happens somewhere in every sufficiently long shift.
//! Before M-03 it deducted `lunch_duration_min` unconditionally, which could
//! deduct more than was worked (negative-then-clamped-to-zero work) or
//! deduct a full lunch from a shift too short to have contained one.
//!
//! Decision: deduct the configured minutes only when strictly more was
//! worked than the lunch itself (`worked_minutes > fallback`); otherwise
//! deduct zero. This single threshold answers both open questions at once:
//! - "never deduct more than was worked" holds automatically, because the
//!   deduction is only ever applied when `worked_minutes > fallback`, so the
//!   result (`worked_minutes - fallback`) is always positive.
//! - "does it make sense to deduct when the shift is shorter than (or equal
//!   to) the lunch itself" — no: such a shift could not have contained a
//!   full, unpaid lunch break without leaving ~0 minutes of real work, and
//!   assuming it did anyway is exactly the defect this fix removes. The
//!   boundary is inclusive of equality (`worked_minutes == fallback`
//!   deducts zero, not the full amount) for the same reason: treating an
//!   entire short shift as nothing but lunch is implausible.
//!
//! `fixed` mode intentionally does not try to correlate an observed punch
//! gap with "the lunch" — any mid-shift gap under `fixed` mode is a separate
//! absence, unrelated to the flat, untracked lunch this mode assumes. That
//! correlation is exactly what `punch` mode exists for.

use super::anomalies::AnomalyCode;
use super::models::DepartmentConfig;

/// Returns `(minutes_deducted, optional_anomaly)`.
///
/// `worked_minutes` is the sum of paired work intervals (already excludes
/// every gap, including a punched lunch). `had_mid_shift_break` is `true`
/// when [`super::aggregation::pair_work_intervals`] produced more than one
/// closed interval — i.e. at least one gap, presumed to be lunch, was
/// actually punched.
pub fn compute_lunch_deduction(
    worked_minutes: i64,
    had_mid_shift_break: bool,
    dept: &DepartmentConfig,
) -> (i64, Option<AnomalyCode>) {
    let fallback = dept.lunch_duration_min.unwrap_or(0);
    match dept.lunch_mode.as_str() {
        "fixed" => {
            if worked_minutes > fallback {
                (fallback, None)
            } else {
                (0, None)
            }
        }
        "punch" => {
            if had_mid_shift_break {
                (0, None)
            } else {
                (fallback, Some(AnomalyCode::LunchPunchMissing))
            }
        }
        _ => (fallback, None),
    }
}

#[cfg(test)]
mod tests {
    use super::super::models::DepartmentConfig;
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

    // -------------------------------------------------------------------
    // punch mode
    // -------------------------------------------------------------------

    /// M-03 signature migration: same scenario as the pre-M-03
    /// `lunch_punch_missing_falls_back_and_flags` test (an 8h/480min span,
    /// no mid-shift break punched) — value unchanged (60, LunchPunchMissing),
    /// only the call now takes `worked_minutes`/`had_mid_shift_break`
    /// instead of a hand-built `Aggregated`, because the pairing scan that
    /// used to live in this module moved to
    /// `aggregation::pair_work_intervals`.
    #[test]
    fn punch_mode_missing_break_falls_back_and_flags() {
        let dept = mk_dept("punch", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(480, false, &dept);
        assert_eq!(mins, 60);
        assert_eq!(anomaly, Some(AnomalyCode::LunchPunchMissing));
    }

    /// M-03: once a mid-shift break was actually punched, the gap is already
    /// excluded from `worked_minutes` by pairing — deducting the fallback on
    /// top would double-count. This is a NEW test: the pre-M-03 module had
    /// no way to express "a break was found, deduct nothing more" because it
    /// computed the deduction itself instead of relying on the caller's
    /// interval sum.
    #[test]
    fn punch_mode_break_already_punched_deducts_nothing_more() {
        let dept = mk_dept("punch", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(420, true, &dept);
        assert_eq!(mins, 0);
        assert!(anomaly.is_none());
    }

    // -------------------------------------------------------------------
    // fixed mode (M-03: no longer unconditional)
    // -------------------------------------------------------------------

    /// M-03 signature migration: same scenario as the pre-M-03
    /// `lunch_fixed_returns_configured_minutes` test (480min worked, 45min
    /// configured lunch) — value unchanged (45, none) because 480 > 45, so
    /// the new threshold still deducts the full configured amount here.
    #[test]
    fn fixed_mode_deducts_configured_minutes_when_shift_exceeds_lunch() {
        let dept = mk_dept("fixed", Some(45));
        let (mins, anomaly) = compute_lunch_deduction(480, false, &dept);
        assert_eq!(mins, 45);
        assert!(anomaly.is_none());
    }

    /// M-03 decision 2: a shift shorter than the configured lunch cannot
    /// have contained a full unpaid lunch break without leaving ~0 minutes
    /// of real work. 45 minutes worked, 60-minute configured lunch — deduct
    /// nothing (NOT 45, which is what a naive `min(fallback, worked)` cap
    /// would give, and NOT the pre-M-03 unconditional 60).
    #[test]
    fn fixed_mode_shift_shorter_than_lunch_deducts_nothing() {
        let dept = mk_dept("fixed", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(45, false, &dept);
        assert_eq!(mins, 0);
        assert!(anomaly.is_none());
    }

    /// M-03 decision 2, boundary: worked_minutes exactly equal to the
    /// configured lunch. Deducting the full lunch here would zero out a
    /// shift that was, however short, actually worked — treated the same as
    /// "shorter than", not as "long enough to deduct".
    #[test]
    fn fixed_mode_shift_exactly_equal_to_lunch_deducts_nothing() {
        let dept = mk_dept("fixed", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(60, false, &dept);
        assert_eq!(mins, 0);
        assert!(anomaly.is_none());
    }

    /// M-03: never deduct more than was worked. One minute over the lunch
    /// threshold still deducts the full configured amount (the threshold is
    /// strict `>`, not `>=`), leaving a small positive work_minutes rather
    /// than clamping to zero.
    #[test]
    fn fixed_mode_one_minute_over_lunch_deducts_full_configured_amount() {
        let dept = mk_dept("fixed", Some(60));
        let (mins, anomaly) = compute_lunch_deduction(61, false, &dept);
        assert_eq!(mins, 60);
        assert!(anomaly.is_none());
    }

    #[test]
    fn unknown_mode_and_missing_fixed_duration_default_to_zero() {
        assert_eq!(
            compute_lunch_deduction(480, false, &mk_dept("custom", Some(25))),
            (25, None)
        );
        assert_eq!(
            compute_lunch_deduction(480, false, &mk_dept("fixed", None)),
            (0, None)
        );
    }
}
