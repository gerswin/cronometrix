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
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::devices::service as devices_service;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::auth::rbac::AuthUser;
use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;
use crate::storage::atomic_file::{read_owned_file, AtomicFileGuard, FileIdentity};

use super::image_pipeline::normalize_face_jpeg;
use super::models::{
    CaptureFromDeviceRequest, CaptureFromDeviceResponse, CaptureResponse, EnrollmentListQuery,
    EnrollmentResponse, EnrollmentSubmitResponse, FaceQualityEvidence, FaceQualityValidationError,
    RetryResponse,
};
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
    pub photo_identity: Option<FileIdentity>,
    pub error_message: Option<String>,
    /// Monotonic admission time; immune to wall-clock jumps.
    pub created_at: Instant,
    /// Monotonic instant when the state became captured/error/timeout.
    pub terminal_at: Option<Instant>,
}

/// Lifecycle owner for capture state and every task that may publish a JPEG.
#[derive(Clone)]
pub struct CapturesMap {
    entries: Arc<RwLock<HashMap<String, CaptureState>>>,
    tasks: Arc<Mutex<JoinSet<()>>>,
    accepting: Arc<AtomicBool>,
    shutdown: CancellationToken,
}

impl CapturesMap {
    pub async fn read(&self) -> RwLockReadGuard<'_, HashMap<String, CaptureState>> {
        self.entries.read().await
    }

    pub async fn write(&self) -> RwLockWriteGuard<'_, HashMap<String, CaptureState>> {
        self.entries.write().await
    }

    pub async fn spawn<F>(&self, future: F) -> anyhow::Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        if !self.accepting.load(Ordering::Acquire) {
            anyhow::bail!("capture lifecycle is shutting down");
        }
        while tasks.try_join_next().is_some() {}
        tasks.spawn(future);
        Ok(())
    }

    /// Atomically admit state and its owner task. There is no suspension point
    /// between map insertion and JoinSet registration, so request cancellation
    /// can leave neither a state-only nor task-only capture.
    pub async fn admit<F>(
        &self,
        capture_id: String,
        state: CaptureState,
        future: F,
    ) -> anyhow::Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        if !self.accepting.load(Ordering::Acquire) {
            anyhow::bail!("capture lifecycle is shutting down");
        }
        while tasks.try_join_next().is_some() {}
        self.entries.write().await.insert(capture_id, state);
        tasks.spawn(future);
        Ok(())
    }

    pub fn cancellation(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub async fn stop_and_join(&self) {
        self.accepting.store(false, Ordering::Release);
        self.shutdown.cancel();
        let mut tasks = self.tasks.lock().await;
        tasks.abort_all();
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    tracing::warn!(%error, "capture task failed during shutdown");
                }
            }
        }
    }
}

