//! Coverage gap-fill for `backend/src/enrollments/models.rs` (08-04B Task 1).
//!
//! Baseline 0.00% line. Target ≥70%.
//!
//! Pure-data module — DTOs + three validators. We exercise:
//!   * Each validator's accepted variants.
//!   * Each validator's rejection branch (covers the negative match arm).
//!   * Serde serialization for the response DTOs (`Serialize` derive lines).
//!   * Deserialization for `CaptureFromDeviceRequest` (the only `Deserialize`).
//!   * `skip_serializing_if = "Option::is_none"` branch on `CaptureResponse.photo_b64`.

use cronometrix_api::enrollments::models::{
    validate_captured_via, validate_enrollment_status, validate_push_status,
    CaptureFromDeviceRequest, CaptureFromDeviceResponse, CaptureResponse, CreateEnrollmentRequest,
    EnrollmentDevicePushResponse, EnrollmentListQuery, EnrollmentResponse,
    EnrollmentSubmitResponse, FaceQualityEvidence, FaceQualityValidationError, RetryResponse,
};

// ---------------------------------------------------------------------------
// validate_captured_via — 3 valid + 1 reject
// ---------------------------------------------------------------------------

#[test]
fn validate_captured_via_accepts_device() {
    assert!(validate_captured_via("device").is_ok());
}

#[test]
fn validate_captured_via_accepts_webcam() {
    assert!(validate_captured_via("webcam").is_ok());
}

#[test]
fn validate_captured_via_accepts_upload() {
    assert!(validate_captured_via("upload").is_ok());
}

#[test]
fn validate_captured_via_rejects_unknown_lowercase() {
    let err = validate_captured_via("camera").unwrap_err();
    assert!(err.contains("captured_via"));
}

#[test]
fn validate_captured_via_rejects_empty() {
    assert!(validate_captured_via("").is_err());
}

#[test]
fn validate_captured_via_rejects_uppercase_variants() {
    // The match is case-sensitive — "Device" must reject.
    assert!(validate_captured_via("Device").is_err());
    assert!(validate_captured_via("WEBCAM").is_err());
}

fn acceptable_face_quality() -> FaceQualityEvidence {
    FaceQualityEvidence {
        face_detected: true,
        luminance_ok: true,
        size_ok: true,
        luminance: 120.0,
        width: 200.0,
        height: 200.0,
    }
}

#[test]
fn face_quality_parses_camel_case_json_and_accepts_current_frontend_contract() {
    let parsed = FaceQualityEvidence::parse_json(
        r#"{"faceDetected":true,"luminanceOk":true,"sizeOk":true,"luminance":120,"width":200,"height":200}"#,
    )
    .unwrap();
    assert!(parsed.validate().is_ok());
}

#[test]
fn face_quality_rejects_non_finite_and_contradictory_evidence() {
    let mut evidence = acceptable_face_quality();
    evidence.luminance = f64::NAN;
    assert!(matches!(
        evidence.validate(),
        Err(FaceQualityValidationError::Invalid(_))
    ));

    let mut evidence = acceptable_face_quality();
    evidence.width = 100.0;
    assert!(matches!(
        evidence.validate(),
        Err(FaceQualityValidationError::Invalid(_))
    ));
}

#[test]
fn face_quality_validation_exercises_each_numeric_and_boolean_edge() {
    for mutate in [
        |e: &mut FaceQualityEvidence| e.width = f64::INFINITY,
        |e: &mut FaceQualityEvidence| e.height = f64::NEG_INFINITY,
        |e: &mut FaceQualityEvidence| e.luminance = -1.0,
        |e: &mut FaceQualityEvidence| e.width = -1.0,
        |e: &mut FaceQualityEvidence| e.height = 20_001.0,
        |e: &mut FaceQualityEvidence| e.luminance = 20.0,
        |e: &mut FaceQualityEvidence| e.height = 100.0,
    ] {
        let mut evidence = acceptable_face_quality();
        mutate(&mut evidence);
        assert!(matches!(
            evidence.validate(),
            Err(FaceQualityValidationError::Invalid(_))
        ));
    }

    for mutate in [
        |e: &mut FaceQualityEvidence| e.luminance_ok = false,
        |e: &mut FaceQualityEvidence| e.size_ok = false,
    ] {
        let mut evidence = acceptable_face_quality();
        mutate(&mut evidence);
        assert_eq!(
            evidence.validate(),
            Err(FaceQualityValidationError::Unacceptable)
        );
    }

    let mut dark_but_self_consistent = acceptable_face_quality();
    dark_but_self_consistent.luminance_ok = false;
    dark_but_self_consistent.luminance = 20.0;
    assert_eq!(
        dark_but_self_consistent.validate(),
        Err(FaceQualityValidationError::Unacceptable)
    );

    let mut small_but_self_consistent = acceptable_face_quality();
    small_but_self_consistent.size_ok = false;
    small_but_self_consistent.width = 100.0;
    assert_eq!(
        small_but_self_consistent.validate(),
        Err(FaceQualityValidationError::Unacceptable)
    );
}

