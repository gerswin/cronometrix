//! HTTP handlers for `/api/v1/daily-records` (viewer-or-above per D-09).

use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{DailyRecordListQuery, DailyRecordResponse};
use super::service;

/// GET /api/v1/daily-records — paginated list with optional employee/department/date filters.
pub async fn list_daily_records(
    State(state): State<AppState>,
    Query(q): Query<DailyRecordListQuery>,
) -> Result<Json<PaginatedResponse<DailyRecordResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, q).await?;
    Ok(Json(result))
}

/// GET /api/v1/daily-records/{id} — single record with anomalies attached.
pub async fn get_daily_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DailyRecordResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(service::get_by_id(&conn, &id).await?))
}
