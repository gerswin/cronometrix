//! LOTTT Art. 178 overtime cap checks (CALC-04, D-09).
//!
//! - Daily cap: total workday (work_minutes + overtime_minutes) must not
//!   exceed 600 minutes (10h). Breach → `OtCapExceededDaily`.
//! - Weekly cap: running weekly OT (Mon-start ISO week) must not exceed 600
//!   minutes (10h). Breach → `OtCapExceededWeekly`.
//! - Annual cap: running per-employee OT in the calendar year must not
//!   exceed 6000 minutes (100h). Breach → `OtCapExceededAnnual`.
//!
//! Anomalies are raised; minutes are still attributed. Operator reviews via
//! the Phase 4 supervisor queue (read endpoint exposed in this plan).

use super::anomalies::AnomalyCode;

pub fn check_overtime_caps(
    work_minutes: i64,
    overtime_minutes: i64,
    weekly_ot_minutes_so_far: i64,
    annual_ot_minutes_so_far: i64,
) -> Vec<AnomalyCode> {
    let mut out = Vec::new();
    if work_minutes + overtime_minutes > 600 {
        out.push(AnomalyCode::OtCapExceededDaily);
    }
    if weekly_ot_minutes_so_far + overtime_minutes > 600 {
        out.push(AnomalyCode::OtCapExceededWeekly);
    }
    if annual_ot_minutes_so_far + overtime_minutes > 6000 {
        out.push(AnomalyCode::OtCapExceededAnnual);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_cap_triggers_only_when_total_exceeds_600() {
        // 600 exact — no anomaly
        assert!(check_overtime_caps(480, 120, 0, 0).is_empty());
        // 601 — daily cap triggers
        let out = check_overtime_caps(480, 121, 0, 0);
        assert!(out.contains(&AnomalyCode::OtCapExceededDaily));
    }

    #[test]
    fn weekly_cap_considers_so_far_aggregate() {
        let out = check_overtime_caps(480, 60, 550, 0);
        assert!(out.contains(&AnomalyCode::OtCapExceededWeekly));
    }

    #[test]
    fn annual_cap_considers_so_far_aggregate() {
        let out = check_overtime_caps(480, 60, 0, 5990);
        assert!(out.contains(&AnomalyCode::OtCapExceededAnnual));
    }
}
