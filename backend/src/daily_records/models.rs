use serde::{Deserialize, Serialize};

/// API response for a successful override creation.
#[derive(Debug, Serialize)]
pub struct OverrideResponse {
    pub id: String,
    pub daily_record_id: String,
    pub override_work_minutes: Option<i64>,
    pub override_entry_at: Option<i64>,
    pub override_exit_at: Option<i64>,
    pub justification: String,
    pub evidence_path: Option<String>,
    pub overridden_by: String,
    pub overridden_at: i64,
    pub status: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// API response shape for a single daily_records row with its anomaly codes.
#[derive(Debug, Serialize)]
pub struct DailyRecordResponse {
    pub id: String,
    pub employee_id: String,
    pub employee_name: Option<String>,
    pub department_id: String,
    pub anchor_date: String,
    pub shift_type: String,
    pub work_minutes: i64,
    pub overtime_minutes: i64,
    pub late_minutes: i64,
    pub early_departure_minutes: i64,
    pub is_rest_day_worked: bool,
    pub entry_at: Option<String>, // ISO 8601
    pub exit_at: Option<String>,  // ISO 8601
    pub leave_id: Option<String>,
    pub computed_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub anomalies: Vec<String>, // AnomalyCode::as_str values
}

/// Filters for `GET /api/v1/daily-records`. `from_date` / `to_date` are
/// inclusive `YYYY-MM-DD` strings matched against `anchor_date`.
#[derive(Debug, Deserialize)]
pub struct DailyRecordListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub employee_id: Option<String>,
    pub department_id: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}
