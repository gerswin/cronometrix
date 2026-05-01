use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use validator::Validate;

use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateEmployeeRequest, Employee, EmployeeListQuery, UpdateEmployeeRequest};
use super::service;
use crate::common::PaginatedResponse;

/// POST /api/v1/employees — Create a new employee.
/// Requires Admin or Supervisor role (enforced at router group level).
/// Returns 201 Created on success.
pub async fn create_employee(
    State(state): State<AppState>,
    Json(body): Json<CreateEmployeeRequest>,
) -> Result<(StatusCode, Json<Employee>), AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let employee = service::create_queued(&state, body).await?;

    Ok((StatusCode::CREATED, Json(employee)))
}

/// GET /api/v1/employees — List employees with optional pagination and filters.
/// Accessible by any authenticated role (Viewer can read per D-09).
pub async fn list_employees(
    State(state): State<AppState>,
    Query(query): Query<EmployeeListQuery>,
) -> Result<Json<PaginatedResponse<Employee>>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, query).await?;

    Ok(Json(result))
}

/// GET /api/v1/employees/:id — Get a single employee by ID.
/// Accessible by any authenticated role.
pub async fn get_employee(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Employee>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let employee = service::get_by_id(&conn, &id).await?;

    Ok(Json(employee))
}

/// PATCH /api/v1/employees/:id — Update employee fields.
/// Requires Admin or Supervisor role. Uses optimistic concurrency via version field.
/// Returns 200 with updated employee.
pub async fn update_employee(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateEmployeeRequest>,
) -> Result<Json<Employee>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let employee = service::update_queued(&state, &id, body).await?;

    Ok(Json(employee))
}

/// DELETE /api/v1/employees/:id — Soft-delete an employee (sets status=inactive).
/// Requires Admin role. Returns 204 No Content on success.
/// No SQL DELETE is executed — per T-01-16 the row is never physically removed.
/// On success, publishes a PurgeRequest to the PurgeWorker (D-15) if the channel is live.
pub async fn deactivate_employee(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    service::deactivate_queued(&state, &id).await?;

    // Publish purge request (D-15). None in test setups — silently skipped.
    if let Some(tx) = &state.purge_tx {
        let _ = tx.send(crate::workers::purge::PurgeRequest {
            employee_id: id.clone(),
        });
    }

    Ok(StatusCode::NO_CONTENT)
}
