// Stub — populated in Task 5.
#![allow(unused)]

/// Request to purge all device face mappings for a deactivated employee (D-15).
#[derive(Debug, Clone)]
pub struct PurgeRequest {
    pub employee_id: String,
}
