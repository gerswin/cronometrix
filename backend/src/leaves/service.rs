//! Leaves CRUD + overlap check + calc-engine overlay query.
//!
//! Public surface (consumed by leaves::handlers + daily_records::service):
//! - create_leave: overlap check → LeaveConflict on collision; INSERT otherwise.
//! - get_by_id / list: Viewer+ reads.
//! - cancel: soft-delete with optimistic concurrency.
//! - fetch_active_leave_for_date: populates EngineInput.leave (D-16 overlay).
//!
//! Filesystem root for evidence files is provided by `state.paths.leaves_root`
//! (Phase 8, D-18/D-19) — handlers read it from AppState rather than calling a
//! free function that reads env at use-site.

use chrono::NaiveDate;
use libsql::{params, Connection};
use uuid::Uuid;

use crate::calc::models::LeaveRow;
use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{CreateLeaveRequest, LeaveListQuery, LeaveResponse};

const LEAVE_SELECT_COLS: &str =
    "id, employee_id, from_date, to_date, leave_type, justification, evidence_path, \
     created_by, status, deleted_at, version, created_at, updated_at";

/// Create a new leave row.
///
/// Validates leave_type against the enum and enforces `medical` → evidence
/// required. Rejects overlapping active leaves for the same employee
/// (T-3-14 mitigation) with `LeaveConflict`. INSERT uses positional bindings
/// so SQL injection via user-supplied fields (employee_id, justification,
/// leave_type) is impossible (T-3-20).
pub async fn create_leave(
    conn: &Connection,
    actor_id: &str,
    req: CreateLeaveRequest,
    evidence_relpath: Option<String>,
) -> Result<LeaveResponse, AppError> {
    // 1. Validate leave_type against the CHECK enum (fail fast before DB hit).
    match req.leave_type.as_str() {
        "medical" | "vacation" | "unpaid" | "manual" => {}
        _ => {
            return Err(AppError::Validation {
                code: "VALIDATION_ERROR",
                message: "leave_type must be one of: medical, vacation, unpaid, manual".into(),
            })
        }
    }

    // 2. Medical leave requires evidence (D-13, T-3-16 mitigation).
    if req.leave_type == "medical" && evidence_relpath.is_none() {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "Medical leave requires evidence file upload".into(),
        });
    }

    // 3. Parse + validate date range (D-14: full-day only).
    let from = NaiveDate::parse_from_str(&req.from_date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be YYYY-MM-DD".into(),
        }
    })?;
    let to =
        NaiveDate::parse_from_str(&req.to_date, "%Y-%m-%d").map_err(|_| AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "to_date must be YYYY-MM-DD".into(),
        })?;
    if from > to {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be <= to_date".into(),
        });
    }

    // 4. Overlap check — T-3-14 mitigation.
    // Two ranges [a1,a2] and [b1,b2] overlap iff a1 <= b2 AND a2 >= b1.
    let overlap_count: i64 = {
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM leaves \
                 WHERE employee_id = ?1 \
                   AND status = 'active' AND deleted_at IS NULL \
                   AND from_date <= ?2 AND to_date >= ?3",
                params![
                    req.employee_id.clone(),
                    to.format("%Y-%m-%d").to_string(),
                    from.format("%Y-%m-%d").to_string(),
                ],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let row = rows
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no row")))?;
        row.get(0).map_err(|e| AppError::Internal(e.into()))?
    };
    if overlap_count > 0 {
        return Err(AppError::LeaveConflict {
            code: "LEAVE_OVERLAP",
            message: format!(
                "Employee {} has an overlapping active leave in range {} to {}",
                req.employee_id, req.from_date, req.to_date
            ),
        });
    }

    // 5. INSERT.
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
         justification, evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            req.employee_id.clone(),
            req.from_date.clone(),
            req.to_date.clone(),
            req.leave_type.clone(),
            req.justification.clone(),
            evidence_relpath,
            actor_id.to_string(),
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    get_by_id(conn, &id).await
}

