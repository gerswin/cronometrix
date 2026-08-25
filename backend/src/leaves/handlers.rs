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
//! - Evidence type restricted to pdf/jpeg/png (T-3-16), verified from magic
//!   bytes — the client-supplied Content-Type header is advisory only (M-07).
//! - Hard size cap 10MB enforced before DB commit (T-3-21).
//! - Create + cancel publish bounded recompute range work after commit so
//!   existing daily_records pick up (or drop) the overlay.

use std::path::PathBuf;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::rbac::AuthUser;
use crate::auth::scope::ActorScope;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;
use crate::storage::atomic_file::AtomicFileGuard;
use crate::storage::evidence_magic::infer_evidence_ext_from_magic;

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
                // M-07: declared content-type is a quick filter only; actual
                // type is verified from the file's magic bytes after reading.
                let ct = field.content_type().unwrap_or("").to_string();
                match ct.as_str() {
                    "application/pdf" | "image/jpeg" | "image/png" => {}
                    _ => {
                        return Err(AppError::Validation {
                            code: "VALIDATION_ERROR",
                            message: format!(
                                "evidence content_type must be application/pdf, image/jpeg, or image/png (got '{}')",
                                ct
                            ),
                        });
                    }
                }
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
                // M-07: authoritative type check via magic bytes — content-type
                // header from the client is untrusted (spoofable in multipart).
                // The stored extension is derived from the bytes, never from
                // the header or the client-supplied filename.
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
    AuthUser(claims): AuthUser,
    Query(q): Query<LeaveListQuery>,
) -> Result<Json<PaginatedResponse<LeaveResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let scope = ActorScope::from_claims(&claims);
    Ok(Json(service::list(&conn, q, &scope).await?))
}

pub async fn get_leave(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<LeaveResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let leave = service::get_by_id(&conn, &id).await?;

    // H-11: a scoped actor cannot see a leave outside its department; 404 (not
    // 403) so the leave's existence is not leaked.
    let scope = ActorScope::from_claims(&claims);
    if !scope.is_unscoped() {
        let dept = service::department_of_leave(&conn, &id).await?;
        if !scope.permits(dept.as_deref()) {
            return Err(AppError::NotFound {
                code: "LEAVE_NOT_FOUND",
                message: format!("Leave '{}' not found", id),
            });
        }
    }

    Ok(Json(leave))
}

#[derive(Debug, Deserialize)]
pub struct CancelQuery {
    pub version: i64,
}

/// DELETE /api/v1/leaves/{id}?version=N — soft-delete + recompute the range.
pub async fn cancel_leave(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<CancelQuery>,
) -> Result<StatusCode, AppError> {
    service::cancel_queued(&state, &claims.sub, &id, q.version).await?;

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
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let leave = service::get_by_id(&conn, &id).await?;

    // H-11 (D2): medical evidence is health data — supervisor+ only (enforced at
    // the route layer) AND confined to the actor's department here; an
    // out-of-scope leave's evidence is 404, never leaked.
    let scope = ActorScope::from_claims(&claims);
    if !scope.is_unscoped() {
        let dept = service::department_of_leave(&conn, &id).await?;
        if !scope.permits(dept.as_deref()) {
            return Err(AppError::NotFound {
                code: "LEAVE_EVIDENCE_NOT_FOUND",
                message: "Evidence not available".into(),
            });
        }
    }

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
