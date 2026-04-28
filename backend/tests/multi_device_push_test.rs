//! Phase 7 — Multi-device concurrent push integration tests (Wave 0 scaffolds).
//!
//! Covers D-06 (JoinSet fan-out), D-08 (partial failure), D-16 (backfill Semaphore=4).
//! Populated in Task 5.

mod common;

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_joinset_fans_out_to_all_active_devices_concurrently() {
    todo!("implement after Task 5 pusher.rs lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_partial_failure_sets_enrollment_status_partial() {
    todo!("implement after Task 5 finalize_enrollment_status lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_zero_devices_succeed_sets_failed() {
    todo!("implement after Task 5 finalize_enrollment_status lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_all_devices_succeed_sets_success() {
    todo!("implement after Task 5 finalize_enrollment_status lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_backfill_respects_semaphore_4_max_in_flight() {
    todo!("implement after Task 5 BackfillWorker + Semaphore lands")
}
