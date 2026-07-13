use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{
    CreateDepartmentRequest, Department, DepartmentListQuery, UpdateDepartmentRequest,
};

/// Validate that lunch_mode is "fixed" or "punch" and that lunch_duration_min
/// is present and positive when lunch_mode is "fixed".
fn validate_lunch(lunch_mode: &str, lunch_duration_min: Option<i64>) -> Result<(), AppError> {
    match lunch_mode {
        "fixed" | "punch" => {}
        other => {
            return Err(AppError::Validation {
                code: "INVALID_LUNCH_MODE",
                message: format!("lunch_mode must be 'fixed' or 'punch', got '{}'", other),
            });
        }
    }

    if lunch_mode == "fixed" {
        match lunch_duration_min {
            None | Some(0) => {
                return Err(AppError::Validation {
                    code: "LUNCH_DURATION_REQUIRED",
                    message:
                        "lunch_duration_min is required and must be > 0 when lunch_mode is 'fixed'"
                            .to_string(),
                });
            }
            Some(d) if d <= 0 => {
                return Err(AppError::Validation {
                    code: "LUNCH_DURATION_REQUIRED",
                    message: "lunch_duration_min must be > 0 when lunch_mode is 'fixed'"
                        .to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(())
}

/// Map a libSQL row to a Department struct.
fn row_to_department(row: libsql::Row) -> Result<Department, AppError> {
    Ok(Department {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        name: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        base_salary_cents: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        shift_start_time: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        shift_end_time: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        lunch_mode: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
        lunch_duration_min: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        deleted_at: epoch_to_iso_opt(row.get(8).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(9).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(10).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(11).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// Create a new department. Returns Conflict with DEPARTMENT_NAME_EXISTS if name is not unique.
pub async fn create(
    conn: &Connection,
    req: CreateDepartmentRequest,
) -> Result<Department, AppError> {
    validate_lunch(&req.lunch_mode, req.lunch_duration_min)?;

    let id = Uuid::new_v4().to_string();

    let result = conn
        .execute(
            "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
             lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 1, unixepoch(), unixepoch())",
            params![
                id.clone(),
                req.name.clone(),
                req.base_salary_cents,
                req.shift_start_time.clone(),
                req.shift_end_time.clone(),
                req.lunch_mode.clone(),
                req.lunch_duration_min
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") && msg.contains("name") {
                return Err(AppError::Conflict {
                    code: "DEPARTMENT_NAME_EXISTS",
                    message: format!("Department name '{}' is already in use", req.name),
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
    req: CreateDepartmentRequest,
) -> Result<Department, AppError> {
    validate_lunch(&req.lunch_mode, req.lunch_duration_min)?;
    let id = Uuid::new_v4().to_string();
    let result = state
        .db_write
        .execute(
            "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
             lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(id.clone()),
                libsql::Value::Text(req.name.clone()),
                libsql::Value::Integer(req.base_salary_cents),
                libsql::Value::Text(req.shift_start_time.clone()),
                libsql::Value::Text(req.shift_end_time.clone()),
                libsql::Value::Text(req.lunch_mode.clone()),
                req.lunch_duration_min
                    .map(libsql::Value::Integer)
                    .unwrap_or(libsql::Value::Null),
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") && msg.contains("name") {
                return Err(AppError::Conflict {
                    code: "DEPARTMENT_NAME_EXISTS",
                    message: format!("Department name '{}' is already in use", req.name),
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

/// List departments with optional pagination and status filter per D-12.
pub async fn list(
    conn: &Connection,
    query: DepartmentListQuery,
) -> Result<PaginatedResponse<Department>, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    let status = query.status.unwrap_or_else(|| "active".to_string());
    predicates.push(format!("status = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(status.clone()));
    fetch_values.push(libsql::Value::Text(status));

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    // Count total
    let count_sql = format!("SELECT COUNT(*) FROM departments {}", where_clause);
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
        "SELECT id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, deleted_at, version, created_at, updated_at \
         FROM departments {} ORDER BY name ASC LIMIT ?{} OFFSET ?{}",
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
        data.push(row_to_department(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

/// Get a single department by ID. Returns NotFound with DEPARTMENT_NOT_FOUND if missing.
pub async fn get_by_id(conn: &Connection, id: &str) -> Result<Department, AppError> {
    let row = conn
        .query(
            "SELECT id, name, base_salary_cents, shift_start_time, shift_end_time, \
             lunch_mode, lunch_duration_min, status, deleted_at, version, created_at, updated_at \
             FROM departments WHERE id = ?1",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "DEPARTMENT_NOT_FOUND",
            message: format!("Department '{}' not found", id),
        })?;

    row_to_department(row)
}

/// Update a department using optimistic concurrency (D-04).
/// Returns Conflict with VERSION_CONFLICT if the version does not match.
pub async fn update(
    conn: &Connection,
    id: &str,
    req: UpdateDepartmentRequest,
) -> Result<Department, AppError> {
    // If lunch_mode is being changed, validate consistency
    let lunch_mode_to_validate = req.lunch_mode.as_deref();
    if let Some(mode) = lunch_mode_to_validate {
        validate_lunch(mode, req.lunch_duration_min)?;
    }

    // Build dynamic SET clause
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }

    if let Some(salary) = req.base_salary_cents {
        sets.push(format!("base_salary_cents = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(salary));
    }

    if let Some(start) = req.shift_start_time {
        sets.push(format!("shift_start_time = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(start));
    }

    if let Some(end) = req.shift_end_time {
        sets.push(format!("shift_end_time = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(end));
    }

    if let Some(mode) = req.lunch_mode {
        sets.push(format!("lunch_mode = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(mode));
    }

    if let Some(dur) = req.lunch_duration_min {
        sets.push(format!("lunch_duration_min = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(dur));
    }

    if sets.is_empty() {
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
        "UPDATE departments SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let rows_affected = conn
        .execute(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM departments WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "DEPARTMENT_NOT_FOUND",
                message: format!("Department '{}' not found", id),
            });
        }

        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message:
                "Department was modified by another request. Fetch the latest version and retry."
                    .to_string(),
        });
    }

    get_by_id(conn, id).await
}

pub async fn update_queued(
    state: &AppState,
    id: &str,
    req: UpdateDepartmentRequest,
) -> Result<Department, AppError> {
    let lunch_mode_to_validate = req.lunch_mode.as_deref();
    if let Some(mode) = lunch_mode_to_validate {
        validate_lunch(mode, req.lunch_duration_min)?;
    }

    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }
    if let Some(salary) = req.base_salary_cents {
        sets.push(format!("base_salary_cents = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(salary));
    }
    if let Some(start) = req.shift_start_time {
        sets.push(format!("shift_start_time = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(start));
    }
    if let Some(end) = req.shift_end_time {
        sets.push(format!("shift_end_time = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(end));
    }
    if let Some(mode) = req.lunch_mode {
        sets.push(format!("lunch_mode = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(mode));
    }
    if let Some(dur) = req.lunch_duration_min {
        sets.push(format!("lunch_duration_min = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(dur));
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
        "UPDATE departments SET {} WHERE id = ?{} AND version = ?{}",
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
                "SELECT id FROM departments WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "DEPARTMENT_NOT_FOUND",
                message: format!("Department '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message:
                "Department was modified by another request. Fetch the latest version and retry."
                    .to_string(),
        });
    }
    get_by_id(&conn, id).await
}
