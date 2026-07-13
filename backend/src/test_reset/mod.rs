//! Phase 9 — test-only route that truncates mutable tables between E2E spec runs.
//!
//! This module is ONLY wired into the router when `CRONOMETRIX_E2E=true` at
//! startup (gated at registration time in main.rs). Defense-in-depth: the
//! handler also re-checks the env and returns 404 if the flag is not set,
//! preventing configuration drift from bypassing the route-registration guard.
//!
//! Threat model: T-09-02 — accidental exposure in production would destroy
//! audit_log and other compliance-critical tables.

use axum::{extract::State, http::StatusCode, Json};

use crate::state::AppState;

/// Truncate all mutable tables used by E2E specs so each describe block starts
/// from a clean state. Returns `{"reset": true}` on success.
///
/// Tables truncated (per D-12):
/// - attendance_events
/// - leaves
/// - daily_record_anomalies
/// - daily_record_overrides
/// - daily_records
/// - audit_log
///
/// Tables NOT truncated (stable fixture data seeded by seed_e2e):
/// - users, departments, employees, devices, device_face_mappings, global_rules
pub async fn test_reset(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Defense in depth: refuse to execute unless flag is still set.
    // Prevents configuration drift if main.rs guard is somehow bypassed.
    if !state.e2e_enabled || !state.test_reset_enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    let conn = state
        .db
        .connect()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for sql in [
        "DELETE FROM attendance_events",
        "DELETE FROM leaves",
        "DELETE FROM daily_record_anomalies",
        "DELETE FROM daily_record_overrides",
        "DELETE FROM daily_records",
        "DELETE FROM audit_log",
    ] {
        conn.execute(sql, ())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(serde_json::json!({ "reset": true })))
}
