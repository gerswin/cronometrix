//! Window filtering + entry/exit bucketing (CALC-01, D-20), and — as of
//! M-03 — punch pairing for worked-time computation.
//!
//! The window is `[shift_start - late_tol - bonus, shift_end + early_tol + bonus]`
//! in the installation's timezone. Events outside the window are ignored. Events
//! with `is_unknown=true` raise `UnknownFaceInWindow` but do not anchor
//! canonical entry/exit.
//!
//! [`aggregate_events`] still exposes `canonical_entry` (earliest entry) and
//! `canonical_exit` (latest exit) — those remain correct for `entry_at`/
//! `exit_at` display, the first and last badge of the day. They are NOT used
//! to compute `work_minutes` anymore: [`pair_work_intervals`] pairs every
//! entry with its own exit so a long absence in the middle of the window, or
//! several disjoint blocks of work, is handled correctly instead of being
//! counted as one uninterrupted span (the pre-M-03 bug).
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

/// Inverse of [`shift_window`] (H-02): given a punch instant, which
/// `anchor_date`(s) does it belong to?
///
/// `shift_window` answers "what window does this anchor_date own?".
/// Publishing a recompute request needs the opposite direction: "given this
/// instant, which anchor_date's window claims it?" — because an overnight
/// exit is captured with a local *date* one day later than the shift that
/// produced it (a 22:00→06:00 shift starting Monday captures its exit on
/// Tuesday's calendar date). Anchoring on the event's own local date leaves
/// Monday with `MissingExit` and invents a spurious exit-only record on
/// Tuesday — that is bug H-02.
///
/// `own_date` is the event's own local date (the caller already has it —
/// resolving an epoch to a local date can fail on pathological input, so we
/// don't repeat that fallible conversion here).
///
/// Decision rule (deliberately chosen, not guessed — see H-02 task brief):
/// - Non-overnight departments are never ambiguous: the shift starts and ends
///   on the same calendar date, so `own_date` is the only possible anchor.
/// - Overnight departments: compare the instant against the REAL window
///   (nominal boundary ± configured tolerance, via `shift_window`) of both
///   candidate anchors — `own_date` and `own_date - 1`:
///     - Inside yesterday's window only → anchor = yesterday. This is the
///       H-02 case: a 06:05 exit falls inside the window opened by
///       yesterday's 22:00 shift start.
///     - Inside today's window only → anchor = `own_date` (an entry, or an
///       exit whose department is overnight but whose captured_at still
///       falls on the same date as the shift start — e.g. a very short
///       shift).
///     - Inside NEITHER → anchor = `own_date`. A stray badge outside any
///       configured window surfaces its anomaly on the day it was physically
///       captured, matching the pre-fix, unambiguous common case.
///     - Inside BOTH — only reachable with pathological tolerance
///       configuration wide enough to make two consecutive overnight windows
///       overlap — return BOTH. `recompute_for_day` is idempotent (`ON
///       CONFLICT (employee_id, anchor_date) DO UPDATE` keyed by the exact
///       pair, replacing rather than accumulating), so requesting the same
///       day more than once, or two days that both legitimately claim the
///       instant, never double-counts work minutes or anomalies.
pub fn anchor_dates_for_instant(
    captured_at: i64,
    own_date: NaiveDate,
    dept: &DepartmentConfig,
    rules: &GlobalRulesRow,
    tz: Tz,
) -> Vec<NaiveDate> {
    if !dept.is_overnight_shift {
        return vec![own_date];
    }

    let Some(yesterday) = own_date.pred_opt() else {
        return vec![own_date];
    };

    let (y_start, y_end, ..) = shift_window(yesterday, dept, rules, tz);
    let (t_start, t_end, ..) = shift_window(own_date, dept, rules, tz);

    let in_yesterday = captured_at >= y_start && captured_at <= y_end;
    let in_today = captured_at >= t_start && captured_at <= t_end;

    match (in_yesterday, in_today) {
        (true, false) => vec![yesterday],
        (true, true) => vec![yesterday, own_date],
        (false, _) => vec![own_date],
    }
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

/// M-03: pairs `entries` with `exits` in chronological order to determine
/// which spans of the window were actually worked.
///
/// This replaces the "first entry, last exit" extremes approach that
/// [`aggregate_events`] still exposes via `canonical_entry`/`canonical_exit`
/// (kept for `entry_at`/`exit_at` display — the first and last badge of the
/// day are still meaningful to show). The extremes approach was wrong for
/// computing *worked time*: a long absence in the middle of the window
/// counted as work, and several disjoint blocks of work collapsed into one
/// span covering the gaps between them.
///
/// Algorithm: merge-scan the two (already time-sorted) event streams as one
/// timeline. An `entry` opens a pending interval; the matching `exit` closes
/// it. Two decisions fall out of this that would otherwise be accidental:
///
/// - A second `entry` while one is already open (e.g. a duplicate badge, or
///   the reader firing twice) is a no-op — it does not reopen or extend
///   anything. Only the first entry of a pending interval anchors it.
/// - An `exit` with nothing open (e.g. a stray duplicate exit tap, or an
///   entry that fell outside the window) has nothing to close and is
///   ignored — it does not fabricate a zero-length or negative interval.
///
/// No minimum-gap merge threshold is applied: every exit→entry gap the
/// engine sees here is treated as unworked, however short. This is
/// deliberate (see the M-03 task brief's third decision) — bounce/duplicate
/// taps are already collapsed before an event reaches this function, by the
/// 30-second dedup bucket in
/// `events::service::persist_attendance_event_queued`
/// ((employee_id, device_id, direction, captured_at/30) uniqueness). A gap
/// that survives that filter represents a real door-open/close cycle, not
/// sensor noise, so counting it as unworked is correct rather than punitive.
///
/// Returns `(closed_intervals, has_open_entry)`. `has_open_entry` is `true`
/// when the scan ends with an entry that never got a matching exit — an odd
/// number of punches (e.g. entry, exit, entry, and nothing else). This
/// function only reports that fact; the caller ([`super::engine`]) decides
/// the policy: the dangling entry is dropped from the worked-minutes sum
/// (there is nothing to pair it with) and a `MISSING_EXIT` anomaly is raised
/// — the same code used when the whole window lacks an exit, because from
/// this entry's point of view it does.
pub fn pair_work_intervals(
    entries: &[AttendanceEventRow],
    exits: &[AttendanceEventRow],
) -> (Vec<(i64, i64)>, bool) {
    #[derive(Clone, Copy)]
    enum Kind {
        Entry,
        Exit,
    }

    let mut timeline: Vec<(i64, Kind)> = Vec::with_capacity(entries.len() + exits.len());
    timeline.extend(entries.iter().map(|e| (e.captured_at, Kind::Entry)));
    timeline.extend(exits.iter().map(|e| (e.captured_at, Kind::Exit)));
    timeline.sort_by_key(|(ts, _)| *ts);

    let mut intervals = Vec::new();
    let mut open_entry: Option<i64> = None;
    for (ts, kind) in timeline {
        match kind {
            Kind::Entry => {
                if open_entry.is_none() {
                    open_entry = Some(ts);
                }
                // else: duplicate entry while already clocked in — ignored.
            }
            Kind::Exit => {
                if let Some(start) = open_entry.take() {
                    intervals.push((start, ts));
                }
                // else: orphan exit with nothing open — ignored.
            }
        }
    }

    (intervals, open_entry.is_some())
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

    // =========================================================================
    // pair_work_intervals — M-03
    // =========================================================================
    mod pair_work_intervals_tests {
        use super::*;

        fn entries(times: &[i64]) -> Vec<AttendanceEventRow> {
            times
                .iter()
                .enumerate()
                .map(|(i, t)| ev(&format!("en{i}"), "entry", *t, false))
                .collect()
        }

        fn exits(times: &[i64]) -> Vec<AttendanceEventRow> {
            times
                .iter()
                .enumerate()
                .map(|(i, t)| ev(&format!("ex{i}"), "exit", *t, false))
                .collect()
        }

        #[test]
        fn single_pair_is_one_interval() {
            let (intervals, open) = pair_work_intervals(&entries(&[100]), &exits(&[500]));
            assert_eq!(intervals, vec![(100, 500)]);
            assert!(!open);
        }

        #[test]
        fn a_midday_gap_produces_two_disjoint_intervals_not_one_span() {
            // entry, exit(lunch out), entry(lunch in), exit — the gap between
            // pairs must not be summed into worked time.
            let (intervals, open) =
                pair_work_intervals(&entries(&[100, 900]), &exits(&[400, 1200]));
            assert_eq!(intervals, vec![(100, 400), (900, 1200)]);
            assert!(!open);
        }

        #[test]
        fn trailing_unmatched_entry_is_reported_but_not_paired() {
            // entry, exit, entry — odd number of punches.
            let (intervals, open) = pair_work_intervals(&entries(&[100, 900]), &exits(&[400]));
            assert_eq!(intervals, vec![(100, 400)]);
            assert!(open, "trailing entry with no exit must be flagged");
        }

        #[test]
        fn duplicate_entry_while_clocked_in_is_a_no_op() {
            // entry, entry, exit — the second entry does not reopen anything.
            let (intervals, open) =
                pair_work_intervals(&entries(&[100, 150]), &exits(&[400]));
            assert_eq!(intervals, vec![(100, 400)]);
            assert!(!open);
        }

        #[test]
        fn orphan_exit_with_nothing_open_is_ignored() {
            // exit (no prior entry), entry, exit — the first exit has
            // nothing to close.
            let (intervals, open) =
                pair_work_intervals(&entries(&[200]), &exits(&[100, 400]));
            assert_eq!(intervals, vec![(200, 400)]);
            assert!(!open);
        }

        #[test]
        fn no_events_yields_no_intervals_and_no_open_entry() {
            let (intervals, open) = pair_work_intervals(&[], &[]);
            assert!(intervals.is_empty());
            assert!(!open);
        }
    }

    // =========================================================================
    // anchor_dates_for_instant — H-02
    // =========================================================================
    mod anchor_dates_for_instant_tests {
        use super::super::*;
        use chrono::TimeZone;
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

        /// Epoch for a Caracas-local `y-m-d h:mi:00`. Caracas has no DST, so
        /// this local→epoch resolution is always unambiguous.
        fn epoch(y: i32, m: u32, d: u32, h: u32, mi: u32) -> i64 {
            Caracas
                .with_ymd_and_hms(y, m, d, h, mi, 0)
                .single()
                .expect("Caracas has no DST; every local datetime is unambiguous")
                .timestamp()
        }

        #[test]
        fn overnight_exit_anchors_to_the_day_the_shift_started() {
            // 22:00-06:00 shift starting Mon Apr 20. Exit at 06:05 Tue Apr 21
            // is within the 10-minute early-departure tolerance past nominal
            // end (06:00) — it belongs to Monday, not to its own local date.
            let d = dept("22:00", "06:00", true);
            let r = rules();
            let captured_at = epoch(2026, 4, 21, 6, 5);
            let own_date = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

            let anchors = anchor_dates_for_instant(captured_at, own_date, &d, &r, Caracas);

            assert_eq!(anchors, vec![NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()]);
        }

        #[test]
        fn day_shift_always_anchors_on_its_own_date() {
            let d = dept("08:00", "17:00", false);
            let r = rules();
            let captured_at = epoch(2026, 4, 20, 8, 0);
            let own_date = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();

            let anchors = anchor_dates_for_instant(captured_at, own_date, &d, &r, Caracas);

            assert_eq!(anchors, vec![own_date]);
        }

        #[test]
        fn exit_just_after_midnight_belongs_to_the_previous_day() {
            // Same 22:00-06:00 shift; exit at 00:30 is well before nominal
            // end (06:00) and squarely inside yesterday's window.
            let d = dept("22:00", "06:00", true);
            let r = rules();
            let captured_at = epoch(2026, 4, 21, 0, 30);
            let own_date = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

            let anchors = anchor_dates_for_instant(captured_at, own_date, &d, &r, Caracas);

            assert_eq!(anchors, vec![NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()]);
        }

        #[test]
        fn stray_event_outside_every_window_anchors_on_its_own_date() {
            // Overnight dept, but the event lands mid-afternoon — inside
            // neither yesterday's nor today's shift window.
            let d = dept("22:00", "06:00", true);
            let r = rules();
            let captured_at = epoch(2026, 4, 21, 14, 0);
            let own_date = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

            let anchors = anchor_dates_for_instant(captured_at, own_date, &d, &r, Caracas);

            assert_eq!(anchors, vec![own_date]);
        }

        #[test]
        fn overlapping_windows_recompute_both_candidate_anchors() {
            // Pathological tolerance configuration (23h bonus) makes
            // consecutive overnight windows overlap almost entirely, so a
            // single instant can legitimately fall inside both. The function
            // must report both anchors rather than pick one arbitrarily.
            let d = dept("22:00", "06:00", true);
            let r = GlobalRulesRow {
                late_arrival_tolerance_min: 0,
                early_departure_tolerance_min: 0,
                bonus_minutes: 23 * 60,
            };
            let captured_at = epoch(2026, 4, 21, 6, 5);
            let own_date = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

            let anchors = anchor_dates_for_instant(captured_at, own_date, &d, &r, Caracas);

            assert_eq!(
                anchors,
                vec![
                    NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
                    NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
                ]
            );
        }
    }
}
