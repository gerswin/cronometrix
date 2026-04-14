use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use validator::Validate;

use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateDepartmentRequest, Department, DepartmentListQuery, UpdateDepartmentRequest};
use super::service;
use crate::common::PaginatedResponse;

/// POST /api/v1/departments — Create a new department.
/// Requires Admin role (enforced at router group level).
/// Returns 201 Created on success.
pub async fn create_department(
    State(state): State<AppState>,
    Json(body): Json<CreateDepartmentRequest>,
) -> Result<(StatusCode, Json<Department>), AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let department = service::create(&conn, body).await?;

    Ok((StatusCode::CREATED, Json(department)))
}

/// GET /api/v1/departments — List departments with optional pagination.
/// Accessible by any authenticated role (Viewer can read per D-09).
pub async fn list_departments(
    State(state): State<AppState>,
    Query(query): Query<DepartmentListQuery>,
) -> Result<Json<PaginatedResponse<Department>>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, query).await?;

    Ok(Json(result))
}

/// GET /api/v1/departments/:id — Get a single department by ID.
/// Accessible by any authenticated role.
pub async fn get_department(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Department>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let department = service::get_by_id(&conn, &id).await?;

    Ok(Json(department))
}

/// PATCH /api/v1/departments/:id — Update department fields.
/// Requires Admin role. Uses optimistic concurrency via version field.
/// Returns 200 with updated department.
pub async fn update_department(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDepartmentRequest>,
) -> Result<Json<Department>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let department = service::update(&conn, &id, body).await?;

    Ok(Json(department))
}
