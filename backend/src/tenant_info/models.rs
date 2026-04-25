use serde::{Deserialize, Serialize};
use validator::Validate;

/// Tenant info singleton record returned by GET /tenant-info.
/// Always represents `id = 1` row. Timestamps are ISO 8601 strings per D-13.
#[derive(Debug, Serialize, Clone)]
pub struct TenantInfo {
    pub client_name: String,
    pub client_rif: String,
    pub address: String,
    pub version: i64,
    pub updated_at: String, // ISO 8601 via crate::common::epoch_to_iso
}

/// Request body for PATCH /tenant-info. All fields optional;
/// `version` is required for optimistic concurrency per D-04.
///
/// Validation is intentionally minimal in v1 (CONTEXT D-30 "minimal scope").
/// Tighter Venezuelan RIF regex deferred — we only enforce length bounds.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTenantInfoRequest {
    #[validate(length(max = 200, message = "client_name max 200 chars"))]
    pub client_name: Option<String>,

    #[validate(length(max = 50, message = "client_rif max 50 chars"))]
    pub client_rif: Option<String>,

    #[validate(length(max = 500, message = "address max 500 chars"))]
    pub address: Option<String>,

    pub version: i64,
}
