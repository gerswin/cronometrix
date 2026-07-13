use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::crypto;
use super::models::{
    validate_direction, validate_ip, validate_scheme, validate_status, Command,
    CreateDeviceRequest, DeviceListQuery, DeviceResponse, DeviceWithPlaintext, UpdateDeviceRequest,
};

/// Outcome tag written to `command_audit_log.outcome`.
/// Carries the variant-specific metadata (device body on success, error details on failure).
#[derive(Debug)]
pub enum CommandAuditOutcome {
    Ok(String),
    Error { code: &'static str, message: String },
    Timeout,
}

impl CommandAuditOutcome {
    fn outcome_str(&self) -> &'static str {
        match self {
            CommandAuditOutcome::Ok(_) => "ok",
            CommandAuditOutcome::Error { .. } => "error",
            CommandAuditOutcome::Timeout => "timeout",
        }
    }
}

/// Map a SELECT row (safe columns only, NO encrypted_password) into DeviceResponse.
fn row_to_device(row: libsql::Row) -> Result<DeviceResponse, AppError> {
    let allow_int: i64 = row.get(7).map_err(|e| AppError::Internal(e.into()))?;
    Ok(DeviceResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        name: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        ip: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        port: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        scheme: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        username: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
        direction: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        allow_insecure_tls: allow_int != 0,
        connection_state: row.get(8).map_err(|e| AppError::Internal(e.into()))?,
        last_seen_at: epoch_to_iso_opt(row.get(9).map_err(|e| AppError::Internal(e.into()))?),
        status: row.get(10).map_err(|e| AppError::Internal(e.into()))?,
        deleted_at: epoch_to_iso_opt(row.get(11).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(12).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(13).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(14).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// SELECT columns for DeviceResponse mapper — encrypted_password is DELIBERATELY absent
/// (RESEARCH § Security Domain rule #2 + D-03).
const DEVICE_SELECT_COLS: &str =
    "id, name, ip, port, scheme, username, direction, allow_insecure_tls, \
     connection_state, last_seen_at, status, deleted_at, version, created_at, updated_at";

/// Translate a `validator::ValidationErrors` into our VALIDATION_ERROR envelope.
fn val_err(msg: impl Into<String>) -> AppError {
    AppError::Validation {
        code: "VALIDATION_ERROR",
        message: msg.into(),
    }
}

/// Create a new device. Encrypts the password with AES-256-GCM per D-01 before
/// INSERT. Returns `Conflict(DEVICE_IP_EXISTS)` on unique-index violation.
pub async fn create(
    conn: &Connection,
    req: CreateDeviceRequest,
    key: &[u8; 32],
) -> Result<DeviceResponse, AppError> {
    // Enum/format checks that validator::Validate can't express inline.
    validate_ip(&req.ip).map_err(val_err)?;
    validate_scheme(&req.scheme).map_err(val_err)?;
    validate_direction(&req.direction).map_err(val_err)?;

    let encrypted_password =
        crypto::encrypt_password(&req.password, key).map_err(|e| AppError::Internal(e.into()))?;

    let id = Uuid::new_v4().to_string();
    let allow_int: i64 = if req.allow_insecure_tls { 1 } else { 0 };

    let result = conn
        .execute(
            "INSERT INTO devices (\
                 id, name, ip, port, scheme, username, encrypted_password, \
                 direction, allow_insecure_tls, connection_state, status, version, \
                 created_at, updated_at\
             ) VALUES (\
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'offline', 'active', 1, unixepoch(), unixepoch()\
             )",
            params![
                id.clone(),
                req.name.clone(),
                req.ip.clone(),
                req.port,
                req.scheme.clone(),
                req.username.clone(),
                encrypted_password,
                req.direction.clone(),
                allow_int,
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            // SQLite reports the partial unique index by index name when it fires.
            if msg.contains("UNIQUE constraint failed")
                && (msg.contains("idx_devices_ip_port_active")
                    || (msg.contains("devices.ip") && msg.contains("devices.port")))
            {
                return Err(AppError::Conflict {
                    code: "DEVICE_IP_EXISTS",
                    message: format!("Device with IP {}:{} is already active", req.ip, req.port),
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
    req: CreateDeviceRequest,
    key: &[u8; 32],
) -> Result<DeviceResponse, AppError> {
    validate_ip(&req.ip).map_err(val_err)?;
    validate_scheme(&req.scheme).map_err(val_err)?;
    validate_direction(&req.direction).map_err(val_err)?;

    let encrypted_password =
        crypto::encrypt_password(&req.password, key).map_err(|e| AppError::Internal(e.into()))?;
    let id = Uuid::new_v4().to_string();
    let allow_int: i64 = if req.allow_insecure_tls { 1 } else { 0 };

    let result = state
        .db_write
        .execute(
            "INSERT INTO devices (\
                 id, name, ip, port, scheme, username, encrypted_password, \
                 direction, allow_insecure_tls, connection_state, status, version, \
                 created_at, updated_at\
             ) VALUES (\
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'offline', 'active', 1, unixepoch(), unixepoch()\
             )",
            vec![
                libsql::Value::Text(id.clone()),
                libsql::Value::Text(req.name.clone()),
                libsql::Value::Text(req.ip.clone()),
                libsql::Value::Integer(req.port),
                libsql::Value::Text(req.scheme.clone()),
                libsql::Value::Text(req.username.clone()),
                libsql::Value::Text(encrypted_password),
                libsql::Value::Text(req.direction.clone()),
                libsql::Value::Integer(allow_int),
            ],
        )
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed")
                && (msg.contains("idx_devices_ip_port_active")
                    || (msg.contains("devices.ip") && msg.contains("devices.port")))
            {
                return Err(AppError::Conflict {
                    code: "DEVICE_IP_EXISTS",
                    message: format!("Device with IP {}:{} is already active", req.ip, req.port),
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

/// List devices with filters + pagination. Viewer and above can read (routed accordingly).
pub async fn list(
    conn: &Connection,
    q: DeviceListQuery,
) -> Result<PaginatedResponse<DeviceResponse>, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    // Default to "active" like employees/departments.
    let status = q.status.unwrap_or_else(|| "active".to_string());
    predicates.push(format!("status = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(status.clone()));
    fetch_values.push(libsql::Value::Text(status));

    if let Some(direction) = q.direction {
        // Cheap guard against CHECK violations; not authoritative.
        validate_direction(&direction).map_err(val_err)?;
        predicates.push(format!("direction = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(direction.clone()));
        fetch_values.push(libsql::Value::Text(direction));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM devices {}", where_clause);
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT {cols} FROM devices {where_clause} \
         ORDER BY name ASC LIMIT ?{lim} OFFSET ?{off}",
        cols = DEVICE_SELECT_COLS,
        where_clause = where_clause,
        lim = fetch_values.len() + 1,
        off = fetch_values.len() + 2,
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
        data.push(row_to_device(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

pub async fn get_by_id(conn: &Connection, id: &str) -> Result<DeviceResponse, AppError> {
    let sql = format!("SELECT {} FROM devices WHERE id = ?1", DEVICE_SELECT_COLS);
    let row = conn
        .query(&sql, params![id.to_string()])
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "DEVICE_NOT_FOUND",
            message: format!("Device '{}' not found", id),
        })?;

    row_to_device(row)
}

/// PATCH. Follows the employees optimistic-concurrency pattern. Re-encrypts the
/// password when `req.password` is present (D-04: password rotation uses PATCH).
///
/// TODO(02-03): when `ip`/`port`/`scheme`/`username`/`password`/`allow_insecure_tls`/`status`
/// changes, the alertStream supervisor introduced in plan 02-03 should observe the
/// updated_at/version bump and reconcile its per-device task. Phase 2-01 does not
/// start the supervisor, so we emit no lifecycle event here.
pub async fn update(
    conn: &Connection,
    id: &str,
    req: UpdateDeviceRequest,
    key: &[u8; 32],
) -> Result<DeviceResponse, AppError> {
    if let Some(ip) = req.ip.as_deref() {
        validate_ip(ip).map_err(val_err)?;
    }
    if let Some(scheme) = req.scheme.as_deref() {
        validate_scheme(scheme).map_err(val_err)?;
    }
    if let Some(direction) = req.direction.as_deref() {
        validate_direction(direction).map_err(val_err)?;
    }
    if let Some(status) = req.status.as_deref() {
        validate_status(status).map_err(val_err)?;
    }

    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }
    if let Some(ip) = req.ip {
        sets.push(format!("ip = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(ip));
    }
    if let Some(port) = req.port {
        sets.push(format!("port = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(port));
    }
    if let Some(scheme) = req.scheme {
        sets.push(format!("scheme = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(scheme));
    }
    if let Some(username) = req.username {
        sets.push(format!("username = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(username));
    }
    if let Some(password) = req.password {
        let encrypted =
            crypto::encrypt_password(&password, key).map_err(|e| AppError::Internal(e.into()))?;
        sets.push(format!("encrypted_password = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(encrypted));
    }
    if let Some(direction) = req.direction {
        sets.push(format!("direction = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(direction));
    }
    if let Some(allow) = req.allow_insecure_tls {
        sets.push(format!("allow_insecure_tls = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(if allow { 1 } else { 0 }));
    }
    if let Some(status) = req.status {
        sets.push(format!("status = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(status));
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
        "UPDATE devices SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let result = conn.execute(&sql, libsql::params_from_iter(values)).await;

    let rows_affected = match result {
        Ok(n) => n,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed")
                && (msg.contains("idx_devices_ip_port_active")
                    || (msg.contains("devices.ip") && msg.contains("devices.port")))
            {
                return Err(AppError::Conflict {
                    code: "DEVICE_IP_EXISTS",
                    message: "Another active device already uses this IP:port".to_string(),
                });
            }
            return Err(AppError::Internal(e.into()));
        }
    };

    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM devices WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "DEVICE_NOT_FOUND",
                message: format!("Device '{}' not found", id),
            });
        }

        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Device was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }

    get_by_id(conn, id).await
}

pub async fn update_queued(
    state: &AppState,
    id: &str,
    req: UpdateDeviceRequest,
    key: &[u8; 32],
) -> Result<DeviceResponse, AppError> {
    if let Some(ip) = req.ip.as_deref() {
        validate_ip(ip).map_err(val_err)?;
    }
    if let Some(scheme) = req.scheme.as_deref() {
        validate_scheme(scheme).map_err(val_err)?;
    }
    if let Some(direction) = req.direction.as_deref() {
        validate_direction(direction).map_err(val_err)?;
    }
    if let Some(status) = req.status.as_deref() {
        validate_status(status).map_err(val_err)?;
    }

    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();
    if let Some(name) = req.name {
        sets.push(format!("name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(name));
    }
    if let Some(ip) = req.ip {
        sets.push(format!("ip = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(ip));
    }
    if let Some(port) = req.port {
        sets.push(format!("port = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(port));
    }
    if let Some(scheme) = req.scheme {
        sets.push(format!("scheme = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(scheme));
    }
    if let Some(username) = req.username {
        sets.push(format!("username = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(username));
    }
    if let Some(password) = req.password {
        let encrypted =
            crypto::encrypt_password(&password, key).map_err(|e| AppError::Internal(e.into()))?;
        sets.push(format!("encrypted_password = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(encrypted));
    }
    if let Some(direction) = req.direction {
        sets.push(format!("direction = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(direction));
    }
    if let Some(allow) = req.allow_insecure_tls {
        sets.push(format!("allow_insecure_tls = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(if allow { 1 } else { 0 }));
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
        "UPDATE devices SET {} WHERE id = ?{} AND version = ?{}",
        set_clause, id_param, version_param
    );

    let result = state.db_write.execute(sql, values).await;
    let rows_affected = match result {
        Ok(n) => n,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed")
                && (msg.contains("idx_devices_ip_port_active")
                    || (msg.contains("devices.ip") && msg.contains("devices.port")))
            {
                return Err(AppError::Conflict {
                    code: "DEVICE_IP_EXISTS",
                    message: "Another active device already uses this IP:port".to_string(),
                });
            }
            return Err(AppError::Internal(e.into()));
        }
    };

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM devices WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "DEVICE_NOT_FOUND",
                message: format!("Device '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Device was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }

    get_by_id(&conn, id).await
}

/// Soft-delete: status=inactive, deleted_at set. Unique index on (ip,port) is
/// partial (active-only), so the same IP+port can be re-registered afterwards.
pub async fn deactivate(conn: &Connection, id: &str) -> Result<(), AppError> {
    let rows_affected = conn
        .execute(
            "UPDATE devices SET status = 'inactive', deleted_at = unixepoch(), \
             updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND status = 'active'",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        return Err(AppError::NotFound {
            code: "DEVICE_NOT_FOUND",
            message: format!("Device '{}' not found or already inactive", id),
        });
    }

    Ok(())
}

pub async fn deactivate_queued(state: &AppState, id: &str) -> Result<(), AppError> {
    let rows_affected = state
        .db_write
        .execute(
            "UPDATE devices SET status = 'inactive', deleted_at = unixepoch(), \
             updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND status = 'active'",
            vec![libsql::Value::Text(id.to_string())],
        )
        .await
        .map_err(AppError::Internal)?;

    if rows_affected == 0 {
        return Err(AppError::NotFound {
            code: "DEVICE_NOT_FOUND",
            message: format!("Device '{}' not found or already inactive", id),
        });
    }

    Ok(())
}

/// Load a device with its plaintext password. Used by command dispatch and
/// the supervisor `Start`/`Restart` lifecycle branches.
///
/// The returned `DeviceWithPlaintext` is NOT Serialize/Debug-leakable; callers
/// must drop it as soon as the ISAPI call or stream completes.
pub async fn get_decrypted(
    conn: &Connection,
    id: &str,
    key: &[u8; 32],
) -> Result<DeviceWithPlaintext, AppError> {
    let row = conn
        .query(
            "SELECT id, name, ip, port, scheme, username, encrypted_password, \
                    direction, allow_insecure_tls, status, version \
             FROM devices WHERE id = ?1 AND status = 'active'",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "DEVICE_NOT_FOUND",
            message: format!("Active device '{}' not found", id),
        })?;

    let device_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let name: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let ip: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let port: i64 = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
    let scheme: String = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
    let username: String = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
    let encrypted: String = row.get(6).map_err(|e| AppError::Internal(e.into()))?;
    let direction: String = row.get(7).map_err(|e| AppError::Internal(e.into()))?;
    let allow_int: i64 = row.get(8).map_err(|e| AppError::Internal(e.into()))?;
    let status: String = row.get(9).map_err(|e| AppError::Internal(e.into()))?;
    let version: i64 = row.get(10).map_err(|e| AppError::Internal(e.into()))?;

    let password =
        crypto::decrypt_password(&encrypted, key).map_err(|e| AppError::Internal(e.into()))?;

    Ok(DeviceWithPlaintext {
        id: device_id,
        name,
        base_url: format!("{}://{}:{}", scheme, ip, port),
        username,
        password,
        direction,
        allow_insecure_tls: allow_int != 0,
        status,
        version,
    })
}

/// List ALL active devices with their plaintext passwords. Used exclusively
/// by the supervisor bootstrap path in `supervisor::Supervisor::run`.
///
/// Returns the same `DeviceWithPlaintext` shape as `get_decrypted` so both
/// lifecycle paths (bootstrap + Start/Restart) share one codegen path.
///
/// Failures to decrypt an individual row are logged and that row is skipped;
/// a single corrupt row (e.g. key rotation mid-migration) must not prevent
/// the supervisor from starting up for the rest of the fleet.
pub async fn list_active(
    conn: &Connection,
    key: &[u8; 32],
) -> Result<Vec<DeviceWithPlaintext>, AppError> {
    let mut rows = conn
        .query(
            "SELECT id, name, ip, port, scheme, username, encrypted_password, \
                    direction, allow_insecure_tls, status, version \
             FROM devices WHERE status = 'active' AND deleted_at IS NULL \
             ORDER BY created_at ASC",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let device_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let name: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let ip: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
        let port: i64 = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
        let scheme: String = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
        let username: String = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
        let encrypted: String = row.get(6).map_err(|e| AppError::Internal(e.into()))?;
        let direction: String = row.get(7).map_err(|e| AppError::Internal(e.into()))?;
        let allow_int: i64 = row.get(8).map_err(|e| AppError::Internal(e.into()))?;
        let status: String = row.get(9).map_err(|e| AppError::Internal(e.into()))?;
        let version: i64 = row.get(10).map_err(|e| AppError::Internal(e.into()))?;

        match crypto::decrypt_password(&encrypted, key) {
            Ok(password) => out.push(DeviceWithPlaintext {
                id: device_id,
                name,
                base_url: format!("{}://{}:{}", scheme, ip, port),
                username,
                password,
                direction,
                allow_insecure_tls: allow_int != 0,
                status,
                version,
            }),
            Err(e) => {
                tracing::error!(
                    device_id = %device_id,
                    err = %e,
                    "failed to decrypt device password during list_active — skipping"
                );
            }
        }
    }
    Ok(out)
}

/// Append a command_audit_log row. Writes on every dispatch outcome (ok/error/timeout).
pub async fn write_command_audit(
    conn: &Connection,
    actor_id: &str,
    device_id: &str,
    command: Command,
    outcome: &CommandAuditOutcome,
    dispatched_at: i64,
    completed_at: i64,
) -> Result<(), AppError> {
    let (result, error_code, error_message) = match outcome {
        CommandAuditOutcome::Ok(body) => (Some(body.clone()), None, None),
        CommandAuditOutcome::Error { code, message } => {
            (None, Some(code.to_string()), Some(message.clone()))
        }
        CommandAuditOutcome::Timeout => (
            None,
            Some("DEVICE_TIMEOUT".to_string()),
            Some("Device did not respond within 10 seconds".to_string()),
        ),
    };

    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO command_audit_log (\
             id, actor_id, device_id, command, outcome, result, error_code, error_message, \
             dispatched_at, completed_at\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            actor_id.to_string(),
            device_id.to_string(),
            command.as_str().to_string(),
            outcome.outcome_str().to_string(),
            result,
            error_code,
            error_message,
            dispatched_at,
            completed_at,
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}

pub async fn write_command_audit_queued(
    state: &crate::state::AppState,
    actor_id: &str,
    device_id: &str,
    command: Command,
    outcome: &CommandAuditOutcome,
    dispatched_at: i64,
    completed_at: i64,
) -> Result<(), AppError> {
    let (result, error_code, error_message) = match outcome {
        CommandAuditOutcome::Ok(body) => (Some(body.clone()), None, None),
        CommandAuditOutcome::Error { code, message } => {
            (None, Some(code.to_string()), Some(message.clone()))
        }
        CommandAuditOutcome::Timeout => (
            None,
            Some("DEVICE_TIMEOUT".to_string()),
            Some("Device did not respond within 10 seconds".to_string()),
        ),
    };

    let id = Uuid::new_v4().to_string();
    state
        .db_write
        .execute(
            "INSERT INTO command_audit_log (\
                 id, actor_id, device_id, command, outcome, result, error_code, error_message, \
                 dispatched_at, completed_at\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            vec![
                libsql::Value::Text(id),
                libsql::Value::Text(actor_id.to_string()),
                libsql::Value::Text(device_id.to_string()),
                libsql::Value::Text(command.as_str().to_string()),
                libsql::Value::Text(outcome.outcome_str().to_string()),
                result
                    .map(libsql::Value::Text)
                    .unwrap_or(libsql::Value::Null),
                error_code
                    .map(libsql::Value::Text)
                    .unwrap_or(libsql::Value::Null),
                error_message
                    .map(libsql::Value::Text)
                    .unwrap_or(libsql::Value::Null),
                libsql::Value::Integer(dispatched_at),
                libsql::Value::Integer(completed_at),
            ],
        )
        .await
        .map_err(AppError::Internal)?;
    Ok(())
}
