use serde::{Deserialize, Serialize};

/// Read-side representation of an attendance event. Intentionally EXCLUDES
/// `raw_xml` (T-2-14: raw XML is kept for forensic re-parsing per D-12 but is
/// NEVER exposed on the public API).
#[derive(Debug, Serialize)]
pub struct AttendanceEventResponse {
    pub id: String,
    pub employee_id: Option<String>,
    pub device_id: String,
    pub direction: String,
    pub captured_at: String, // ISO 8601 (converted from epoch)
    pub is_unknown: bool,
    pub face_id: Option<String>,
    pub employee_no_string: Option<String>,
    pub photo_path: Option<String>, // relative path; served via /events/:id/photo
    pub created_at: String,
}

/// Write-side shape accepted by `events::service::persist_attendance_event`.
/// `photo_bytes` is held in memory until the DB INSERT succeeds — this keeps
/// dedup-hits from leaving orphan JPEG files on disk (D-13).
#[derive(Debug)]
pub struct NewAttendanceEvent {
    pub id: String,
    pub employee_id: Option<String>,
    pub device_id: String,
    pub direction: String, // "entry" | "exit"
    pub captured_at: i64,  // UTC epoch seconds
    pub is_unknown: bool,
    pub face_id: Option<String>,
    pub employee_no_string: Option<String>,
    pub raw_xml: String,
    pub photo_bytes: Option<Vec<u8>>, // in-memory; persist helper writes to disk on INSERT success only
}

/// Outcome tag returned by the persist helper. `Inserted` carries the relative
/// `photo_path` that was written (if any) so callers can correlate DB rows to disk.
#[derive(Debug, PartialEq, Eq)]
pub enum PersistOutcome {
    Inserted { photo_path: Option<String> },
    Deduplicated,
}

/// Query string parameters accepted by `GET /api/v1/events`.
/// `from`/`to` are UTC epoch seconds (inclusive / exclusive respectively).
#[derive(Debug, Deserialize)]
pub struct EventListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub employee_id: Option<String>,
    pub device_id: Option<String>,
    pub from: Option<i64>, // UTC epoch seconds (inclusive)
    pub to: Option<i64>,   // UTC epoch seconds (exclusive)
    #[serde(default)]
    pub include_unknown: Option<bool>,
}
