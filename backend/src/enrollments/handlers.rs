//! Enrollment HTTP handlers.
//!
//! Six canonical endpoints (all Admin-only via admin_routes in main.rs — D-18):
//!   GET    /enrollments                               → list_enrollments (200)
//!   POST   /enrollments                               → create_enrollment (202)
//!   GET    /enrollments/:id                           → get_enrollment (200)
//!   POST   /enrollments/:id/pushes/:dev_id/retry      → retry_push (202)
//!   POST   /enrollments/captures                      → capture_from_device (202)
//!   GET    /enrollments/captures/:capture_id          → get_capture (200)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::devices::service as devices_service;
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

use super::image_pipeline::normalize_face_jpeg;
use super::models::{
    CaptureFromDeviceRequest, CaptureFromDeviceResponse, CaptureResponse, EnrollmentListQuery,
    EnrollmentResponse, EnrollmentSubmitResponse, FaceQualityEvidence, FaceQualityValidationError,
    RetryResponse,
};
use super::pusher::{push_one_device, spawn_enrollment_pushes};
use super::service;

/// Maximum size of a photo upload field (2 MB, per D-04 frontend cap).
const MAX_UPLOAD_BYTES: usize = 2 * 1024 * 1024;

/// Kiosk capture timeout — 30 seconds to match the device-side capture window.
const CAPTURE_TIMEOUT_SECS: u64 = 30;

// =============================================================================
// In-memory capture state (D-02 LOCKED — kiosk capture session)
// =============================================================================

/// In-memory capture state for a single kiosk capture session.
#[derive(Debug, Clone)]
pub struct CaptureState {
    pub status: String, // capturing | captured | timeout | error
    pub source_device_id: String,
    pub photo_path: Option<String>, // set when status == "captured"
    pub error_message: Option<String>,
}

/// Type alias for the shared captures map.
/// Stored on AppState via Arc<RwLock<...>> — see state.rs Task 5 extension.
pub type CapturesMap = Arc<RwLock<HashMap<String, CaptureState>>>;

/// Create a fresh CapturesMap. Called during AppState construction in main.rs.
pub fn new_captures_map() -> CapturesMap {
    Arc::new(RwLock::new(HashMap::new()))
}

// =============================================================================
// POST /enrollments — create_enrollment
// =============================================================================

