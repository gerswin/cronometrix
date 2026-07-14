use std::path::Path;

use chrono::{TimeZone, Utc};
use libsql::{params, Connection};

use crate::common::{epoch_to_iso, PaginatedResponse};
use crate::errors::AppError;
use crate::recompute::RecomputeRequest;
use crate::state::{AppState, AttendanceEventSSEPayload};
use crate::storage::atomic_file::AtomicFileGuard;

use super::models::{AttendanceEventResponse, EventListQuery, NewAttendanceEvent, PersistOutcome};

/// Compatibility entry point retained for the ISAPI stream. Queued persistence
/// now publishes recompute as a worker-owned post-commit callback.
pub fn publish_recompute_if_employee(state: &AppState, event: &NewAttendanceEvent) {
    // Persistence owns recompute publication as a post-commit callback. Keep
    // this compatibility entry point for the ISAPI stream without duplicating
    // the request after a successful insert.
    let _ = (state, event);
}

fn base_sse_payload(event: &NewAttendanceEvent, has_photo: bool) -> AttendanceEventSSEPayload {
    AttendanceEventSSEPayload {
        id: event.id.clone(),
        employee_id: event.employee_id.clone(),
        employee_name: None,
        department: None,
        captured_at: epoch_to_iso(event.captured_at),
        direction: event.direction.clone(),
        has_photo,
    }
}

/// Build the wire payload for a persisted event, enriching a known employee
/// with the current display name and department name when those rows exist.
pub async fn build_sse_payload(
    conn: &Connection,
    event: &NewAttendanceEvent,
    has_photo: bool,
) -> Result<AttendanceEventSSEPayload, AppError> {
    let mut payload = base_sse_payload(event, has_photo);
    let Some(employee_id) = event.employee_id.as_ref() else {
        return Ok(payload);
    };

    let mut rows = conn
        .query(
            "SELECT e.name, d.name \
             FROM employees e \
             LEFT JOIN departments d ON d.id = e.department_id \
             WHERE e.id = ?1",
            params![employee_id.clone()],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    if let Some(row) = rows
        .next()
        .await
        .map_err(|error| AppError::Internal(error.into()))?
    {
        payload.employee_name = row
            .get(0)
            .map_err(|error| AppError::Internal(error.into()))?;
        payload.department = row
            .get(1)
            .map_err(|error| AppError::Internal(error.into()))?;
    }
    Ok(payload)
}

/// Broadcast a newly-inserted attendance event to all SSE stream subscribers.
/// Enrichment is best-effort: a database read failure emits a base payload and
/// never compensates or propagates past the already-successful insert.
pub async fn publish_sse_event(
    state: &AppState,
    event: &NewAttendanceEvent,
    photo_path: &Option<String>,
) {
    // Clone before the first await so no borrow of shared state crosses it.
    let Some(tx) = state.event_broadcast.clone() else {
        return;
    };
    let has_photo = photo_path.is_some();
    let payload = match state.db.connect() {
        Ok(conn) => match build_sse_payload(&conn, event, has_photo).await {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(
                    event_id = %event.id,
                    err = %error,
                    "SSE event enrichment failed; broadcasting base payload"
                );
                base_sse_payload(event, has_photo)
            }
        },
        Err(error) => {
            tracing::warn!(
                event_id = %event.id,
                err = %error,
                "SSE event enrichment connection failed; broadcasting base payload"
            );
            base_sse_payload(event, has_photo)
        }
    };
    // Non-fatal send: broadcast::SendError means no active receivers
    let _ = tx.send(payload);
}

/// SELECT column list for read-side mappers. `raw_xml` is DELIBERATELY absent
/// (T-2-14 — raw XML is never exposed on the API).
const EVENT_SELECT_COLS: &str =
    "id, employee_id, device_id, direction, captured_at, is_unknown, face_id, \
     employee_no_string, photo_path, created_at";

