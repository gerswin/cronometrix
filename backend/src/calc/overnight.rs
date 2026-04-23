//! Overnight-aware shift window construction. Handles D-05 (anchor=start date),
//! D-06 (opt-in flag), and D-08 (DST-safe `.earliest()` path with anomaly flag
//! for future DST-capable markets; dead code in Venezuela).
//!
//! Plan 03-02 extends Plan 03-01's same-day `shift_window()` to support
//! overnight shifts (e.g., 22:00 → 06:00) where `end_date = anchor_date + 1`.
//! The local→epoch resolution path uses `.earliest()` instead of a panicking
//! `.single()` + unwrap so a future DST-observing market (e.g., if Colombia
//! ever re-adopts DST) cannot panic the calc thread on a fall-back / spring-
//! forward boundary — the caller receives an `ambiguous=true` flag instead
//! and the engine emits `AnomalyCode::OvernightInferenceAmbiguous`.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Tz;

use super::models::{DepartmentConfig, GlobalRulesRow};

/// Resolve a `NaiveDateTime` in a given TZ to a UTC epoch.
///
/// Returns `(epoch, ambiguous)` where `ambiguous=true` indicates the local
/// datetime fell in a DST fall-back (ambiguous) or spring-forward gap
/// (nonexistent) boundary.
///
/// Uses `.earliest()` (safe for every `LocalResult` variant):
///   - `Single(dt)`     → `(dt.timestamp(), false)`
///   - `Ambiguous(e,_)` → `(e.timestamp(), true)` — pick earliest occurrence
///   - `None`           → spring-forward gap; try `ndt + 1h` then `ndt + 2h`.
///                         If still None, fall back to interpreting `ndt` as
///                         UTC and mark ambiguous. The emitted anomaly
///                         surfaces the degenerate case to the operator.
pub fn resolve_local_epoch(tz: Tz, ndt: NaiveDateTime) -> (i64, bool) {
    use chrono::LocalResult;
    match tz.from_local_datetime(&ndt) {
        LocalResult::Single(dt) => (dt.timestamp(), false),
        LocalResult::Ambiguous(earliest, _latest) => (earliest.timestamp(), true),
        LocalResult::None => {
            // Spring-forward gap: try one hour forward, then two.
            let bump1 = ndt + chrono::Duration::hours(1);
            match tz.from_local_datetime(&bump1) {
                LocalResult::Single(dt) => (dt.timestamp(), true),
                LocalResult::Ambiguous(e, _) => (e.timestamp(), true),
                LocalResult::None => {
                    let bump2 = ndt + chrono::Duration::hours(2);
                    match tz.from_local_datetime(&bump2) {
                        LocalResult::Single(dt) => (dt.timestamp(), true),
                        LocalResult::Ambiguous(e, _) => (e.timestamp(), true),
                        LocalResult::None => {
                            // Extreme degenerate case: treat as UTC. Emitted
                            // anomaly lets operator notice & manually review.
                            (ndt.and_utc().timestamp(), true)
                        }
                    }
                }
            }
        }
    }
}

