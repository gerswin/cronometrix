use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use libsql::params;
use tokio::time::timeout;
use validator::Validate;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;
use crate::supervisor::DeviceLifecycleEvent;

use super::models::{
    Command, CommandRequest, CommandResult, CreateDeviceRequest, DeviceListQuery,
    DeviceResponse, UpdateDeviceRequest,
};
use super::service::{self, CommandAuditOutcome};

/// Send a lifecycle event to the supervisor if one is attached to AppState.
/// No-op in contexts where the supervisor isn't running (Phase 1 / 02-01 /
/// 02-02 test harnesses construct AppState with `lifecycle_tx: None`).
fn emit_lifecycle(state: &AppState, ev: DeviceLifecycleEvent) {
    if let Some(tx) = state.lifecycle_tx.as_ref() {
        if let Err(e) = tx.send(ev.clone()) {
            tracing::warn!(err = %e, event = ?ev, "failed to emit lifecycle event");
        }
    }
}

/// Columns needed to detect which fields changed during PATCH (Pitfall 7).
/// Only connection-affecting changes trigger a Restart.
struct PreUpdateSnapshot {
    ip: String,
    port: i64,
    scheme: String,
    username: String,
    encrypted_password: String,
    allow_insecure_tls: bool,
    status: String,
}

async fn load_snapshot(conn: &libsql::Connection, id: &str) -> Result<Option<PreUpdateSnapshot>, AppError> {
    let mut rows = conn
        .query(
            "SELECT ip, port, scheme, username, encrypted_password, allow_insecure_tls, status \
             FROM devices WHERE id = ?1",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? else {
        return Ok(None);
    };
    let allow_int: i64 = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
    Ok(Some(PreUpdateSnapshot {
        ip: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        port: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        scheme: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        username: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        encrypted_password: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        allow_insecure_tls: allow_int != 0,
        status: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
    }))
}

/// POST /api/v1/devices — Admin only. Returns 201 Created.
///
/// Emits `DeviceLifecycleEvent::Start(id)` after a successful write so the
/// supervisor can spawn a per-device stream task without a process restart.
pub async fn create_device(
    State(state): State<AppState>,
    Json(body): Json<CreateDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let device = service::create(&conn, body, &state.config.device_creds_key).await?;

    emit_lifecycle(&state, DeviceLifecycleEvent::Start(device.id.clone()));

    Ok((StatusCode::CREATED, Json(device)))
}

/// GET /api/v1/devices — any authenticated role.
pub async fn list_devices(
    State(state): State<AppState>,
    Query(q): Query<DeviceListQuery>,
) -> Result<Json<PaginatedResponse<DeviceResponse>>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let page = service::list(&conn, q).await?;
    Ok(Json(page))
}

/// GET /api/v1/devices/:id — any authenticated role.
pub async fn get_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let device = service::get_by_id(&conn, &id).await?;
    Ok(Json(device))
}

/// PATCH /api/v1/devices/:id — Admin only.
///
/// Lifecycle semantics (Pitfall 7 — "device edit without supervisor restart"):
/// - ip / port / scheme / username / password / allow_insecure_tls / status
///   changing => emit Restart(id)
/// - name / direction only => NO lifecycle event (connection is unaffected;
///   `direction` is consumed by `ingest_pair` fresh from the new DB row each
///   time an event arrives).
pub async fn update_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;

    // Snapshot the pre-patch row so we can diff connection-affecting fields.
    let pre = load_snapshot(&conn, &id).await?;

    // Track whether the request itself touches a connection-affecting field.
    // Password diff is a special case — we have the cleartext in `body`, and
    // comparing to the existing CIPHERTEXT is ambiguous (the new ciphertext
    // is a fresh nonce + AEAD tag even for the same password). We treat any
    // `Some(_)` password field as a potential connection change to avoid
    // missing a restart.
    let req_touches_connection = body.ip.is_some()
        || body.port.is_some()
        || body.scheme.is_some()
        || body.username.is_some()
        || body.password.is_some()
        || body.allow_insecure_tls.is_some()
        || body.status.is_some();

    let device =
        service::update(&conn, &id, body, &state.config.device_creds_key).await?;

    if req_touches_connection {
        let changed = match pre {
            Some(snap) => {
                snap.ip != device.ip
                    || snap.port != device.port
                    || snap.scheme != device.scheme
                    || snap.username != device.username
                    || snap.allow_insecure_tls != device.allow_insecure_tls
                    || snap.status != device.status
                    || {
                        // Password: re-fetch the current ciphertext to see if it changed.
                        let mut rows = conn
                            .query(
                                "SELECT encrypted_password FROM devices WHERE id = ?1",
                                params![id.clone()],
                            )
                            .await
                            .map_err(|e| AppError::Internal(e.into()))?;
                        if let Some(row) =
                            rows.next().await.map_err(|e| AppError::Internal(e.into()))?
                        {
                            let new_ct: String =
                                row.get(0).map_err(|e| AppError::Internal(e.into()))?;
                            new_ct != snap.encrypted_password
                        } else {
                            false
                        }
                    }
            }
            None => true, // shouldn't happen — PATCH against missing row already errored
        };
        if changed {
            emit_lifecycle(&state, DeviceLifecycleEvent::Restart(id.clone()));
        }
    }

    Ok(Json(device))
}

