use axum::http::request::Parts;
use axum::{
    extract::{FromRequestParts, Request, State},
    middleware::Next,
    response::Response,
};

use crate::{
    auth::models::{Claims, Role},
    auth::service,
    errors::AppError,
    state::AppState,
};

/// Helper extractor that reads Claims from request extensions (set by require_auth middleware).
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or(AppError::Unauthorized)
    }
}

/// Middleware: requires a valid Bearer token AND Admin role.
/// Extracts and validates the JWT, then checks claims.role == Admin.
pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = service::verify_access_token(token, state.config.jwt_secret.as_bytes())?;

    if claims.role != Role::Admin {
        return Err(AppError::Forbidden);
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Middleware: requires a valid Bearer token AND Admin or Supervisor role.
/// Viewer tokens are rejected with 403 Forbidden.
pub async fn require_supervisor_or_above(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = service::verify_access_token(token, state.config.jwt_secret.as_bytes())?;

    match claims.role {
        Role::Admin | Role::Supervisor => {}
        Role::Viewer => return Err(AppError::Forbidden),
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
