use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use validator::Validate;

use crate::auth::rbac::AuthUser;
use crate::auth::scope::ActorScope;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateEmployeeRequest, Employee, EmployeeListQuery, UpdateEmployeeRequest};
use super::service;
use crate::common::PaginatedResponse;

/// H-11: 404 (not 403) when a scoped actor touches an employee outside its
/// department — returning 403 would leak that the employee exists.
fn out_of_scope_not_found(id: &str) -> AppError {
    AppError::NotFound {
        code: "EMPLOYEE_NOT_FOUND",
        message: format!("Employee '{}' not found", id),
    }
}

/// POST /api/v1/employees — Create a new employee.
/// Requires Admin or Supervisor role (enforced at router group level).
/// Returns 201 Created on success.
pub async fn create_employee(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateEmployeeRequest>,
) -> Result<(StatusCode, Json<Employee>), AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    // H-11: a scoped actor may only create within its own department — the
    // request's department_id is never trusted to widen the actor's scope.
    if !ActorScope::from_claims(&claims).permits(Some(&body.department_id)) {
        return Err(AppError::Forbidden);
    }

    let employee = service::create_queued(&state, body).await?;

    Ok((StatusCode::CREATED, Json(employee)))
}

/// GET /api/v1/employees — List employees with optional pagination and filters.
/// Accessible by any authenticated role (Viewer can read per D-09).
pub async fn list_employees(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(mut query): Query<EmployeeListQuery>,
) -> Result<Json<PaginatedResponse<Employee>>, AppError> {
    // H-11: impose the actor's department scope on the query. For a scoped
    // actor the filter is derived from identity, not the caller — it overrides
    // any department_id the caller supplied, so a scoped actor can never widen
    // beyond its own department. An unscoped actor (admin / org-wide) keeps the
    // caller's optional filter.
    if let Some(dept) = ActorScope::from_claims(&claims).department_id() {
        query.department_id = Some(dept.to_string());
    }

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, query).await?;

    Ok(Json(result))
}

/// GET /api/v1/employees/:id — Get a single employee by ID.
/// Accessible by any authenticated role.
pub async fn get_employee(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Employee>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let employee = service::get_by_id(&conn, &id).await?;

    // H-11: a scoped actor cannot see an employee outside its department; 404
    // (not 403) so the employee's existence is not leaked.
    if !ActorScope::from_claims(&claims).permits(Some(&employee.department_id)) {
        return Err(out_of_scope_not_found(&id));
    }

    Ok(Json(employee))
}

/// PATCH /api/v1/employees/:id — Update employee fields.
/// Requires Admin or Supervisor role. Uses optimistic concurrency via version field.
/// Returns 200 with updated employee.
pub async fn update_employee(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateEmployeeRequest>,
) -> Result<Json<Employee>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let scope = ActorScope::from_claims(&claims);

    // H-11: the target employee must be within the actor's scope (404 if not,
    // so existence is not leaked), and a scoped actor may not move an employee
    // into a department it does not own.
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let existing = service::get_by_id(&conn, &id).await?;
    if !scope.permits(Some(&existing.department_id)) {
        return Err(out_of_scope_not_found(&id));
    }
    if let Some(target_dept) = &body.department_id {
        if !scope.permits(Some(target_dept)) {
            return Err(AppError::Forbidden);
        }
    }

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

    // H-10: purge the enrolled face template — its purpose ends with employment.
    // Runs synchronously here (right after the row is set inactive) so there is
    // no re-activation window, and is UNCONDITIONAL of the device-side revocation
    // below (best-effort). Best-effort itself: deactivation already committed, so
    // a purge failure is logged loudly but must not fail the response or
    // un-deactivate the employee. Touches only enrollments_root — attendance
    // evidence (H-09) is untouched.
    if let Err(e) = crate::enrollments::service::purge_enrolled_faces(&state, &id).await {
        tracing::error!(employee_id = %id, err = %e, "H-10: failed to purge enrolled face template on deactivation");
    }

    // Publish purge request (D-15) — revokes the face mapping on the readers.
    // None in test setups — silently skipped.
    if let Some(tx) = &state.purge_tx {
        let _ = tx.send(crate::workers::purge::PurgeRequest {
            employee_id: id.clone(),
        });
    }

    Ok(StatusCode::NO_CONTENT)
}