/// DELETE /api/v1/devices/:id — Admin only. Returns 204.
///
/// Emits `DeviceLifecycleEvent::Stop(id)` so the supervisor cancels the
/// per-device task immediately — a deactivated device must not continue to
/// accumulate events.
pub async fn deactivate_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    service::deactivate(&conn, &id).await?;
    emit_lifecycle(&state, DeviceLifecycleEvent::Stop(id.clone()));
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/devices/:id/commands — Admin only.
///
/// Flow per D-09 + D-11:
/// 1. Parse + validate the command string (422 on unknown).
/// 2. Load device with decrypted password (404 if inactive/missing).
/// 3. Build DeviceConnection; fire the ISAPI call inside `timeout(10s, ...)`.
/// 4. Record the outcome in `command_audit_log` (EVERY branch: Ok / Err / Timeout).
/// 5. Map Ok -> 200 CommandResult; Err -> 502 DEVICE_ERROR; Timeout -> 504 DEVICE_TIMEOUT.
///
/// The audit write happens BEFORE the response is returned so a client disconnect
/// cannot lose the audit trail for a door-open event.
pub async fn dispatch_command(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(device_id): Path<String>,
    Json(body): Json<CommandRequest>,
) -> Result<Json<CommandResult>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let command = Command::from_request_str(&body.command).ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: format!(
            "command must be one of door_open, reboot, enrollment_mode (got '{}')",
            body.command
        ),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let device =
        service::get_decrypted(&conn, &device_id, &state.config.device_creds_key).await?;

    let isapi = DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )
    .map_err(|e| AppError::Internal(e.into()))?;

    let dispatched_at = chrono::Utc::now().timestamp();

    // Hold the future value explicitly to keep the match arms readable.
    let result = match command {
        Command::DoorOpen => timeout(Duration::from_secs(10), isapi.door_open()).await,
        Command::Reboot => timeout(Duration::from_secs(10), isapi.reboot()).await,
        Command::EnrollmentMode => {
            timeout(Duration::from_secs(10), isapi.enrollment_mode()).await
        }
    };

    let completed_at = chrono::Utc::now().timestamp();

    // Shape the audit outcome BEFORE consuming `result` for the response below.
    let audit_outcome = match &result {
        Ok(Ok(body)) => CommandAuditOutcome::Ok(body.clone()),
        Ok(Err(e)) => CommandAuditOutcome::Error {
            code: "DEVICE_ERROR",
            message: e.to_string(),
        },
        Err(_) => CommandAuditOutcome::Timeout,
    };

    service::write_command_audit(
        &conn,
        &claims.sub,
        &device_id,
        command,
        &audit_outcome,
        dispatched_at,
        completed_at,
    )
    .await?;

    match result {
        Ok(Ok(text)) => Ok(Json(CommandResult {
            outcome: "ok".to_string(),
            device_response: text,
            dispatched_at: crate::common::epoch_to_iso(dispatched_at),
            completed_at: crate::common::epoch_to_iso(completed_at),
        })),
        Ok(Err(e)) => Err(AppError::BadGateway {
            code: "DEVICE_ERROR",
            message: e.to_string(),
        }),
        Err(_) => Err(AppError::Timeout {
            code: "DEVICE_TIMEOUT",
            message: "Device did not respond within 10 seconds".to_string(),
        }),
    }
}
