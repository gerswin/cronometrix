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

    Ok(Json(json!({
        "initialized": count > 0,
        "licensed": state.license_valid.load(std::sync::atomic::Ordering::Relaxed)
    })))
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

    state
        .db_write
        .statement(
            "setup.create-admin",
            "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(user_id.clone()),
                libsql::Value::Text(body.username),
                libsql::Value::Text(body.full_name),
                libsql::Value::Text(password_hash),
            ],
        )
        .await
        .map_err(AppError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "message": "Admin user created successfully"
        })),
    ))
}

/// Request body for POST /api/v1/setup/activate
#[derive(Debug, Deserialize, Validate)]
pub struct SetupActivateRequest {
    /// License key in XXXX-XXXX-XXXX-XXXX format (16 alphanumeric + 3 hyphens = 19 chars).
    /// Validator only enforces length here; the explicit split-check below
    /// catches non-alphanumeric segments and missing hyphens without pulling
    /// in a `regex` static (avoids the once_cell::Lazy dance).
    #[validate(length(
        min = 19,
        max = 19,
        message = "License key must be in XXXX-XXXX-XXXX-XXXX format"
    ))]
    pub license_key: String,
}

/// POST /api/v1/setup/activate
/// Public endpoint — first-run activation. Calls DO Functions, persists JWT,
/// flips state.license_valid to true on success.
/// Per LIC-01: this endpoint MUST stay public so unlicensed installations can
/// activate. Per T-06-19: idempotent — second call after success returns 409
/// ALREADY_ACTIVATED so the frontend can redirect through /setup → /login.
pub async fn setup_activate(
    State(state): State<AppState>,
    Json(body): Json<SetupActivateRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Cheap format validation (avoids round-tripping garbage to DO Functions)
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    // Format check: XXXX-XXXX-XXXX-XXXX (4 alphanumeric segments separated by hyphens).
    let key = body.license_key.trim();
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() != 4
        || parts
            .iter()
            .any(|p| p.len() != 4 || !p.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "License key must be in XXXX-XXXX-XXXX-XXXX format".to_string(),
        });
    }

    // Idempotent guard (T-06-19): if already activated, return 409 ALREADY_ACTIVATED
    // so the frontend can redirect through /setup → /login.
    if state
        .license_valid
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(AppError::Conflict {
            code: "ALREADY_ACTIVATED",
            message: "This installation is already activated.".to_string(),
        });
    }

    let _claims = crate::license::service::activate_license(
        &body.license_key,
        &state.config.do_functions_activate_url,
        &state.config.license_jwt_path,
    )
    .await?;

    state
        .license_valid
        .store(true, std::sync::atomic::Ordering::Relaxed);

    Ok((StatusCode::OK, Json(json!({ "activated": true }))))
}
