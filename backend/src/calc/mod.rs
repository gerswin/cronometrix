//! Pure domain engine for attendance calculation. No I/O, no async.
//! Plan 03-01 wires the foundation; Plan 03-02 adds overnight + chrono-tz;
//! Plan 03-03 wires leave overlay.

pub mod aggregation;
pub mod anomalies;
pub mod engine;
pub mod lunch;
pub mod models;
pub mod overnight;
pub mod overtime;

pub use anomalies::AnomalyCode;
pub use engine::compute_daily_record;
pub use models::{DailyRecordOutput, EngineInput};