fn row_to_event(row: libsql::Row) -> Result<AttendanceEventResponse, AppError> {
    let is_unknown_int: i64 = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
    Ok(AttendanceEventResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        device_id: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        direction: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        captured_at: epoch_to_iso(row.get(4).map_err(|e| AppError::Internal(e.into()))?),
        is_unknown: is_unknown_int != 0,
        face_id: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        employee_no_string: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        photo_path: row.get(8).map_err(|e| AppError::Internal(e.into()))?,
        created_at: epoch_to_iso(row.get(9).map_err(|e| AppError::Internal(e.into()))?),
    })
}

/// Persist an attendance event with DB-level dedup. Returns `Deduplicated` when
/// another event with the same (employee_id, device_id, direction, bucket_30s)
/// tuple already exists (D-05/D-06). The photo is protected before the INSERT;
/// deduplication or any database failure drops the guard and removes it (D-13).
///
/// `events_root` is the filesystem root for the JPEG (Phase 8, D-18/D-19): the
/// caller passes `&state.paths.events_root` from production code or a tempdir
/// path from tests, eliminating the cwd-dependent + env-var-race anti-pattern.
pub async fn persist_attendance_event(
    conn: &Connection,
    events_root: &Path,
    event: NewAttendanceEvent,
) -> Result<PersistOutcome, AppError> {
    let bucket = event.captured_at / 30;
    let photo_relpath: Option<String> = event.photo_bytes.as_ref().map(|_| {
        let date = Utc
            .timestamp_opt(event.captured_at, 0)
            .single()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown-date".into());
        format!("{}/{}.jpg", date, event.id)
    });

    let photo_guard = match (event.photo_bytes.as_ref(), photo_relpath.as_ref()) {
        (Some(bytes), Some(relative_path)) => Some(
            AtomicFileGuard::write(events_root, relative_path, bytes)
                .map_err(AppError::Internal)?,
        ),
        _ => None,
    };

    let rows_affected = conn
        .execute(
            "INSERT OR IGNORE INTO attendance_events \
             (id, employee_id, device_id, direction, captured_at, bucket_30s, \
              is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())",
            params![
                event.id.clone(),
                event.employee_id.clone(),
                event.device_id.clone(),
                event.direction.clone(),
                event.captured_at,
                bucket,
                event.is_unknown as i64,
                event.face_id.clone(),
                event.employee_no_string.clone(),
                event.raw_xml.clone(),
                photo_relpath.clone(),
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        return Ok(PersistOutcome::Deduplicated);
    }

    if let Some(guard) = photo_guard {
        guard.keep();
    }

    Ok(PersistOutcome::Inserted {
        photo_path: photo_relpath,
    })
}

pub async fn persist_attendance_event_queued(
    state: &AppState,
    events_root: &Path,
    event: NewAttendanceEvent,
) -> Result<PersistOutcome, AppError> {
    let bucket = event.captured_at / 30;
    let photo_relpath: Option<String> = event.photo_bytes.as_ref().map(|_| {
        let date = Utc
            .timestamp_opt(event.captured_at, 0)
            .single()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown-date".into());
        format!("{}/{}.jpg", date, event.id)
    });

    let photo_guard = match (event.photo_bytes.as_ref(), photo_relpath.as_ref()) {
        (Some(bytes), Some(relative_path)) => Some(
            AtomicFileGuard::write(events_root, relative_path, bytes)
                .map_err(AppError::Internal)?,
        ),
        _ => None,
    };

    let queued_photo_relpath = photo_relpath.clone();
    let recompute_tx = state.recompute_tx.clone();
    let recompute_request = event.employee_id.as_ref().and_then(|employee_id| {
        Utc.timestamp_opt(event.captured_at, 0)
            .single()
            .map(|captured_at| RecomputeRequest {
                employee_id: employee_id.clone(),
                anchor_date: captured_at
                    .with_timezone(&state.config.timezone)
                    .date_naive(),
            })
    });
    let rows_affected = state
        .db_write
            .transact("events.ingest-attendance", move |tx| {
                Box::pin(async move {
                    let rows_affected = tx
                        .statement(
                            "INSERT OR IGNORE INTO attendance_events \
                             (id, employee_id, device_id, direction, captured_at, bucket_30s, \
                              is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())",
                            params![
                                event.id,
                                event.employee_id,
                                event.device_id,
                                event.direction,
                                event.captured_at,
                                bucket,
                                event.is_unknown as i64,
                                event.face_id,
                                event.employee_no_string,
                                event.raw_xml,
                                queued_photo_relpath,
                            ],
                        )
                        .await?;
                    if rows_affected > 0 {
                        tx.after_commit(move || {
                            if let Some(guard) = photo_guard {
                                guard.keep();
                            }
                            if let (Some(sender), Some(request)) =
                                (recompute_tx, recompute_request)
                            {
                                let _ = sender.send(request);
                            }
                        });
                    }
                    Ok(rows_affected)
                })
            })
            .await
            .map_err(AppError::from)?;
    if rows_affected == 0 {
        Ok(PersistOutcome::Deduplicated)
    } else {
        Ok(PersistOutcome::Inserted {
            photo_path: photo_relpath,
        })
    }
}

/// Compatibility wrapper for non-transactional enrollment photo callers.
/// Publishes durably without clobbering and immediately keeps the file.
pub fn write_photo_atomic(root: &Path, relpath: &str, bytes: &[u8]) -> anyhow::Result<()> {
    AtomicFileGuard::write(root, relpath, bytes)?.keep();
    Ok(())
}

/// Resolve an event to an employee given the (device_id, face_id, employee_no_string)
/// triple emitted by Hikvision alertStream. Priority: (1) device_face_mappings, then
/// (2) employees.employee_code == employee_no_string fallback (per A3 in 02-RESEARCH).
pub async fn lookup_employee_for_event(
    conn: &Connection,
    device_id: &str,
    face_id: Option<&str>,
    employee_no_string: Option<&str>,
) -> Result<Option<String>, AppError> {
    // Priority 1: device_face_mappings lookup
    if let Some(fid) = face_id {
        let mut rows = conn
            .query(
                "SELECT employee_id FROM device_face_mappings WHERE device_id = ?1 AND face_id = ?2",
                params![device_id.to_string(), fid.to_string()],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| AppError::Internal(e.into()))?
        {
            return Ok(Some(row.get(0).map_err(|e| AppError::Internal(e.into()))?));
        }
    }

    // Priority 2: employees.employee_code == employee_no_string fallback
    if let Some(ens) = employee_no_string {
        if !ens.is_empty() {
            let mut rows = conn
                .query(
                    "SELECT id FROM employees WHERE employee_code = ?1 AND status = 'active'",
                    params![ens.to_string()],
                )
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| AppError::Internal(e.into()))?
            {
                return Ok(Some(row.get(0).map_err(|e| AppError::Internal(e.into()))?));
            }
        }
    }

    Ok(None)
}

