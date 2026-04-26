//! Reports test seeders. Mirrors the leave_tests.rs seeder pattern but with
//! Phase 5–specific helpers:
//! - `seed_dept` — takes shift_type so we can mismatch dept policy vs per-day
//!   shift (W-6 test).
//! - `seed_daily_record` — accepts day_shift_type explicitly so a "day" dept
//!   can have a "night" daily_record.
//! - `seed_leave` — supports leaves with NO daily_records (W-5 vacation-week
//!   without captures).
//!
//! Included via `#[path = "fixtures/reports/seed.rs"]` from `reports_test.rs`.

#![allow(dead_code)]

use libsql::params;
use uuid::Uuid;

/// Seed a department row. `shift_type` is the POLICY/default — daily_records
/// can override on a per-day basis (see seed_daily_record).
pub async fn seed_dept(
    db: &libsql::Database,
    name: &str,
    base_cents: i64,
    ord_min: i64,
    dept_shift_type: &str, // "day" | "night" | "mixed"
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, '08:00', '16:00', 'fixed', 60, ?4, 0, ?5, 'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            name.to_string(),
            base_cents,
            dept_shift_type.to_string(),
            ord_min,
        ],
    )
    .await
    .expect("seed dept");
    id
}

/// Seed an employee. `position` populates `cargo` in the report row.
pub async fn seed_employee(
    db: &libsql::Database,
    code: &str,
    name: &str,
    dept_id: &str,
    position: &str,
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, position, \
         hire_date, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', ?5, NULL, 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            code.to_string(),
            name.to_string(),
            dept_id.to_string(),
            position.to_string(),
        ],
    )
    .await
    .expect("seed employee");
    id
}

/// Seed an inactive employee (status='inactive') for include_inactive testing.
pub async fn seed_inactive_employee(
    db: &libsql::Database,
    code: &str,
    name: &str,
    dept_id: &str,
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, position, \
         hire_date, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'inactive', '', NULL, 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            code.to_string(),
            name.to_string(),
            dept_id.to_string(),
        ],
    )
    .await
    .expect("seed employee");
    id
}

/// Seed a daily_record row. `day_shift_type` is the per-day ACTUAL shift
/// (W-6 source for night-premium gating). `leave_id_opt` attaches a leave
/// overlay if Some.
#[allow(clippy::too_many_arguments)]
pub async fn seed_daily_record(
    db: &libsql::Database,
    employee_id: &str,
    dept_id: &str,
    anchor_date: &str,        // YYYY-MM-DD
    day_shift_type: &str,     // "day" | "night" | "mixed"
    work_min: i64,
    ot_min: i64,
    late_min: i64,
    is_rest_day_worked: i64,  // 0 | 1
    leave_id_opt: Option<&str>,
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    let leave_id_val: Option<String> = leave_id_opt.map(|s| s.to_string());
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, \
         work_minutes, overtime_minutes, late_minutes, early_departure_minutes, is_rest_day_worked, \
         entry_at, exit_at, leave_id, computed_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, NULL, NULL, ?10, unixepoch(), unixepoch(), unixepoch())",
        params![
            id.clone(),
            employee_id.to_string(),
            dept_id.to_string(),
            anchor_date.to_string(),
            day_shift_type.to_string(),
            work_min,
            ot_min,
            late_min,
            is_rest_day_worked,
            leave_id_val,
        ],
    )
    .await
    .expect("seed daily_record");
    id
}

/// Seed an active override on a daily_record. Only `override_work_minutes` is
/// exercised by the report (Pitfall 3 — override merge).
pub async fn seed_override(
    db: &libsql::Database,
    daily_record_id: &str,
    override_work_min: i64,
    overridden_by: &str,
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO daily_record_overrides (id, daily_record_id, override_work_minutes, \
         override_entry_at, override_exit_at, justification, evidence_path, overridden_by, \
         overridden_at, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, NULL, NULL, 'test override', NULL, ?4, unixepoch(), \
         'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            daily_record_id.to_string(),
            override_work_min,
            overridden_by.to_string(),
        ],
    )
    .await
    .expect("seed override");
    id
}

/// Seed a leave row. `from_date` / `to_date` are inclusive YYYY-MM-DD strings.
/// W-5 lever: tests can seed leaves WITHOUT corresponding daily_records to
/// verify the secondary aggregation correctly counts vacation/medical days.
pub async fn seed_leave(
    db: &libsql::Database,
    employee_id: &str,
    leave_type: &str,
    from_date: &str,
    to_date: &str,
    created_by: &str,
) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
         justification, evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'test', NULL, ?6, 'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            employee_id.to_string(),
            from_date.to_string(),
            to_date.to_string(),
            leave_type.to_string(),
            created_by.to_string(),
        ],
    )
    .await
    .expect("seed leave");
    id
}

/// Seed an anomaly row attached to a daily_record. Inserted into
/// daily_record_anomalies so the GROUP_CONCAT sub-query in service.rs picks
/// it up.
pub async fn seed_anomaly(db: &libsql::Database, daily_record_id: &str, code: &str) {
    let conn = db.connect().expect("connect");
    conn.execute(
        "INSERT INTO daily_record_anomalies (id, daily_record_id, code, detail, created_at) \
         VALUES (?1, ?2, ?3, NULL, unixepoch())",
        params![
            Uuid::new_v4().to_string(),
            daily_record_id.to_string(),
            code.to_string(),
        ],
    )
    .await
    .expect("seed anomaly");
}

/// Set tenant_info branding fields so the report header has non-empty values
/// for tests that assert on it. Default seed leaves them blank.
pub async fn set_tenant_branding(db: &libsql::Database, client_name: &str, client_rif: &str) {
    let conn = db.connect().expect("connect");
    conn.execute(
        "UPDATE tenant_info SET client_name = ?1, client_rif = ?2, updated_at = unixepoch() WHERE id = 1",
        params![client_name.to_string(), client_rif.to_string()],
    )
    .await
    .expect("set tenant branding");
}
