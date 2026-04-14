use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{auth::service, errors::AppError, state::AppState};

/// POST /api/v1/setup/status
/// Returns {"initialized": true/false} based on whether any user exists.
/// This endpoint is PUBLIC — no auth required.
pub async fn setup_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut rows = conn
        .query("SELECT COUNT(*) FROM users", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let count: i64 = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map(|row| row.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);

    Ok(Json(json!({ "initialized": count > 0 })))
}

/// Request body for POST /api/v1/setup/init
#[derive(Debug, Deserialize, Validate)]
pub struct SetupInitRequest {
    #[validate(length(min = 1, message = "Full name is required"))]
    pub full_name: String,
    #[validate(length(min = 1, message = "Username is required"))]
    pub username: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

/// POST /api/v1/setup/init
/// Creates the first admin user. Returns 409 if already initialized.
/// Per D-07: setup wizard endpoint blocked after first admin exists.
/// Per T-01-11: SELECT COUNT(*) + 409 prevents race condition duplicate admins.
pub async fn setup_init(
    State(state): State<AppState>,
    Json(body): Json<SetupInitRequest>,
) -> Result<impl IntoResponse, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    // Check if already initialized — T-01-11
    let mut rows = conn
        .query("SELECT COUNT(*) FROM users", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let count: i64 = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map(|row| row.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);

    if count > 0 {
        return Err(AppError::Conflict {
            code: "SETUP_ALREADY_COMPLETE",
            message: "System has already been initialized. An admin user already exists."
                .to_string(),
        });
    }

    let password_hash = service::hash_password(&body.password)?;
    let user_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![
            user_id.clone(),
            body.username,
            body.full_name,
            password_hash
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "message": "Admin user created successfully"
        })),
    ))
}
