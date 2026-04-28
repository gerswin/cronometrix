//! Phase 7 — Enrollment lifecycle integration tests (Wave 0 scaffolds).
//!
//! Covers: re-enrollment (D-14), employee deactivation → purge (D-15),
//! PurgeWorker Pitfall-10 guard, new device → backfill (D-16), audit triggers (D-17).
//! Populated in Tasks 2, 5, and 6.

mod common;

// ---------------------------------------------------------------------------
// Task 2 — audit trigger test (populated after migrations 016/017 land)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_audit_log_rows_written_for_enrollments_face_enrollments_device_face_mappings() {
    // This test is populated in Task 2 with real DB assertions.
    // It is NOT #[ignore] because Task 2 populates it before committing.
    // Until Task 2, it is a compile-check placeholder.
    #[allow(dead_code)]
    let _db = common::test_db().await;
    // Task 2 will fill in: INSERT rows -> SELECT audit_log -> assert count.
    // For now: pass (no assertions yet).
}

// ---------------------------------------------------------------------------
// Task 5 — lifecycle tests (populated after pusher/workers land)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_re_enrollment_keeps_face_id_constant() {
    todo!("implement after Task 4 start_enrollment + D-14 logic lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_employee_deactivation_publishes_purge_request() {
    todo!("implement after Task 5 purge_tx wired in deactivate_employee")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_purge_worker_calls_userinfodetail_delete_per_mapped_device() {
    todo!("implement after Task 5 PurgeWorker lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_purge_worker_aborts_if_employee_reactivated_mid_loop() {
    todo!("implement after Task 5 Pitfall-10 guard lands in PurgeWorker")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_new_device_registration_publishes_backfill_request() {
    todo!("implement after Task 5 backfill_tx wired in create_device")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_backfill_pushes_every_active_face_id_employee_to_new_device() {
    todo!("implement after Task 5 BackfillWorker lands")
}
