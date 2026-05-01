use axum::{
    extract::State,
    Json,
};
use libsql::params;
use validator::Validate;

use crate::common::epoch_to_iso;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{GlobalRules, UpdateRulesRequest};

/// Map a libSQL row to a GlobalRules struct.
fn row_to_rules(row: libsql::Row) -> Result<GlobalRules, AppError> {
    Ok(GlobalRules {
        late_arrival_tolerance_min: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        early_departure_tolerance_min: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        bonus_minutes: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        effective_from: epoch_to_iso(row.get(3).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        updated_at: epoch_to_iso(row.get(5).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// GET /api/v1/rules — Return the singleton global rules row.
/// Accessible by any authenticated role (Viewer can read per D-09).
pub async fn get_rules(
    State(state): State<AppState>,
) -> Result<Json<GlobalRules>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;

    let row = conn
        .query(
            "SELECT late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, \
             effective_from, version, updated_at \
             FROM global_rules WHERE id = 'singleton'",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("global_rules singleton row missing")))?;

    Ok(Json(row_to_rules(row)?))
}

/// PATCH /api/v1/rules — Update global tolerance rules.
/// Requires Admin role (enforced at router group level).
/// Always sets effective_from = unixepoch() on update per RULE-03.
/// Uses optimistic concurrency via version field per D-04.
pub async fn update_rules(
    State(state): State<AppState>,
    Json(body): Json<UpdateRulesRequest>,
) -> Result<Json<GlobalRules>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    // Build dynamic SET clause
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(val) = body.late_arrival_tolerance_min {
        sets.push(format!("late_arrival_tolerance_min = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(val));
    }

    if let Some(val) = body.early_departure_tolerance_min {
        sets.push(format!("early_departure_tolerance_min = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(val));
    }

    if let Some(val) = body.bonus_minutes {
        sets.push(format!("bonus_minutes = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(val));
    }

    if sets.is_empty() {
        // Nothing to update — return current state
        return get_rules(State(state)).await;
    }

    // RULE-03: always update effective_from when rules change
    sets.push("effective_from = unixepoch()".to_string());
    sets.push("updated_at = unixepoch()".to_string());
    sets.push("version = version + 1".to_string());

    let set_clause = sets.join(", ");
    let version_param = values.len() + 1;

    values.push(libsql::Value::Integer(body.version));

    let sql = format!(
        "UPDATE global_rules SET {} WHERE id = 'singleton' AND version = ?{}",
        set_clause, version_param
    );

    let rows_affected = state
        .db_write
        .execute(sql, values)
        .await
        .map_err(AppError::Internal)?;

    if rows_affected == 0 {
        // Version conflict — singleton always exists
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Rules were modified by another request. Fetch the latest version and retry.".to_string(),
        });
    }

    // Return updated singleton
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let row = conn
        .query(
            "SELECT late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, \
             effective_from, version, updated_at \
             FROM global_rules WHERE id = 'singleton'",
            params![],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("global_rules singleton row missing after update")))?;

    Ok(Json(row_to_rules(row)?))
}
