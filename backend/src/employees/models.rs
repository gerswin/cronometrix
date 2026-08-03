use serde::{Deserialize, Serialize};
use validator::Validate;

/// Unit in which `base_salary_cents` is expressed (H-08).
///
/// Before this type existed there was none: `money.rs` assumed "pay for one
/// ordinary day" while the UI only said "Sueldo Base (USD)". A monthly salary
/// entered there paid a full month for every single day worked — the period
/// was multiplied by ~30.
///
/// Serializes/deserializes as the lowercase strings that match the
/// `salary_kind` CHECK constraint added in migration 024:
/// `"hourly"` | `"daily"` | `"monthly"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SalaryKind {
    Hourly,
    Daily,
    Monthly,
}

impl SalaryKind {
    /// The DB TEXT representation, matching migration 024's CHECK constraint.
    pub fn as_db_str(self) -> &'static str {
        match self {
            SalaryKind::Hourly => "hourly",
            SalaryKind::Daily => "daily",
            SalaryKind::Monthly => "monthly",
        }
    }

    /// Parse the DB TEXT column into a `SalaryKind`. Returns `None` for
    /// anything else, including a NULL column (mapped to `None` by the
    /// caller before this is reached) — a missing/unrecognized unit must
    /// surface as a data error, never silently become a default (H-08).
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "hourly" => Some(SalaryKind::Hourly),
            "daily" => Some(SalaryKind::Daily),
            "monthly" => Some(SalaryKind::Monthly),
            _ => None,
        }
    }

    /// Multiplier/divisor pair that brings `base_salary_cents` to "one
    /// ordinary day", returned separately on purpose: the caller builds ONE
    /// fraction with them. Pre-computing a daily salary would introduce an
    /// extra division and lose up to 29 cents/day in integer-cents math —
    /// exactly what `money.rs`'s "multiply numerators first, divide once"
    /// rule (money.rs:3-4) exists to avoid.
    ///
    /// - `Daily` -> base as-is
    /// - `Monthly` -> base / 30 (Venezuelan daily-salary convention; pending
    ///   accounting confirmation)
    /// - `Hourly` -> base * ordinary_daily_minutes / 60
    pub(crate) fn to_daily(self, ordinary_daily_minutes: i64) -> (i64, i64) {
        match self {
            SalaryKind::Daily => (1, 1),
            SalaryKind::Monthly => (1, 30),
            SalaryKind::Hourly => (ordinary_daily_minutes, 60),
        }
    }
}

/// Employee record returned by GET endpoints. Timestamps are ISO 8601 strings per D-13.
///
/// Phase 5 D-30a adds `position` (cargo) and `hire_date` (epoch seconds → ISO date) so the
/// pre-payroll report can populate the `cargo` identity column. `position` is NOT NULL with
/// empty-string default; `hire_date` is nullable (None = unknown).
#[derive(Debug, Serialize)]
pub struct Employee {
    pub id: String,
    pub employee_code: String,
    pub name: String,
    pub department_id: String,
    pub status: String,            // "active" | "inactive"
    pub position: String,          // empty string renders as '—' in UI per D-30a
    pub hire_date: Option<String>, // ISO YYYY-MM-DD or null
    /// Employment end date (migration 026, C-05). `None` = still employed.
    /// Stamped automatically by `deactivate_queued` at the moment `status`
    /// flips to 'inactive' — it is not user-settable via
    /// Create/UpdateEmployeeRequest, mirroring how `deleted_at` is derived,
    /// not authored. The payroll report reads this (not `status`) to decide
    /// whether an employee's days in a period are payable.
    pub terminated_on: Option<String>, // ISO YYYY-MM-DD or null
    /// Per-employee base salary in cents (migration 018). Authoritative for payroll math.
    /// Department-level salary remains as a "default suggestion" only.
    pub base_salary_cents: i64,
    /// Unit `base_salary_cents` is expressed in (H-08, migration 024).
    /// `None` when the row predates migration 024 or otherwise has no valid
    /// unit set (the column has no DEFAULT on purpose — see the migration's
    /// comment). The edit form (Critical 1 / H-08 follow-up) reads this to
    /// prefill its unit selector instead of always rendering blank; it must
    /// stay `Option`, never defaulted to a guessed unit here.
    pub salary_kind: Option<SalaryKind>,
    pub deleted_at: Option<String>, // ISO 8601 or null
    pub version: i64,
    pub created_at: String, // ISO 8601
    pub updated_at: String, // ISO 8601
}

/// Request body for POST /employees. `position` and `hire_date` are optional;
/// defaults are empty string and NULL respectively.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmployeeRequest {
    #[validate(length(min = 1, max = 50, message = "Employee code is required (1-50 chars)"))]
    pub employee_code: String,
    #[validate(length(min = 1, max = 200, message = "Name is required (1-200 chars)"))]
    pub name: String,
    #[validate(length(min = 1, message = "Department ID is required"))]
    pub department_id: String,
    #[validate(length(max = 100, message = "Position max 100 chars"))]
    pub position: Option<String>,
    /// YYYY-MM-DD; service parses to epoch seconds. Empty string treated as NULL.
    pub hire_date: Option<String>,
    /// Per-employee base salary in cents. Required (C-03): the service layer
    /// rejects `None` and non-positive values instead of defaulting to 0.
    pub base_salary_cents: Option<i64>,
    /// Unit `base_salary_cents` is expressed in (H-08). Required — the
    /// service layer rejects `None` rather than assuming a unit.
    pub salary_kind: Option<SalaryKind>,
}

/// Request body for PATCH /employees/:id. All fields optional; `version` is required
/// for optimistic concurrency per D-04.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEmployeeRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub department_id: Option<String>,
    #[validate(length(max = 100, message = "Position max 100 chars"))]
    pub position: Option<String>,
    /// YYYY-MM-DD; service parses to epoch seconds. Empty string treated as NULL clear.
    pub hire_date: Option<String>,
    /// A `Some(salary)` with `salary <= 0` is rejected with `SALARY_INVALID`
    /// (C-03) — never silently coerced to `None`/no-op.
    pub base_salary_cents: Option<i64>,
    /// Unit `base_salary_cents` is expressed in (H-08). Optional on update —
    /// omit to leave the employee's existing unit unchanged.
    pub salary_kind: Option<SalaryKind>,
    pub version: i64,
}

/// Query parameters for GET /employees pagination and filtering per D-12.
#[derive(Debug, Deserialize)]
pub struct EmployeeListQuery {
    pub limit: Option<i64>,   // default 20, max 100
    pub offset: Option<i64>,  // default 0
    pub name: Option<String>, // partial LIKE match
    pub department_id: Option<String>,
    pub status: Option<String>, // "active" | "inactive"
}
