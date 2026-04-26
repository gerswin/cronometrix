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

/// Pro-rated salary for the worked minutes: `work_min × base_cents / ord_min`.
/// Returns 0 if `ord_min <= 0` (defensive — misconfigured department).
pub fn work_pay_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    work_minutes
        .checked_mul(base_salary_cents)
        .map(|p| p / ordinary_daily_minutes)
        .unwrap_or(0)
}

/// Overtime pay at +50% premium (LOTTT Art. 118):
/// `ot_min × base_cents × 150 / (100 × ord_min)`.
pub fn ot_pay_cents(
    ot_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    ot_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(150))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Night premium = +30% ADDITIVE on top of work_pay (D-31, LOTTT Art. 117).
/// Caller is responsible for gating this on `daily_records.shift_type == "night"`
/// (W-6 — per-day actual shift, NOT departments.shift_type).
/// Formula: `work_min × base_cents × 30 / (100 × ord_min)`.
pub fn night_premium_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(30))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Rest-day surcharge = +50% (D-03, LOTTT Art. 120).
/// Caller gates on `daily_records.is_rest_day_worked == 1`.
/// Formula: `work_min × base_cents × 50 / (100 × ord_min)`.
pub fn rest_day_surcharge_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(50))
        .map(|p| p / (100 * ordinary_daily_minutes))
        .unwrap_or(0)
}

/// Late-arrival deduction = pro-rated salary on late minutes (D-05). Returned as
/// a positive number; caller subtracts. `total_a_pagar_cents` does the subtract.
pub fn late_deduction_cents(
    late_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    late_minutes
        .checked_mul(base_salary_cents)
        .map(|p| p / ordinary_daily_minutes)
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
        assert_eq!(work_pay_cents(240, 100_000, 480), 50_000);
    }

    #[test]
    fn work_pay_zero_minutes() {
        assert_eq!(work_pay_cents(0, 100_000, 480), 0);
    }

    #[test]
    fn work_pay_full_day() {
        assert_eq!(work_pay_cents(480, 100_000, 480), 100_000);
    }

    #[test]
    fn work_pay_misconfig_returns_zero() {
        // ord_min == 0 must not divide-by-zero
        assert_eq!(work_pay_cents(240, 100_000, 0), 0);
        assert_eq!(work_pay_cents(240, 100_000, -1), 0);
    }

    #[test]
    fn ot_pay_one_hour() {
        // 60 OT min at 1.5× over 480-min ordinary day at $1000 → $187.50
        assert_eq!(ot_pay_cents(60, 100_000, 480), 18_750);
    }

    #[test]
    fn ot_pay_zero() {
        assert_eq!(ot_pay_cents(0, 100_000, 480), 0);
    }

    #[test]
    fn ot_pay_misconfig_returns_zero() {
        assert_eq!(ot_pay_cents(60, 100_000, 0), 0);
    }

    #[test]
    fn night_premium_full_shift() {
        // 480 min × $1000 × 30 / (100 × 480) = $300.00 (30% of $1000) — ADDITIVE per D-31
        assert_eq!(night_premium_cents(480, 100_000, 480), 30_000);
    }

    #[test]
    fn night_premium_half_shift() {
        assert_eq!(night_premium_cents(240, 100_000, 480), 15_000);
    }

    #[test]
    fn night_premium_misconfig_returns_zero() {
        assert_eq!(night_premium_cents(480, 100_000, 0), 0);
    }

    #[test]
    fn rest_day_surcharge_full() {
        // 480 min × $1000 × 50 / (100 × 480) = $500.00 (+50%)
        assert_eq!(rest_day_surcharge_cents(480, 100_000, 480), 50_000);
    }

    #[test]
    fn rest_day_surcharge_zero() {
        assert_eq!(rest_day_surcharge_cents(0, 100_000, 480), 0);
    }

    #[test]
    fn late_deduction_quarter_hour() {
        // 15 late min × $1000 / 480 = $31.25
        assert_eq!(late_deduction_cents(15, 100_000, 480), 3_125);
    }

    #[test]
    fn late_deduction_zero() {
        assert_eq!(late_deduction_cents(0, 100_000, 480), 0);
    }

    #[test]
    fn total_a_pagar_composition() {
        // 50_000 + 18_750 + 30_000 + 0 - 3_125 = 95_625
        assert_eq!(total_a_pagar_cents(50_000, 18_750, 30_000, 0, 3_125), 95_625);
    }

    #[test]
    fn total_a_pagar_with_rest_day() {
        // work=full day + rest day surcharge, no OT, no night, no late
        assert_eq!(total_a_pagar_cents(100_000, 0, 0, 50_000, 0), 150_000);
    }

    // ----- Property tests: structural guarantees over realistic input ranges. -----

    proptest! {
        /// Monotonicity: more work minutes → at least as much pay (never regress).
        #[test]
        fn work_pay_monotonic(
            b in 0i64..100_000_000_00,
            o in 360i64..600,
            m1 in 0i64..43200,
            m2 in 0i64..43200,
        ) {
            let lo = m1.min(m2);
            let hi = m1.max(m2);
            prop_assert!(work_pay_cents(lo, b, o) <= work_pay_cents(hi, b, o));
        }

        /// No panic on plausible inputs. 10k cases covers the realistic range
        /// (work 0..43200, base up to $10M/day cents, ord 360..600 min).
        #[test]
        fn no_panic_on_random_inputs(
            work in 0i64..43200,
            ot in 0i64..43200,
            late in 0i64..43200,
            base in 0i64..100_000_000_00,
            ord in 360i64..600,
        ) {
            // Must not panic on any of the six functions.
            let w = work_pay_cents(work, base, ord);
            let o = ot_pay_cents(ot, base, ord);
            let n = night_premium_cents(work, base, ord);
            let r = rest_day_surcharge_cents(work, base, ord);
            let l = late_deduction_cents(late, base, ord);
            let _t = total_a_pagar_cents(w, o, n, r, l);
        }
    }
}
