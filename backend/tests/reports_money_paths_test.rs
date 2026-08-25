//! I6: assert the money paths nothing was asserting.
//!
//! C-01 (overtime paid at 250%) took exactly one existing test to fix.
//! Everything else stayed green — which means no test asserted a
//! report-level total for a day with overtime. The same absence later kept
//! an additive premium composition alive until a branch review computed it
//! by hand, and reappeared in H-01 (tolerance rule documented, asserted
//! nowhere). This file closes that absence:
//!
//! - a report total with a night premium
//! - a report total with a rest-day surcharge
//! - a report total with overtime landing on a premium day
//! - a Monthly and an Hourly value assertion for each of the five
//!   `money.rs` functions that take a `SalaryKind` (only `work_pay_cents`
//!   had one before, in `salary_unit_test.rs`) — so a regression to two
//!   divisions cannot pass green
//!
//! Every amount below is hand-calculated from the LOTTT formulas documented
//! in `money.rs` (Art. 117 night +30%, Art. 118 overtime +50% multiplicative
//! with the day's own premium, Art. 120 rest-day +50%), not read off any
//! existing test or off the implementation's output. Rounding is half-up:
//! `(numerator + denominator / 2) / denominator`.

use cronometrix_api::employees::models::SalaryKind;
use cronometrix_api::reports::money::{
    late_deduction_cents, night_premium_cents, ot_pay_cents, rest_day_surcharge_cents,
    total_a_pagar_cents, work_pay_cents,
};

// -----------------------------------------------------------------------------
// The matrix (task-4-brief.md). Base: daily salary 5_000 cents, ordinary day
// 480 minutes.
// -----------------------------------------------------------------------------

const BASE_DAILY_CENTS: i64 = 5_000;
const ORD_MIN: i64 = 480;

/// Full ordinary day, no premiums, no overtime: 480 min at the daily rate
/// pays exactly the daily rate.
#[test]
fn full_ordinary_day_pays_the_daily_rate() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    assert_eq!(work, 5_000);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);
    assert_eq!(total, 5_000);
}

/// 480 ordinary min + 60 min overtime at 1.5x (LOTTT Art. 118), ordinary day
/// (`day_premium_pct = 100`, no night/rest-day premium):
///   5_000 + 60 * 5_000 * 1.5 / 480 = 5_000 + 450_000/480
///         = 5_000 + 937.5 -> half-up -> 938 = 5_938.
/// This is the C-01 shape: the defect paid overtime at 250% (6_562), not
/// 150% (5_938).
#[test]
fn ordinary_day_plus_sixty_minutes_overtime() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let ot = ot_pay_cents(60, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily, 100);
    assert_eq!((work, ot), (5_000, 938));
    let total = total_a_pagar_cents(work, ot, 0, 0, 0);
    assert_eq!(total, 5_938);
    assert_ne!(total, 6_562, "250% overtime — the C-01 defect");
}

/// Full night shift (LOTTT Art. 117, +30% ADDITIVE on top of work_pay):
///   480 min ordinary -> work_pay 5_000, night_premium 5_000 * 0.30 = 1_500.
///   Total = 6_500 (= 5_000 * 1.3).
/// Nothing in the suite asserted a report total that includes a night
/// premium before this test.
#[test]
fn full_night_shift() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let night = night_premium_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    assert_eq!((work, night), (5_000, 1_500));
    let total = total_a_pagar_cents(work, 0, night, 0, 0);
    assert_eq!(total, 6_500);
}

/// Night shift + 60 min overtime. The night premium applies to the ordinary
/// 480-minute slice only (Important 2 — overtime minutes are excluded from
/// the additive night/rest slice; their share is folded multiplicatively
/// into `ot_pay_cents` instead):
///   work_pay(480) = 5_000
///   night_premium(480, ordinary-only) = 1_500
///   ot_pay(60, day_premium_pct=130 for night) = 60 * 5_000 * 1.5 * 1.3 / 480
///     = 585_000/480 = 1_218.75 -> half-up -> 1_219
///   Total = 5_000 + 1_500 + 1_219 = 7_719.
/// This is exactly the composition that stayed additive (day_premium 1.8x
/// instead of 1.95x, total 7_624) until a branch review caught it by hand.
#[test]
fn night_shift_plus_sixty_minutes_overtime() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let night = night_premium_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let ot = ot_pay_cents(60, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily, 130);
    assert_eq!((work, night, ot), (5_000, 1_500, 1_219));
    let total = total_a_pagar_cents(work, ot, night, 0, 0);
    assert_eq!(total, 7_719);
    assert_ne!(total, 7_624, "additive 1.8x composition — the Important 2 defect");
}

