//! Leaves CRUD + overlap check + calc-engine overlay query.
//!
//! Public surface (consumed by leaves::handlers + daily_records::service):
//! - create_leave: overlap check → LeaveConflict on collision; INSERT otherwise.
//! - get_by_id / list: Viewer+ reads.
//! - cancel: soft-delete with optimistic concurrency.
//! - fetch_active_leave_for_date: populates EngineInput.leave (D-16 overlay).
//! - leaves_root: filesystem root for evidence files (env-overridable in tests).
//!
//! Implementation is filled in Task 2.

#![allow(dead_code)]

use std::path::PathBuf;

// Placeholder — full implementation lands in Task 2 of this plan.
// Kept as a module so `backend/src/leaves/mod.rs` compiles before Task 2.

/// Root directory for evidence files. Configurable via env for tests.
/// Mirrors `events::service::events_root()` pattern.
pub fn leaves_root() -> PathBuf {
    std::env::var("CRONOMETRIX_LEAVES_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data/leaves"))
}
