use std::sync::atomic::Ordering;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{errors::AppError, state::AppState};

/// Tower middleware that rejects all requests when the system is unlicensed.
/// Mirrors the `require_auth` pattern. Applied AFTER require_auth in the
/// `route_layer` chain so it runs FIRST on the request path (axum 0.8 reverses
/// `route_layer` ordering): an unlicensed installation returns 403 UNLICENSED
/// before checking tokens — no information leak about auth state on unlicensed
/// boxes (T-06-17).
pub async fn require_license(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if !state.license_valid.load(Ordering::Relaxed) {
        return Err(AppError::Unlicensed);
    }
    Ok(next.run(req).await)
}
