use serde::{Deserialize, Serialize};

/// A single audit log entry (read-only view of the audit_log table).
/// The audit_log table is append-only — database triggers reject UPDATE and DELETE.
/// old_data and new_data are stored as TEXT (JSON) and deserialized here to
/// serde_json::Value so the frontend can render diffs without a second parse.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub id: String,
    pub table_name: String,
    pub record_id: String,
    pub operation: String,                   // "INSERT" | "UPDATE" | "DELETE"
    pub old_data: Option<serde_json::Value>, // parsed from old_data TEXT column
    pub new_data: Option<serde_json::Value>, // parsed from new_data TEXT column
    pub actor_id: Option<String>,
    pub created_at: i64, // epoch seconds
}

/// A distinct actor that appears in the audit_log, joined with users for display.
///
/// LEFT JOIN preserves rows where actor_id is NULL (system triggers) or references
/// a deleted user (LEFT JOIN miss). All fields are Option<String> for that reason.
/// This struct is response-only — no Deserialize needed.
#[derive(Debug, Clone, Serialize)]
pub struct AuditActor {
    pub actor_id: Option<String>, // NULL when audit_log.actor_id IS NULL
    pub username: Option<String>, // NULL when user was deleted (LEFT JOIN miss)
    pub role: Option<String>,     // NULL same
}

/// Query parameters for `GET /api/v1/audit`.
/// All fields are optional — omitting a filter returns all rows.
#[derive(Debug, Deserialize)]
pub struct AuditListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub actor_id: Option<String>,
    pub table_name: Option<String>,
    pub record_id: Option<String>,
    pub operation: Option<String>,
    pub from_ts: Option<i64>, // epoch seconds inclusive lower bound
    pub to_ts: Option<i64>,   // epoch seconds inclusive upper bound
}