pub async fn create_leave_queued(
    state: &AppState,
    actor_id: &str,
    req: CreateLeaveRequest,
    evidence_relpath: Option<String>,
) -> Result<LeaveResponse, AppError> {
    match req.leave_type.as_str() {
        "medical" | "vacation" | "unpaid" | "manual" => {}
        _ => {
            return Err(AppError::Validation {
                code: "VALIDATION_ERROR",
                message: "leave_type must be one of: medical, vacation, unpaid, manual".into(),
            })
        }
    }
    if req.leave_type == "medical" && evidence_relpath.is_none() {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "Medical leave requires evidence file upload".into(),
        });
    }
    let from = NaiveDate::parse_from_str(&req.from_date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be YYYY-MM-DD".into(),
        }
    })?;
    let to =
        NaiveDate::parse_from_str(&req.to_date, "%Y-%m-%d").map_err(|_| AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "to_date must be YYYY-MM-DD".into(),
        })?;
    if from > to {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be <= to_date".into(),
        });
    }
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let overlap_count: i64 = {
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM leaves \
                 WHERE employee_id = ?1 \
                   AND status = 'active' AND deleted_at IS NULL \
                   AND from_date <= ?2 AND to_date >= ?3",
                params![
                    req.employee_id.clone(),
                    to.format("%Y-%m-%d").to_string(),
                    from.format("%Y-%m-%d").to_string(),
                ],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let row = rows
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no row")))?;
        row.get(0).map_err(|e| AppError::Internal(e.into()))?
    };
    if overlap_count > 0 {
        return Err(AppError::LeaveConflict {
            code: "LEAVE_OVERLAP",
            message: format!(
                "Employee {} has an overlapping active leave in range {} to {}",
                req.employee_id, req.from_date, req.to_date
            ),
        });
    }
    let id = Uuid::new_v4().to_string();
    state
        .db_write
        .statement(
            "leaves.create",
            "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
             justification, evidence_path, created_by, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(id.clone()),
                libsql::Value::Text(req.employee_id.clone()),
                libsql::Value::Text(req.from_date.clone()),
                libsql::Value::Text(req.to_date.clone()),
                libsql::Value::Text(req.leave_type.clone()),
                libsql::Value::Text(req.justification.clone()),
                evidence_relpath
                    .map(libsql::Value::Text)
                    .unwrap_or(libsql::Value::Null),
                libsql::Value::Text(actor_id.to_string()),
            ],
        )
        .await
        .map_err(AppError::from)?;
    get_by_id(&conn, &id).await
}

/// Fetch a single leave row by id.
pub async fn get_by_id(conn: &Connection, id: &str) -> Result<LeaveResponse, AppError> {
    let sql = format!("SELECT {} FROM leaves WHERE id = ?1", LEAVE_SELECT_COLS);
    let row = conn
        .query(&sql, params![id.to_string()])
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "LEAVE_NOT_FOUND",
            message: format!("Leave '{}' not found", id),
        })?;
    row_to_leave(row)
}