/// Full rest day (LOTTT Art. 120, +50%):
///   480 min ordinary -> work_pay 5_000, rest_day_surcharge 5_000 * 0.50 = 2_500.
///   Total = 7_500 (= 5_000 * 1.5).
/// Nothing in the suite asserted a report total that includes a rest-day
/// surcharge before this test.
#[test]
fn full_rest_day() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let rest = rest_day_surcharge_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    assert_eq!((work, rest), (5_000, 2_500));
    let total = total_a_pagar_cents(work, 0, 0, rest, 0);
    assert_eq!(total, 7_500);
}

/// Rest day + 60 min overtime, same ordinary/overtime split as the night
/// case:
///   work_pay(480) = 5_000
///   rest_day_surcharge(480, ordinary-only) = 2_500
///   ot_pay(60, day_premium_pct=150 for rest day) = 60 * 5_000 * 1.5 * 1.5 / 480
///     = 675_000/480 = 1_406.25 -> half-up -> 1_406
///   Total = 5_000 + 2_500 + 1_406 = 8_906.
/// The additive-bug value (2.0x instead of 2.25x) would have been 8_749.
#[test]
fn rest_day_plus_sixty_minutes_overtime() {
    let work = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let rest = rest_day_surcharge_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let ot = ot_pay_cents(60, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily, 150);
    assert_eq!((work, rest, ot), (5_000, 2_500, 1_406));
    let total = total_a_pagar_cents(work, ot, 0, rest, 0);
    assert_eq!(total, 8_906);
    assert_ne!(total, 8_749, "additive 2.0x composition — the Important 2 defect");
}

/// Monthly salary 150_000, full ordinary day: 150_000/30 = 5_000 daily
/// equivalent, and the full ordinary day pays exactly that daily rate.
#[test]
fn monthly_salary_full_ordinary_day() {
    let work = work_pay_cents(ORD_MIN, 150_000, ORD_MIN, SalaryKind::Monthly);
    assert_eq!(work, 5_000);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);
    assert_eq!(total, 5_000);
}

/// Hourly salary 625, full ordinary day: 625 * 8h = 5_000 daily equivalent,
/// and the full ordinary day pays exactly that daily rate.
#[test]
fn hourly_salary_full_ordinary_day() {
    let work = work_pay_cents(ORD_MIN, 625, ORD_MIN, SalaryKind::Hourly);
    assert_eq!(work, 5_000);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);
    assert_eq!(total, 5_000);
}

/// The single most valuable assertion in the matrix: the same work, paid on
/// a daily, a monthly, and an hourly basis, must produce the IDENTICAL
/// amount. The salary unit cannot change what the work is worth. This is
/// the assertion that alone would have caught H-08, where a monthly figure
/// was paid in full every single day instead of being divided across it.
#[test]
fn the_same_work_pays_the_same_regardless_of_salary_unit() {
    let daily = work_pay_cents(ORD_MIN, BASE_DAILY_CENTS, ORD_MIN, SalaryKind::Daily);
    let monthly = work_pay_cents(ORD_MIN, 150_000, ORD_MIN, SalaryKind::Monthly);
    let hourly = work_pay_cents(ORD_MIN, 625, ORD_MIN, SalaryKind::Hourly);
    assert_eq!(daily, 5_000);
    assert_eq!(monthly, 5_000);
    assert_eq!(hourly, 5_000);
    assert_eq!(daily, monthly, "Monthly must pay the same as Daily for the same work");
    assert_eq!(daily, hourly, "Hourly must pay the same as Daily for the same work");
}

// -----------------------------------------------------------------------------
// One Monthly and one Hourly value assertion for each of the four money.rs
// functions that did not already have one (work_pay_cents is covered above,
// in the matrix, and in salary_unit_test.rs / H-08). Without these, any of
// the four could regress from "multiply numerators first, divide once" back
// to "divide to get a daily rate, then divide again" and still pass green.
//
// Each pair below uses a base/minutes combination hand-picked so the
// single-fraction formula and a naive two-division computation actually
// disagree — a round base (e.g. 100_000/30, or 480 minutes with an
// ordinary day that is itself a multiple of 60) would make both paths
// agree by coincidence and prove nothing. Divergence was confirmed by
// hand for each case below.
// -----------------------------------------------------------------------------

