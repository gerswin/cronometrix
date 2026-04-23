//! First-entry / last-exit aggregation (CALC-01, D-20).
//!
//! The window is `[shift_start - late_tol - bonus, shift_end + early_tol + bonus]`
//! in the installation's timezone. Events outside the window are ignored. Events
//! with `is_unknown=true` raise `UnknownFaceInWindow` but do not anchor
//! canonical entry/exit.
//!
//! Plan 03-02 routes shift-window construction through
//! [`crate::calc::overnight::shift_window_overnight_aware`] so overnight shifts
//! (`dept.is_overnight_shift = true`) correctly cross midnight via
//! `anchor_date.succ_opt()`, and the DST-ambiguous path is reported via a
//! boolean flag (future-market safety; always `false` in Venezuela).

use chrono::NaiveDate;
use chrono_tz::Tz;

use super::models::{AttendanceEventRow, DepartmentConfig, GlobalRulesRow};
use super::overnight::shift_window_overnight_aware;

/// Returns `(window_start_epoch, window_end_epoch, nominal_shift_start_epoch,
/// nominal_shift_end_epoch)` all in UTC epoch seconds.
///
/// Backward-compatible Plan 03-01 API — discards the DST-ambiguity flag.
/// Callers that need the flag (engine.rs) use [`shift_window_with_ambiguity`].
pub fn shift_window(
    anchor_date: NaiveDate,
    dept: &DepartmentConfig,
    rules: &GlobalRulesRow,
    tz: Tz,
) -> (i64, i64, i64, i64) {
    let (ws, we, ns, ne, _amb) = shift_window_overnight_aware(anchor_date, dept, rules, tz);
    (ws, we, ns, ne)
}

/// 5-tuple variant exposing the DST-ambiguity flag. Used by
/// `engine::compute_daily_record` to emit
/// [`AnomalyCode::OvernightInferenceAmbiguous`](super::anomalies::AnomalyCode::OvernightInferenceAmbiguous)
/// when the local→epoch resolution fell on a DST boundary.
///
/// In Venezuela (America/Caracas, no DST) `ambiguous` is always `false`; this
/// surface exists for future DST-observing markets (D-08).
pub fn shift_window_with_ambiguity(
    anchor_date: NaiveDate,
    dept: &DepartmentConfig,
    rules: &GlobalRulesRow,
    tz: Tz,
) -> (i64, i64, i64, i64, bool) {
    shift_window_overnight_aware(anchor_date, dept, rules, tz)
}

/// Result of filtering + bucketing the events into a single calc window.
#[derive(Debug, Clone)]
pub struct Aggregated {
    pub canonical_entry: Option<i64>,
    pub canonical_exit: Option<i64>,
    pub unknown_in_window: bool,
    pub entries_in_window: Vec<AttendanceEventRow>,
    pub exits_in_window: Vec<AttendanceEventRow>,
}

/// Apply the [`shift_window`] filter then pick the earliest `entry` and latest
/// `exit`. Unknown-face events set the `unknown_in_window` flag but are excluded
/// from anchoring.
pub fn aggregate_events(
    events: &[AttendanceEventRow],
    window_start: i64,
    window_end: i64,
) -> Aggregated {
    let mut entries: Vec<AttendanceEventRow> = Vec::new();
    let mut exits: Vec<AttendanceEventRow> = Vec::new();
    let mut unknown_in_window = false;

    for ev in events {
        if ev.captured_at < window_start || ev.captured_at > window_end {
            continue;
        }
        if ev.is_unknown {
            unknown_in_window = true;
            continue;
        }
        match ev.direction.as_str() {
            "entry" => entries.push(ev.clone()),
            "exit" => exits.push(ev.clone()),
            _ => {}
        }
    }

    entries.sort_by_key(|e| e.captured_at);
    exits.sort_by_key(|e| e.captured_at);

    let canonical_entry = entries.first().map(|e| e.captured_at);
    let canonical_exit = exits.last().map(|e| e.captured_at);

    Aggregated {
        canonical_entry,
        canonical_exit,
        unknown_in_window,
        entries_in_window: entries,
        exits_in_window: exits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str, direction: &str, captured_at: i64, is_unknown: bool) -> AttendanceEventRow {
        AttendanceEventRow {
            id: id.to_string(),
            employee_id: if is_unknown { None } else { Some("e1".into()) },
            device_id: "d1".into(),
            direction: direction.into(),
            captured_at,
            is_unknown,
        }
    }

    #[test]
    fn aggregation_excludes_events_outside_window() {
        let events = vec![
            ev("a", "entry", 100, false),
            ev("b", "entry", 500, false),
            ev("c", "exit", 1500, false),
            ev("d", "exit", 2000, false),
        ];
        let agg = aggregate_events(&events, 400, 1600);
        assert_eq!(agg.canonical_entry, Some(500));
        assert_eq!(agg.canonical_exit, Some(1500));
    }

    #[test]
    fn aggregation_picks_earliest_entry_latest_exit() {
        let events = vec![
            ev("a", "entry", 600, false),
            ev("b", "entry", 500, false),
            ev("c", "exit", 1500, false),
            ev("d", "exit", 1400, false),
        ];
        let agg = aggregate_events(&events, 400, 1600);
        assert_eq!(agg.canonical_entry, Some(500));
        assert_eq!(agg.canonical_exit, Some(1500));
    }

    #[test]
    fn aggregation_flags_unknown_face() {
        let events = vec![
            ev("a", "entry", 500, false),
            ev("b", "entry", 700, true),
            ev("c", "exit", 1500, false),
        ];
        let agg = aggregate_events(&events, 400, 1600);
        assert!(agg.unknown_in_window);
        assert_eq!(agg.canonical_entry, Some(500));
    }
}
