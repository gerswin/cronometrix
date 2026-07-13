//! DTOs for the enrollment endpoints.
//!
//! Follows the Phase 1/2 convention: `*Response` structs are serialised; `*Request`
//! structs are deserialised from multipart fields and validated.

use serde::{Deserialize, Serialize};

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

/// Full enrollment status — returned by GET /enrollments/:id.
#[derive(Debug, Serialize)]
pub struct EnrollmentResponse {
    pub id: String,
    pub employee_id: String,
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

/// Immediate 202 response from POST /enrollments/:id/devices/:device_id/retry.
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
    pub photo_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_b64: Option<String>, // Some(base64 JPEG) iff status=="captured"
    pub error_message: Option<String>,
}

/// Immediate 202 response from POST /enrollments/capture-from-device.
#[derive(Debug, Serialize)]
pub struct CaptureFromDeviceResponse {
    pub capture_id: String,
    pub status: String, // always "capturing" on first response
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
    pub face_quality_score: Option<String>,
    pub photo_bytes: Vec<u8>,
}

/// JSON body for POST /enrollments/capture-from-device (D-02 LOCKED).
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
