use libsql::{params, Connection};
use uuid::Uuid;

use crate::auth::service as auth_service;
use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateUserRequest, UpdateUserRequest, User, UserListQuery};

const VALID_ROLES: &[&str] = &["admin", "supervisor", "viewer"];
const VALID_STATUSES: &[&str] = &["active", "inactive"];

fn validate_role(role: &str) -> Result<(), AppError> {
    if VALID_ROLES.contains(&role) {
        Ok(())
    } else {
        Err(AppError::Validation {
            code: "INVALID_ROLE",
            message: format!("role must be one of {:?}, got '{}'", VALID_ROLES, role),
        })
    }
}

fn validate_status(status: &str) -> Result<(), AppError> {
    if VALID_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(AppError::Validation {
            code: "INVALID_STATUS",
            message: format!(
                "status must be one of {:?}, got '{}'",
                VALID_STATUSES, status
            ),
        })
    }
}

fn row_to_user(row: libsql::Row) -> Result<User, AppError> {
    Ok(User {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        username: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        full_name: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        role: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        deleted_at: epoch_to_iso_opt(row.get(5).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(7).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(8).map_err(|e| AppError::Internal(e.into()))?),
    })
}

const SELECT_COLS: &str =
    "id, username, full_name, role, status, deleted_at, version, created_at, updated_at";

pub async fn create(state: &AppState, req: CreateUserRequest) -> Result<User, AppError> {
    validate_role(&req.role)?;
    let password_hash = auth_service::hash_password(&req.password)?;
    let id = Uuid::new_v4().to_string();

    let result = state
        .db_write
        .statement(
            "users.create",
            "INSERT INTO users \
             (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(id.clone()),
                libsql::Value::Text(req.username.clone()),
                libsql::Value::Text(req.full_name.clone()),
                libsql::Value::Text(password_hash),
                libsql::Value::Text(req.role.clone()),
            ],
        )
        .await;

    if let Err(e) = result {
        let msg = e.to_string();
        if msg.contains("UNIQUE constraint failed") && msg.contains("username") {
            return Err(AppError::Conflict {
                code: "USERNAME_EXISTS",
                message: format!("Username '{}' is already in use", req.username),
            });
        }
        return Err(AppError::from(e));
    }

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    get_by_id(&conn, &id).await
}

pub async fn list(
    conn: &Connection,
    query: UserListQuery,
) -> Result<PaginatedResponse<User>, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    let status = query.status.unwrap_or_else(|| "active".to_string());
    validate_status(&status)?;
    predicates.push(format!("status = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(status.clone()));
    fetch_values.push(libsql::Value::Text(status));

    if let Some(role) = query.role {
        validate_role(&role)?;
        predicates.push(format!("role = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(role.clone()));
        fetch_values.push(libsql::Value::Text(role));
    }

    let where_clause = format!("WHERE {}", predicates.join(" AND "));

    let count_sql = format!("SELECT COUNT(*) FROM users {}", where_clause);
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

    let fetch_sql = format!(
        "SELECT {} FROM users {} ORDER BY username ASC LIMIT ?{} OFFSET ?{}",
        SELECT_COLS,
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
        data.push(row_to_user(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

pub async fn get_by_id(conn: &Connection, id: &str) -> Result<User, AppError> {
    let row = conn
        .query(
            &format!("SELECT {} FROM users WHERE id = ?1", SELECT_COLS),
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "USER_NOT_FOUND",
            message: format!("User '{}' not found", id),
        })?;

    row_to_user(row)
}

/// Update a user. Self-protection rules:
///   - actor cannot demote themselves (role change blocked when id == actor_id)
///   - actor cannot deactivate themselves (status='inactive' blocked when id == actor_id)
pub async fn update(
    state: &AppState,
    actor_id: &str,
    id: &str,
    req: UpdateUserRequest,
) -> Result<User, AppError> {
    if let Some(role) = req.role.as_deref() {
        validate_role(role)?;
        if id == actor_id {
            return Err(AppError::Validation {
                code: "CANNOT_CHANGE_OWN_ROLE",
                message: "An admin cannot change their own role".to_string(),
            });
        }
    }
    if let Some(status) = req.status.as_deref() {
        validate_status(status)?;
        if status == "inactive" && id == actor_id {
            return Err(AppError::Validation {
                code: "CANNOT_DEACTIVATE_SELF",
                message: "An admin cannot deactivate their own account".to_string(),
            });
        }
    }

    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(full_name) = req.full_name {
        sets.push(format!("full_name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(full_name));
    }
    if let Some(role) = req.role {
        sets.push(format!("role = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(role));
    }
    if let Some(password) = req.password {
        let password_hash = auth_service::hash_password(&password)?;
        sets.push(format!("password_hash = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(password_hash));
        // Force refresh-token rotation: clear it so existing sessions die.
        sets.push("refresh_token_hash = NULL".to_string());
    }
    if let Some(status) = req.status {
        sets.push(format!("status = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(status));
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
        "UPDATE users SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let rows_affected = state
        .db_write
        .statement("users.update", sql, values)
        .await
        .map_err(AppError::from)?;

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM users WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "USER_NOT_FOUND",
                message: format!("User '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "User was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }
    get_by_id(&conn, id).await
}

/// Soft-delete: set status='inactive'. Self-deactivation forbidden.
pub async fn deactivate(
    state: &AppState,
    actor_id: &str,
    id: &str,
    version: i64,
) -> Result<User, AppError> {
    if id == actor_id {
        return Err(AppError::Validation {
            code: "CANNOT_DEACTIVATE_SELF",
            message: "An admin cannot deactivate their own account".to_string(),
        });
    }

    let rows_affected = state
        .db_write
        .statement(
            "users.deactivate",
            "UPDATE users SET status = 'inactive', deleted_at = unixepoch(), \
             refresh_token_hash = NULL, updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND version = ?2",
            vec![
                libsql::Value::Text(id.to_string()),
                libsql::Value::Integer(version),
            ],
        )
        .await
        .map_err(AppError::from)?;

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM users WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "USER_NOT_FOUND",
                message: format!("User '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "User was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }

    get_by_id(&conn, id).await
}