#[test]
fn face_quality_rejects_frontend_unacceptable_decision() {
    let mut evidence = acceptable_face_quality();
    evidence.face_detected = false;
    assert_eq!(
        evidence.validate(),
        Err(FaceQualityValidationError::Unacceptable)
    );
}

// ---------------------------------------------------------------------------
// validate_enrollment_status — 4 valid + reject
// ---------------------------------------------------------------------------

#[test]
fn validate_enrollment_status_accepts_in_progress() {
    assert!(validate_enrollment_status("in_progress").is_ok());
}

#[test]
fn validate_enrollment_status_accepts_success() {
    assert!(validate_enrollment_status("success").is_ok());
}

#[test]
fn validate_enrollment_status_accepts_partial() {
    assert!(validate_enrollment_status("partial").is_ok());
}

#[test]
fn validate_enrollment_status_accepts_failed() {
    assert!(validate_enrollment_status("failed").is_ok());
}

#[test]
fn validate_enrollment_status_rejects_random() {
    let err = validate_enrollment_status("done").unwrap_err();
    assert!(err.contains("enrollment status"));
}

// ---------------------------------------------------------------------------
// validate_push_status — 4 valid + reject
// ---------------------------------------------------------------------------

#[test]
fn validate_push_status_accepts_pending() {
    assert!(validate_push_status("pending").is_ok());
}

#[test]
fn validate_push_status_accepts_in_progress() {
    assert!(validate_push_status("in_progress").is_ok());
}

#[test]
fn validate_push_status_accepts_success() {
    assert!(validate_push_status("success").is_ok());
}

#[test]
fn validate_push_status_accepts_failed() {
    assert!(validate_push_status("failed").is_ok());
}

#[test]
fn validate_push_status_rejects_random() {
    let err = validate_push_status("retry").unwrap_err();
    assert!(err.contains("push status"));
}

// ---------------------------------------------------------------------------
// Response DTO serialization
// ---------------------------------------------------------------------------

#[test]
fn enrollment_device_push_response_serializes_with_all_fields() {
    let resp = EnrollmentDevicePushResponse {
        id: "push-1".into(),
        device_id: "dev-1".into(),
        device_name: "K1T-A".into(),
        status: "success".into(),
        error_message: Some("oops".into()),
        started_at: Some("2026-04-28T10:00:00Z".into()),
        completed_at: Some("2026-04-28T10:00:30Z".into()),
    };
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"id\":\"push-1\""));
    assert!(s.contains("\"device_id\":\"dev-1\""));
    assert!(s.contains("\"device_name\":\"K1T-A\""));
    assert!(s.contains("\"status\":\"success\""));
    assert!(s.contains("\"error_message\":\"oops\""));
    assert!(s.contains("\"started_at\":\"2026-04-28T10:00:00Z\""));
}

#[test]
fn enrollment_response_serializes_nested_device_pushes() {
    let resp = EnrollmentResponse {
        id: "enr-1".into(),
        employee_id: "emp-1".into(),
        employee_name: "Ada Lovelace".into(),
        employee_code: "EMP-001".into(),
        status: "in_progress".into(),
        started_at: "2026-04-28T10:00:00Z".into(),
        completed_at: None,
        version: 1,
        device_pushes: vec![EnrollmentDevicePushResponse {
            id: "push-1".into(),
            device_id: "dev-1".into(),
            device_name: "K1T".into(),
            status: "pending".into(),
            error_message: None,
            started_at: None,
            completed_at: None,
        }],
    };
    let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["id"], "enr-1");
    assert_eq!(v["employee_name"], "Ada Lovelace");
    assert_eq!(v["employee_code"], "EMP-001");
    assert_eq!(v["status"], "in_progress");
    assert_eq!(v["device_pushes"][0]["device_id"], "dev-1");
    // completed_at is Option<String> with no skip — must be present as null.
    assert!(v["completed_at"].is_null());
}

#[test]
fn enrollment_submit_response_serializes_face_id() {
    let resp = EnrollmentSubmitResponse {
        enrollment_id: "enr-1".into(),
        face_id: "face-uuid".into(),
        device_pushes: vec![],
    };
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"face_id\":\"face-uuid\""));
    assert!(s.contains("\"device_pushes\":[]"));
}

#[test]
fn retry_response_serializes() {
    let resp = RetryResponse {
        enrollment_id: "enr-1".into(),
        device_id: "dev-1".into(),
        status: "pending".into(),
    };
    let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["enrollment_id"], "enr-1");
    assert_eq!(v["device_id"], "dev-1");
    assert_eq!(v["status"], "pending");
}

#[test]
fn capture_from_device_response_serializes() {
    let resp = CaptureFromDeviceResponse {
        capture_id: "cap-1".into(),
        status: "capturing".into(),
        source_device_id: "dev-1".into(),
    };
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"capture_id\":\"cap-1\""));
    assert!(s.contains("\"status\":\"capturing\""));
    assert!(s.contains("\"source_device_id\":\"dev-1\""));
}

// ---------------------------------------------------------------------------
// CaptureResponse — skip_serializing_if branches
// ---------------------------------------------------------------------------

