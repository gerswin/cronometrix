use axum::{extract::State, Json};
use validator::Validate;

use crate::errors::AppError;
use crate::state::AppState;

use super::models::{TenantInfo, UpdateTenantInfoRequest};
use super::service;

/// GET /api/v1/tenant-info — Return the singleton tenant_info row.
/// Accessible by any authenticated role (Viewer can read per Phase 1 D-09).
pub async fn get_tenant_info(State(state): State<AppState>) -> Result<Json<TenantInfo>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let info = service::get_tenant_info(&conn).await?;
    Ok(Json(info))
}

/// PATCH /api/v1/tenant-info — Update tenant_info fields.
/// Requires Admin role (enforced at router group level).
/// Uses optimistic concurrency via version field per D-04.
pub async fn patch_tenant_info(
    State(state): State<AppState>,
    Json(body): Json<UpdateTenantInfoRequest>,
) -> Result<Json<TenantInfo>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let info = service::update_tenant_info_queued(&state, body).await?;
    Ok(Json(info))
}
