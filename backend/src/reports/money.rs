//! Pure cents-i64 money math for LOTTT premiums (D-01..D-07, D-31).
//!
//! All formulas are integer-cents arithmetic. The pattern is "multiply numerators
//! first, divide once at the end" so we never lose precision via early division.
//! Every multi-step formula uses `checked_mul` to avoid panics on overflow and
//! `total_a_pagar_cents` uses `saturating_add`/`saturating_sub` for the same
//! reason — a misconfigured department should never crash the report.
//!
//! LOTTT references:
//! - Art. 117 — Jornada nocturna: +30% premium on night shifts (D-04, D-31 ADDITIVE)
//! - Art. 118 — Horas extraordinarias: +50% premium on overtime (D-06)
//! - Art. 120 — Prima dominical: +50% surcharge on Sunday/rest-day work (D-03)
//!
//! H-08: every formula below also takes a `SalaryKind` so `base_salary_cents`
//! can be normalized to "one ordinary day" as part of the SAME fraction —
//! never as a separate pre-division (see `SalaryKind::to_daily`'s doc comment
//! for why that would lose money).

use crate::employees::models::SalaryKind;

/// Pro-rated salary for the worked minutes: `work_min × base_cents / ord_min`,
/// normalized by `kind` to one ordinary day first.
/// Returns 0 if `ord_min <= 0` (defensive — misconfigured department).
pub fn work_pay_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (den * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Overtime pay at +50% premium (LOTTT Art. 118), composed MULTIPLICATIVELY
/// with the day's own premium (Art. 117 night +30%, Art. 120 rest-day +50%)
/// rather than additively (Important 2 fix).
///
/// The LOTTT reading is that Art. 117/120 load the value of the hour itself
/// (a night hour or a rest-day hour is worth more), and Art. 118's overtime
/// surcharge then applies to THAT already-loaded hour — not a flat +50% of
/// the base rate added on top of a separately-added day premium. An
/// overtime minute on a night shift therefore earns 1.5 × 1.3 = 1.95× the
/// base rate, not 1.5 + 0.3 = 1.8×; on a rest day, 1.5 × 1.5 = 2.25×, not
/// 1.5 + 0.5 = 2.0×.
///
/// `day_premium_pct` is the day-type rate as a percentage of the base rate
/// (100 = ordinary day, 130 = night per Art. 117, 150 = rest day per
/// Art. 120, 180 = both — additive stacking of the two day premiums,
/// mirroring how the ordinary-hour rate stacks them when both gates are
/// true; see `night_premium_cents`/`rest_day_surcharge_cents`, which are
/// unchanged and still additive for the ordinary-minutes slice). Passing
/// `day_premium_pct = 100` reproduces the flat 1.5× ordinary-day overtime
/// case exactly — that case is unchanged by this fix.
///
/// Formula: `ot_min × base_cents × 150 × day_premium_pct × num / (100 × 100 × den × ord_min)`
/// — still exactly one integer division at the end (money.rs:3-4): the two
/// premiums are multiplied together as numerators, never divided
/// separately and re-summed.
pub fn ot_pay_cents(
    ot_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
    day_premium_pct: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    ot_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(150))
        .and_then(|p| p.checked_mul(day_premium_pct))
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (100 * 100 * den * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Night premium = +30% ADDITIVE on top of work_pay (D-31, LOTTT Art. 117).
/// Caller is responsible for gating this on `daily_records.shift_type == "night"`
/// (W-6 — per-day actual shift, NOT departments.shift_type).
/// Formula: `work_min × base_cents × 30 / (100 × ord_min)`, normalized by `kind`.
///
/// Important 2: `work_minutes` here MUST be the ORDINARY slice only (exclude
/// any overtime minutes) when the day also has overtime. This function stays
/// additive on top of `work_pay_cents` for that ordinary slice — the
/// ordinary-hours case is unchanged by the Important 2 fix. The overtime
/// slice's share of the night premium is folded into `ot_pay_cents`'s
/// `day_premium_pct` instead (multiplicative, not summed on top of this).
pub fn night_premium_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(30))
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (100 * den * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Rest-day surcharge = +50% (D-03, LOTTT Art. 120).
/// Caller gates on `daily_records.is_rest_day_worked == 1`.
/// Formula: `work_min × base_cents × 50 / (100 × ord_min)`, normalized by `kind`.
///
/// Important 2: same rule as `night_premium_cents` above — `work_minutes`
/// must be the ORDINARY slice only when the day also has overtime; the
/// overtime slice's rest-day share is composed multiplicatively inside
/// `ot_pay_cents` via `day_premium_pct` instead.
pub fn rest_day_surcharge_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(50))
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (100 * den * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Late-arrival deduction = pro-rated salary on late minutes (D-05), normalized
/// by `kind`. Returned as a positive number; caller subtracts.
/// `total_a_pagar_cents` does the subtract.
pub fn late_deduction_cents(
    late_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    late_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (den * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Sum the components into the final per-row total. `late_deduction` is
/// subtracted (it is provided as a positive number). Saturating arithmetic so
/// astronomical inputs degrade gracefully rather than panic.
pub fn total_a_pagar_cents(
    work_pay: i64,
    ot_pay: i64,
    night_premium: i64,
    rest_day_surcharge: i64,
    late_deduction: i64,
) -> i64 {
    work_pay
        .saturating_add(ot_pay)
        .saturating_add(night_premium)
        .saturating_add(rest_day_surcharge)
        .saturating_sub(late_deduction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ----- Unit tests: anchor each formula to the reference example. -----

    #[test]
    fn work_pay_half_day() {
        // 240 min worked, $1000/day base, 480 min/day → $500.00
        assert_eq!(work_pay_cents(240, 100_000, 480, SalaryKind::Daily), 50_000);
    }

    #[test]
    fn work_pay_zero_minutes() {
        assert_eq!(work_pay_cents(0, 100_000, 480, SalaryKind::Daily), 0);
    }

    #[test]
    fn work_pay_full_day() {
        assert_eq!(work_pay_cents(480, 100_000, 480, SalaryKind::Daily), 100_000);
    }

    #[test]
    fn work_pay_misconfig_returns_zero() {
        // ord_min == 0 must not divide-by-zero
        assert_eq!(work_pay_cents(240, 100_000, 0, SalaryKind::Daily), 0);
        assert_eq!(work_pay_cents(240, 100_000, -1, SalaryKind::Daily), 0);
    }

    #[test]
    fn ot_pay_one_hour() {
        // 60 OT min at 1.5× over 480-min ordinary day at $1000, ordinary day
        // (day_premium_pct=100) → $187.50. Reproduces the old flat-1.5x
        // behavior exactly — the ordinary-day overtime case is unchanged.
        assert_eq!(
            ot_pay_cents(60, 100_000, 480, SalaryKind::Daily, 100),
            18_750
        );
    }

    #[test]
    fn ot_pay_zero() {
        assert_eq!(ot_pay_cents(0, 100_000, 480, SalaryKind::Daily, 100), 0);
    }

    #[test]
    fn ot_pay_misconfig_returns_zero() {
        assert_eq!(ot_pay_cents(60, 100_000, 0, SalaryKind::Daily, 100), 0);
    }

    // ----- Important 2: overtime composes MULTIPLICATIVELY with the day's
    // own premium, not additively. Hand-calculated against the task's
    // worked example: $50/day (5_000 cents), 480 min ordinary day, 60 min
    // (1h) overtime. -----

    #[test]
    fn ot_pay_night_composes_multiplicatively_not_additively() {
        // Art 118 (1.5x) x Art 117 night (1.3x) = 1.95x, NOT 1.5+0.3=1.8x.
        // 60 min x $50/day x 1.95 / 480 min = $12.1875 -> floors to 1_218 cents.
        assert_eq!(
            ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 130),
            1_218
        );
        // The additive-bug value (1.8x) would have been:
        // 60 x 5_000 x 180 / (100 x 480) = 1_125 cents. Confirm we do NOT
        // reproduce it.
        assert_ne!(ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 130), 1_125);
    }

    #[test]
    fn ot_pay_rest_day_composes_multiplicatively_not_additively() {
        // Art 118 (1.5x) x Art 120 rest day (1.5x) = 2.25x, NOT 1.5+0.5=2.0x.
        // 60 min x $50/day x 2.25 / 480 min = $14.0625 -> floors to 1_406 cents.
        assert_eq!(
            ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 150),
            1_406
        );
        // The additive-bug value (2.0x) would have been:
        // 60 x 5_000 x 200 / (100 x 480) = 1_250 cents. Confirm we do NOT
        // reproduce it.
        assert_ne!(ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 150), 1_250);
    }

    #[test]
    fn night_premium_full_shift() {
        // 480 min × $1000 × 30 / (100 × 480) = $300.00 (30% of $1000) — ADDITIVE per D-31
        assert_eq!(night_premium_cents(480, 100_000, 480, SalaryKind::Daily), 30_000);
    }

    #[test]
    fn night_premium_half_shift() {
        assert_eq!(night_premium_cents(240, 100_000, 480, SalaryKind::Daily), 15_000);
    }

    #[test]
    fn night_premium_misconfig_returns_zero() {
        assert_eq!(night_premium_cents(480, 100_000, 0, SalaryKind::Daily), 0);
    }

    #[test]
    fn rest_day_surcharge_full() {
        // 480 min × $1000 × 50 / (100 × 480) = $500.00 (+50%)
        assert_eq!(rest_day_surcharge_cents(480, 100_000, 480, SalaryKind::Daily), 50_000);
    }

    #[test]
    fn rest_day_surcharge_zero() {
        assert_eq!(rest_day_surcharge_cents(0, 100_000, 480, SalaryKind::Daily), 0);
    }

    #[test]
    fn rest_day_surcharge_misconfig_returns_zero() {
        assert_eq!(rest_day_surcharge_cents(480, 100_000, 0, SalaryKind::Daily), 0);
        assert_eq!(rest_day_surcharge_cents(480, 100_000, -1, SalaryKind::Daily), 0);
    }

    #[test]
    fn late_deduction_quarter_hour() {
        // 15 late min × $1000 / 480 = $31.25
        assert_eq!(late_deduction_cents(15, 100_000, 480, SalaryKind::Daily), 3_125);
    }

    #[test]
    fn late_deduction_zero() {
        assert_eq!(late_deduction_cents(0, 100_000, 480, SalaryKind::Daily), 0);
    }

    #[test]
    fn late_deduction_misconfig_returns_zero() {
        assert_eq!(late_deduction_cents(15, 100_000, 0, SalaryKind::Daily), 0);
        assert_eq!(late_deduction_cents(15, 100_000, -1, SalaryKind::Daily), 0);
    }

    #[test]
    fn total_a_pagar_composition() {
        // 50_000 + 18_750 + 30_000 + 0 - 3_125 = 95_625
        assert_eq!(
            total_a_pagar_cents(50_000, 18_750, 30_000, 0, 3_125),
            95_625
        );
    }

    #[test]
    fn total_a_pagar_with_rest_day() {
        // work=full day + rest day surcharge, no OT, no night, no late
        assert_eq!(total_a_pagar_cents(100_000, 0, 0, 50_000, 0), 150_000);
    }

    // ----- Important 2: whole-day totals, hand-calculated from the task's
    // worked example ($50/day, 8h ordinary + 1h overtime). Composes
    // work_pay (ordinary slice only) + night/rest premium (ordinary slice
    // only) + the multiplicatively-composed ot_pay, exactly the way
    // reports/service.rs now calls these three functions. -----

    #[test]
    fn whole_day_total_night_shift_with_overtime() {
        // $50/day = 5_000 cents; 480 min ordinary day; 60 min (1h) overtime.
        // Ordinary 480 min: work_pay=5_000, night_premium(ordinary-only)=1_500
        //   -> 6_500 (= 1.3x of 5_000, the unchanged ordinary-hours case).
        // Overtime 60 min at 1.5 x 1.3 = 1.95x -> 1_218 (floors from 1218.75).
        // Total = 5_000 + 1_500 + 1_218 = 7_718 cents ($77.18).
        // Exact (pre-floor) value is $77.1875 (7_718.75 cents) — matches the
        // reviewer's target; the implementation floors the OT component once.
        let work = work_pay_cents(480, 5_000, 480, SalaryKind::Daily);
        let night = night_premium_cents(480, 5_000, 480, SalaryKind::Daily);
        let ot = ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 130);
        assert_eq!((work, night, ot), (5_000, 1_500, 1_218));
        let total = total_a_pagar_cents(work, ot, night, 0, 0);
        assert_eq!(total, 7_718);
        assert_ne!(total, 7_624, "additive composition — the Important 2 defect");
    }

    #[test]
    fn whole_day_total_rest_day_with_overtime() {
        // Same $50/day / 480 ordinary / 60 OT setup, rest day instead of night.
        // Ordinary 480 min: work_pay=5_000, rest_day_surcharge(ordinary-only)=2_500
        //   -> 7_500 (= 1.5x of 5_000, the unchanged ordinary-hours case).
        // Overtime 60 min at 1.5 x 1.5 = 2.25x -> 1_406 (floors from 1406.25).
        // Total = 5_000 + 2_500 + 1_406 = 8_906 cents ($89.06).
        // Exact (pre-floor) value is $89.0625 (8_906.25 cents) — matches the
        // reviewer's target.
        let work = work_pay_cents(480, 5_000, 480, SalaryKind::Daily);
        let rest = rest_day_surcharge_cents(480, 5_000, 480, SalaryKind::Daily);
        let ot = ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 150);
        assert_eq!((work, rest, ot), (5_000, 2_500, 1_406));
        let total = total_a_pagar_cents(work, ot, 0, rest, 0);
        assert_eq!(total, 8_906);
        assert_ne!(total, 8_749, "additive composition — the Important 2 defect");
    }

    // ----- Property tests: structural guarantees over realistic input ranges. -----

    proptest! {
        /// Monotonicity: more work minutes → at least as much pay (never regress).
        #[test]
        fn work_pay_monotonic(
            b in 0i64..10_000_000_000,
            o in 360i64..600,
            m1 in 0i64..43200,
            m2 in 0i64..43200,
        ) {
            let lo = m1.min(m2);
            let hi = m1.max(m2);
            prop_assert!(work_pay_cents(lo, b, o, SalaryKind::Daily) <= work_pay_cents(hi, b, o, SalaryKind::Daily));
        }

        /// No panic on plausible inputs. 10k cases covers the realistic range
        /// (work 0..43200, base up to $10M/day cents, ord 360..600 min), across
        /// all three `SalaryKind` variants so `to_daily`'s Hourly/Monthly
        /// branches also get overflow-safety coverage, not just Daily.
        #[test]
        fn no_panic_on_random_inputs(
            work in 0i64..43200,
            ot in 0i64..43200,
            late in 0i64..43200,
            base in 0i64..10_000_000_000,
            ord in 360i64..600,
            kind_idx in 0u8..3,
            // Important 2: 100 (ordinary day) | 130 (night) | 150 (rest day) |
            // 180 (both) — the only values reports/service.rs ever passes.
            day_premium_pct in prop_oneof![Just(100i64), Just(130i64), Just(150i64), Just(180i64)],
        ) {
            let kind = match kind_idx {
                0 => SalaryKind::Hourly,
                1 => SalaryKind::Daily,
                _ => SalaryKind::Monthly,
            };
            // Must not panic on any of the six functions.
            let w = work_pay_cents(work, base, ord, kind);
            let o = ot_pay_cents(ot, base, ord, kind, day_premium_pct);
            let n = night_premium_cents(work, base, ord, kind);
            let r = rest_day_surcharge_cents(work, base, ord, kind);
            let l = late_deduction_cents(late, base, ord, kind);
            let _t = total_a_pagar_cents(w, o, n, r, l);
        }
    }
}
