//! HTTP handlers for `/api/v1/daily-records` (viewer-or-above per D-09).

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::rbac::AuthUser;
use crate::auth::scope::ActorScope;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;
use crate::storage::evidence_magic::infer_evidence_ext_from_magic;

use super::models::{DailyRecordListQuery, DailyRecordResponse, OverrideResponse};
use super::service;

/// GET /api/v1/daily-records — paginated list with optional employee/department/date filters.
pub async fn list_daily_records(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<DailyRecordListQuery>,
) -> Result<Json<PaginatedResponse<DailyRecordResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let scope = ActorScope::from_claims(&claims);
    let result = service::list(&conn, q, &scope).await?;
    Ok(Json(result))
}

/// GET /api/v1/daily-records/{id} — single record with anomalies attached.
pub async fn get_daily_record(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<DailyRecordResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let record = service::get_by_id(&conn, &id).await?;

    // H-11: a scoped actor cannot see a record outside its department; 404 (not
    // 403) so the record's existence is not leaked. daily_records carries its
    // department_id directly, so no extra lookup is needed.
    if !ActorScope::from_claims(&claims).permits(Some(&record.department_id)) {
        return Err(AppError::NotFound {
            code: "DAILY_RECORD_NOT_FOUND",
            message: format!("Daily record '{}' not found", id),
        });
    }

    Ok(Json(record))
}

