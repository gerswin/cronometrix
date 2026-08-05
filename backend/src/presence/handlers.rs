use axum::{extract::State, Json};
use chrono::Utc;

use crate::auth::rbac::AuthUser;
use crate::auth::scope::ActorScope;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::PresenceToday;
use super::service;

/// GET /api/v1/presence/today — viewer-or-above (D-09), con scope H-11.
pub async fn presence_today(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PresenceToday>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let scope = ActorScope::from_claims(&claims);
    let today = Utc::now().date_naive();
    Ok(Json(service::today(&conn, today, &scope).await?))
}
