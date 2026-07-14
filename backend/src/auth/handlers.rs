use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde_json::json;
use time::Duration as TimeDuration;
use validator::Validate;

use crate::{
    auth::{
        models::{LoginRequest, LoginResponse, Role, UserInfo},
        service,
    },
    errors::AppError,
    state::AppState,
};

/// POST /api/v1/auth/login
/// Accepts username + password, returns access token + httpOnly refresh cookie.
/// Per T-01-08: generic "Invalid credentials" on failure — no username enumeration.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut rows = conn
        .query(
            "SELECT id, username, full_name, password_hash, role FROM users WHERE username = ?1 AND status = 'active'",
            [body.username.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or(AppError::Unauthorized)?;

    let user_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let username: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let full_name: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let password_hash: String = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
    let role_str: String = row.get(4).map_err(|e| AppError::Internal(e.into()))?;

    // Timing-safe comparison via Argon2id — T-01-05
    service::verify_password(&body.password, &password_hash)?;

    let role: Role = role_str
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid role in database")))?;

    let secret = state.config.jwt_secret.as_bytes();
    let access_token = service::issue_access_token(&user_id, &role, secret)?;
    let refresh_token = service::issue_refresh_token(&user_id, &role, secret)?;
    let refresh_hash = service::hash_token(&refresh_token);

    state
        .db_write
        .statement(
            "auth.login.refresh-token",
            "UPDATE users SET refresh_token_hash = ?1, updated_at = unixepoch() WHERE id = ?2",
            vec![
                libsql::Value::Text(refresh_hash),
                libsql::Value::Text(user_id.clone()),
            ],
        )
        .await
        .map_err(AppError::from)?;

    // Build httpOnly refresh cookie — SameSite=Lax per review fix (not Strict)
    // Lax allows navigation from email/portal links while still blocking CSRF POST attacks
    let cookie = Cookie::build(("refresh_token", refresh_token))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/api/v1/auth")
        .max_age(TimeDuration::days(7))
        .build();

    let jar = CookieJar::new().add(cookie);

    let response_body = Json(LoginResponse {
        access_token,
        user: UserInfo {
            id: user_id,
            username,
            full_name,
            role,
        },
    });

    Ok((StatusCode::OK, jar, response_body))
}

/// POST /api/v1/auth/refresh
/// Validates refresh cookie, rotates both tokens, updates DB hash.
pub async fn refresh(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let refresh_token = jar
        .get("refresh_token")
        .map(|c| c.value().to_string())
        .ok_or(AppError::Unauthorized)?;

    let secret = state.config.jwt_secret.as_bytes();
    let claims = service::verify_refresh_token(&refresh_token, secret)?;

    let token_hash = service::hash_token(&refresh_token);

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    // Load the active user; the conditional write below is the sole hash comparison.
    let mut rows = conn
        .query(
            "SELECT id, username, full_name, role FROM users WHERE id = ?1 AND status = 'active'",
            [claims.sub.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or(AppError::Unauthorized)?;

    let user_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let username: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let full_name: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let role_str: String = row.get(3).map_err(|e| AppError::Internal(e.into()))?;

    let role: Role = role_str
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid role in database")))?;

    // Issue new tokens (rotation)
    let new_access_token = service::issue_access_token(&user_id, &role, secret)?;
    let new_refresh_token = service::issue_refresh_token(&user_id, &role, secret)?;
    let new_refresh_hash = service::hash_token(&new_refresh_token);

    let rows_affected = state
        .db_write
        .statement(
            "auth.refresh.rotate-token",
            "UPDATE users \
             SET refresh_token_hash = ?1, updated_at = unixepoch() \
             WHERE id = ?2 \
               AND refresh_token_hash = ?3 \
               AND status = 'active'",
            vec![
                libsql::Value::Text(new_refresh_hash),
                libsql::Value::Text(user_id.clone()),
                libsql::Value::Text(token_hash),
            ],
        )
        .await
        .map_err(AppError::from)?;

    if rows_affected != 1 {
        return Err(AppError::Unauthorized);
    }

    let cookie = Cookie::build(("refresh_token", new_refresh_token))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/api/v1/auth")
        .max_age(TimeDuration::days(7))
        .build();

    let jar = CookieJar::new().add(cookie);

    let response_body = Json(LoginResponse {
        access_token: new_access_token,
        user: UserInfo {
            id: user_id,
            username,
            full_name,
            role,
        },
    });

    Ok((StatusCode::OK, jar, response_body))
}

/// POST /api/v1/auth/logout
/// Clears refresh token hash in DB and expires the cookie.
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let refresh_token = jar
        .get("refresh_token")
        .map(|c| c.value().to_string())
        .ok_or(AppError::Unauthorized)?;

    let secret = state.config.jwt_secret.as_bytes();
    let claims = service::verify_refresh_token(&refresh_token, secret)?;
    let token_hash = service::hash_token(&refresh_token);

    state
        .db_write
        .statement(
            "auth.logout.clear-token",
            "UPDATE users SET refresh_token_hash = NULL, updated_at = unixepoch() \
             WHERE id = ?1 AND refresh_token_hash = ?2",
            vec![
                libsql::Value::Text(claims.sub),
                libsql::Value::Text(token_hash),
            ],
        )
        .await
        .map_err(AppError::from)?;

    // Expire the cookie
    let expired_cookie = Cookie::build(("refresh_token", ""))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/api/v1/auth")
        .max_age(TimeDuration::ZERO)
        .build();

    let jar = CookieJar::new().add(expired_cookie);

    Ok((
        StatusCode::OK,
        jar,
        Json(json!({"message": "Logged out successfully"})),
    ))
}