/// POST /api/v1/daily-records/{id}/overrides — Admin only, multipart/form-data.
///
/// Writes to daily_record_overrides table. SQLite audit trigger on INSERT fires
/// automatically (migration 011), producing an immutable audit_log entry (TS-05).
///
/// Required form fields: justification (text), evidence (file PDF/JPG/PNG, req'd per TS-04)
/// Optional form fields: override_entry_at (ISO 8601 string), override_exit_at (ISO 8601 string),
///                       override_work_minutes (integer string)
pub async fn create_override(
    State(state): State<AppState>,
    Path(daily_record_id): Path<String>,
    AuthUser(claims): AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<OverrideResponse>), AppError> {
    use crate::storage::atomic_file::AtomicFileGuard;

    const MAX_EVIDENCE_BYTES: usize = 10 * 1024 * 1024; // 10MB backend cap; frontend enforces 5MB

    let mut justification: Option<String> = None;
    let mut override_entry_at: Option<i64> = None;
    let mut override_exit_at: Option<i64> = None;
    let mut override_work_minutes: Option<i64> = None;
    let mut evidence_bytes: Option<Vec<u8>> = None;
    let mut evidence_ext: Option<&'static str> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation {
            code: "VALIDATION_ERROR",
            message: format!("malformed multipart: {}", e),
        })?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "justification" => {
                let val = field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?;
                justification = Some(val);
            }
            "override_entry_at" => {
                let val = field.text().await.unwrap_or_default();
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&val) {
                    override_entry_at = Some(dt.timestamp());
                }
            }
            "override_exit_at" => {
                let val = field.text().await.unwrap_or_default();
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&val) {
                    override_exit_at = Some(dt.timestamp());
                }
            }
            "override_work_minutes" => {
                let val = field.text().await.unwrap_or_default();
                override_work_minutes = val.parse::<i64>().ok();
            }
            "evidence" => {
                // CR-03: declared content-type is a quick filter only; actual
                // type is verified from the file's magic bytes after reading.
                let ct = field.content_type().unwrap_or("").to_string();
                match ct.as_str() {
                    "application/pdf" | "image/jpeg" | "image/png" => {}
                    _ => {
                        return Err(AppError::Validation {
                            code: "VALIDATION_ERROR",
                            message: format!("evidence must be PDF, JPEG, or PNG (got '{}')", ct),
                        });
                    }
                }
                let bytes = field.bytes().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: format!("reading evidence: {}", e),
                })?;
                if bytes.len() > MAX_EVIDENCE_BYTES {
                    return Err(AppError::Validation {
                        code: "VALIDATION_ERROR",
                        message: format!("evidence exceeds 10MB ({} bytes)", bytes.len()),
                    });
                }
                // CR-03: authoritative type check via magic bytes — content-type
                // header from the client is untrusted (spoofable in multipart).
                let magic_ext =
                    infer_evidence_ext_from_magic(&bytes).ok_or_else(|| AppError::Validation {
                        code: "VALIDATION_ERROR",
                        message: "evidence bytes do not match a supported file type (PDF/JPEG/PNG)"
                            .into(),
                    })?;
                evidence_ext = Some(magic_ext);
                evidence_bytes = Some(bytes.to_vec());
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let justification = justification.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "justification required (TS-03)".into(),
    })?;
    if justification.trim().is_empty() {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "justification cannot be empty (TS-03)".into(),
        });
    }
    // TS-04: evidence required for override
    if evidence_bytes.is_none() {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "evidence file required (TS-04)".into(),
        });
    }

    // WR-06: enforce override_exit_at > override_entry_at when both are present.
    // Mirror of the frontend novedadSchema refinement so a malformed pair never
    // produces a logically incoherent audit record.
    if let (Some(entry), Some(exit)) = (override_entry_at, override_exit_at) {
        if exit <= entry {
            return Err(AppError::Validation {
                code: "VALIDATION_ERROR",
                message: "override_exit_at must be after override_entry_at".into(),
            });
        }
    }

    // Write evidence to disk — UUID path (same pattern as leaves, T-4-10 mitigation).
    // Phase 8 (D-18/D-19): the overrides root comes from `state.paths.overrides_root`
    // (populated once at startup from `DATA_DIR`/overrides via Paths::from_env).
    let (evidence_relpath, evidence_guard) =
        if let (Some(bytes), Some(ext)) = (evidence_bytes.as_ref(), evidence_ext) {
            let rel = format!("{}.{}", Uuid::new_v4(), ext);
            let overrides_root = state.paths.overrides_root.clone();
            let guard =
                AtomicFileGuard::write(&overrides_root, &rel, bytes).map_err(AppError::Internal)?;
            (Some(rel), Some(guard))
        } else {
            (None, None)
        };

    let now = Utc::now().timestamp();
    let id = Uuid::new_v4().to_string();
    let actor_id = claims.sub;
    let recompute_tx = state.recompute_tx.clone();
    let response = state
        .db_write
        .transact(
            "daily-records.create-override",
            move |tx| {
                Box::pin(async move {
                    let daily_record = tx
                        .query(
                            "SELECT employee_id, anchor_date FROM daily_records WHERE id = ?1",
                            libsql::params![daily_record_id.clone()],
                        )
                        .await?
                        .next()
                        .await?
                        .ok_or_else(|| {
                            anyhow::Error::new(AppError::NotFound {
                                code: "DAILY_RECORD_NOT_FOUND",
                                message: "daily_record not found".into(),
                            })
                        })?;
                    let employee_id: String = daily_record.get(0)?;
                    let anchor_date: String = daily_record.get(1)?;

                    // C-04: revoke any currently-active override for this record
                    // before inserting the new one, in the same transaction. Doing
                    // this as two separate operations would leave a window with no
                    // active override at all; skipping it entirely would make the
                    // legitimate "replace an override" action fail once the unique
                    // partial index (idx_overrides_one_active_per_record) is in
                    // place. Never DELETE — the revoked row is evidence.
                    tx.statement(
                        "UPDATE daily_record_overrides \
                            SET status = 'revoked', updated_at = unixepoch() \
                          WHERE daily_record_id = ?1 AND status = 'active' AND deleted_at IS NULL",
                        libsql::params![daily_record_id.clone()],
                    )
                    .await?;

                    tx.statement(
                        "INSERT INTO daily_record_overrides
                           (id, daily_record_id, override_work_minutes, override_entry_at, override_exit_at,
                            justification, evidence_path, overridden_by, overridden_at, status, version, created_at, updated_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'active',1,?9,?9)",
                        libsql::params![
                            id.clone(),
                            daily_record_id.clone(),
                            override_work_minutes,
                            override_entry_at,
                            override_exit_at,
                            justification.clone(),
                            evidence_relpath.clone(),
                            actor_id.clone(),
                            now,
                        ],
                    )
                    .await?;

                    let response = OverrideResponse {
                        id,
                        daily_record_id,
                        override_work_minutes,
                        override_entry_at,
                        override_exit_at,
                        justification,
                        evidence_path: evidence_relpath,
                        overridden_by: actor_id,
                        overridden_at: now,
                        status: "active".into(),
                        version: 1,
                        created_at: now,
                        updated_at: now,
                    };
                    tx.after_commit(move || {
                        if let Some(guard) = evidence_guard {
                            guard.keep();
                        }
                        if let (Some(sender), Ok(anchor_date)) = (
                            recompute_tx,
                            chrono::NaiveDate::parse_from_str(&anchor_date, "%Y-%m-%d"),
                        ) {
                            if sender
                                .send(crate::recompute::RecomputeRequest::Day {
                                    employee_id,
                                    anchor_date,
                                })
                                .is_err()
                            {
                                tracing::warn!(
                                    operation = "daily-records.create-override",
                                    "post-commit recompute unavailable; identifiers omitted"
                                );
                            }
                        }
                    });
                    Ok(response)
                })
            },
        )
        .await
        .map_err(AppError::from)?;

    Ok((StatusCode::CREATED, Json(response)))
}
