//! Leave management (LEAVE-01..04, Plan 03-03).
//!
//! Full-day leave only (D-14), immediate approval (D-15), overlay precedence
//! into the calc engine (D-16). Evidence upload via multipart form-data is
//! required for `medical` leaves (T-3-15 mitigation: server-generated paths
//! under ./data/leaves/, never user-controlled).

pub mod handlers;
pub mod models;
pub mod service;
