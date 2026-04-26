//! Reports HTTP handlers — Task 2 will populate `generate_json` end-to-end.

use super::models::{ReportParamsRequest, ReportPayload};
use super::service;
use crate::{auth::rbac::AuthUser, errors::AppError, state::AppState};
use axum::{extract::State, response::Json};
use validator::Validate;

/// `POST /api/v1/reports/json` — generates a per-employee aggregated report.
///
/// Gated by `require_supervisor_or_above` at the route layer. Validates the
/// payload, then delegates to `service::compute_report`.
pub async fn generate_json(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(params): Json<ReportParamsRequest>,
) -> Result<Json<ReportPayload>, AppError> {
    params.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;
    let payload = service::compute_report(&state, &claims.sub, &params, "json").await?;
    Ok(Json(payload))
}