/// Returns `(window_start_epoch, window_end_epoch, nominal_shift_start_epoch,
/// nominal_shift_end_epoch, ambiguous)`.
///
/// - For `dept.is_overnight_shift = false`: `end_date = anchor_date` (same-day shift).
/// - For `dept.is_overnight_shift = true`:  `end_date = anchor_date.succ_opt()`
///   (crosses midnight; D-05 anchor rule).
///
/// All epochs are UTC seconds. `ambiguous = true` iff either the nominal
/// shift_start or shift_end landed on a DST boundary (spring-forward gap or
/// fall-back ambiguity). In Venezuela / America/Caracas this is always false.
pub fn shift_window_overnight_aware(
    anchor_date: NaiveDate,
    dept: &DepartmentConfig,
    rules: &GlobalRulesRow,
    tz: Tz,
) -> (i64, i64, i64, i64, bool) {
    let shift_start = NaiveTime::parse_from_str(&dept.shift_start_time, "%H:%M")
        .expect("dept.shift_start_time must be HH:MM — validated at department create time");
    let shift_end = NaiveTime::parse_from_str(&dept.shift_end_time, "%H:%M")
        .expect("dept.shift_end_time must be HH:MM");

    let start_local = anchor_date.and_time(shift_start);
    let end_date = if dept.is_overnight_shift {
        anchor_date
            .succ_opt()
            .expect("NaiveDate::succ_opt returns Some for realistic calendar dates")
    } else {
        anchor_date
    };
    let end_local = end_date.and_time(shift_end);

    let (nominal_start, amb1) = resolve_local_epoch(tz, start_local);
    let (nominal_end, amb2) = resolve_local_epoch(tz, end_local);

    let tol_before_s = (rules.late_arrival_tolerance_min + rules.bonus_minutes) * 60;
    let tol_after_s = (rules.early_departure_tolerance_min + rules.bonus_minutes) * 60;
    let window_start = nominal_start - tol_before_s;
    let window_end = nominal_end + tol_after_s;

    (
        window_start,
        window_end,
        nominal_start,
        nominal_end,
        amb1 || amb2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::America::Caracas;

    fn dept(start: &str, end: &str, overnight: bool) -> DepartmentConfig {
        DepartmentConfig {
            id: "d".into(),
            shift_start_time: start.into(),
            shift_end_time: end.into(),
            shift_type: "day".into(),
            is_overnight_shift: overnight,
            ordinary_daily_minutes: 480,
            lunch_mode: "fixed".into(),
            lunch_duration_min: Some(60),
        }
    }

    fn rules() -> GlobalRulesRow {
        GlobalRulesRow {
            late_arrival_tolerance_min: 10,
            early_departure_tolerance_min: 10,
            bonus_minutes: 0,
        }
    }

    #[test]
    fn non_overnight_same_day_window() {
        let (ws, we, ns, ne, amb) = shift_window_overnight_aware(
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            &dept("09:00", "17:00", false),
            &rules(),
            Caracas,
        );
        assert!(!amb);
        // window is shift ± 10 min.
        assert_eq!(we - ws, 8 * 60 * 60 + 20 * 60); // 8h + 20min total tolerance
        assert_eq!(ne - ns, 8 * 60 * 60);
    }

    #[test]
    fn overnight_crosses_midnight() {
        // 22:00 Mon Apr 20 → 06:00 Tue Apr 21 Caracas (UTC-4) == 02:00 Tue → 10:00 Tue UTC.
        let (ws, we, ns, ne, amb) = shift_window_overnight_aware(
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            &dept("22:00", "06:00", true),
            &rules(),
            Caracas,
        );
        assert!(!amb);
        // Shift body = 8h; window adds 20min total tolerance.
        assert_eq!(ne - ns, 8 * 60 * 60);
        assert_eq!(we - ws, 8 * 60 * 60 + 20 * 60);
        // Nominal end is strictly greater than nominal start (crosses midnight).
        assert!(ne > ns);
    }

    #[test]
    fn overnight_anchor_attribution() {
        // An event at 06:00 Tue local (anchor_date=Mon) must fall INSIDE the shift window.
        let d = dept("22:00", "06:00", true);
        let (ws, we, _ns, ne, _amb) = shift_window_overnight_aware(
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            &d,
            &rules(),
            Caracas,
        );
        // 06:00 Tue local == 10:00 Tue UTC.
        let six_am_tue_utc = chrono::NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        assert!(six_am_tue_utc >= ws && six_am_tue_utc <= we);
        assert!(six_am_tue_utc <= ne + 10 * 60); // within early-tolerance of nominal_end
    }

    #[test]
    fn resolve_local_epoch_caracas_is_never_ambiguous() {
        // Venezuela has no DST since 2016. Every local datetime is unambiguous.
        let ndt = NaiveDate::from_ymd_opt(2026, 4, 20)
            .unwrap()
            .and_hms_opt(22, 0, 0)
            .unwrap();
        let (_epoch, amb) = resolve_local_epoch(Caracas, ndt);
        assert!(!amb);
    }

    #[test]
    fn resolve_local_epoch_spring_forward_gap() {
        // 2026-03-08 02:30 America/New_York is inside the spring-forward gap
        // (02:00 → 03:00). resolve_local_epoch must return ambiguous=true and
        // NOT panic.
        use chrono_tz::America::New_York;
        let ndt = NaiveDate::from_ymd_opt(2026, 3, 8)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();
        let (_epoch, amb) = resolve_local_epoch(New_York, ndt);
        assert!(
            amb,
            "spring-forward gap must be flagged ambiguous (future-DST safety)"
        );
    }

    #[test]
    fn resolve_local_epoch_fall_back_ambiguous() {
        // 2026-11-01 01:30 America/New_York occurs twice (DST fall-back at 02:00
        // → 01:00). resolve_local_epoch must pick `.earliest()` and flag ambiguous.
        use chrono_tz::America::New_York;
        let ndt = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(1, 30, 0)
            .unwrap();
        let (_epoch, amb) = resolve_local_epoch(New_York, ndt);
        assert!(
            amb,
            "fall-back ambiguity must be flagged (future-DST safety)"
        );
    }
}
