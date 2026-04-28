// Stub — populated in Task 5.
#![allow(unused)]

/// Request to backfill all active employee face profiles to a newly registered device (D-16).
#[derive(Debug, Clone)]
pub struct BackfillRequest {
    pub device_id: String,
}
