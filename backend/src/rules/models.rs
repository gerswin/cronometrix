use serde::{Deserialize, Serialize};
use validator::Validate;

/// Global rules singleton record returned by GET /rules.
/// Timestamps are ISO 8601 strings per D-13.
#[derive(Debug, Serialize)]
pub struct GlobalRules {
    pub late_arrival_tolerance_min: i64,
    pub early_departure_tolerance_min: i64,
    pub bonus_minutes: i64,
    pub effective_from: String,  // ISO 8601
    pub version: i64,
    pub updated_at: String,      // ISO 8601
}

/// Request body for PATCH /rules. All fields optional; `version` required per D-04.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRulesRequest {
    #[validate(range(min = 0, max = 60, message = "Tolerance must be 0-60 minutes"))]
    pub late_arrival_tolerance_min: Option<i64>,
    #[validate(range(min = 0, max = 60, message = "Tolerance must be 0-60 minutes"))]
    pub early_departure_tolerance_min: Option<i64>,
    #[validate(range(min = 0, max = 60, message = "Bonus minutes must be 0-60"))]
    pub bonus_minutes: Option<i64>,
    pub version: i64,
}
