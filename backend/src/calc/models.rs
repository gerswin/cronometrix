use chrono::NaiveDate;
use chrono_tz::Tz;

use super::anomalies::AnomalyCode;

/// Subset of `attendance_events` columns consumed by the engine.
#[derive(Debug, Clone)]
pub struct AttendanceEventRow {
    pub id: String,
    pub employee_id: Option<String>,
    pub device_id: String,
    pub direction: String, // "entry" | "exit"
    pub captured_at: i64,  // UTC epoch seconds
    pub is_unknown: bool,
}

/// Department fields required by the calc engine (mirrors columns in
/// `departments` after migration 012).
#[derive(Debug, Clone)]
pub struct DepartmentConfig {
    pub id: String,
    pub shift_start_time: String, // "HH:MM"
    pub shift_end_time: String,   // "HH:MM"
    pub shift_type: String,       // "day" | "night" | "mixed"
    pub is_overnight_shift: bool, // Plan 03-01 defaults false; Plan 03-02 wires true
    pub ordinary_daily_minutes: i64,
    pub lunch_mode: String, // "fixed" | "punch"
    pub lunch_duration_min: Option<i64>,
}

/// Active global_rules singleton snapshot for the recompute window.
#[derive(Debug, Clone)]
pub struct GlobalRulesRow {
    pub late_arrival_tolerance_min: i64,
    pub early_departure_tolerance_min: i64,
    pub bonus_minutes: i64,
}

/// Active `leaves` overlay row for the anchor date, if any.
/// Plan 03-01 always passes `None`; Plan 03-03 populates from the `leaves` table.
#[derive(Debug, Clone)]
pub struct LeaveRow {
    pub id: String,
    pub employee_id: String,
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
    pub leave_type: String,
}

/// All inputs required to deterministically compute one DailyRecord.
#[derive(Debug, Clone)]
pub struct EngineInput {
    pub events: Vec<AttendanceEventRow>,
    pub dept: DepartmentConfig,
    pub rules: GlobalRulesRow,
    pub leave: Option<LeaveRow>,
    pub anchor_date: NaiveDate,
    pub tz: Tz,
    pub weekly_ot_minutes_so_far: i64,
    pub annual_ot_minutes_so_far: i64,
    /// True iff the prior daily_records row existed — signals RECOMPUTE_AFTER_EDIT.
    pub prior_record_existed: bool,
}

/// Pure output of the engine. Persistence writes these values into
/// `daily_records` (+ anomaly rows into `daily_record_anomalies`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyRecordOutput {
    pub work_minutes: i64,
    pub overtime_minutes: i64,
    pub late_minutes: i64,
    pub early_departure_minutes: i64,
    pub is_rest_day_worked: bool,
    pub entry_at: Option<i64>,
    pub exit_at: Option<i64>,
    pub leave_id: Option<String>,
    pub anomalies: Vec<AnomalyCode>,
}
