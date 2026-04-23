//! Pure `compute_daily_record()` orchestrator — no I/O, no async.
//! Task 2 of Plan 03-01 fills this module with the real computation.

use super::models::{DailyRecordOutput, EngineInput};

/// Placeholder until Task 2. Always returns a zero-valued DailyRecordOutput so
/// downstream code compiles. Task 2 replaces the implementation.
pub fn compute_daily_record(_input: &EngineInput) -> DailyRecordOutput {
    DailyRecordOutput {
        work_minutes: 0,
        overtime_minutes: 0,
        late_minutes: 0,
        early_departure_minutes: 0,
        is_rest_day_worked: false,
        entry_at: None,
        exit_at: None,
        leave_id: None,
        anomalies: Vec::new(),
    }
}
