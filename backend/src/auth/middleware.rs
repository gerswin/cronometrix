use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{auth::service, errors::AppError, state::AppState};

/// Tower middleware that validates the Bearer token in the Authorization header.
/// On success, inserts decoded Claims into request extensions for downstream handlers.
pub async fn require_auth(
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

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
