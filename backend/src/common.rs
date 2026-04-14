use serde::Serialize;

/// Paginated response wrapper returned by list endpoints per D-12.
/// `total` is the total count matching the filter (before pagination).
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Convert a UTC epoch integer to an ISO 8601 / RFC 3339 string per D-13.
/// Returns empty string if the epoch is zero or out of range.
pub fn epoch_to_iso(epoch: i64) -> String {
    chrono::DateTime::from_timestamp(epoch, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// Convert an optional UTC epoch integer to an optional ISO 8601 string.
/// Used for nullable timestamp columns such as `deleted_at`.
pub fn epoch_to_iso_opt(epoch: Option<i64>) -> Option<String> {
    epoch.map(epoch_to_iso)
}
