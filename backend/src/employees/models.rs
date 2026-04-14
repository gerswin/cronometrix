use serde::{Deserialize, Serialize};
use validator::Validate;

/// Employee record returned by GET endpoints. Timestamps are ISO 8601 strings per D-13.
#[derive(Debug, Serialize)]
pub struct Employee {
    pub id: String,
    pub employee_code: String,
    pub name: String,
    pub department_id: String,
    pub status: String,              // "active" | "inactive"
    pub deleted_at: Option<String>,  // ISO 8601 or null
    pub version: i64,
    pub created_at: String,          // ISO 8601
    pub updated_at: String,          // ISO 8601
}

/// Request body for POST /employees. All fields required.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmployeeRequest {
    #[validate(length(min = 1, max = 50, message = "Employee code is required (1-50 chars)"))]
    pub employee_code: String,
    #[validate(length(min = 1, max = 200, message = "Name is required (1-200 chars)"))]
    pub name: String,
    #[validate(length(min = 1, message = "Department ID is required"))]
    pub department_id: String,
}

/// Request body for PATCH /employees/:id. All fields optional; `version` is required
/// for optimistic concurrency per D-04.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEmployeeRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub department_id: Option<String>,
    pub version: i64,
}

/// Query parameters for GET /employees pagination and filtering per D-12.
#[derive(Debug, Deserialize)]
pub struct EmployeeListQuery {
    pub limit: Option<i64>,         // default 20, max 100
    pub offset: Option<i64>,        // default 0
    pub name: Option<String>,       // partial LIKE match
    pub department_id: Option<String>,
    pub status: Option<String>,     // "active" | "inactive"
}
