//! DTOs for the enrollment endpoints.
//!
//! Follows the Phase 1/2 convention: `*Response` structs are serialised; `*Request`
//! structs are deserialised from multipart fields and validated.

use serde::{Deserialize, Serialize};

const MIN_ACCEPTABLE_LUMINANCE: f64 = 80.0;
const MAX_ACCEPTABLE_LUMINANCE: f64 = 200.0;
const MIN_ACCEPTABLE_FACE_SIZE: f64 = 160.0;
const MAX_EVIDENCE_DIMENSION: f64 = 10_000.0;

// =============================================================================
// Response types
// =============================================================================

/// Per-device push status row — embedded in `EnrollmentResponse`.
#[derive(Debug, Serialize)]
pub struct EnrollmentDevicePushResponse {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    pub status: String, // pending | in_progress | success | failed
    pub error_message: Option<String>,
    pub started_at: Option<String>,   // ISO-8601 or None
    pub completed_at: Option<String>, // ISO-8601 or None
}

/// Enriched enrollment status shared by list and single-item reads.
#[derive(Debug, Serialize)]
pub struct EnrollmentResponse {
    pub id: String,
    pub employee_id: String,
    pub employee_name: String,
    pub employee_code: String,
    pub status: String,               // in_progress | success | partial | failed
    pub started_at: String,           // ISO-8601
    pub completed_at: Option<String>, // ISO-8601 or None
    pub version: i64,
    pub device_pushes: Vec<EnrollmentDevicePushResponse>,
}

/// Immediate 202 response from POST /enrollments.
#[derive(Debug, Serialize)]
pub struct EnrollmentSubmitResponse {
    pub enrollment_id: String,
    pub face_id: String,
    pub device_pushes: Vec<EnrollmentDevicePushResponse>,
}

/// Immediate 202 response from POST /enrollments/:id/pushes/:device_id/retry.
#[derive(Debug, Serialize)]
pub struct RetryResponse {
    pub enrollment_id: String,
    pub device_id: String,
    pub status: String,
}

/// Kiosk-capture session status — returned by GET /enrollments/captures/:capture_id.
///
/// `photo_b64` is Some(base64-encoded JPEG bytes) iff `status == "captured"`.
/// For all other states (capturing / timeout / error), `photo_b64` is None and is
/// omitted from the JSON response (`skip_serializing_if`).
///
/// Contract reconciled with 07-02 Task 3 (kiosk-capture-tab.tsx):
///   Frontend decodes via `atob(photo_b64) → Uint8Array → Blob → URL.createObjectURL`.
#[derive(Debug, Serialize)]
pub struct CaptureResponse {
    pub capture_id: String,
    pub status: String, // capturing | captured | timeout | error
    pub source_device_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_b64: Option<String>, // Some(base64 JPEG) iff status=="captured"
    pub error_message: Option<String>,
}

/// Immediate 202 response from POST /enrollments/captures.
#[derive(Debug, Serialize)]
pub struct CaptureFromDeviceResponse {
    pub capture_id: String,
    pub status: String, // always "capturing" on first response
    pub source_device_id: String,
}

// =============================================================================
// Request types (assembled from multipart fields, then validated in handlers)
// =============================================================================

/// Assembled from multipart fields after drain. Validated in `create_enrollment`.
#[derive(Debug)]
pub struct CreateEnrollmentRequest {
    pub employee_id: String,
    pub captured_via: String,
    pub source_device_id: Option<String>,
    pub face_quality_score: FaceQualityEvidence,
    pub photo_bytes: Vec<u8>,
}

/// Typed evidence produced by the browser's face-analysis pipeline.
///
/// Trust boundary: the backend does not run a second face detector. It rejects
/// malformed or internally inconsistent client evidence, enforces the same
/// small acceptance thresholds published by `frontend/src/lib/face-detection.ts`,
/// and separately decodes/normalizes the submitted JPEG before persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FaceQualityEvidence {
    pub face_detected: bool,
    pub luminance_ok: bool,
    pub size_ok: bool,
    pub luminance: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FaceQualityValidationError {
    Invalid(&'static str),
    Unacceptable,
}

impl FaceQualityEvidence {
    pub fn parse_json(raw: &str) -> Result<Self, FaceQualityValidationError> {
        serde_json::from_str(raw)
            .map_err(|_| FaceQualityValidationError::Invalid("face quality must be valid JSON"))
    }

    pub fn validate(&self) -> Result<(), FaceQualityValidationError> {
        if !self.luminance.is_finite() || !self.width.is_finite() || !self.height.is_finite() {
            return Err(FaceQualityValidationError::Invalid(
                "face quality numbers must be finite",
            ));
        }
        if !(0.0..=255.0).contains(&self.luminance)
            || !(0.0..=MAX_EVIDENCE_DIMENSION).contains(&self.width)
            || !(0.0..=MAX_EVIDENCE_DIMENSION).contains(&self.height)
        {
            return Err(FaceQualityValidationError::Invalid(
                "face quality numbers are out of range",
            ));
        }
        if self.luminance_ok
            && !(MIN_ACCEPTABLE_LUMINANCE..=MAX_ACCEPTABLE_LUMINANCE).contains(&self.luminance)
        {
            return Err(FaceQualityValidationError::Invalid(
                "luminanceOk contradicts luminance",
            ));
        }
        if self.size_ok
            && (self.width < MIN_ACCEPTABLE_FACE_SIZE || self.height < MIN_ACCEPTABLE_FACE_SIZE)
        {
            return Err(FaceQualityValidationError::Invalid(
                "sizeOk contradicts face dimensions",
            ));
        }
        if !self.face_detected || !self.luminance_ok || !self.size_ok {
            return Err(FaceQualityValidationError::Unacceptable);
        }
        Ok(())
    }
}

/// Query string for GET /enrollments.
#[derive(Debug, Deserialize, Default)]
pub struct EnrollmentListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// JSON body for POST /enrollments/captures (D-02 LOCKED).
#[derive(Debug, Deserialize)]
pub struct CaptureFromDeviceRequest {
    pub device_id: String,
    pub employee_id: String,
}

// =============================================================================
// Validation helpers (mirrors devices/models.rs validate_status pattern)
// =============================================================================

pub fn validate_captured_via(s: &str) -> Result<(), &'static str> {
    match s {
        "device" | "webcam" | "upload" => Ok(()),
        _ => Err("captured_via must be 'device', 'webcam', or 'upload'"),
    }
}

pub fn validate_enrollment_status(s: &str) -> Result<(), &'static str> {
    match s {
        "in_progress" | "success" | "partial" | "failed" => Ok(()),
        _ => Err("enrollment status must be 'in_progress', 'success', 'partial', or 'failed'"),
    }
}

pub fn validate_push_status(s: &str) -> Result<(), &'static str> {
    match s {
        "pending" | "in_progress" | "success" | "failed" => Ok(()),
        _ => Err("push status must be 'pending', 'in_progress', 'success', or 'failed'"),
    }
}