/// Multipart enrollment handler (D-06, D-10, D-11).
///
/// Drains multipart fields: employee_id, captured_via, source_device_id (opt),
/// face_quality_score (required typed JSON), photo (JPEG bytes ≤2 MB).
/// Validates JPEG magic bytes, runs server-side downscale in spawn_blocking,
/// persists face_enrollments + enrollments + N push rows, fires JoinSet fan-out.
/// Returns 202 immediately with enrollment_id and per-device push status.
pub async fn create_enrollment(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<EnrollmentSubmitResponse>), AppError> {
    // Drain multipart fields.
    let mut employee_id: Option<String> = None;
    let mut captured_via: Option<String> = None;
    let mut source_device_id: Option<String> = None;
    let mut face_quality_score: Option<FaceQualityEvidence> = None;
    let mut photo_bytes: Option<Vec<u8>> = None;

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
            "captured_via" => {
                captured_via = Some(field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?);
            }
            "source_device_id" => {
                let val = field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?;
                if !val.is_empty() {
                    source_device_id = Some(val);
                }
            }
            "face_quality_score" => {
                let val = field.text().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: e.to_string(),
                })?;
                face_quality_score =
                    Some(FaceQualityEvidence::parse_json(&val).map_err(|error| {
                        AppError::Validation {
                            code: "FACE_QUALITY_INVALID",
                            message: match error {
                                FaceQualityValidationError::Invalid(message) => message.to_string(),
                                FaceQualityValidationError::Unacceptable => {
                                    "face quality evidence is unacceptable".to_string()
                                }
                            },
                        }
                    })?);
            }
            "photo" => {
                // Size guard: reject >2MB before reading fully
                let bytes = field.bytes().await.map_err(|e| AppError::Validation {
                    code: "VALIDATION_ERROR",
                    message: format!("failed to read photo field: {}", e),
                })?;
                if bytes.len() > MAX_UPLOAD_BYTES {
                    return Err(AppError::Validation {
                        code: "PHOTO_TOO_LARGE",
                        message: format!(
                            "photo exceeds {} bytes (received {})",
                            MAX_UPLOAD_BYTES,
                            bytes.len()
                        ),
                    });
                }
                // Magic byte check: must be JPEG (Pitfall 2 / RESEARCH).
                if bytes.len() < 3 || bytes[..3] != [0xFF, 0xD8, 0xFF] {
                    return Err(AppError::Validation {
                        code: "PHOTO_NOT_JPEG",
                        message: "photo must be a valid JPEG (magic bytes 0xFF 0xD8 0xFF)".into(),
                    });
                }
                photo_bytes = Some(bytes.to_vec());
            }
            _ => {
                // Discard unknown fields.
                let _ = field.bytes().await;
            }
        }
    }

    // Required field validation.
    let employee_id = employee_id.ok_or_else(|| AppError::Validation {
        code: "MISSING_FIELD",
        message: "employee_id is required".into(),
    })?;
    let captured_via = captured_via.ok_or_else(|| AppError::Validation {
        code: "MISSING_FIELD",
        message: "captured_via is required".into(),
    })?;
    let photo_bytes = photo_bytes.ok_or_else(|| AppError::Validation {
        code: "MISSING_FIELD",
        message: "photo is required".into(),
    })?;

    // Validate enum values.
    super::models::validate_captured_via(&captured_via).map_err(|e| AppError::Validation {
        code: "INVALID_CAPTURED_VIA",
        message: e.to_string(),
    })?;

    let face_quality_score = face_quality_score.ok_or_else(|| AppError::Validation {
        code: "FACE_QUALITY_REQUIRED",
        message: "face_quality_score is required".into(),
    })?;
    face_quality_score.validate().map_err(|error| match error {
        FaceQualityValidationError::Invalid(message) => AppError::Validation {
            code: "FACE_QUALITY_INVALID",
            message: message.to_string(),
        },
        FaceQualityValidationError::Unacceptable => AppError::Validation {
            code: "FACE_QUALITY_UNACCEPTABLE",
            message: "face quality evidence is unacceptable".into(),
        },
    })?;
    let face_quality_json = serde_json::to_string(&face_quality_score)
        .map_err(|error| AppError::Internal(error.into()))?;

    // Normalise JPEG in a blocking thread (CPU-bound decode/resize).
    let bytes_for_blocking = photo_bytes.clone();
    let normalized = tokio::task::spawn_blocking(move || normalize_face_jpeg(&bytes_for_blocking))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking task panicked: {e}")))?
        .map_err(|e| AppError::Validation {
            code: "PHOTO_INVALID",
            message: e.to_string(),
        })?;

    // Persist enrollment + push rows + write photo to disk.
    let submit_response = service::start_enrollment_queued(
        &state,
        &claims.sub,
        &employee_id,
        &captured_via,
        source_device_id.as_deref(),
        Some(&face_quality_json),
        &normalized,
    )
    .await?;

    // Retrieve active devices for the fan-out.
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let devices = devices_service::list_active(&conn, &state.config.device_creds_key).await?;
    drop(conn);

    let enrollment_id = submit_response.enrollment_id.clone();
    let face_id = submit_response.face_id.clone();
    let normalized_arc = Arc::new(normalized);

    // Fire-and-forget JoinSet fan-out (D-06/D-09 — outlives this request).
    spawn_enrollment_pushes(
        state,
        enrollment_id,
        face_id,
        normalized_arc,
        employee_id,
        devices,
    );

    Ok((StatusCode::ACCEPTED, Json(submit_response)))
}