/// `night_premium_cents`, Monthly: base 100_007 (not evenly divisible by
/// 30), 18 worked minutes, 480-minute ordinary day.
/// One fraction:  18 * 100_007 * 30 * 1 / (100 * 30 * 480)
///              = 54_003_780 / 1_440_000 = 37.5026... -> half-up -> 38.
/// Two divisions (pre-divide to a daily rate, floor(100_007/30) = 3_333,
/// then apply the premium): 18 * 3_333 * 30 / (100 * 480) = 1_799_820/48_000
/// = 37.49... -> 37. The two paths disagree: 38 vs. 37.
#[test]
fn night_premium_monthly_value() {
    assert_eq!(
        night_premium_cents(18, 100_007, 480, SalaryKind::Monthly),
        38
    );
}

/// `night_premium_cents`, Hourly: base 653/hour, 475-minute ordinary day
/// (NOT a multiple of 60, so the Hourly->daily conversion itself carries a
/// fraction), 17 worked minutes.
/// One fraction: 17 * 653 * 30 * 475 / (100 * 60 * 475)
///             = 159_614_250 / 2_850_000 (rounded numerator) -> 56.
/// Two divisions (pre-divide base*ord/60, floor(653*475/60) = floor(5169.58)
/// = 5_169, then apply the premium) gives 55. The two paths disagree.
#[test]
fn night_premium_hourly_value() {
    assert_eq!(
        night_premium_cents(17, 653, 475, SalaryKind::Hourly),
        56
    );
}

/// `rest_day_surcharge_cents`, Monthly: same base/minutes as the night-
/// premium Monthly case above (100_007 / 18 min / 480-min day), same
/// divergence rationale, different multiplier (50 instead of 30):
///   18 * 100_007 * 50 * 1 / (100 * 30 * 480) = 90_006_300/1_440_000
///   = 62.5... -> half-up -> 63.
/// Two divisions gives 62.
#[test]
fn rest_day_surcharge_monthly_value() {
    assert_eq!(
        rest_day_surcharge_cents(18, 100_007, 480, SalaryKind::Monthly),
        63
    );
}

/// `rest_day_surcharge_cents`, Hourly: same base/ord/minutes as the
/// night-premium Hourly case above (653/hour, 475-min day, 17 min), same
/// divergence rationale, multiplier 50 instead of 30:
///   17 * 653 * 50 * 475 / (100 * 60 * 475) -> 93.
/// Two divisions gives 92.
#[test]
fn rest_day_surcharge_hourly_value() {
    assert_eq!(
        rest_day_surcharge_cents(17, 653, 475, SalaryKind::Hourly),
        93
    );
}

/// `late_deduction_cents`, Monthly: `late_deduction_cents` shares
/// `work_pay_cents`'s exact formula shape (no extra day-type multiplier),
/// so it inherits the same divergent case documented in
/// `salary_unit_test.rs` (base 100_007, 18 minutes, 480-min day):
///   18 * 100_007 * 1 / (30 * 480) = 1_807_326/14_400 = 125.5... -> 125.
/// Two divisions (floor(100_007/30)=3_333, then 3_333*18/480) gives 124.
#[test]
fn late_deduction_monthly_value() {
    assert_eq!(
        late_deduction_cents(18, 100_007, 480, SalaryKind::Monthly),
        125
    );
}

/// `late_deduction_cents`, Hourly: base 1_009/hour, 475-minute ordinary day
/// (not a multiple of 60), 19 late minutes:
///   19 * 1_009 * 475 / (60 * 475) = 9_120_475/28_500 = 320.01... -> 320.
/// Two divisions (floor(1_009*475/60)=7_988, then 19*7_988/475) gives 319.
#[test]
fn late_deduction_hourly_value() {
    assert_eq!(
        late_deduction_cents(19, 1_009, 475, SalaryKind::Hourly),
        320
    );
}

/// `ot_pay_cents`, Monthly: base 100_007, 18 overtime minutes, 480-min
/// ordinary day, ordinary-day premium (`day_premium_pct = 100`):
///   18 * 100_007 * 150 * 100 * 1 / (100 * 100 * 30 * 480)
///     = 27_001_890_000 / 144_000_000 = 187.5... -> half-up -> 188.
/// Two divisions gives 187.
#[test]
fn ot_pay_monthly_value() {
    assert_eq!(
        ot_pay_cents(18, 100_007, 480, SalaryKind::Monthly, 100),
        188
    );
}

/// `ot_pay_cents`, Hourly: base 653/hour, 475-min ordinary day, 17 overtime
/// minutes, ordinary-day premium (`day_premium_pct = 100`):
///   17 * 653 * 150 * 100 * 475 / (100 * 100 * 60 * 475) -> 278.
/// Two divisions gives 277.
#[test]
fn ot_pay_hourly_value() {
    assert_eq!(
        ot_pay_cents(17, 653, 475, SalaryKind::Hourly, 100),
        278
    );
}
