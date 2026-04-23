//! HTTP handlers for `/api/v1/anomalies` (supervisor-or-above — T-3-04).
//!
//! Implements GET /api/v1/anomalies with pagination + filters (code, employee_id,
//! from_date, to_date). Join daily_record_anomalies (dra) -> daily_records (dr)
//! so supervisors see anchor date + employee context without N+1 lookups.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::common::{epoch_to_iso, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AnomalyListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub code: Option<String>,
    pub employee_id: Option<String>,
    pub from_date: Option<String>,  // YYYY-MM-DD
    pub to_date: Option<String>,    // YYYY-MM-DD
}

#[derive(Debug, Serialize)]
pub struct AnomalyResponse {
    pub id: String,
    pub daily_record_id: String,
    pub employee_id: String,
    pub anchor_date: String,
    pub code: String,
    pub detail: Option<String>,
    pub created_at: String,  // ISO 8601
}

/// GET /api/v1/anomalies — supervisor queue. Mirrors events/service.rs list
/// pattern with dynamic WHERE predicates + positional parameters.
pub async fn list_anomalies(
    State(state): State<AppState>,
    Query(q): Query<AnomalyListQuery>,
) -> Result<Json<PaginatedResponse<AnomalyResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    if let Some(code) = &q.code {
        predicates.push(format!("dra.code = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(code.clone()));
        fetch_values.push(libsql::Value::Text(code.clone()));
    }
    if let Some(emp) = &q.employee_id {
        predicates.push(format!("dr.employee_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(emp.clone()));
        fetch_values.push(libsql::Value::Text(emp.clone()));
    }
    if let Some(from) = &q.from_date {
        predicates.push(format!("dr.anchor_date >= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(from.clone()));
        fetch_values.push(libsql::Value::Text(from.clone()));
    }
    if let Some(to) = &q.to_date {
        predicates.push(format!("dr.anchor_date <= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(to.clone()));
        fetch_values.push(libsql::Value::Text(to.clone()));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    let count_sql = format!(
        "SELECT COUNT(*) FROM daily_record_anomalies dra \
         JOIN daily_records dr ON dr.id = dra.daily_record_id {}",
        where_clause
    );
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT query returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT dra.id, dra.daily_record_id, dr.employee_id, dr.anchor_date, \
            dra.code, dra.detail, dra.created_at \
         FROM daily_record_anomalies dra \
         JOIN daily_records dr ON dr.id = dra.daily_record_id {} \
         ORDER BY dra.created_at DESC, dra.id ASC LIMIT ?{lim} OFFSET ?{off}",
        where_clause,
        lim = fetch_values.len() + 1,
        off = fetch_values.len() + 2,
    );
    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data: Vec<AnomalyResponse> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let created_at: i64 = row.get(6).map_err(|e| AppError::Internal(e.into()))?;
        data.push(AnomalyResponse {
            id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
            daily_record_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
            employee_id: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
            anchor_date: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
            code: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
            detail: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
            created_at: epoch_to_iso(created_at),
        });
    }

    Ok(Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    }))
}
