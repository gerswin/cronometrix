//! Reports module — Phase 5 calculation API.
//!
//! Submodules:
//! - `money` — pure cents-i64 LOTTT premium math (Art. 117/118/120). No I/O.
//! - `periods` — period boundary math (ISO weekly + calendar bi-weekly + monthly + custom). No I/O.
//! - `models` — DTOs for the JSON API surface.
//! - `service` — SQL aggregation across daily_records + overrides + leaves + anomalies, plus
//!   secondary leaves aggregation (W-5) and app-code audit insert (D-21).
//! - `handlers` — Axum handler `generate_json` for `POST /api/v1/reports/json`.

pub mod handlers;
pub mod models;
pub mod money;
pub mod periods;
pub mod service;

pub use models::{
    Aggregates, BrandingHeader, EmployeeReportRow, ReportParamsRequest, ReportPayload,
};
pub use service::compute_report;
