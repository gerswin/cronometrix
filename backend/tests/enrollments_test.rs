//! Phase 7 — Enrollment endpoint integration tests (Wave 0 scaffolds).
//!
//! These tests are populated progressively across Tasks 2-6.
//! Scaffolds marked #[ignore] compile but do not execute until the
//! production modules they reference are implemented.

mod common;

// ---------------------------------------------------------------------------
// Task 4 — populated: validation, downscale, face_id stability, status polling
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_create_enrollment_returns_202_with_enrollment_id() {
    todo!("implement after Task 4 handlers land")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_create_enrollment_rejects_non_jpeg_magic_bytes() {
    todo!("implement after Task 4 handlers land")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6 (RequestBodyLimitLayer wired in main.rs)"]
async fn test_create_enrollment_rejects_over_2mb_upload_with_413() {
    todo!("implement after Task 6 body limit wired")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_create_enrollment_downscales_4mb_jpeg_to_under_200kb() {
    todo!("implement after Task 4 image_pipeline + handlers land")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_get_enrollment_returns_per_device_pushes() {
    todo!("implement after Task 4 service land")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6"]
async fn test_retry_push_re_fires_single_device() {
    todo!("implement after Task 6 route wired")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_face_id_assigned_on_first_enrollment_stable_thereafter() {
    todo!("implement after Task 4 start_enrollment logic lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 2"]
async fn test_audit_log_rows_written_for_enrollments_face_enrollments_device_face_mappings() {
    todo!("implement in Task 2 — trigger test")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6"]
async fn test_non_admin_role_403_on_every_enrollment_endpoint() {
    todo!("implement after Task 6 routes wired in admin_routes")
}
