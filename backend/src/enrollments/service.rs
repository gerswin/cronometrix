//! Enrollment service — CRUD operations against enrollments, face_enrollments,
//! enrollment_device_pushes, and device_face_mappings tables.
//!
//! Follows the Phase 1/2 service convention: pure I/O functions, no business
//! logic beyond persistence. Business logic lives in handlers and pusher.

use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt};
use crate::devices::service as devices_service;
use crate::errors::AppError;
use crate::events::service::write_photo_atomic;
use crate::state::AppState;

use super::models::{
    EnrollmentDevicePushResponse, EnrollmentResponse, EnrollmentSubmitResponse,
};

// =============================================================================
// Filesystem roots
// =============================================================================
//
// Phase 8 (D-18/D-19): the canonical enrollment photo root and the kiosk
// capture tmp root live on `state.paths` (Paths::from_env in production,
// Paths::for_test(tempdir) in tests). The free-function helpers that used to
// read env vars at use-site (`enrollments_root()` / `captures_tmp_root()`)
// were removed because they made tests cwd-dependent and parallel-unsafe.

// =============================================================================
// Row mappers
// =============================================================================

const PUSH_SELECT_COLS: &str =
    "edp.id, edp.device_id, d.name, edp.status, edp.error_message, \
     edp.started_at, edp.completed_at";

fn row_to_push(row: &libsql::Row) -> Result<EnrollmentDevicePushResponse, AppError> {
    Ok(EnrollmentDevicePushResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        device_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        device_name: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        error_message: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        started_at: epoch_to_iso_opt(row.get(5).map_err(|e| AppError::Internal(e.into()))?),
        completed_at: epoch_to_iso_opt(row.get(6).map_err(|e| AppError::Internal(e.into()))?),
    })
}

// =============================================================================
// Read operations
// =============================================================================

