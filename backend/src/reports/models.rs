//! DTOs for the Reports JSON API surface.
//!
//! Wire format is JSON. Money values stay in `i64` cents to preserve precision;
//! the frontend formats them via `Intl.NumberFormat('en-US', {style:'currency',
//! currency:'USD'})` per D-33.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request body for `POST /api/v1/reports/json`. Validator only enforces the
/// `YYYY-MM-DD` shape via `length(equal = 10)` — the actual date parse is done
/// by `periods::parse_period` so unknown `period_type` strings surface a single
/// AppError::Validation with a meaningful message.
#[derive(Debug, Deserialize, Validate, Clone)]
pub struct ReportParamsRequest {
    #[validate(length(equal = 10, message = "from_date must be YYYY-MM-DD (10 chars)"))]
    pub from_date: String,
    #[validate(length(equal = 10, message = "to_date must be YYYY-MM-DD (10 chars)"))]
    pub to_date: String,
    pub period_type: String,
    pub department_ids: Option<Vec<String>>,
    pub include_inactive: Option<bool>,
    pub employee_id: Option<String>,
    pub shift_type: Option<String>,
}

/// Top-level payload returned by the JSON handler. Matches the Excel layout
/// (D-26..D-28) so Plan 05-03 (Excel) and Plan 05-04 (PDF) consume the same
/// shape with no transformation.
#[derive(Debug, Serialize)]
pub struct ReportPayload {
    pub header: BrandingHeader,
    pub rows: Vec<EmployeeReportRow>,
    pub dept_subtotals: Vec<DeptSubtotal>,
    pub grand_total: Aggregates,
    pub departments_in_order: Vec<DeptSummary>,
}

/// Branding header for Excel rows 1–3 / PDF first page (D-28). Empty
/// `client_name` / `client_rif` render as `—` in the UI.
#[derive(Debug, Serialize)]
pub struct BrandingHeader {
    pub client_name: String,
    pub client_rif: String,
    pub from_date: String,
    pub to_date: String,
    pub generated_at_iso: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeptSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeptSubtotal {
    pub dept_id: String,
    pub dept_name: String,
    pub aggregates: Aggregates,
}

/// Numeric aggregates — used both per-employee and per-department-subtotal
/// and as the grand total. All money fields are USD cents (D-02, D-33).
#[derive(Debug, Serialize, Clone, Default)]
pub struct Aggregates {
    pub work_min: i64,
    pub ot_min: i64,
    pub late_min: i64,
    pub days_worked: i64,
    pub days_absent: i64,
    pub work_pay_cents: i64,
    pub ot_pay_cents: i64,
    pub night_premium_cents: i64,
    pub rest_day_surcharge_cents: i64,
    pub late_deduction_cents: i64,
    pub total_a_pagar_cents: i64,
    pub days_ivss: i64,
    pub days_vacation: i64,
    pub days_permission: i64,
    pub days_unpaid: i64,
}

/// One row per employee per period. `aggregates` is flattened so the wire shape
/// is `{ employee_id, …, work_min, ot_min, …, anomaly_codes, anomaly_count }`.
#[derive(Debug, Serialize, Clone)]
pub struct EmployeeReportRow {
    pub employee_id: String,
    pub dept_id: String,
    pub cedula: String,
    pub nombre: String,
    pub departamento: String,
    pub cargo: String,
    pub shift_type: String,
    #[serde(flatten)]
    pub aggregates: Aggregates,
    pub anomaly_codes: Vec<String>,
    pub anomaly_count: i64,
}
