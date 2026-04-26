//! Period boundary math (D-08..D-10). Pure — no I/O, no DB.
//!
//! Venezuela target market = `America/Caracas` (no DST), so we work in
//! `chrono::NaiveDate` exclusively. ISO 8601 weekly boundary (Mon–Sun).
//! Calendar bi-weekly (1–15 / 16–EOM, VE payroll convention).

use chrono::{Datelike, Duration, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeriodPreset {
    /// ISO 8601 Monday–Sunday containing `ref_date`.
    Weekly,
    /// 1st through 15th of `ref_date`'s month.
    BiweeklyFirst,
    /// 16th through end-of-month of `ref_date`'s month.
    BiweeklySecond,
    /// 1st through end-of-month of `ref_date`'s month.
    Monthly,
    /// Operator-supplied `(from, to)` passed through verbatim.
    Custom(NaiveDate, NaiveDate),
}

/// Resolve a preset + reference date to a concrete `(from, to)` inclusive range.
pub fn resolve_period(preset: PeriodPreset, ref_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    match preset {
        PeriodPreset::Weekly => {
            let dow = ref_date.weekday().num_days_from_monday() as i64;
            let mon = ref_date - Duration::days(dow);
            let sun = mon + Duration::days(6);
            (mon, sun)
        }
        PeriodPreset::BiweeklyFirst => {
            let first = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 1).unwrap();
            let fifteenth =
                NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 15).unwrap();
            (first, fifteenth)
        }
        PeriodPreset::BiweeklySecond => {
            let sixteenth =
                NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 16).unwrap();
            let eom = last_day_of_month(ref_date.year(), ref_date.month());
            (sixteenth, eom)
        }
        PeriodPreset::Monthly => {
            let first = NaiveDate::from_ymd_opt(ref_date.year(), ref_date.month(), 1).unwrap();
            let eom = last_day_of_month(ref_date.year(), ref_date.month());
            (first, eom)
        }
        PeriodPreset::Custom(from, to) => (from, to),
    }
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap() - Duration::days(1)
}

/// Parse a `period_type` string + ISO `from_date`/`to_date` into a `PeriodPreset`.
/// Returns `AppError::Validation` (→ HTTP 422) for unknown strings or bad dates.
pub fn parse_period(
    period_type: &str,
    from_date: &str,
    to_date: &str,
) -> Result<PeriodPreset, crate::errors::AppError> {
    use crate::errors::AppError;
    let from = NaiveDate::parse_from_str(from_date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be YYYY-MM-DD".to_string(),
        }
    })?;
    let to = NaiveDate::parse_from_str(to_date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "to_date must be YYYY-MM-DD".to_string(),
        }
    })?;
    match period_type {
        "weekly" => Ok(PeriodPreset::Weekly),
        "biweekly_first" => Ok(PeriodPreset::BiweeklyFirst),
        "biweekly_second" => Ok(PeriodPreset::BiweeklySecond),
        "monthly" => Ok(PeriodPreset::Monthly),
        "custom" => Ok(PeriodPreset::Custom(from, to)),
        other => Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: format!("Unknown period_type: {}", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn weekly_wraps_through_iso_week() {
        // 2026-04-25 is a Saturday. ISO Mon = 2026-04-20, Sun = 2026-04-26.
        let (f, t) = resolve_period(PeriodPreset::Weekly, d(2026, 4, 25));
        assert_eq!(f, d(2026, 4, 20));
        assert_eq!(t, d(2026, 4, 26));
    }

    #[test]
    fn weekly_anchored_on_monday_returns_same_week() {
        // 2026-04-20 is Monday — should return (2026-04-20, 2026-04-26).
        let (f, t) = resolve_period(PeriodPreset::Weekly, d(2026, 4, 20));
        assert_eq!(f, d(2026, 4, 20));
        assert_eq!(t, d(2026, 4, 26));
    }

    #[test]
    fn biweekly_first_april() {
        let (f, t) = resolve_period(PeriodPreset::BiweeklyFirst, d(2026, 4, 10));
        assert_eq!(f, d(2026, 4, 1));
        assert_eq!(t, d(2026, 4, 15));
    }

    #[test]
    fn biweekly_february_leap_year() {
        let (f, t) = resolve_period(PeriodPreset::BiweeklySecond, d(2024, 2, 20));
        assert_eq!(f, d(2024, 2, 16));
        assert_eq!(t, d(2024, 2, 29));
    }

    #[test]
    fn biweekly_february_non_leap_year() {
        let (f, t) = resolve_period(PeriodPreset::BiweeklySecond, d(2026, 2, 20));
        assert_eq!(f, d(2026, 2, 16));
        assert_eq!(t, d(2026, 2, 28));
    }

    #[test]
    fn monthly_february_leap_year() {
        let (f, t) = resolve_period(PeriodPreset::Monthly, d(2024, 2, 5));
        assert_eq!(f, d(2024, 2, 1));
        assert_eq!(t, d(2024, 2, 29));
    }

    #[test]
    fn monthly_april() {
        let (f, t) = resolve_period(PeriodPreset::Monthly, d(2026, 4, 15));
        assert_eq!(f, d(2026, 4, 1));
        assert_eq!(t, d(2026, 4, 30));
    }

    #[test]
    fn monthly_december_handles_year_rollover() {
        // EOM(Dec 2026) calls last_day_of_month, which rolls year forward.
        let (f, t) = resolve_period(PeriodPreset::Monthly, d(2026, 12, 5));
        assert_eq!(f, d(2026, 12, 1));
        assert_eq!(t, d(2026, 12, 31));
    }

    #[test]
    fn custom_passthrough() {
        let from = d(2025, 1, 1);
        let to = d(2025, 1, 31);
        let (f, t) = resolve_period(PeriodPreset::Custom(from, to), d(1999, 1, 1));
        assert_eq!(f, from);
        assert_eq!(t, to);
    }

    #[test]
    fn parse_period_known_strings() {
        assert!(matches!(
            parse_period("weekly", "2026-04-01", "2026-04-30").unwrap(),
            PeriodPreset::Weekly
        ));
        assert!(matches!(
            parse_period("biweekly_first", "2026-04-01", "2026-04-30").unwrap(),
            PeriodPreset::BiweeklyFirst
        ));
        assert!(matches!(
            parse_period("biweekly_second", "2026-04-01", "2026-04-30").unwrap(),
            PeriodPreset::BiweeklySecond
        ));
        assert!(matches!(
            parse_period("monthly", "2026-04-01", "2026-04-30").unwrap(),
            PeriodPreset::Monthly
        ));
        assert!(matches!(
            parse_period("custom", "2026-04-01", "2026-04-30").unwrap(),
            PeriodPreset::Custom(_, _)
        ));
    }

    #[test]
    fn parse_period_unknown_returns_validation_error() {
        let err = parse_period("yearly", "2026-04-01", "2026-04-30").unwrap_err();
        match err {
            crate::errors::AppError::Validation { code, .. } => {
                assert_eq!(code, "VALIDATION_ERROR");
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn parse_period_bad_date_returns_validation_error() {
        let err = parse_period("monthly", "not-a-date", "2026-04-30").unwrap_err();
        match err {
            crate::errors::AppError::Validation { code, .. } => {
                assert_eq!(code, "VALIDATION_ERROR");
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }
}
