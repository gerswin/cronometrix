//! Integration tests for leave management (LEAVE-01..04, Plan 03-03).
//!
//! Coverage lands in Task 2: this Wave-0 scaffold just verifies that the
//! module tree compiles cleanly so downstream tests can build on top of it.

mod common;

#[tokio::test]
async fn wave_zero_marker() {
    // Task 1 scaffold: migrations register, leaves module compiles.
    assert!(true, "Wave 0 scaffold compiles");
}
