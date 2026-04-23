pub mod nightly;
pub mod worker;

use chrono::NaiveDate;

/// Request published by event ingestion to trigger a recompute of a single
/// (employee_id, anchor_date) DailyRecord. Consumed by [`worker::RecomputeWorker`].
#[derive(Debug, Clone)]
pub struct RecomputeRequest {
    pub employee_id: String,
    pub anchor_date: NaiveDate,
}
