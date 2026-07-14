//! Leaves HTTP handlers.
//!
//! Route placement (set in main.rs):
//! - POST   /api/v1/leaves                 — require_admin, multipart
//! - GET    /api/v1/leaves                 — require_auth
//! - GET    /api/v1/leaves/{id}            — require_auth
//! - GET    /api/v1/leaves/{id}/evidence   — require_auth
//! - DELETE /api/v1/leaves/{id}?version=N  — require_admin
//!
//! Security invariants:
//! - Evidence paths are SERVER-GENERATED from UUID + extension (T-3-15).
//! - Evidence read path canonicalizes + verifies under `state.paths.leaves_root` (T-3-18).
//! - Content-Type enum restricted to pdf/jpeg/png (T-3-16).
//! - Hard size cap 10MB enforced before DB commit (T-3-21).
//! - Create + cancel publish RecomputeRequest for each anchor_date in the
//!   leave range so existing daily_records pick up (or drop) the overlay.

use std::path::PathBuf;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::recompute::RecomputeRequest;
use crate::state::AppState;
use crate::storage::atomic_file::AtomicFileGuard;

use super::models::{CreateLeaveRequest, LeaveListQuery, LeaveResponse};
use super::service;

const MAX_EVIDENCE_BYTES: usize = 10 * 1024 * 1024; // 10MB — T-3-21

/// POST /api/v1/leaves — multipart/form-data. Admin only.
///
/// Form fields:
/// - employee_id      (text, required)
/// - from_date        (text, YYYY-MM-DD, required)
/// - to_date          (text, YYYY-MM-DD, required)
/// - leave_type       (text, medical|vacation|unpaid|manual, required)
/// - justification    (text, required)
/// - evidence         (file, optional unless leave_type=medical)
pub async fn create_leave(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<LeaveResponse>), AppError> {
    // 1. Stream multipart fields into resolved values.
    let mut employee_id: Option<String> = None;
    let mut from_date: Option<String> = None;
    let mut to_date: Option<String> = None;
    let mut leave_type: Option<String> = None;
    let mut justification: Option<String> = None;
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
            "employee_id" => {
                employee_id = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "from_date" => {
                from_date = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "to_date" => {
                to_date = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "leave_type" => {
                leave_type = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "justification" => {
                justification = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "evidence" => {
                let ct = field.content_type().unwrap_or("").to_string();
                evidence_ext = match ct.as_str() {
                    "application/pdf" => Some("pdf"),
                    "image/jpeg" => Some("jpg"),
                    "image/png" => Some("png"),
                    _ => {
                        return Err(AppError::Validation {
                            code: "VALIDATION_ERROR",
                            message: format!(
                                "evidence content_type must be application/pdf, image/jpeg, or image/png (got '{}')",
                                ct
                            ),
                        });
                    }
                };
                let bytes = field.bytes().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: format!("reading evidence bytes: {}", e),
                })?;
                if bytes.len() > MAX_EVIDENCE_BYTES {
                    return Err(AppError::Validation {
                        code: "VALIDATION_ERROR",
                        message: format!("evidence file exceeds 10MB (got {} bytes)", bytes.len()),
                    });
                }
                evidence_bytes = Some(bytes.to_vec());
            }
            _ => {
                // Discard unknown fields — don't error, just drain bytes.
                let _ = field.bytes().await;
            }
        }
    }

    let employee_id = employee_id.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "employee_id required".into(),
    })?;
    let from_date = from_date.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "from_date required".into(),
    })?;
    let to_date = to_date.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "to_date required".into(),
    })?;
    let leave_type = leave_type.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "leave_type required".into(),
    })?;
    let justification = justification.ok_or_else(|| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: "justification required".into(),
    })?;

    // 2. Write evidence to disk if present. Path is SERVER-GENERATED — user
    //    filename is discarded (T-3-15 mitigation). UUID v4 is cryptographically
    //    random so collisions require ≫ 2^122 leaves.
    let (evidence_relpath, evidence_guard) =
        if let (Some(bytes), Some(ext)) = (evidence_bytes.as_ref(), evidence_ext) {
            let rel = format!("{}.{}", Uuid::new_v4(), ext);
            let guard = AtomicFileGuard::write(&state.paths.leaves_root, &rel, bytes)
                .map_err(AppError::Internal)?;
            (Some(rel), Some(guard))
        } else {
            (None, None)
        };

    // 3. Call service with the resolved evidence path.
    let req = CreateLeaveRequest {
        employee_id: employee_id.clone(),
        from_date: from_date.clone(),
        to_date: to_date.clone(),
        leave_type,
        justification,
    };
    let leave = service::create_leave_queued_guarded(
        &state,
        &claims.sub,
        req,
        evidence_relpath,
        evidence_guard,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(leave)))
}

