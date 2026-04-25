//! HTTP handlers for `/api/v1/daily-records` (viewer-or-above per D-09).

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use chrono::Utc;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{DailyRecordListQuery, DailyRecordResponse, OverrideResponse};
use super::service;

/// GET /api/v1/daily-records — paginated list with optional employee/department/date filters.
pub async fn list_daily_records(
    State(state): State<AppState>,
    Query(q): Query<DailyRecordListQuery>,
) -> Result<Json<PaginatedResponse<DailyRecordResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, q).await?;
    Ok(Json(result))
}

/// GET /api/v1/daily-records/{id} — single record with anomalies attached.
pub async fn get_daily_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DailyRecordResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(service::get_by_id(&conn, &id).await?))
}

/// CR-03 mitigation: derive evidence file extension from magic bytes rather
/// than the client-supplied multipart Content-Type. Returns the canonical
/// extension (`pdf`, `jpg`, `png`) when the bytes start with a known signature,
/// otherwise `None`.
fn infer_evidence_ext_from_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF") {
        return Some("pdf");
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    None
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
    use crate::events::service::write_photo_atomic;

    const MAX_EVIDENCE_BYTES: usize = 10 * 1024 * 1024; // 10MB backend cap; frontend enforces 5MB

    let mut justification: Option<String> = None;
    let mut override_entry_at: Option<i64> = None;
    let mut override_exit_at: Option<i64> = None;
    let mut override_work_minutes: Option<i64> = None;
    let mut evidence_bytes: Option<Vec<u8>> = None;
    let mut evidence_ext: Option<&'static str> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: format!("malformed multipart: {}", e),
    })? {
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
                let magic_ext = infer_evidence_ext_from_magic(&bytes).ok_or_else(|| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: "evidence bytes do not match a supported file type (PDF/JPEG/PNG)".into(),
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

    // WR-03: verify daily_record exists FIRST so a 404 path does not leave
    // an orphaned evidence file on disk. write_photo_atomic comes after.
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let exists: bool = conn.query(
        "SELECT 1 FROM daily_records WHERE id = ?1 LIMIT 1",
        libsql::params![daily_record_id.clone()],
    ).await.map_err(|e| AppError::Internal(e.into()))?.next().await
        .map_err(|e| AppError::Internal(e.into()))?.is_some();
    if !exists {
        return Err(AppError::NotFound {
            code: "DAILY_RECORD_NOT_FOUND",
            message: "daily_record not found".into(),
        });
    }

    // Write evidence to disk — UUID path (same pattern as leaves, T-4-10 mitigation)
    let evidence_relpath = if let (Some(bytes), Some(ext)) = (evidence_bytes.as_ref(), evidence_ext) {
        let rel = format!("{}.{}", Uuid::new_v4(), ext);
        let overrides_root = std::path::PathBuf::from(
            std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into())
        ).join("overrides");
        write_photo_atomic(&overrides_root, &rel, bytes).map_err(AppError::Internal)?;
        Some(rel)
    } else {
        None
    };

    let now = Utc::now().timestamp();
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO daily_record_overrides
           (id, daily_record_id, override_work_minutes, override_entry_at, override_exit_at,
            justification, evidence_path, overridden_by, overridden_at, status, version, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'active',1,?9,?9)",
        libsql::params![
            id.clone(), daily_record_id.clone(),
            override_work_minutes, override_entry_at, override_exit_at,
            justification.clone(), evidence_relpath.clone(),
            claims.sub.clone(), now,
        ],
    ).await.map_err(|e| AppError::Internal(e.into()))?;

    // Publish recompute so the daily_record reflects the override promptly
    if let Some(tx) = state.recompute_tx.as_ref() {
        if let Ok(mut rows) = conn.query(
            "SELECT anchor_date, employee_id FROM daily_records WHERE id = ?1",
            libsql::params![daily_record_id.clone()],
        ).await {
            if let Ok(Some(row)) = rows.next().await {
                if let (Ok(date_str), Ok(emp_id)) = (row.get::<String>(0), row.get::<String>(1)) {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                        let _ = tx.send(crate::recompute::RecomputeRequest {
                            employee_id: emp_id,
                            anchor_date: date,
                        });
                    }
                }
            }
        }
    }

    Ok((StatusCode::CREATED, Json(OverrideResponse {
        id,
        daily_record_id,
        override_work_minutes,
        override_entry_at,
        override_exit_at,
        justification,
        evidence_path: evidence_relpath,
        overridden_by: claims.sub,
        overridden_at: now,
        status: "active".into(),
        version: 1,
        created_at: now,
        updated_at: now,
    })))
}