/// Fetch an enrollment + all its device push rows. Single LEFT JOIN — O(n devices).
pub async fn get_enrollment_with_pushes(
    conn: &Connection,
    id: &str,
) -> Result<EnrollmentResponse, AppError> {
    // Enrollment header
    let mut rows = conn
        .query(
            "SELECT id, employee_id, status, started_at, completed_at, version \
             FROM enrollments WHERE id = ?1",
            params![id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "ENROLLMENT_NOT_FOUND",
            message: format!("Enrollment '{}' not found", id),
        })?;

    let enr_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let employee_id: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let status: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let started_at: i64 = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
    let completed_at: Option<i64> = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
    let version: i64 = row.get(5).map_err(|e| AppError::Internal(e.into()))?;

    // Device push rows
    let mut push_rows = conn
        .query(
            &format!(
                "SELECT {PUSH_SELECT_COLS} \
                 FROM enrollment_device_pushes edp \
                 LEFT JOIN devices d ON d.id = edp.device_id \
                 WHERE edp.enrollment_id = ?1 \
                 ORDER BY edp.started_at ASC"
            ),
            params![enr_id.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut device_pushes = Vec::new();
    while let Some(push_row) = push_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        device_pushes.push(row_to_push(&push_row)?);
    }

    Ok(EnrollmentResponse {
        id: enr_id,
        employee_id,
        status,
        started_at: epoch_to_iso(started_at),
        completed_at: epoch_to_iso_opt(completed_at),
        version,
        device_pushes,
    })
}

// =============================================================================
// Write operations
// =============================================================================

/// Persist a full enrollment session atomically and return the 202 response body.
///
/// Steps (all in the same connection — single WAL writer):
/// 1. INSERT face_enrollments
/// 2. INSERT enrollments
/// 3. Ensure employees.face_id is set (COALESCE — stable per D-10)
/// 4. UPDATE employees.current_face_enrollment_id
/// 5. INSERT enrollment_device_pushes (one per active device)
/// 6. Write JPEG to disk via write_photo_atomic
///
/// Note: `spawn_enrollment_pushes` is called by the handler AFTER this returns 202.
pub async fn start_enrollment(
    state: &AppState,
    actor_id: &str,
    employee_id: &str,
    captured_via: &str,
    source_device_id: Option<&str>,
    face_quality_score: Option<&str>,
    normalized_bytes: &[u8],
) -> Result<EnrollmentSubmitResponse, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;

    // Fetch all active devices once for fan-out planning.
    let devices = devices_service::list_active(&conn, &state.config.device_creds_key).await?;

    // Generate IDs before insert so we can compose the photo path.
    let face_enrollment_id = Uuid::new_v4().to_string();
    let enrollment_id = Uuid::new_v4().to_string();
    let new_face_id = Uuid::new_v4().to_string();

    // Compose the photo path: {employee_id}/{enrollment_id}.jpg
    let photo_relpath = format!("{}/{}.jpg", employee_id, enrollment_id);

    // 1. INSERT face_enrollments
    conn.execute(
        "INSERT INTO face_enrollments \
         (id, employee_id, captured_via, source_device_id, photo_path, face_quality_score, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())",
        params![
            face_enrollment_id.clone(),
            employee_id.to_string(),
            captured_via.to_string(),
            source_device_id.map(|s| s.to_string()),
            photo_relpath.clone(),
            face_quality_score.map(|s| s.to_string()),
            actor_id.to_string(),
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // 2. INSERT enrollments
    conn.execute(
        "INSERT INTO enrollments \
         (id, employee_id, face_enrollment_id, status, started_by, started_at, version) \
         VALUES (?1, ?2, ?3, 'in_progress', ?4, unixepoch(), 1)",
        params![
            enrollment_id.clone(),
            employee_id.to_string(),
            face_enrollment_id.clone(),
            actor_id.to_string(),
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // 3. Set employees.face_id only if not yet assigned (D-10: stable per employee).
    conn.execute(
        "UPDATE employees SET face_id = COALESCE(face_id, ?1) WHERE id = ?2",
        params![new_face_id.clone(), employee_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // 4. Update employees.current_face_enrollment_id to the new enrollment.
    conn.execute(
        "UPDATE employees SET current_face_enrollment_id = ?1 WHERE id = ?2",
        params![face_enrollment_id.clone(), employee_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    // Retrieve the actual face_id (may differ from new_face_id if COALESCE preserved existing).
    let mut fid_rows = conn
        .query(
            "SELECT face_id FROM employees WHERE id = ?1",
            params![employee_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let fid_row = fid_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "EMPLOYEE_NOT_FOUND",
            message: format!("Employee '{}' not found", employee_id),
        })?;
    let face_id: String = fid_row.get(0).map_err(|e| AppError::Internal(e.into()))?;

    // 5. INSERT enrollment_device_pushes (one per active device, status=pending).
    let mut push_responses = Vec::new();
    for device in &devices {
        let push_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR REPLACE INTO enrollment_device_pushes \
             (id, enrollment_id, device_id, status, error_message, started_at, completed_at) \
             VALUES (?1, ?2, ?3, 'pending', NULL, NULL, NULL)",
            params![
                push_id.clone(),
                enrollment_id.clone(),
                device.id.clone(),
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

        push_responses.push(EnrollmentDevicePushResponse {
            id: push_id,
            device_id: device.id.clone(),
            device_name: device.name.clone(),
            status: "pending".to_string(),
            error_message: None,
            started_at: None,
            completed_at: None,
        });
    }

    // 6. Write JPEG to disk atomically (write_photo_atomic handles create_dir_all).
    write_photo_atomic(&state.paths.enrollments_root, &photo_relpath, normalized_bytes)
        .map_err(AppError::Internal)?;

    Ok(EnrollmentSubmitResponse {
        enrollment_id,
        face_id,
        device_pushes: push_responses,
    })
}

/// Update a single push row's status and error_message.
pub async fn update_push_status(
    conn: &Connection,
    push_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE enrollment_device_pushes \
         SET status = ?1, error_message = ?2, completed_at = unixepoch() \
         WHERE id = ?3",
        params![
            status.to_string(),
            error_message.map(|s| s.to_string()),
            push_id.to_string(),
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Mark a push row as in_progress (records started_at).
pub async fn mark_push_in_progress(
    conn: &Connection,
    push_id: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE enrollment_device_pushes \
         SET status = 'in_progress', started_at = unixepoch() \
         WHERE id = ?1",
        params![push_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Look up the push row id for a given (enrollment_id, device_id) pair.
pub async fn get_push_id(
    conn: &Connection,
    enrollment_id: &str,
    device_id: &str,
) -> Result<String, AppError> {
    let mut rows = conn
        .query(
            "SELECT id FROM enrollment_device_pushes \
             WHERE enrollment_id = ?1 AND device_id = ?2",
            params![enrollment_id.to_string(), device_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "PUSH_NOT_FOUND",
            message: format!("No push row for enrollment={} device={}", enrollment_id, device_id),
        })?;
    row.get(0).map_err(|e| AppError::Internal(e.into()))
}

/// Re-set a push row to pending so the retry path can re-fire it.
pub async fn reset_push_to_pending(
    conn: &Connection,
    enrollment_id: &str,
    device_id: &str,
) -> Result<String, AppError> {
    // Upsert: INSERT OR REPLACE preserves the UNIQUE(enrollment_id, device_id) constraint.
    let push_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO enrollment_device_pushes \
         (id, enrollment_id, device_id, status, error_message, started_at, completed_at) \
         VALUES (?1, ?2, ?3, 'pending', NULL, NULL, NULL)",
        params![push_id.clone(), enrollment_id.to_string(), device_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(push_id)
}

/// Finalise enrollment status after all push tasks settle.
///
/// Counts success/failed push rows; sets enrollments.status and completed_at.
/// Called by pusher::finalize_enrollment_status after the JoinSet drains.
pub async fn finalize_enrollment_status(
    conn: &Connection,
    enrollment_id: &str,
) -> Result<(), AppError> {
    let mut rows = conn
        .query(
            "SELECT \
               SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), \
               SUM(CASE WHEN status='failed'  THEN 1 ELSE 0 END), \
               COUNT(*) \
             FROM enrollment_device_pushes \
             WHERE enrollment_id = ?1",
            params![enrollment_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!(
            "no push rows for enrollment {}", enrollment_id
        )))?;

    let success: i64 = row.get::<Option<i64>>(0).map_err(|e| AppError::Internal(e.into()))?.unwrap_or(0);
    let failed: i64  = row.get::<Option<i64>>(1).map_err(|e| AppError::Internal(e.into()))?.unwrap_or(0);
    let total: i64   = row.get::<Option<i64>>(2).map_err(|e| AppError::Internal(e.into()))?.unwrap_or(0);

    let final_status = if total == 0 {
        "failed"
    } else if success == 0 {
        "failed"
    } else if failed == 0 {
        "success"
    } else {
        "partial"
    };

    conn.execute(
        "UPDATE enrollments \
         SET status = ?1, completed_at = unixepoch(), version = version + 1 \
         WHERE id = ?2",
        params![final_status.to_string(), enrollment_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}

/// Upsert a device_face_mappings row (INSERT OR REPLACE) on push success (D-13).
pub async fn upsert_device_face_mapping(
    conn: &Connection,
    device_id: &str,
    face_id: &str,
    employee_id: &str,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO device_face_mappings \
         (id, device_id, face_id, employee_id, state, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        params![
            id,
            device_id.to_string(),
            face_id.to_string(),
            employee_id.to_string(),
        ],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Get the face_enrollment photo path for a given employee's current enrollment.
/// Returns None if the employee has no face_id or no current_face_enrollment_id.
pub async fn get_current_photo_path(
    conn: &Connection,
    employee_id: &str,
) -> Result<Option<String>, AppError> {
    let mut rows = conn
        .query(
            "SELECT fe.photo_path \
             FROM employees e \
             LEFT JOIN face_enrollments fe ON fe.id = e.current_face_enrollment_id \
             WHERE e.id = ?1 AND e.face_id IS NOT NULL AND e.current_face_enrollment_id IS NOT NULL",
            params![employee_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        let path: Option<String> = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        Ok(path)
    } else {
        Ok(None)
    }
}

/// List all active employees who have a face_id and current_face_enrollment_id set.
/// Used by the backfill worker (D-16).
pub async fn list_employees_with_face(
    conn: &Connection,
) -> Result<Vec<(String, String, String)>, AppError> {
    // Returns (employee_id, face_id, current_face_enrollment_id) triples.
    let mut rows = conn
        .query(
            "SELECT id, face_id, current_face_enrollment_id \
             FROM employees \
             WHERE face_id IS NOT NULL \
               AND current_face_enrollment_id IS NOT NULL \
               AND status = 'active'",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        let emp_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let face_id: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let cfe_id: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
        out.push((emp_id, face_id, cfe_id));
    }
    Ok(out)
}

/// List all device_face_mappings for an employee.
/// Used by the purge worker (D-15).
pub async fn list_mappings_for_employee(
    conn: &Connection,
    employee_id: &str,
) -> Result<Vec<(String, String, String)>, AppError> {
    // Returns (mapping_id, device_id, face_id) triples.
    let mut rows = conn
        .query(
            "SELECT id, device_id, face_id \
             FROM device_face_mappings \
             WHERE employee_id = ?1 AND state IN ('active','pending_delete')",
            params![employee_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        let mapping_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let device_id: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let face_id: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
        out.push((mapping_id, device_id, face_id));
    }
    Ok(out)
}

/// Mark a device_face_mapping row as pending_delete (purge failed — will retry).
pub async fn mark_mapping_pending_delete(
    conn: &Connection,
    mapping_id: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE device_face_mappings \
         SET state = 'pending_delete', version = version + 1, updated_at = unixepoch() \
         WHERE id = ?1",
        params![mapping_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Delete a device_face_mapping row after successful device purge.
pub async fn delete_mapping(conn: &Connection, mapping_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM device_face_mappings WHERE id = ?1",
        params![mapping_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Fetch the employee's current status. Used by PurgeWorker Pitfall-10 guard.
pub async fn get_employee_status(
    conn: &Connection,
    employee_id: &str,
) -> Result<String, AppError> {
    let mut rows = conn
        .query(
            "SELECT status FROM employees WHERE id = ?1",
            params![employee_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "EMPLOYEE_NOT_FOUND",
            message: format!("Employee '{}' not found", employee_id),
        })?;
    row.get(0).map_err(|e| AppError::Internal(e.into()))
}

/// Retrieve the enrollment face_id and full_name for a given enrollment_id.
/// Used by pusher to reconstruct ISAPI push parameters from state.
pub async fn get_enrollment_push_params(
    conn: &Connection,
    enrollment_id: &str,
) -> Result<(String, String, String), AppError> {
    // Returns (employee_id, face_id, employee_name).
    let mut rows = conn
        .query(
            "SELECT e.id, e.face_id, e.name \
             FROM enrollments enr \
             JOIN employees e ON e.id = enr.employee_id \
             WHERE enr.id = ?1",
            params![enrollment_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let row = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "ENROLLMENT_NOT_FOUND",
            message: format!("Enrollment '{}' not found", enrollment_id),
        })?;
    let employee_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
    let face_id: Option<String> = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let name: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
    let face_id = face_id.ok_or_else(|| AppError::Internal(anyhow::anyhow!(
        "employee {} has no face_id", employee_id
    )))?;
    Ok((employee_id, face_id, name))
}