#[test]
fn capture_response_omits_photo_b64_when_none() {
    let resp = CaptureResponse {
        capture_id: "cap-1".into(),
        status: "capturing".into(),
        source_device_id: "dev-1".into(),
        photo_b64: None,
        error_message: None,
    };
    let s = serde_json::to_string(&resp).unwrap();
    assert!(
        !s.contains("photo_path"),
        "internal capture paths must never be public: {s}"
    );
    // photo_b64 must be absent (skip_serializing_if = "Option::is_none").
    assert!(
        !s.contains("photo_b64"),
        "expected photo_b64 omitted, got: {s}"
    );
}

#[test]
fn capture_response_includes_photo_b64_when_some() {
    let resp = CaptureResponse {
        capture_id: "cap-1".into(),
        status: "captured".into(),
        source_device_id: "dev-1".into(),
        photo_b64: Some("aGVsbG8=".into()),
        error_message: None,
    };
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"photo_b64\":\"aGVsbG8=\""));
}

// ---------------------------------------------------------------------------
// CaptureFromDeviceRequest deserialization
// ---------------------------------------------------------------------------

#[test]
fn capture_from_device_request_deserializes_json() {
    let json = serde_json::json!({
        "device_id": "dev-1",
        "employee_id": "emp-1",
    });
    let req: CaptureFromDeviceRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.device_id, "dev-1");
    assert_eq!(req.employee_id, "emp-1");
}

#[test]
fn capture_from_device_request_rejects_missing_field() {
    let json = serde_json::json!({"device_id": "dev-1"});
    let result: Result<CaptureFromDeviceRequest, _> = serde_json::from_value(json);
    assert!(result.is_err(), "expected missing field rejection");
}

// ---------------------------------------------------------------------------
// EnrollmentListQuery deserialization/defaults
// ---------------------------------------------------------------------------

#[test]
fn enrollment_list_query_deserializes_and_defaults() {
    let query: EnrollmentListQuery = serde_json::from_value(serde_json::json!({
        "status": "in_progress",
        "limit": 25,
        "offset": 5,
    }))
    .unwrap();
    assert_eq!(query.status.as_deref(), Some("in_progress"));
    assert_eq!(query.limit, Some(25));
    assert_eq!(query.offset, Some(5));

    let default = EnrollmentListQuery::default();
    assert!(default.status.is_none());
    assert!(default.limit.is_none());
    assert!(default.offset.is_none());
    assert!(format!("{default:?}").contains("EnrollmentListQuery"));
}

// ---------------------------------------------------------------------------
// Debug impls — exercise the derive
// ---------------------------------------------------------------------------

#[test]
fn dtos_have_debug_impls() {
    let r = RetryResponse {
        enrollment_id: "x".into(),
        device_id: "y".into(),
        status: "pending".into(),
    };
    let s = format!("{:?}", r);
    assert!(s.contains("RetryResponse"));

    let c = CaptureResponse {
        capture_id: "c".into(),
        status: "capturing".into(),
        source_device_id: "d".into(),
        photo_b64: None,
        error_message: None,
    };
    let s = format!("{:?}", c);
    assert!(s.contains("CaptureResponse"));

    let s = format!(
        "{:?}",
        CaptureFromDeviceResponse {
            capture_id: "c".into(),
            status: "capturing".into(),
            source_device_id: "d".into(),
        }
    );
    assert!(s.contains("CaptureFromDeviceResponse"));

    let s = format!(
        "{:?}",
        EnrollmentDevicePushResponse {
            id: "p".into(),
            device_id: "d".into(),
            device_name: "n".into(),
            status: "pending".into(),
            error_message: None,
            started_at: None,
            completed_at: None,
        }
    );
    assert!(s.contains("EnrollmentDevicePushResponse"));

    let s = format!(
        "{:?}",
        EnrollmentResponse {
            id: "e".into(),
            employee_id: "emp".into(),
            employee_name: "Employee".into(),
            employee_code: "EMP-1".into(),
            status: "in_progress".into(),
            started_at: "ts".into(),
            completed_at: None,
            version: 1,
            device_pushes: vec![],
        }
    );
    assert!(s.contains("EnrollmentResponse"));

    let s = format!(
        "{:?}",
        EnrollmentSubmitResponse {
            enrollment_id: "e".into(),
            face_id: "f".into(),
            device_pushes: vec![],
        }
    );
    assert!(s.contains("EnrollmentSubmitResponse"));

    let s = format!(
        "{:?}",
        CaptureFromDeviceRequest {
            device_id: "d".into(),
            employee_id: "e".into(),
        }
    );
    assert!(s.contains("CaptureFromDeviceRequest"));

    let s = format!(
        "{:?}",
        CreateEnrollmentRequest {
            employee_id: "emp-1".into(),
            captured_via: "upload".into(),
            source_device_id: None,
            face_quality_score: acceptable_face_quality(),
            photo_bytes: vec![0xFF, 0xD8, 0xFF],
        }
    );
    assert!(s.contains("CreateEnrollmentRequest"));
}