/// List leaves with filters + pagination. Filters: employee_id, leave_type,
/// status, from_date, to_date. Default status filter is 'active'. Pagination
/// via limit (clamp 1..=100, default 20) + offset (>= 0).
///
/// `from_date` / `to_date` on the query match rows whose leave range overlaps
/// the supplied [from, to]: `leaves.to_date >= from` AND `leaves.from_date <= to`.
pub async fn list(
    conn: &Connection,
    q: LeaveListQuery,
) -> Result<PaginatedResponse<LeaveResponse>, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    // Default status to 'active' so routine listings hide cancelled leaves.
    let status = q.status.unwrap_or_else(|| "active".to_string());
    predicates.push(format!("status = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(status.clone()));
    fetch_values.push(libsql::Value::Text(status));

    if let Some(emp) = &q.employee_id {
        predicates.push(format!("employee_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(emp.clone()));
        fetch_values.push(libsql::Value::Text(emp.clone()));
    }

    if let Some(lt) = &q.leave_type {
        predicates.push(format!("leave_type = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(lt.clone()));
        fetch_values.push(libsql::Value::Text(lt.clone()));
    }

    if let Some(from) = &q.from_date {
        // match any leave whose end is >= the filter's from (leaf overlaps window start)
        predicates.push(format!("to_date >= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(from.clone()));
        fetch_values.push(libsql::Value::Text(from.clone()));
    }

    if let Some(to) = &q.to_date {
        // match any leave whose start is <= the filter's to
        predicates.push(format!("from_date <= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(to.clone()));
        fetch_values.push(libsql::Value::Text(to.clone()));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM leaves {}", where_clause);
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT {cols} FROM leaves {where_clause} \
         ORDER BY from_date DESC, id ASC LIMIT ?{lim} OFFSET ?{off}",
        cols = LEAVE_SELECT_COLS,
        where_clause = where_clause,
        lim = fetch_values.len() + 1,
        off = fetch_values.len() + 2,
    );
    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data: Vec<LeaveResponse> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        data.push(row_to_leave(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

/// Soft-delete a leave: `status='cancelled'`, `deleted_at` set, version bumped.
/// Returns `Conflict(LEAVE_VERSION_CONFLICT)` if version stale or already
/// cancelled; `NotFound` if the row doesn't exist at all.
pub async fn cancel(conn: &Connection, id: &str, version: i64) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE leaves SET status = 'cancelled', deleted_at = unixepoch(), \
             version = version + 1, updated_at = unixepoch() \
             WHERE id = ?1 AND version = ?2 AND status = 'active'",
            params![id.to_string(), version],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if affected == 0 {
        // Distinguish 404 (missing id) from 409 (stale version / already cancelled).
        let exists: i64 = {
            let mut rows = conn
                .query(
                    "SELECT COUNT(*) FROM leaves WHERE id = ?1",
                    params![id.to_string()],
                )
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            let row = rows
                .next()
                .await
                .map_err(|e| AppError::Internal(e.into()))?
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no row")))?;
            row.get(0).map_err(|e| AppError::Internal(e.into()))?
        };
        if exists == 0 {
            return Err(AppError::NotFound {
                code: "LEAVE_NOT_FOUND",
                message: format!("Leave '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "LEAVE_VERSION_CONFLICT",
            message: "Leave was modified concurrently or already cancelled".into(),
        });
    }
    Ok(())
}

pub async fn cancel_queued(state: &AppState, id: &str, version: i64) -> Result<(), AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let rows_affected = state
        .db_write
        .statement(
            "leaves.cancel",
            "UPDATE leaves SET status = 'cancelled', deleted_at = unixepoch(), updated_at = unixepoch(), version = version + 1 \
             WHERE id = ?1 AND status = 'active' AND version = ?2",
            vec![
                libsql::Value::Text(id.to_string()),
                libsql::Value::Integer(version),
            ],
        )
        .await
        .map_err(AppError::from)?;
    if rows_affected == 0 {
        let exists = conn
            .query(
                "SELECT id FROM leaves WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(AppError::NotFound {
                code: "LEAVE_NOT_FOUND",
                message: format!("Leave '{}' not found", id),
            });
        }
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Leave was modified by another request. Fetch the latest version and retry."
                .to_string(),
        });
    }
    Ok(())
}

/// Engine-overlay query: fetch the single active leave row, if any, that
/// covers the given anchor_date for the employee. Returns the typed
/// `LeaveRow` (NaiveDate fields, not strings) consumed by `EngineInput.leave`.
///
/// Query semantics: `from_date <= anchor AND to_date >= anchor` — the same
/// range predicate used in the overlap check, specialized to a single day.
/// `LIMIT 1` because the overlap check in `create_leave` guarantees at most
/// one active leave covers any given date per employee.
pub async fn fetch_active_leave_for_date(
    conn: &Connection,
    employee_id: &str,
    anchor_date: NaiveDate,
) -> Result<Option<LeaveRow>, AppError> {
    let anchor_str = anchor_date.format("%Y-%m-%d").to_string();
    let mut rows = conn
        .query(
            "SELECT id, employee_id, from_date, to_date, leave_type FROM leaves \
             WHERE employee_id = ?1 \
               AND from_date <= ?2 AND to_date >= ?2 \
               AND status = 'active' AND deleted_at IS NULL \
             LIMIT 1",
            params![employee_id.to_string(), anchor_str],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    else {
        return Ok(None);
    };
    let from_str: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let to_str: String = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
    Ok(Some(LeaveRow {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        from_date: NaiveDate::parse_from_str(&from_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad from_date in leaves: {}", e)))?,
        to_date: NaiveDate::parse_from_str(&to_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("bad to_date in leaves: {}", e)))?,
        leave_type: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
    }))
}

fn row_to_leave(row: libsql::Row) -> Result<LeaveResponse, AppError> {
    Ok(LeaveResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        from_date: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        to_date: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        leave_type: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        justification: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
        evidence_path: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        created_by: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(8).map_err(|e| AppError::Internal(e.into()))?,
        deleted_at: epoch_to_iso_opt(row.get(9).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(10).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(11).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(12).map_err(|e| AppError::Internal(e.into()))?),
    })
}
