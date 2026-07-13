//! HTTP handlers for `GET /api/v1/audit` — read-only paginated audit log.
//!
//! Registered in `supervisor_read_routes` (require_supervisor_or_above middleware).
//! No POST/PATCH/DELETE handlers exist — the audit_log is append-only by design.

use axum::{
    extract::{Query, State},
    Json,
};

use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{AuditActor, AuditEntry, AuditListQuery};
use super::service;

/// `GET /api/v1/audit` — read paginated audit_log entries.
///
/// RBAC: Admin + Supervisor (via require_supervisor_or_above middleware).
/// Viewer → 403 Forbidden.  Anonymous → 401 Unauthorized.
pub async fn list_audit(
    State(state): State<AppState>,
    Query(query): Query<AuditListQuery>,
) -> Result<Json<PaginatedResponse<AuditEntry>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list_audit(&conn, query).await?;
    Ok(Json(result))
}

/// `GET /api/v1/audit/actors` — distinct audit actors with username/role join.
///
/// RBAC: Admin + Supervisor (via require_supervisor_or_above middleware applied at the
/// supervisor_read_routes group level). Viewer → 403. Anonymous → 401.
/// No query params, no pagination — cardinality is bounded by user count.
pub async fn list_actors(State(state): State<AppState>) -> Result<Json<Vec<AuditActor>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list_actors(&conn).await?;
    Ok(Json(result))
}
