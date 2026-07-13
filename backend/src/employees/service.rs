use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateEmployeeRequest, Employee, EmployeeListQuery, UpdateEmployeeRequest};

/// Convert an optional epoch-seconds (UTC midnight) to an ISO YYYY-MM-DD string.
/// Returns None if the input is None.
fn epoch_to_iso_date_opt(epoch: Option<i64>) -> Option<String> {
    epoch.and_then(|t| {
        chrono::DateTime::<chrono::Utc>::from_timestamp(t, 0)
            .map(|dt| dt.naive_utc().date().to_string())
    })
}

/// Parse a YYYY-MM-DD string to epoch seconds at UTC midnight.
/// Returns Ok(None) when input is None or empty (caller treats empty as "clear").
fn parse_hire_date(input: Option<&str>) -> Result<Option<i64>, AppError> {
    match input {
        Some(s) if !s.is_empty() => {
            let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
                AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: "hire_date must be YYYY-MM-DD".to_string(),
                }
            })?;
            let dt = date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "hire_date midnight conversion failed for {}",
                        s
                    ))
                })?
                .and_utc();
            Ok(Some(dt.timestamp()))
        }
        _ => Ok(None),
    }
}

/// Map a libSQL row to an Employee struct.
/// Column order is fixed by the SELECT statements below:
///   id, employee_code, name, department_id, status, position, hire_date,
///   base_salary_cents, deleted_at, version, created_at, updated_at
fn row_to_employee(row: libsql::Row) -> Result<Employee, AppError> {
    Ok(Employee {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_code: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        name: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        department_id: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        position: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
        hire_date: epoch_to_iso_date_opt(row.get(6).map_err(|e| AppError::Internal(e.into()))?),
        base_salary_cents: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        deleted_at: epoch_to_iso_opt(row.get(8).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(9).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(10).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(11).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// Create a new employee, validating that the referenced department exists and is active.
/// Returns Conflict with EMPLOYEE_CODE_EXISTS if employee_code is not unique.
/// Returns NotFound with DEPARTMENT_NOT_FOUND if department does not exist or is inactive.
pub async fn create(conn: &Connection, req: CreateEmployeeRequest) -> Result<Employee, AppError> {
    // EMP-04: Validate department exists and is active
    let dept_check = conn
        .query(
            "SELECT id FROM departments WHERE id = ?1 AND status = 'active'",
            params![req.department_id.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if dept_check.is_none() {
        return Err(AppError::NotFound {
            code: "DEPARTMENT_NOT_FOUND",
            message: format!(
                "Department '{}' not found or is inactive",
                req.department_id
            ),
        });
    }

    let id = Uuid::new_v4().to_string();

    // Phase 5 D-30a: position + hire_date.
    let position = req.position.clone().unwrap_or_default();
    let hire_date_epoch = parse_hire_date(req.hire_date.as_deref())?;
    let hire_date_value = match hire_date_epoch {
        Some(t) => libsql::Value::Integer(t),
        None => libsql::Value::Null,
    };

    let salary = req.base_salary_cents.unwrap_or(0);
    let result = conn
        .execute(
            "INSERT INTO employees (id, employee_code, name, department_id, status, position, hire_date, base_salary_cents, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, 1, unixepoch(), unixepoch())",
            libsql::params![
                id.clone(),
                req.employee_code.clone(),
                req.name.clone(),
                req.department_id.clone(),
                position,
                hire_date_value,
                salary,
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") && msg.contains("employee_code") {
                return Err(AppError::Conflict {
                    code: "EMPLOYEE_CODE_EXISTS",
                    message: format!("Employee code '{}' is already in use", req.employee_code),
                });
            }
            return Err(AppError::Internal(e.into()));
        }
        Ok(_) => {}
    }

    get_by_id(conn, &id).await
}

pub async fn create_queued(
    state: &AppState,
    req: CreateEmployeeRequest,
) -> Result<Employee, AppError> {
    let dept_check = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?
        .query(
            "SELECT id FROM departments WHERE id = ?1 AND status = 'active'",
            params![req.department_id.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if dept_check.is_none() {
        return Err(AppError::NotFound {
            code: "DEPARTMENT_NOT_FOUND",
            message: format!(
                "Department '{}' not found or is inactive",
                req.department_id
            ),
        });
    }

    let id = Uuid::new_v4().to_string();
    let position = req.position.clone().unwrap_or_default();
    let hire_date_epoch = parse_hire_date(req.hire_date.as_deref())?;
    let hire_date_value = match hire_date_epoch {
        Some(t) => libsql::Value::Integer(t),
        None => libsql::Value::Null,
    };

    let salary = req.base_salary_cents.unwrap_or(0);
    let result = state
        .db_write
        .execute(
            "INSERT INTO employees (id, employee_code, name, department_id, status, position, hire_date, base_salary_cents, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(id.clone()),
                libsql::Value::Text(req.employee_code.clone()),
                libsql::Value::Text(req.name.clone()),
                libsql::Value::Text(req.department_id.clone()),
                libsql::Value::Text(position),
                hire_date_value,
                libsql::Value::Integer(salary),
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") && msg.contains("employee_code") {
                return Err(AppError::Conflict {
                    code: "EMPLOYEE_CODE_EXISTS",
                    message: format!("Employee code '{}' is already in use", req.employee_code),
                });
            }
            return Err(AppError::Internal(e.into()));
        }
        Ok(_) => {}
    }

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    get_by_id(&conn, &id).await
}

/// List employees with optional pagination and filters.
/// Pagination clamped: limit 1..=100 (default 20), offset >= 0 (default 0) per D-12.
pub async fn list(
    conn: &Connection,
    query: EmployeeListQuery,
) -> Result<PaginatedResponse<Employee>, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    // Build dynamic WHERE predicates
    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    // Status filter (default to active if not specified)
    let status = query.status.unwrap_or_else(|| "active".to_string());
    predicates.push(format!("status = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(status.clone()));
    fetch_values.push(libsql::Value::Text(status));

    if let Some(name) = &query.name {
        predicates.push(format!("name LIKE ?{}", predicates.len() + 1));
        let pattern = format!("%{}%", name);
        count_values.push(libsql::Value::Text(pattern.clone()));
        fetch_values.push(libsql::Value::Text(pattern));
    }

    if let Some(dept_id) = &query.department_id {
        predicates.push(format!("department_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(dept_id.clone()));
        fetch_values.push(libsql::Value::Text(dept_id.clone()));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    // Count total matching rows
    let count_sql = format!("SELECT COUNT(*) FROM employees {}", where_clause);
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT query returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    // Fetch page
    let fetch_sql = format!(
        "SELECT id, employee_code, name, department_id, status, position, hire_date, base_salary_cents, deleted_at, version, created_at, updated_at \
         FROM employees {} ORDER BY name ASC LIMIT ?{} OFFSET ?{}",
        where_clause,
        fetch_values.len() + 1,
        fetch_values.len() + 2
    );

    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        data.push(row_to_employee(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

/// Get a single employee by ID. Returns NotFound with EMPLOYEE_NOT_FOUND if missing.
pub async fn get_by_id(conn: &Connection, id: &str) -> Result<Employee, AppError> {
    let row = conn
        .query(
            "SELECT id, employee_code, name, department_id, status, position, hire_date, base_salary_cents, deleted_at, version, created_at, updated_at \
             FROM employees WHERE id = ?1",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "EMPLOYEE_NOT_FOUND",
            message: format!("Employee '{}' not found", id),
        })?;

    row_to_employee(row)
}

/// Update an employee using optimistic concurrency (D-04).
/// Returns Conflict with VERSION_CONFLICT if the version does not match.
pub async fn update(
    conn: &Connection,
    id: &str,
    req: UpdateEmployeeRequest,
) -> Result<Employee, AppError> {
    // Validate that the department exists if being changed
    if let Some(ref dept_id) = req.department_id {
        let dept_check = conn
            .query(
                "SELECT id FROM departments WHERE id = ?1 AND status = 'active'",
                params![dept_id.clone()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        if dept_check.is_none() {
            return Err(AppError::NotFound {
                code: "DEPARTMENT_NOT_FOUND",
                message: format!("Department '{}' not found or is inactive", dept_id),
            });
        }
    }

    // Build dynamic SET clause
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }

    if let Some(dept_id) = req.department_id {
        sets.push(format!("department_id = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(dept_id));
    }

    if let Some(pos) = req.position {
        sets.push(format!("position = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(pos));
    }

    // hire_date: empty string clears (NULL); YYYY-MM-DD parses to epoch.
    if let Some(hd) = req.hire_date.as_deref() {
        let val = if hd.is_empty() {
            libsql::Value::Null
        } else {
            let epoch = parse_hire_date(Some(hd))?.ok_or_else(|| AppError::Validation {
                code: "VALIDATION_ERROR",
                message: "hire_date must be YYYY-MM-DD".to_string(),
            })?;
            libsql::Value::Integer(epoch)
        };
        sets.push(format!("hire_date = ?{}", values.len() + 1));
        values.push(val);
    }

    if let Some(salary) = req.base_salary_cents {
        sets.push(format!("base_salary_cents = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(salary));
    }

    if sets.is_empty() {
        // Nothing to update — return current state
        return get_by_id(conn, id).await;
    }

    sets.push("updated_at = unixepoch()".to_string());
    sets.push("version = version + 1".to_string());

    let set_clause = sets.join(", ");
    let version_param = values.len() + 1;
    let id_param = values.len() + 2;

    values.push(libsql::Value::Integer(req.version));
    values.push(libsql::Value::Text(id.to_string()));

    let sql = format!(
        "UPDATE employees SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let rows_affected = conn
        .execute(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        // Could be version conflict or missing row — check
        let exists = conn
            .query(
                "SELECT id FROM employees WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "EMPLOYEE_NOT_FOUND",
                message: format!("Employee '{}' not found", id),
            });
        }

        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message:
                "Employee was modified by another request. Fetch the latest version and retry."
                    .to_string(),
        });
    }

    get_by_id(conn, id).await
}

pub async fn update_queued(
    state: &AppState,
    id: &str,
    req: UpdateEmployeeRequest,
) -> Result<Employee, AppError> {
    if let Some(ref dept_id) = req.department_id {
        let dept_check = state
            .db
            .connect()
            .map_err(|e| AppError::Internal(e.into()))?
            .query(
                "SELECT id FROM departments WHERE id = ?1 AND status = 'active'",
                params![dept_id.clone()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if dept_check.is_none() {
            return Err(AppError::NotFound {
                code: "DEPARTMENT_NOT_FOUND",
                message: format!("Department '{}' not found or is inactive", dept_id),
            });
        }
    }

    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }
    if let Some(dept_id) = req.department_id {
        sets.push(format!("department_id = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(dept_id));
    }
    if let Some(pos) = req.position {
        sets.push(format!("position = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(pos));
    }
    if let Some(hd) = req.hire_date.as_deref() {
        let val = if hd.is_empty() {
            libsql::Value::Null
        } else {
            let epoch = parse_hire_date(Some(hd))?.ok_or_else(|| AppError::Validation {
                code: "VALIDATION_ERROR",
                message: "hire_date must be YYYY-MM-DD".to_string(),
            })?;
            libsql::Value::Integer(epoch)
        };
        sets.push(format!("hire_date = ?{}", values.len() + 1));
        values.push(val);
    }
    if let Some(salary) = req.base_salary_cents {
        sets.push(format!("base_salary_cents = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(salary));
    }

    if sets.is_empty() {
        let conn = state
            .db
            .connect()
            .map_err(|e| AppError::Internal(e.into()))?;
        return get_by_id(&conn, id).await;
    }

    sets.push("updated_at = unixepoch()".to_string());
    sets.push("version = version + 1".to_string());
    let set_clause = sets.join(", ");
    let version_param = values.len() + 1;
    let id_param = values.len() + 2;
    values.push(libsql::Value::Integer(req.version));
    values.push(libsql::Value::Text(id.to_string()));
    let sql = format!(
        "UPDATE employees SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let rows_affected = state
        .db_write
        .execute(sql, values)
        .await
        .map_err(AppError::Internal)?;

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM employees WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "EMPLOYEE_NOT_FOUND",
                message: format!("Employee '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message:
                "Employee was modified by another request. Fetch the latest version and retry."
                    .to_string(),
        });
    }
    get_by_id(&conn, id).await
}

/// Soft-delete an employee by setting status=inactive and deleted_at per D-03.
/// Returns NotFound with EMPLOYEE_NOT_FOUND if not found or already inactive.
pub async fn deactivate(conn: &Connection, id: &str) -> Result<(), AppError> {
    let rows_affected = conn
        .execute(
            "UPDATE employees SET status = 'inactive', deleted_at = unixepoch(), \
             updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND status = 'active'",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        return Err(AppError::NotFound {
            code: "EMPLOYEE_NOT_FOUND",
            message: format!("Employee '{}' not found or already inactive", id),
        });
    }

    Ok(())
}

pub async fn deactivate_queued(state: &AppState, id: &str) -> Result<(), AppError> {
    let rows_affected = state
        .db_write
        .execute(
            "UPDATE employees SET status = 'inactive', deleted_at = unixepoch(), \
             updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND status = 'active'",
            vec![libsql::Value::Text(id.to_string())],
        )
        .await
        .map_err(AppError::Internal)?;

    if rows_affected == 0 {
        return Err(AppError::NotFound {
            code: "EMPLOYEE_NOT_FOUND",
            message: format!("Employee '{}' not found or already inactive", id),
        });
    }

    Ok(())
}