/// List attendance events with optional filters + pagination per D-12.
/// Filters: employee_id, device_id, captured_at range [from, to).
/// When `include_unknown` is None or true, is_unknown rows are included;
/// when false, rows with is_unknown=1 are omitted.
pub async fn list(
    conn: &Connection,
    q: EventListQuery,
) -> Result<PaginatedResponse<AttendanceEventResponse>, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    if let Some(emp) = &q.employee_id {
        predicates.push(format!("employee_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(emp.clone()));
        fetch_values.push(libsql::Value::Text(emp.clone()));
    }

    if let Some(dev) = &q.device_id {
        predicates.push(format!("device_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(dev.clone()));
        fetch_values.push(libsql::Value::Text(dev.clone()));
    }

    if let Some(from) = q.from {
        predicates.push(format!("captured_at >= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Integer(from));
        fetch_values.push(libsql::Value::Integer(from));
    }

    if let Some(to) = q.to {
        predicates.push(format!("captured_at < ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Integer(to));
        fetch_values.push(libsql::Value::Integer(to));
    }

    // include_unknown: default true (None => include); false => filter out unknowns
    if matches!(q.include_unknown, Some(false)) {
        predicates.push("is_unknown = 0".to_string());
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM attendance_events {}", where_clause);
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT query returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT {cols} FROM attendance_events {where_clause} \
         ORDER BY captured_at DESC, id ASC LIMIT ?{lim} OFFSET ?{off}",
        cols = EVENT_SELECT_COLS,
        where_clause = where_clause,
        lim = fetch_values.len() + 1,
        off = fetch_values.len() + 2,
    );

    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        data.push(row_to_event(row)?);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

/// Fetch a single event by id. Returns `NotFound(EVENT_NOT_FOUND)` when absent.
pub async fn get_by_id(conn: &Connection, id: &str) -> Result<AttendanceEventResponse, AppError> {
    let sql = format!(
        "SELECT {} FROM attendance_events WHERE id = ?1",
        EVENT_SELECT_COLS
    );
    let row = conn
        .query(&sql, params![id.to_string()])
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "EVENT_NOT_FOUND",
            message: format!("Event '{}' not found", id),
        })?;

    row_to_event(row)
}

/// Fetch the relative photo path for an event. Returns:
/// - `NotFound(EVENT_NOT_FOUND)` — the event row itself doesn't exist
/// - `NotFound(EVENT_PHOTO_NOT_FOUND)` — the event exists but has no photo_path
pub async fn get_photo_path(conn: &Connection, id: &str) -> Result<String, AppError> {
    let row = conn
        .query(
            "SELECT photo_path FROM attendance_events WHERE id = ?1",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "EVENT_NOT_FOUND",
            message: format!("Event '{}' not found", id),
        })?;

    let photo_path: Option<String> = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    photo_path.ok_or_else(|| AppError::NotFound {
        code: "EVENT_PHOTO_NOT_FOUND",
        message: "Photo not available for this event".to_string(),
    })
}

// =============================================================================
// Unit tests — exercise persist helper against isolated file-backed libSQL DBs.
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use libsql::{Builder, Database};
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Each test owns a TempDir that supplies the `events_root` path directly
    /// to `persist_attendance_event` (Phase 8, D-18/D-19). No env mutation, no
    /// process-global mutex — TempDir cleanup happens at end of test scope.
    fn fresh_events_root() -> TempDir {
        TempDir::new().expect("temp dir")
    }

    async fn setup_db() -> Database {
        // Unique temp path per test so tests are isolated.
        let tmp_path = format!("/tmp/cronometrix_events_test_{}.db", Uuid::new_v4());
        let db = Builder::new_local(&tmp_path).build().await.expect("db");
        let conn = db.connect().expect("connect");
        conn.execute("PRAGMA foreign_keys = ON;", ())
            .await
            .expect("pragma");
        crate::db::run_migrations(&conn)
            .await
            .expect("migrations applied");
        db
    }

    /// Seed a device with a unique (ip, port) derived from `id` so callers can
    /// create multiple devices in one test without tripping the partial UNIQUE
    /// index on (ip, port) for active rows.
    async fn seed_device(conn: &Connection, id: &str) {
        // Derive a unique port from the id digest so cross-device tests work.
        let hash: u32 = id
            .as_bytes()
            .iter()
            .fold(0u32, |acc, b| acc.wrapping_mul(131).wrapping_add(*b as u32));
        let port = 1024 + (hash % 60000) as i64;
        let ip = format!("10.0.{}.{}", (hash >> 8) & 0xFF, hash & 0xFF);
        conn.execute(
            "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
             direction, allow_insecure_tls, connection_state, status, version, \
             created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'https', 'admin', 'ciphertext', \
             'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
            params![id.to_string(), format!("dev-{}", id), ip, port],
        )
        .await
        .expect("seed device");
    }

    async fn seed_employee(conn: &Connection, id: &str, code: &str) {
        // Seed a department first (FK requirement).
        let dept_id = format!("dept-{}", id);
        conn.execute(
            "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
             lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
             VALUES (?1, ?2, 0, '09:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
            params![dept_id.clone(), format!("Dept {}", id)],
        )
        .await
        .expect("seed dept");
        conn.execute(
            "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
            params![id.to_string(), code.to_string(), format!("Emp {}", id), dept_id],
        )
        .await
        .expect("seed employee");
    }

    fn sample_event(
        id: &str,
        employee_id: Option<&str>,
        device_id: &str,
        direction: &str,
        captured_at: i64,
    ) -> NewAttendanceEvent {
        NewAttendanceEvent {
            id: id.to_string(),
            employee_id: employee_id.map(str::to_string),
            device_id: device_id.to_string(),
            direction: direction.to_string(),
            captured_at,
            is_unknown: false,
            face_id: Some("42".to_string()),
            employee_no_string: Some("EMP001".to_string()),
            raw_xml: "<EventNotificationAlert/>".to_string(),
            photo_bytes: None,
        }
    }

    #[tokio::test]
    async fn persist_dedup_within_30s() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        // bucket_30s is floor(captured_at / 30). Bucket 33 = [990..=1019].
        // Two inserts inside the same bucket must dedup.
        let e1 = sample_event("evt-1", Some("e1"), "d1", "entry", 1000);
        let o1 = persist_attendance_event(&conn, tmp.path(), e1)
            .await
            .unwrap();
        assert!(matches!(o1, PersistOutcome::Inserted { .. }));

        let e2 = sample_event("evt-2", Some("e1"), "d1", "entry", 1019);
        let o2 = persist_attendance_event(&conn, tmp.path(), e2)
            .await
            .unwrap();
        assert_eq!(o2, PersistOutcome::Deduplicated);
    }

    #[tokio::test]
    async fn persist_cross_device_within_30s() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_device(&conn, "d2").await;
        seed_employee(&conn, "e1", "EMP001").await;

        let e1 = sample_event("evt-1", Some("e1"), "d1", "entry", 1000);
        let e2 = sample_event("evt-2", Some("e1"), "d2", "entry", 1010);

        let o1 = persist_attendance_event(&conn, tmp.path(), e1)
            .await
            .unwrap();
        let o2 = persist_attendance_event(&conn, tmp.path(), e2)
            .await
            .unwrap();
        assert!(matches!(o1, PersistOutcome::Inserted { .. }));
        assert!(matches!(o2, PersistOutcome::Inserted { .. }));
    }

    #[tokio::test]
    async fn persist_adjacent_buckets() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        // bucket 33 (1000 / 30) and bucket 34 (1030 / 30) — adjacent.
        let e1 = sample_event("evt-1", Some("e1"), "d1", "entry", 1000);
        let e2 = sample_event("evt-2", Some("e1"), "d1", "entry", 1030);

        let o1 = persist_attendance_event(&conn, tmp.path(), e1)
            .await
            .unwrap();
        let o2 = persist_attendance_event(&conn, tmp.path(), e2)
            .await
            .unwrap();
        assert!(matches!(o1, PersistOutcome::Inserted { .. }));
        assert!(matches!(o2, PersistOutcome::Inserted { .. }));
    }

    #[tokio::test]
    async fn persist_epoch_is_utc_integer() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        let epoch = 1_700_000_000_i64;
        let ev = sample_event("evt-1", Some("e1"), "d1", "entry", epoch);
        let _ = persist_attendance_event(&conn, tmp.path(), ev)
            .await
            .unwrap();

        let mut rows = conn
            .query(
                "SELECT captured_at FROM attendance_events WHERE id = ?1",
                params!["evt-1".to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("row");
        let stored: i64 = row.get(0).unwrap();
        assert_eq!(
            stored, epoch,
            "captured_at must round-trip as UTC epoch int"
        );
    }

    #[tokio::test]
    async fn persist_raw_xml_round_trip() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        let xml = "<EventNotificationAlert version=\"2.0\">\
                   <employeeNoString>EMP001</employeeNoString>\
                   <faceID>42</faceID>\
                   <dateTime>2026-04-19T12:34:56+00:00</dateTime>\
                   </EventNotificationAlert>";
        let mut ev = sample_event("evt-1", Some("e1"), "d1", "entry", 1000);
        ev.raw_xml = xml.to_string();
        let _ = persist_attendance_event(&conn, tmp.path(), ev)
            .await
            .unwrap();

        let mut rows = conn
            .query(
                "SELECT raw_xml FROM attendance_events WHERE id = ?1",
                params!["evt-1".to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("row");
        let stored: String = row.get(0).unwrap();
        assert_eq!(stored, xml, "raw_xml must round-trip byte-for-byte");
    }

    #[tokio::test]
    async fn persist_unknown_face_sets_is_unknown() {
        let tmp = fresh_events_root();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;

        let mut ev = sample_event("evt-1", None, "d1", "entry", 1000);
        ev.is_unknown = true;
        ev.employee_no_string = None; // unknown faces have no resolved employee
        ev.face_id = Some("42".to_string());
        let _ = persist_attendance_event(&conn, tmp.path(), ev)
            .await
            .unwrap();

        let mut rows = conn
            .query(
                "SELECT employee_id, is_unknown, face_id FROM attendance_events WHERE id = ?1",
                params!["evt-1".to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("row");
        let employee_id: Option<String> = row.get(0).unwrap();
        let is_unknown: i64 = row.get(1).unwrap();
        let face_id: Option<String> = row.get(2).unwrap();
        assert!(
            employee_id.is_none(),
            "unknown event must have NULL employee_id"
        );
        assert_eq!(is_unknown, 1);
        assert_eq!(face_id.as_deref(), Some("42"));
    }

    #[tokio::test]
    async fn persist_photo_written_on_insert() {
        let tmp = fresh_events_root();
        let root = tmp.path().to_path_buf();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        let captured_at = 1_700_000_000_i64;
        let mut ev = sample_event("evt-photo-1", Some("e1"), "d1", "entry", captured_at);
        ev.photo_bytes = Some(vec![0xFF, 0xD8, 0xFF, 0xE0, b'J', b'F', b'I', b'F']);

        let outcome = persist_attendance_event(&conn, &root, ev).await.unwrap();
        let relpath = match outcome {
            PersistOutcome::Inserted { photo_path } => photo_path.expect("relpath"),
            other => panic!("expected Inserted, got {:?}", other),
        };
        // Computed date for epoch 1_700_000_000 UTC is 2023-11-14.
        assert_eq!(relpath, "2023-11-14/evt-photo-1.jpg");

        let full = root.join(&relpath);
        assert!(full.exists(), "photo file must exist at {:?}", full);
        let bytes = std::fs::read(&full).unwrap();
        assert_eq!(&bytes[..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
    }

    #[tokio::test]
    async fn persist_photo_skipped_on_dedup() {
        let tmp = fresh_events_root();
        let root = tmp.path().to_path_buf();
        let db = setup_db().await;
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1").await;
        seed_employee(&conn, "e1", "EMP001").await;

        let first_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x01];
        let second_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x02];

        // bucket 33 covers [990..=1019]; both inserts must land in the same bucket.
        let mut e1 = sample_event("evt-dup-1", Some("e1"), "d1", "entry", 1000);
        e1.photo_bytes = Some(first_bytes.clone());
        let _ = persist_attendance_event(&conn, &root, e1).await.unwrap();

        let mut e2 = sample_event("evt-dup-2", Some("e1"), "d1", "entry", 1015);
        e2.photo_bytes = Some(second_bytes.clone());
        let outcome2 = persist_attendance_event(&conn, &root, e2).await.unwrap();
        assert_eq!(outcome2, PersistOutcome::Deduplicated);

        // Walk the events root and count JPEGs — must be exactly one from the first insert.
        let mut jpeg_count = 0usize;
        for entry in walkdir(&root) {
            if entry.extension().and_then(|s| s.to_str()) == Some("jpg") {
                jpeg_count += 1;
                let bytes = std::fs::read(&entry).unwrap();
                assert_eq!(
                    bytes, first_bytes,
                    "file must contain the first event's bytes (no overwrite)"
                );
            }
        }
        assert_eq!(
            jpeg_count, 1,
            "dedup must leave exactly one JPEG on disk, found {}",
            jpeg_count
        );
    }

    fn walkdir(root: &Path) -> Vec<PathBuf> {
        // Minimal recursive walker — avoids adding `walkdir` as a dev-dep.
        fn go(p: &Path, out: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        go(&path, out);
                    } else {
                        out.push(path);
                    }
                }
            }
        }
        let mut out = Vec::new();
        go(root, &mut out);
        out
    }
}