pub async fn list_leaves(
    State(state): State<AppState>,
    Query(q): Query<LeaveListQuery>,
) -> Result<Json<PaginatedResponse<LeaveResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(service::list(&conn, q).await?))
}

pub async fn get_leave(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LeaveResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(service::get_by_id(&conn, &id).await?))
}

#[derive(Debug, Deserialize)]
pub struct CancelQuery {
    pub version: i64,
}

/// DELETE /api/v1/leaves/{id}?version=N — soft-delete + recompute the range.
pub async fn cancel_leave(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<CancelQuery>,
) -> Result<StatusCode, AppError> {
    let leave = service::cancel_queued(&state, &id, q.version).await?;

    publish_recompute_for_range(&state, &leave.employee_id, &leave.from_date, &leave.to_date);

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/leaves/{id}/evidence — stream the uploaded evidence file.
///
/// Defence in depth (T-3-15 + T-3-18): `evidence_path` is server-generated,
/// but we still reject any stored value containing `..` or starting with `/`,
/// then canonicalize the resolved absolute path and verify it stays under
/// `state.paths.leaves_root`. If canonicalize/read fails, we return 404 with
/// `LEAVE_EVIDENCE_NOT_FOUND` (never 500) so a missing file never leaks as
/// an internal error.
pub async fn get_leave_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let leave = service::get_by_id(&conn, &id).await?;
    let relpath = leave.evidence_path.ok_or_else(|| AppError::NotFound {
        code: "LEAVE_EVIDENCE_NOT_FOUND",
        message: "Leave has no evidence attached".into(),
    })?;

    if relpath.contains("..") || relpath.starts_with('/') {
        tracing::warn!(
            leave_id = %id,
            %relpath,
            "rejecting evidence path with traversal or absolute path marker"
        );
        return Err(AppError::NotFound {
            code: "LEAVE_EVIDENCE_NOT_FOUND",
            message: "Evidence not available".into(),
        });
    }

    let root = state.paths.leaves_root.clone();
    let root_canonical = root.canonicalize().map_err(|e| {
        tracing::error!(?root, error = %e, "leaves_root canonicalize failed");
        AppError::NotFound {
            code: "LEAVE_EVIDENCE_NOT_FOUND",
            message: "Evidence not available".into(),
        }
    })?;
    let full = root_canonical.join(&relpath);
    let canonical = match full.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return Err(AppError::NotFound {
                code: "LEAVE_EVIDENCE_NOT_FOUND",
                message: "Evidence not found on disk".into(),
            });
        }
    };
    if !canonical.starts_with(&root_canonical) {
        tracing::error!(
            leave_id = %id, ?canonical, ?root_canonical,
            "canonicalized evidence path escapes leaves_root — rejecting"
        );
        return Err(AppError::NotFound {
            code: "LEAVE_EVIDENCE_NOT_FOUND",
            message: "Evidence not available".into(),
        });
    }

    let bytes = tokio::fs::read(&canonical)
        .await
        .map_err(|_| AppError::NotFound {
            code: "LEAVE_EVIDENCE_NOT_FOUND",
            message: "Evidence not found on disk".into(),
        })?;

    let content_type = match PathBuf::from(&relpath).extension().and_then(|s| s.to_str()) {
        Some("pdf") => "application/pdf",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(match content_type {
            "application/pdf" => "application/pdf",
            "image/jpeg" => "image/jpeg",
            "image/png" => "image/png",
            _ => "application/octet-stream",
        }),
    );
    Ok((StatusCode::OK, headers, bytes).into_response())
}

/// Publish a RecomputeRequest for every anchor_date in [from_date, to_date].
/// Silently no-ops if recompute_tx is None or the date strings don't parse.
fn publish_recompute_for_range(
    state: &AppState,
    employee_id: &str,
    from_date: &str,
    to_date: &str,
) {
    let Some(tx) = state.recompute_tx.as_ref() else {
        return;
    };
    let Ok(from) = NaiveDate::parse_from_str(from_date, "%Y-%m-%d") else {
        return;
    };
    let Ok(to) = NaiveDate::parse_from_str(to_date, "%Y-%m-%d") else {
        return;
    };
    let mut d = from;
    while d <= to {
        if let Err(e) = tx.send(RecomputeRequest {
            employee_id: employee_id.to_string(),
            anchor_date: d,
        }) {
            tracing::warn!(err = %e, "recompute_tx send failed (worker down?)");
            return;
        }
        let Some(next) = d.succ_opt() else { break };
        d = next;
    }
}
