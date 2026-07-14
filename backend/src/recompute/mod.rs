pub mod nightly;
pub mod worker;

use chrono::NaiveDate;

/// Bounded work unit consumed by [`worker::RecomputeWorker`]. A leave range is
/// represented by two dates and expanded by the worker in constant memory,
/// never by a database post-commit callback.
#[derive(Debug, Clone)]
pub enum RecomputeRequest {
    Day {
        employee_id: String,
        anchor_date: NaiveDate,
    },
    Range {
        employee_id: String,
        from_date: NaiveDate,
        to_date: NaiveDate,
    },
}
