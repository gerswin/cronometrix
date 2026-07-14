//! Leave DTOs and validator-derived request structs.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// API response shape for a single leave row. Dates are ISO YYYY-MM-DD, timestamps
/// are RFC 3339 via `epoch_to_iso` (Phase 1 D-13).
#[derive(Debug, Serialize, Clone)]
pub struct LeaveResponse {
    pub id: String,
    pub employee_id: String,
    pub from_date: String, // YYYY-MM-DD
    pub to_date: String,   // YYYY-MM-DD
    pub leave_type: String,
    pub justification: String,
    pub evidence_path: Option<String>,
    pub created_by: String,
    pub cancelled_by: Option<String>,
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Body fields parsed out of the multipart request.
/// (Multipart bodies are consumed field-by-field in handlers.rs; this struct
/// just wraps the resolved text fields so service::create_leave can validate
/// them in one place.)
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateLeaveRequest {
    pub employee_id: String,
    #[validate(length(equal = 10, message = "from_date must be YYYY-MM-DD"))]
    pub from_date: String,
    #[validate(length(equal = 10, message = "to_date must be YYYY-MM-DD"))]
    pub to_date: String,
    #[validate(length(min = 1, max = 32))]
    pub leave_type: String, // "medical" | "vacation" | "unpaid" | "manual"
    #[validate(length(min = 1, max = 2000))]
    pub justification: String,
}

/// PATCH payload (reserved for future — e.g. editing a justification in place).
/// Not currently wired into any route, but part of the public module surface for
/// Phase 4 timesheet-related workflows.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLeaveRequest {
    #[validate(length(min = 1, max = 2000))]
    pub justification: Option<String>,
    pub version: i64,
}

#[derive(Debug, Deserialize)]
pub struct LeaveListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub employee_id: Option<String>,
    pub leave_type: Option<String>,
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}