/// Create a fresh CapturesMap. Called during AppState construction in main.rs.
pub fn new_captures_map() -> CapturesMap {
    CapturesMap {
        entries: Arc::new(RwLock::new(HashMap::new())),
        tasks: Arc::new(Mutex::new(JoinSet::new())),
        accepting: Arc::new(AtomicBool::new(true)),
        shutdown: CancellationToken::new(),
    }
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
    let started = service::start_enrollment(
        &state,
        &claims.sub,
        &employee_id,
        &captured_via,
        source_device_id.as_deref(),
        Some(&face_quality_json),
        &normalized,
    )
    .await?;

    let submit_response = started.response;

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
    let response = service::retry_enrollment_push(&state, &enrollment_id, &device_id).await?;
    Ok((StatusCode::ACCEPTED, Json(response)))
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
    let source_device_id = body.device_id.clone();
    // Every fallible setup step precedes map admission. A rejected device or
    // invalid URL therefore cannot strand either state or a temporary JPEG.
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let device =
        devices_service::get_decrypted(&conn, &body.device_id, &state.config.device_creds_key)
            .await?;
    drop(conn);

    reqwest::Url::parse(&device.base_url)
        .map_err(|error| AppError::Internal(anyhow::anyhow!("invalid device base URL: {error}")))?;

    let isapi = DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )
    .map_err(AppError::Internal)?;

    let capture_id = Uuid::new_v4().to_string();
    let admitted_at = Instant::now();
    let initial_state = CaptureState {
        status: "capturing".to_string(),
        source_device_id: source_device_id.clone(),
        photo_path: None,
        photo_identity: None,
        error_message: None,
        created_at: admitted_at,
        terminal_at: None,
    };

    let captures = state.captures.clone();
    let cid = capture_id.clone();
    let source_device_id_for_task = source_device_id.clone();
    let captures_root = state.paths.captures_tmp_root.clone();
    let capture_shutdown = captures.cancellation();
    let password = device.password.clone();

    // The registry owns the task after request admission. Request cancellation
    // cannot drop the JPEG guard or detach work from graceful shutdown.
    let task = async move {
        let result = tokio::select! {
            _ = capture_shutdown.cancelled() => return,
            result = timeout(
                Duration::from_secs(CAPTURE_TIMEOUT_SECS),
                isapi.capture_face_image(),
            ) => result,
        };

        match result {
            Ok(Ok(jpeg_bytes)) => {
                let relative = format!("{cid}.jpg");
                match AtomicFileGuard::write(&captures_root, &relative, &jpeg_bytes) {
                    Ok(guard) => {
                        let path = captures_root.join(relative);
                        let identity = guard.identity();
                        let mut map = captures.write().await;
                        map.insert(
                            cid.clone(),
                            CaptureState {
                                status: "captured".to_string(),
                                source_device_id: source_device_id_for_task.clone(),
                                photo_path: Some(path.to_string_lossy().into_owned()),
                                photo_identity: Some(identity),
                                error_message: None,
                                created_at: admitted_at,
                                terminal_at: Some(Instant::now()),
                            },
                        );
                        guard.keep();
                    }
                    Err(_error) => {
                        tracing::warn!(
                            reason = "capture-write-failed",
                            "capture image persistence failed; paths omitted"
                        );
                        let mut map = captures.write().await;
                        map.insert(
                            cid.clone(),
                            CaptureState {
                                status: "error".to_string(),
                                source_device_id: source_device_id_for_task.clone(),
                                photo_path: None,
                                photo_identity: None,
                                error_message: Some("Failed to persist capture image".into()),
                                created_at: admitted_at,
                                terminal_at: Some(Instant::now()),
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
                        photo_identity: None,
                        error_message: Some(e.to_string().replace(&password, "[redacted]")),
                        created_at: admitted_at,
                        terminal_at: Some(Instant::now()),
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
                        photo_identity: None,
                        error_message: Some("Device did not respond within 30 seconds".to_string()),
                        created_at: admitted_at,
                        terminal_at: Some(Instant::now()),
                    },
                );
            }
        }
    };
    state
        .captures
        .admit(capture_id.clone(), initial_state, task)
        .await
        .map_err(AppError::Internal)?;

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
    let capture_state = state
        .captures
        .read()
        .await
        .get(&capture_id)
        .cloned()
        .ok_or_else(|| AppError::NotFound {
            code: "CAPTURE_NOT_FOUND",
            message: format!("Capture session '{}' not found or expired", capture_id),
        })?;
    // Inline photo_b64 when status == "captured" (T-7-13 mitigated: admin-only endpoint).
    let photo_b64 = if capture_state.status == "captured" {
        if let Some(ref path) = capture_state.photo_path {
            let root = state.paths.captures_tmp_root.clone();
            let path = std::path::PathBuf::from(path);
            let bytes = tokio::task::spawn_blocking(move || read_owned_file(&root, &path))
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("capture read task failed: {e}")))?
                .map_err(|e| AppError::Internal(anyhow::anyhow!("read capture file: {e}")))?;
            Some(B64.encode(&bytes))
        } else {
            None
        }
    } else {
        None
    };

    let response = CaptureResponse {
        capture_id,
        status: capture_state.status.clone(),
        source_device_id: capture_state.source_device_id.clone(),
        photo_b64,
        error_message: capture_state.error_message.clone(),
    };
    Ok(Json(response))
}