// =============================================================================
// GET /enrollments — list_enrollments
// =============================================================================

/// Returns a resumable, enriched page of enrollment states.
pub async fn list_enrollments(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Query(query): Query<EnrollmentListQuery>,
) -> Result<Json<PaginatedResponse<EnrollmentResponse>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let page = service::list_enrollments(&conn, query).await?;
    Ok(Json(page))
}

// =============================================================================
// GET /enrollments/:id — get_enrollment
// =============================================================================

/// Polling endpoint for per-device sync status (D-07).
/// Returns an enriched enrollment header + deterministically ordered pushes.
pub async fn get_enrollment(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<EnrollmentResponse>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let enrollment = service::get_enrollment_with_pushes(&conn, &id).await?;
    Ok(Json(enrollment))
}

// =============================================================================
// POST /enrollments/:id/pushes/:device_id/retry — retry_push
// =============================================================================

/// Re-fire a single failed device push without re-running the full fan-out (D-08).
pub async fn retry_push(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path((enrollment_id, device_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<RetryResponse>), AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    // Reset the push row to pending (idempotent via INSERT OR REPLACE).
    let _push_id =
        service::reset_push_to_pending_queued(&state, &enrollment_id, &device_id).await?;
    drop(conn);

    // Retrieve parameters needed for the push.
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let (employee_id, face_id, full_name) =
        service::get_enrollment_push_params(&conn, &enrollment_id).await?;

    // Get current photo from disk.
    let photo_path = service::get_current_photo_path(&conn, &employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound {
            code: "PHOTO_NOT_FOUND",
            message: "Employee has no current face enrollment photo".into(),
        })?;
    drop(conn);

    let photo_bytes = tokio::fs::read(state.paths.enrollments_root.join(&photo_path))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("read photo for retry: {e}")))?;

    let device = devices_service::get_decrypted(
        &state
            .db
            .connect()
            .map_err(|e| AppError::Internal(e.into()))?,
        &device_id,
        &state.config.device_creds_key,
    )
    .await?;

    let photo_arc = Arc::new(photo_bytes);

    // Spawn single-device push detached.
    tokio::spawn({
        let state = state.clone();
        let enrollment_id = enrollment_id.clone();
        async move {
            if let Err(e) = push_one_device(
                &state,
                &enrollment_id,
                &face_id,
                &photo_arc,
                &employee_id,
                &full_name,
                &device,
            )
            .await
            {
                tracing::warn!(
                    enrollment_id = %enrollment_id,
                    device_id = %device.id,
                    err = %e,
                    "retry push failed"
                );
            }
            // Finalise enrollment status after single retry settles.
            if let Err(e) = service::finalize_enrollment_status_queued(&state, &enrollment_id).await
            {
                tracing::error!(enrollment_id = %enrollment_id, err = %e, "retry finalize failed");
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(RetryResponse {
            enrollment_id: enrollment_id.clone(),
            device_id: device_id.clone(),
            status: "pending".to_string(),
        }),
    ))
}

// =============================================================================
// POST /enrollments/captures — capture_from_device
// =============================================================================

/// Kiosk capture step 1 (D-02 LOCKED): spawn a device-side capture, return
/// capture_id immediately (202). Frontend polls GET /captures/:id.
pub async fn capture_from_device(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<CaptureFromDeviceRequest>,
) -> Result<(StatusCode, Json<CaptureFromDeviceResponse>), AppError> {
    let capture_id = Uuid::new_v4().to_string();
    let source_device_id = body.device_id.clone();

    // Insert initial state into the shared map.
    {
        let mut map = state.captures.write().await;
        map.insert(
            capture_id.clone(),
            CaptureState {
                status: "capturing".to_string(),
                source_device_id: source_device_id.clone(),
                photo_path: None,
                error_message: None,
            },
        );
    }

    // Build device connection.
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let device =
        devices_service::get_decrypted(&conn, &body.device_id, &state.config.device_creds_key)
            .await?;
    drop(conn);

    let isapi = DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )
    .map_err(AppError::Internal)?;

    let captures = state.captures.clone();
    let cid = capture_id.clone();
    let source_device_id_for_task = source_device_id.clone();

    // Spawn capture task with 30s timeout.
    tokio::spawn(async move {
        let result = timeout(
            Duration::from_secs(CAPTURE_TIMEOUT_SECS),
            isapi.capture_face_image(),
        )
        .await;

        match result {
            Ok(Ok(jpeg_bytes)) => {
                // Write to /tmp/enrollments-captures/{capture_id}.jpg
                let tmp_root = state.paths.captures_tmp_root.clone();
                let _ = tokio::fs::create_dir_all(&tmp_root).await;
                let path = tmp_root.join(format!("{}.jpg", cid));
                match tokio::fs::write(&path, &jpeg_bytes).await {
                    Ok(_) => {
                        let mut map = captures.write().await;
                        map.insert(
                            cid.clone(),
                            CaptureState {
                                status: "captured".to_string(),
                                source_device_id: source_device_id_for_task.clone(),
                                photo_path: Some(path.to_string_lossy().into_owned()),
                                error_message: None,
                            },
                        );
                    }
                    Err(e) => {
                        let mut map = captures.write().await;
                        map.insert(
                            cid.clone(),
                            CaptureState {
                                status: "error".to_string(),
                                source_device_id: source_device_id_for_task.clone(),
                                photo_path: None,
                                error_message: Some(format!("failed to write capture: {e}")),
                            },
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                let mut map = captures.write().await;
                map.insert(
                    cid.clone(),
                    CaptureState {
                        status: "error".to_string(),
                        source_device_id: source_device_id_for_task.clone(),
                        photo_path: None,
                        error_message: Some(e.to_string()),
                    },
                );
            }
            Err(_) => {
                let mut map = captures.write().await;
                map.insert(
                    cid.clone(),
                    CaptureState {
                        status: "timeout".to_string(),
                        source_device_id: source_device_id_for_task,
                        photo_path: None,
                        error_message: Some("Device did not respond within 30 seconds".to_string()),
                    },
                );
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CaptureFromDeviceResponse {
            capture_id,
            status: "capturing".to_string(),
            source_device_id,
        }),
    ))
}

// =============================================================================
// GET /enrollments/captures/:capture_id — get_capture
// =============================================================================

/// Kiosk capture step 2 (D-02 LOCKED): poll capture status.
///
/// When status == "captured": reads the JPEG bytes from the tmp file,
/// base64-encodes them, and attaches as `photo_b64` in the response.
/// This allows the frontend kiosk-capture-tab.tsx to preview the photo
/// without a second HTTP round-trip (07-02 Task 3 contract).
///
/// When status != "captured": `photo_b64` is None and omitted from JSON
/// (`#[serde(skip_serializing_if = "Option::is_none")]`).
pub async fn get_capture(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(capture_id): Path<String>,
) -> Result<Json<CaptureResponse>, AppError> {
    let map = state.captures.read().await;
    let capture_state = map
        .get(&capture_id)
        .cloned()
        .ok_or_else(|| AppError::NotFound {
            code: "CAPTURE_NOT_FOUND",
            message: format!("Capture session '{}' not found or expired", capture_id),
        })?;
    drop(map);

    // Inline photo_b64 when status == "captured" (T-7-13 mitigated: admin-only endpoint).
    let photo_b64 = if capture_state.status == "captured" {
        if let Some(ref path) = capture_state.photo_path {
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("read capture file: {e}")))?;
            Some(B64.encode(&bytes))
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(CaptureResponse {
        capture_id,
        status: capture_state.status,
        source_device_id: capture_state.source_device_id,
        photo_path: capture_state.photo_path,
        photo_b64,
        error_message: capture_state.error_message,
    }))
}
