use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use validator::Validate;

use crate::auth::models::Claims;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateUserRequest, UpdateUserRequest, User, UserListQuery};
use super::service;

/// POST /api/v1/users — Create a new user. Admin only.
pub async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let user = service::create(&state, body).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

/// GET /api/v1/users — List users with pagination. Admin only.
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserListQuery>,
) -> Result<Json<PaginatedResponse<User>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, query).await?;
    Ok(Json(result))
}

/// GET /api/v1/users/:id — Get a user by ID. Admin only.
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<User>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let user = service::get_by_id(&conn, &id).await?;
    Ok(Json(user))
}

/// PATCH /api/v1/users/:id — Update user (full_name, role, password, status).
/// Admin only. Optimistic concurrency via version. Self cannot demote/deactivate.
pub async fn update_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<User>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let user = service::update(&state, &claims.sub, &id, body).await?;
    Ok(Json(user))
}

#[derive(Debug, Deserialize)]
pub struct DeactivateQuery {
    pub version: i64,
}

/// DELETE /api/v1/users/:id?version=N — Soft-delete (status='inactive').
/// Admin only. Cannot deactivate self.
pub async fn deactivate_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(q): Query<DeactivateQuery>,
) -> Result<StatusCode, AppError> {
    service::deactivate(&state, &claims.sub, &id, q.version).await?;
    Ok(StatusCode::NO_CONTENT)
}
