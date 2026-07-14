//! Reports HTTP handlers.
//!
//! - `generate_json` — `POST /api/v1/reports/json` returns the JSON payload.
//! - `generate_excel` — `POST /api/v1/reports/excel` returns binary xlsx bytes
//!   with attachment Content-Disposition headers (D-22). The synchronous
//!   `excel::build_workbook` is wrapped in `tokio::task::spawn_blocking` to
//!   avoid blocking the async runtime (Pitfall 6).

use super::excel;
use super::models::{ReportParamsRequest, ReportPayload};
use super::service;
use crate::{auth::rbac::AuthUser, errors::AppError, state::AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json},
};
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
    let payload = service::compute_report(&state, &params).await?;
    service::record_export(&state, &claims.sub, &params, "json").await?;
    Ok(Json(payload))
}

/// `POST /api/v1/reports/excel` — generates the Phase 5 pre-payroll workbook
/// and returns the xlsx bytes inline.
///
/// Response headers (D-22, Pitfall 9):
/// - `Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`
/// - `Content-Disposition: attachment; filename="prenomina_{from}_{to}.xlsx"`
///
/// Gated by `require_supervisor_or_above` at the route layer (D-20). The audit
/// is recorded only after the workbook is fully built, and before any bytes are
/// returned. The synchronous workbook builder is wrapped in
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime (Pitfall 6
/// / T-05-15).
pub async fn generate_excel(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(params): Json<ReportParamsRequest>,
) -> Result<axum::response::Response, AppError> {
    params.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let payload = service::compute_report(&state, &params).await?;

    // Pitfall 6: spawn_blocking to avoid blocking the async runtime. The xlsx
    // builder is CPU-bound and synchronous (zip compression + format string
    // building), so it must NOT run on the tokio worker thread pool reserved
    // for async I/O.
    let bytes = tokio::task::spawn_blocking(move || excel::build_workbook(&payload))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("spawn_blocking join: {}", e)))??;

    service::record_export(&state, &claims.sub, &params, "excel").await?;

    let filename = format!("prenomina_{}_{}.xlsx", params.from_date, params.to_date);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    // Pitfall 9: filename always quoted per RFC 6266.
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::try_from(format!("attachment; filename=\"{}\"", filename))
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid filename header")))?,
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}
