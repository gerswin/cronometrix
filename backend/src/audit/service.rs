use libsql::Connection;

use crate::common::PaginatedResponse;
use crate::errors::AppError;

use super::models::{AuditEntry, AuditListQuery};

/// List audit_log entries with optional filters and pagination.
///
/// Filter axes:
///   - actor_id: exact match on audit_log.actor_id
///   - table_name: exact match on audit_log.table_name
///   - record_id: exact match on audit_log.record_id
///   - operation: exact match on audit_log.operation (INSERT/UPDATE/DELETE)
///   - from_ts: created_at >= from_ts (epoch seconds, inclusive)
///   - to_ts: created_at <= to_ts (epoch seconds, inclusive)
///
/// Sort: created_at DESC, id DESC (deterministic tie-break).
/// Pagination: limit clamped to [1, 200], default 50; offset >= 0, default 0.
///
/// old_data / new_data TEXT columns are parsed via serde_json::from_str. If
/// parsing fails (corrupt data), the field returns None rather than erroring —
/// defensive behavior keeps the audit log always readable.
pub async fn list_audit(
    conn: &Connection,
    query: AuditListQuery,
) -> Result<PaginatedResponse<AuditEntry>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    // Build dynamic WHERE predicates + positional params (mirror employees/service.rs pattern)
    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    if let Some(actor_id) = &query.actor_id {
        let idx = predicates.len() + 1;
        predicates.push(format!("actor_id = ?{}", idx));
        count_values.push(libsql::Value::Text(actor_id.clone()));
        fetch_values.push(libsql::Value::Text(actor_id.clone()));
    }

    if let Some(table_name) = &query.table_name {
        let idx = predicates.len() + 1;
        predicates.push(format!("table_name = ?{}", idx));
        count_values.push(libsql::Value::Text(table_name.clone()));
        fetch_values.push(libsql::Value::Text(table_name.clone()));
    }

    if let Some(record_id) = &query.record_id {
        let idx = predicates.len() + 1;
        predicates.push(format!("record_id = ?{}", idx));
        count_values.push(libsql::Value::Text(record_id.clone()));
        fetch_values.push(libsql::Value::Text(record_id.clone()));
    }

    if let Some(operation) = &query.operation {
        let idx = predicates.len() + 1;
        predicates.push(format!("operation = ?{}", idx));
        count_values.push(libsql::Value::Text(operation.clone()));
        fetch_values.push(libsql::Value::Text(operation.clone()));
    }

    if let Some(from_ts) = query.from_ts {
        let idx = predicates.len() + 1;
        predicates.push(format!("created_at >= ?{}", idx));
        count_values.push(libsql::Value::Integer(from_ts));
        fetch_values.push(libsql::Value::Integer(from_ts));
    }

    if let Some(to_ts) = query.to_ts {
        let idx = predicates.len() + 1;
        predicates.push(format!("created_at <= ?{}", idx));
        count_values.push(libsql::Value::Integer(to_ts));
        fetch_values.push(libsql::Value::Integer(to_ts));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    // COUNT(*) with the same WHERE for total
    let count_sql = format!("SELECT COUNT(*) FROM audit_log {}", where_clause);
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

    // SELECT page with ORDER BY created_at DESC, id DESC
    let lim_idx = fetch_values.len() + 1;
    let off_idx = fetch_values.len() + 2;
    let fetch_sql = format!(
        "SELECT id, table_name, record_id, operation, old_data, new_data, actor_id, created_at \
         FROM audit_log {} ORDER BY created_at DESC, id DESC LIMIT ?{} OFFSET ?{}",
        where_clause, lim_idx, off_idx
    );

    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data: Vec<AuditEntry> = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        let old_data_raw: Option<String> = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
        let new_data_raw: Option<String> = row.get(5).map_err(|e| AppError::Internal(e.into()))?;

        // Defensive parse — corrupt JSON in TEXT column returns None rather than erroring.
        let old_data = old_data_raw.and_then(|s| serde_json::from_str(&s).ok());
        let new_data = new_data_raw.and_then(|s| serde_json::from_str(&s).ok());

        data.push(AuditEntry {
            id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
            table_name: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
            record_id: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
            operation: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
            old_data,
            new_data,
            actor_id: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
            created_at: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        });
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}
