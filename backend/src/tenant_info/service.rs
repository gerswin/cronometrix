use libsql::Connection;

use crate::common::epoch_to_iso;
use crate::errors::AppError;

use super::models::{TenantInfo, UpdateTenantInfoRequest};

/// Map a libSQL row (client_name, client_rif, address, version, updated_at) to TenantInfo.
fn row_to_tenant_info(row: libsql::Row) -> Result<TenantInfo, AppError> {
    Ok(TenantInfo {
        client_name: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        client_rif: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        address: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        version: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        updated_at: epoch_to_iso(row.get(4).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// Read the singleton tenant_info row (always id = 1).
/// Returns Internal error if the seed row is missing (should never happen post-migration 013).
pub async fn get_tenant_info(conn: &Connection) -> Result<TenantInfo, AppError> {
    let row = conn
        .query(
            "SELECT client_name, client_rif, address, version, updated_at \
             FROM tenant_info WHERE id = 1",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("tenant_info singleton row missing"))
        })?;

    row_to_tenant_info(row)
}

/// Update the singleton tenant_info row using optimistic concurrency (D-04).
/// Returns Conflict with VERSION_CONFLICT if the version does not match.
/// If no fields are provided (only `version`), returns the current row unchanged.
///
/// Always pins WHERE id = 1 (RESEARCH Pitfall 8) to ensure we never accidentally
/// update a non-singleton row in case the CHECK constraint is ever bypassed.
pub async fn update_tenant_info(
    conn: &Connection,
    req: UpdateTenantInfoRequest,
) -> Result<TenantInfo, AppError> {
    // Build dynamic SET clause matching rules::handlers::update_rules pattern.
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(val) = req.client_name {
        sets.push(format!("client_name = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(val));
    }

    if let Some(val) = req.client_rif {
        sets.push(format!("client_rif = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(val));
    }

    if let Some(val) = req.address {
        sets.push(format!("address = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(val));
    }

    if sets.is_empty() {
        // Nothing to update — return current state unchanged.
        return get_tenant_info(conn).await;
    }

    sets.push("updated_at = unixepoch()".to_string());
    sets.push("version = version + 1".to_string());

    let set_clause = sets.join(", ");
    let version_param = values.len() + 1;

    values.push(libsql::Value::Integer(req.version));

    let sql = format!(
        "UPDATE tenant_info SET {} WHERE id = 1 AND version = ?{}",
        set_clause, version_param
    );

    let rows_affected = conn
        .execute(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        // Singleton always exists (seeded by migration 013); the only way to fail
        // here is a stale version.
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Tenant info was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }

    get_tenant_info(conn).await
}
