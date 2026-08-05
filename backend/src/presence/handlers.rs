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
    // C-01: "hoy" es un día LOCAL, no UTC. `daily_records.anchor_date` se
    // escribe con `state.config.timezone` (events/service.rs, y el mismo
    // patrón en daily_records/service.rs), así que resolver el día en UTC
    // consultaba una fecha sin filas durante las horas de desfase (en
    // America/Caracas, UTC−4, entre las 20:00 y medianoche locales) y además
    // evaluaba el día de la semana equivocado en `expected_minutes`.
    let today = Utc::now()
        .with_timezone(&state.config.timezone)
        .date_naive();
    Ok(Json(service::today(&conn, today, &scope).await?))
}
