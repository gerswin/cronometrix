use serde::{Deserialize, Serialize};
use validator::Validate;

/// Department record returned by GET endpoints. Timestamps are ISO 8601 strings per D-13.
#[derive(Debug, Serialize)]
pub struct Department {
    pub id: String,
    pub name: String,
    pub base_salary_cents: i64,
    pub shift_start_time: String,        // "HH:MM"
    pub shift_end_time: String,          // "HH:MM"
    pub lunch_mode: String,              // "fixed" | "punch"
    pub lunch_duration_min: Option<i64>, // non-null when lunch_mode = "fixed"
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,              // ISO 8601
    pub updated_at: String,              // ISO 8601
}

/// Request body for POST /departments.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateDepartmentRequest {
    #[validate(length(min = 1, max = 200, message = "Name is required (1-200 chars)"))]
    pub name: String,
    pub base_salary_cents: i64,
    pub shift_start_time: String,
    pub shift_end_time: String,
    /// "fixed" or "punch"
    pub lunch_mode: String,
    /// Required when lunch_mode = "fixed"
    pub lunch_duration_min: Option<i64>,
}

/// Request body for PATCH /departments/:id. All fields optional; `version` required per D-04.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDepartmentRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub base_salary_cents: Option<i64>,
    pub shift_start_time: Option<String>,
    pub shift_end_time: Option<String>,
    pub lunch_mode: Option<String>,
    pub lunch_duration_min: Option<i64>,
    pub version: i64,
}

/// Query parameters for GET /departments pagination and filtering per D-12.
#[derive(Debug, Deserialize)]
pub struct DepartmentListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}
