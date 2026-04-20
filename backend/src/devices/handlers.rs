use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use tokio::time::timeout;
use validator::Validate;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

use super::models::{
    Command, CommandRequest, CommandResult, CreateDeviceRequest, DeviceListQuery,
    DeviceResponse, UpdateDeviceRequest,
};
use super::service::{self, CommandAuditOutcome};

/// POST /api/v1/devices — Admin only. Returns 201 Created.
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
    let device =
        service::update(&conn, &id, body, &state.config.device_creds_key).await?;
    Ok(Json(device))
}

/// DELETE /api/v1/devices/:id — Admin only. Returns 204.
pub async fn deactivate_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    service::deactivate(&conn, &id).await?;
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
