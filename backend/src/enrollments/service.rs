//! Enrollment service — CRUD operations against enrollments, face_enrollments,
//! enrollment_device_pushes, and device_face_mappings tables.
//!
//! Follows the Phase 1/2 service convention: pure I/O functions, no business
//! logic beyond persistence. Business logic lives in handlers and pusher.

use std::sync::Arc;

use libsql::{params, Connection};
use uuid::Uuid;

use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::devices::{crypto, models::DeviceWithPlaintext, service as devices_service};
use crate::errors::AppError;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;
use crate::storage::atomic_file::AtomicFileGuard;

use super::dispatcher::{AuthorizedAttempt, AuthorizedDispatchCommand, AuthorizedDispatchTarget};
use super::models::{
    validate_enrollment_status, EnrollmentDevicePushResponse, EnrollmentListQuery,
    EnrollmentResponse, EnrollmentSubmitResponse, RetryResponse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceOperationState {
    Prepared,
    DeviceApplied,
    Manual,
}

pub struct CheckpointAdmission {
    pub state: DeviceOperationState,
    pub fresh: bool,
}

fn checkpoint_state(value: &str) -> anyhow::Result<DeviceOperationState> {
    match value {
        "prepared" => Ok(DeviceOperationState::Prepared),
        "device_applied" => Ok(DeviceOperationState::DeviceApplied),
        "manual" => Ok(DeviceOperationState::Manual),
        _ => anyhow::bail!("invalid device operation checkpoint state"),
    }
}

pub fn enrollment_checkpoint_key(push_id: &str) -> String {
    format!("enrollment:{push_id}")
}

pub fn backfill_checkpoint_key(device_id: &str, face_id: &str) -> String {
    format!("backfill:{device_id}:{face_id}")
}

pub fn purge_checkpoint_key(mapping_id: &str) -> String {
    format!("purge:{mapping_id}")
}

pub async fn admit_device_operation(
    state: &AppState,
    operation_key: &str,
    operation: &str,
) -> Result<CheckpointAdmission, AppError> {
    let operation_key = operation_key.to_string();
    let operation = operation.to_string();
    state
        .db_write
        .background_transact("enrollments.admit-device-operation", move |tx| {
            Box::pin(async move {
                let inserted = tx
                    .statement(
                        "INSERT OR IGNORE INTO device_operation_checkpoints \
                         (operation_key, operation, state, updated_at) \
                         VALUES (?1, ?2, 'prepared', unixepoch())",
                        params![operation_key.clone(), operation],
                    )
                    .await?
                    == 1;
                let mut rows = tx
                    .query(
                        "SELECT state FROM device_operation_checkpoints WHERE operation_key=?1",
                        params![operation_key],
                    )
                    .await?;
                let row = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("checkpoint disappeared during admission"))?;
                let state: String = row.get(0)?;
                Ok(CheckpointAdmission {
                    state: checkpoint_state(&state)?,
                    fresh: inserted,
                })
            })
        })
        .await
        .map_err(AppError::from)
}

pub async fn mark_device_operation(
    state: &AppState,
    operation_key: &str,
    checkpoint: DeviceOperationState,
) -> Result<(), AppError> {
    let checkpoint = match checkpoint {
        DeviceOperationState::Prepared => "prepared",
        DeviceOperationState::DeviceApplied => "device_applied",
        DeviceOperationState::Manual => "manual",
    };
    state
        .db_write
        .background_statement(
            "enrollments.mark-device-operation",
            "UPDATE device_operation_checkpoints \
             SET state=?1, updated_at=unixepoch() WHERE operation_key=?2",
            vec![
                libsql::Value::Text(checkpoint.to_string()),
                libsql::Value::Text(operation_key.to_string()),
            ],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

pub async fn clear_device_operation(state: &AppState, operation_key: &str) -> Result<(), AppError> {
    state
        .db_write
        .background_statement(
            "enrollments.clear-device-operation",
            "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
            vec![libsql::Value::Text(operation_key.to_string())],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Atomic enrollment result plus the exact decrypted device snapshot whose
/// rows were created in the same transaction. This type never serializes or
/// derives Debug, so plaintext credentials cannot escape into API/log output.
pub struct StartedEnrollment {
    pub response: EnrollmentSubmitResponse,
}

impl std::fmt::Debug for StartedEnrollment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartedEnrollment")
            .field("response", &self.response)
            .finish()
    }
}

impl std::ops::Deref for StartedEnrollment {
    type Target = EnrollmentSubmitResponse;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

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

const PUSH_SELECT_COLS: &str = "edp.id, edp.device_id, d.name, edp.status, edp.error_message, \
     edp.started_at, edp.completed_at";

const ENROLLMENT_SELECT_COLS: &str = "enr.id, enr.employee_id, emp.name, emp.employee_code, \
     enr.status, enr.started_at, enr.completed_at, enr.version";

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

fn row_to_enrollment(row: &libsql::Row) -> Result<EnrollmentResponse, AppError> {
    Ok(EnrollmentResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        employee_name: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        employee_code: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        status: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        started_at: epoch_to_iso(row.get(5).map_err(|e| AppError::Internal(e.into()))?),
        completed_at: epoch_to_iso_opt(row.get(6).map_err(|e| AppError::Internal(e.into()))?),
        version: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        device_pushes: Vec::new(),
    })
}

async fn list_device_pushes(
    conn: &Connection,
    enrollment_id: &str,
) -> Result<Vec<EnrollmentDevicePushResponse>, AppError> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {PUSH_SELECT_COLS} \
                 FROM enrollment_device_pushes edp \
                 LEFT JOIN devices d ON d.id = edp.device_id \
                 WHERE edp.enrollment_id = ?1 \
                 ORDER BY edp.started_at ASC, edp.id ASC"
            ),
            params![enrollment_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut pushes = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        pushes.push(row_to_push(&row)?);
    }
    Ok(pushes)
}

// =============================================================================
// Read operations
// =============================================================================

/// Fetch an enrollment enriched with employee identity + all device push rows.
pub async fn get_enrollment_with_pushes(
    conn: &Connection,
    id: &str,
) -> Result<EnrollmentResponse, AppError> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {ENROLLMENT_SELECT_COLS} \
                 FROM enrollments enr \
                 JOIN employees emp ON emp.id = enr.employee_id \
                 WHERE enr.id = ?1"
            ),
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

    let mut enrollment = row_to_enrollment(&row)?;
    drop(rows);
    enrollment.device_pushes = list_device_pushes(conn, &enrollment.id).await?;
    Ok(enrollment)
}

/// List enrollment headers first, then attach all push rows for the selected page.
pub async fn list_enrollments(
    conn: &Connection,
    query: EnrollmentListQuery,
) -> Result<PaginatedResponse<EnrollmentResponse>, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();
    let where_clause = if let Some(status) = query.status {
        validate_enrollment_status(&status).map_err(|message| AppError::Validation {
            code: "VALIDATION_ERROR",
            message: message.to_string(),
        })?;
        count_values.push(libsql::Value::Text(status.clone()));
        fetch_values.push(libsql::Value::Text(status));
        "WHERE enr.status = ?1"
    } else {
        ""
    };

    let count_sql = format!("SELECT COUNT(*) FROM enrollments enr {where_clause}");
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT {ENROLLMENT_SELECT_COLS} \
         FROM enrollments enr \
         JOIN employees emp ON emp.id = enr.employee_id \
         {where_clause} \
         ORDER BY enr.started_at DESC, enr.id ASC LIMIT ?{limit_param} OFFSET ?{offset_param}",
        limit_param = fetch_values.len() + 1,
        offset_param = fetch_values.len() + 2,
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
        data.push(row_to_enrollment(&row)?);
    }
    drop(rows);

    for enrollment in &mut data {
        enrollment.device_pushes = list_device_pushes(conn, &enrollment.id).await?;
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

// =============================================================================
// Write operations
// =============================================================================

/// Persist a full enrollment session atomically and return the 202 response body.
///
/// Steps (all in one queued transaction owned by the single WAL writer):
/// 1. INSERT face_enrollments
/// 2. INSERT enrollments
/// 3. Ensure employees.face_id is set (COALESCE — stable per D-10)
/// 4. UPDATE employees.current_face_enrollment_id
/// 5. INSERT enrollment_device_pushes (one per active device)
///
/// The JPEG is durably published under `AtomicFileGuard` before admission and
/// kept by a worker-owned after-commit callback. Rollback, queue rejection, and
/// request cancellation therefore cannot leave a row/file mismatch.
pub async fn start_enrollment(
    state: &AppState,
    actor_id: &str,
    employee_id: &str,
    captured_via: &str,
    source_device_id: Option<&str>,
    face_quality_score: Option<&str>,
    normalized_bytes: &[u8],
) -> Result<StartedEnrollment, AppError> {
    let face_enrollment_id = Uuid::new_v4().to_string();
    let enrollment_id = Uuid::new_v4().to_string();
    let new_face_id = Uuid::new_v4().to_string();
    let photo_relpath = format!("{}/{}.jpg", employee_id, enrollment_id);
    let guard = AtomicFileGuard::write(
        &state.paths.enrollments_root,
        &photo_relpath,
        normalized_bytes,
    )
    .map_err(AppError::Internal)?;
    let actor_id = actor_id.to_string();
    let employee_id = employee_id.to_string();
    let captured_via = captured_via.to_string();
    let source_device_id = source_device_id.map(str::to_string);
    let face_quality_score = face_quality_score.map(str::to_string);
    let device_creds_key = state.config.device_creds_key;
    let dispatcher = state.enrollment_dispatcher.clone();
    let photo_bytes = Arc::new(normalized_bytes.to_vec());
    let dispatch_error = Arc::new(std::sync::Mutex::new(None::<String>));
    let callback_error = dispatch_error.clone();

    let started = state
        .db_write
        .transact("enrollments.start", move |tx| {
            Box::pin(async move {
                tx.statement(
                    "INSERT INTO face_enrollments \
                     (id, employee_id, captured_via, source_device_id, photo_path, face_quality_score, created_by, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())",
                    params![
                        face_enrollment_id.clone(),
                        employee_id.clone(),
                        captured_via,
                        source_device_id,
                        photo_relpath,
                        face_quality_score,
                        actor_id.clone(),
                    ],
                )
                .await?;
                tx.statement(
                    "INSERT INTO enrollments \
                     (id, employee_id, face_enrollment_id, status, started_by, started_at, version) \
                     VALUES (?1, ?2, ?3, 'in_progress', ?4, unixepoch(), 1)",
                    params![
                        enrollment_id.clone(),
                        employee_id.clone(),
                        face_enrollment_id.clone(),
                        actor_id,
                    ],
                )
                .await?;
                let employee_updated = tx.statement(
                    "UPDATE employees \
                     SET face_id=COALESCE(face_id, ?1), current_face_enrollment_id=?2 \
                     WHERE id=?3",
                    params![new_face_id, face_enrollment_id, employee_id.clone()],
                )
                .await?;
                if employee_updated != 1 {
                    anyhow::bail!("employee disappeared during enrollment");
                }

                let mut face_rows = tx
                    .query(
                        "SELECT face_id, name FROM employees WHERE id=?1",
                        params![employee_id.clone()],
                    )
                    .await?;
                let face_row = face_rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("employee disappeared during enrollment"))?;
                let face_id: String = face_row.get(0)?;
                let employee_name: String = face_row.get(1)?;
                drop(face_rows);

                let mut device_rows = tx
                    .query(
                        "SELECT id, name, ip, port, scheme, username, encrypted_password, \
                                direction, allow_insecure_tls, status, version \
                         FROM devices \
                         WHERE status='active' AND deleted_at IS NULL \
                         ORDER BY created_at, id",
                        (),
                    )
                    .await?;
                let mut devices = Vec::new();
                while let Some(row) = device_rows.next().await? {
                    let encrypted: String = row.get(6)?;
                    let password = crypto::decrypt_password(&encrypted, &device_creds_key)?;
                    let scheme: String = row.get(4)?;
                    let ip: String = row.get(2)?;
                    let port: i64 = row.get(3)?;
                    devices.push(DeviceWithPlaintext {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        base_url: format!("{scheme}://{ip}:{port}"),
                        username: row.get(5)?,
                        password,
                        direction: row.get(7)?,
                        allow_insecure_tls: row.get::<i64>(8)? != 0,
                        status: row.get(9)?,
                        version: row.get(10)?,
                    });
                }
                drop(device_rows);

                let mut device_pushes = Vec::with_capacity(devices.len());
                let mut targets = Vec::with_capacity(devices.len());
                for device in devices {
                    DeviceConnection::new(
                        &device.base_url,
                        &device.username,
                        &device.password,
                        device.allow_insecure_tls,
                    )
                    .map_err(|error| anyhow::anyhow!("invalid device connection: {error}"))?;
                    let push_id = Uuid::new_v4().to_string();
                    tx.statement(
                        "INSERT INTO enrollment_device_pushes \
                         (id, enrollment_id, device_id, status, error_message, started_at, completed_at) \
                         VALUES (?1, ?2, ?3, 'pending', NULL, NULL, NULL)",
                        params![push_id.clone(), enrollment_id.clone(), device.id.clone()],
                    )
                    .await?;
                    tx.statement(
                        "INSERT INTO device_operation_checkpoints \
                         (operation_key, operation, state, updated_at) \
                         VALUES (?1, 'enrollment_push', 'prepared', unixepoch())",
                        params![enrollment_checkpoint_key(&push_id)],
                    )
                    .await?;
                    device_pushes.push(EnrollmentDevicePushResponse {
                        id: push_id,
                        device_id: device.id.clone(),
                        device_name: device.name.clone(),
                        status: "pending".into(),
                        error_message: None,
                        started_at: None,
                        completed_at: None,
                    });
                    targets.push(AuthorizedDispatchTarget {
                        push_id: device_pushes.last().expect("push just inserted").id.clone(),
                        device,
                        attempt: AuthorizedAttempt::committed(),
                    });
                }

                let response = EnrollmentSubmitResponse {
                    enrollment_id: enrollment_id.clone(),
                    face_id: face_id.clone(),
                    device_pushes,
                };
                tx.after_commit(move || {
                    guard.keep();
                    if let Err(error) = dispatcher.enqueue(AuthorizedDispatchCommand {
                        enrollment_id,
                        face_id,
                        photo_bytes,
                        employee_id,
                        employee_name,
                        targets,
                    }) {
                        *callback_error
                            .lock()
                            .expect("dispatch receipt mutex poisoned") =
                            Some(error.to_string());
                    }
                });
                Ok(StartedEnrollment {
                    response,
                })
            })
        })
        .await
        .map_err(AppError::from)?;
    if let Some(error) = dispatch_error
        .lock()
        .expect("dispatch receipt mutex poisoned")
        .take()
    {
        return Err(AppError::Internal(anyhow::anyhow!(
            "post-commit enrollment dispatch failed: {error}"
        )));
    }
    Ok(started)
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

pub async fn update_push_status_queued(
    state: &AppState,
    push_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    state
        .db_write
        .background_statement(
            "enrollments.finish-device-push",
            "UPDATE enrollment_device_pushes \
             SET status = ?1, error_message = ?2, completed_at = unixepoch() \
             WHERE id = ?3",
            vec![
                libsql::Value::Text(status.to_string()),
                error_message
                    .map(|s| libsql::Value::Text(s.to_string()))
                    .unwrap_or(libsql::Value::Null),
                libsql::Value::Text(push_id.to_string()),
            ],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Commit the terminal push state and its device mapping as one background
/// transaction. The queue retries admission only; an accepted job is never
/// replayed, so successful device side effects are not duplicated.
pub async fn complete_push_success(
    state: &AppState,
    push_id: &str,
    device_id: &str,
    face_id: &str,
    employee_id: &str,
) -> Result<(), AppError> {
    let push_id = push_id.to_string();
    let device_id = device_id.to_string();
    let face_id = face_id.to_string();
    let employee_id = employee_id.to_string();
    let mapping_id = Uuid::new_v4().to_string();
    let checkpoint_key = enrollment_checkpoint_key(&push_id);
    state
        .db_write
        .background_transact("enrollments.complete-device-push", move |tx| {
            Box::pin(async move {
                let push_updated = tx
                    .statement(
                        "UPDATE enrollment_device_pushes \
                     SET status='success', error_message=NULL, completed_at=unixepoch() \
                     WHERE id=?1",
                        params![push_id],
                    )
                    .await?;
                if push_updated != 1 {
                    anyhow::bail!("push row not found during successful completion");
                }
                tx.statement(
                    "INSERT INTO device_face_mappings \
                     (id, device_id, face_id, employee_id, state, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch()) \
                     ON CONFLICT(device_id, face_id) DO UPDATE SET \
                       employee_id=excluded.employee_id, state='active', \
                       version=device_face_mappings.version+1, updated_at=unixepoch()",
                    params![mapping_id, device_id, face_id, employee_id],
                )
                .await?;
                tx.statement(
                    "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
                    params![checkpoint_key],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Persist a terminal push failure through the background admission policy.
pub async fn complete_push_failure(
    state: &AppState,
    push_id: &str,
    error_message: &str,
) -> Result<(), AppError> {
    let push_id = push_id.to_string();
    let error_message = error_message.to_string();
    let checkpoint_key = enrollment_checkpoint_key(&push_id);
    state
        .db_write
        .background_transact("enrollments.fail-device-push", move |tx| {
            Box::pin(async move {
                let push_updated = tx
                    .statement(
                        "UPDATE enrollment_device_pushes \
                     SET status='failed', error_message=?1, completed_at=unixepoch() \
                     WHERE id=?2",
                        params![error_message, push_id],
                    )
                    .await?;
                if push_updated != 1 {
                    anyhow::bail!("push row not found during failed completion");
                }
                tx.statement(
                    "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
                    params![checkpoint_key],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Persist a terminal/manual recovery state without deleting the durable
/// checkpoint. This is used after an ambiguous or confirmed-success device
/// side effect so a public retry can only perform DB recovery, never replay
/// the external call.
pub async fn record_push_recovery_failure(
    state: &AppState,
    push_id: &str,
    error_message: &str,
    manual: bool,
) -> Result<(), AppError> {
    let push_id = push_id.to_string();
    let error_message = error_message.to_string();
    let checkpoint_key = enrollment_checkpoint_key(&push_id);
    state
        .db_write
        .background_transact("enrollments.record-push-recovery", move |tx| {
            Box::pin(async move {
                if manual {
                    tx.statement(
                        "UPDATE device_operation_checkpoints \
                         SET state='manual', updated_at=unixepoch() WHERE operation_key=?1",
                        params![checkpoint_key],
                    )
                    .await?;
                }
                let updated = tx
                    .statement(
                        "UPDATE enrollment_device_pushes \
                         SET status='failed', error_message=?1, completed_at=unixepoch() \
                         WHERE id=?2",
                        params![error_message, push_id],
                    )
                    .await?;
                if updated != 1 {
                    anyhow::bail!("push row not found during recovery persistence");
                }
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Catch-all used by the dispatcher wrapper after a target returns or panics.
/// It terminalizes only an unfinished push and only converts `prepared` to
/// `manual`; a confirmed `device_applied` checkpoint is preserved for DB-only
/// startup recovery.
pub async fn terminalize_authorized_failure(
    state: &AppState,
    push_id: &str,
    error_message: &str,
) -> Result<(), AppError> {
    let push_id = push_id.to_string();
    let checkpoint_key = enrollment_checkpoint_key(&push_id);
    let error_message = error_message.to_string();
    state
        .db_write
        .background_transact("enrollments.terminalize-authorized-push", move |tx| {
            Box::pin(async move {
                tx.statement(
                    "UPDATE device_operation_checkpoints SET state='manual', updated_at=unixepoch() \
                     WHERE operation_key=?1 AND state='prepared'",
                    params![checkpoint_key],
                )
                .await?;
                tx.statement(
                    "UPDATE enrollment_device_pushes \
                     SET status='failed', error_message=?1, completed_at=unixepoch() \
                     WHERE id=?2 AND status IN ('pending','in_progress')",
                    params![error_message, push_id],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Terminalize every not-yet-finished device push when the committed
/// enrollment cannot be admitted to the lifecycle owner. The API still
/// returns its committed 202 snapshot; recovery is visible in persisted
/// state instead of being converted into a misleading request failure.
pub async fn fail_enrollment_dispatch(
    state: &AppState,
    enrollment_id: &str,
    error_message: &str,
) -> Result<(), AppError> {
    let enrollment_id = enrollment_id.to_string();
    let error_message = error_message.to_string();
    state
        .db_write
        .background_transact("enrollments.fail-dispatch-admission", move |tx| {
            Box::pin(async move {
                tx.statement(
                    "UPDATE enrollment_device_pushes \
                     SET status='failed', error_message=?1, completed_at=unixepoch() \
                     WHERE enrollment_id=?2 AND status IN ('pending', 'in_progress')",
                    params![error_message, enrollment_id.clone()],
                )
                .await?;
                tx.statement(
                    "UPDATE enrollments \
                     SET status='failed', completed_at=unixepoch(), version=version+1 \
                     WHERE id=?1 AND status='in_progress'",
                    params![enrollment_id],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Mark a push row as in_progress (records started_at).
pub async fn mark_push_in_progress(conn: &Connection, push_id: &str) -> Result<(), AppError> {
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

pub async fn mark_push_in_progress_queued(state: &AppState, push_id: &str) -> Result<(), AppError> {
    state
        .db_write
        .background_statement(
            "enrollments.start-device-push",
            "UPDATE enrollment_device_pushes \
             SET status = 'in_progress', started_at = unixepoch() \
             WHERE id = ?1",
            vec![libsql::Value::Text(push_id.to_string())],
        )
        .await
        .map_err(AppError::from)?;
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
            message: format!(
                "No push row for enrollment={} device={}",
                enrollment_id, device_id
            ),
        })?;
    row.get(0).map_err(|e| AppError::Internal(e.into()))
}

/// Re-set a push row to pending so the retry path can re-fire it.
pub async fn reset_push_to_pending(
    conn: &Connection,
    enrollment_id: &str,
    device_id: &str,
) -> Result<String, AppError> {
    let push_id = get_push_id(conn, enrollment_id, device_id).await?;
    let mut status_rows = conn
        .query(
            "SELECT status FROM enrollment_device_pushes WHERE id=?1",
            params![push_id.clone()],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let status: String = status_rows
        .next()
        .await
        .map_err(|error| AppError::Internal(error.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "PUSH_NOT_FOUND",
            message: "Push row disappeared before retry".into(),
        })?
        .get(0)
        .map_err(|error| AppError::Internal(error.into()))?;
    if status != "failed" {
        return Err(AppError::Conflict {
            code: "PUSH_NOT_RETRYABLE",
            message: "Only failed device pushes may be retried".into(),
        });
    }
    conn.execute(
        "UPDATE enrollment_device_pushes \
         SET status='pending', error_message=NULL, started_at=NULL, completed_at=NULL \
         WHERE id=?1",
        params![push_id.clone()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(push_id)
}

pub async fn reset_push_to_pending_queued(
    state: &AppState,
    enrollment_id: &str,
    device_id: &str,
) -> Result<String, AppError> {
    let enrollment_id = enrollment_id.to_string();
    let device_id = device_id.to_string();
    state
        .db_write
        .transact("enrollments.retry-device-push", move |tx| {
            Box::pin(async move {
                let mut rows = tx
                    .query(
                        "SELECT id, status FROM enrollment_device_pushes \
                         WHERE enrollment_id=?1 AND device_id=?2",
                        params![enrollment_id, device_id],
                    )
                    .await?;
                let row = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("push row not found for retry"))?;
                let push_id: String = row.get(0)?;
                let status: String = row.get(1)?;
                drop(rows);
                if status != "failed" {
                    return Err(anyhow::Error::new(AppError::Conflict {
                        code: "PUSH_NOT_RETRYABLE",
                        message: "Only failed device pushes may be retried".into(),
                    }));
                }
                tx.statement(
                    "UPDATE enrollment_device_pushes \
                     SET status='pending', error_message=NULL, started_at=NULL, completed_at=NULL \
                     WHERE id=?1",
                    params![push_id.clone()],
                )
                .await?;
                Ok(push_id)
            })
        })
        .await
        .map_err(AppError::from)
}

/// Prepare a failed push for retry and transfer the exact command to the
/// post-commit dispatcher. All fallible filesystem/device setup happens before
/// the transaction; the handler has no work left that cancellation can lose.
pub async fn retry_enrollment_push(
    state: &AppState,
    enrollment_id: &str,
    device_id: &str,
) -> Result<RetryResponse, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|error| AppError::Internal(error.into()))?;
    let (employee_id, face_id, employee_name) =
        get_enrollment_push_params(&conn, enrollment_id).await?;
    let photo_path = get_current_photo_path(&conn, &employee_id)
        .await?
        .ok_or_else(|| AppError::NotFound {
            code: "PHOTO_NOT_FOUND",
            message: "Employee has no current face enrollment photo".into(),
        })?;
    drop(conn);

    let photo_bytes = Arc::new(
        tokio::fs::read(state.paths.enrollments_root.join(photo_path))
            .await
            .map_err(|error| {
                AppError::Internal(anyhow::anyhow!("read photo for retry: {error}"))
            })?,
    );
    let device = devices_service::get_decrypted(
        &state
            .db
            .connect()
            .map_err(|error| AppError::Internal(error.into()))?,
        device_id,
        &state.config.device_creds_key,
    )
    .await?;
    DeviceConnection::new(
        &device.base_url,
        &device.username,
        &device.password,
        device.allow_insecure_tls,
    )
    .map_err(|error| AppError::Internal(anyhow::anyhow!("invalid device connection: {error}")))?;

    let enrollment_id_owned = enrollment_id.to_string();
    let device_id_owned = device_id.to_string();
    let dispatcher = state.enrollment_dispatcher.clone();
    let dispatch_error = Arc::new(std::sync::Mutex::new(None::<String>));
    let callback_error = Arc::clone(&dispatch_error);
    let response = state
        .db_write
        .transact("enrollments.retry-device-push", move |tx| {
            Box::pin(async move {
                let mut rows = tx
                    .query(
                        "SELECT id, status FROM enrollment_device_pushes \
                         WHERE enrollment_id=?1 AND device_id=?2",
                        params![enrollment_id_owned.clone(), device_id_owned.clone()],
                    )
                    .await?;
                let row = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("push row not found for retry"))?;
                let push_id: String = row.get(0)?;
                let status: String = row.get(1)?;
                drop(rows);
                if status != "failed" {
                    return Err(anyhow::Error::new(AppError::Conflict {
                        code: "PUSH_NOT_RETRYABLE",
                        message: "Only failed device pushes may be retried".into(),
                    }));
                }
                let checkpoint_key = enrollment_checkpoint_key(&push_id);
                let mut checkpoint_rows = tx
                    .query(
                        "SELECT state FROM device_operation_checkpoints WHERE operation_key=?1",
                        params![checkpoint_key.clone()],
                    )
                    .await?;
                if checkpoint_rows.next().await?.is_some() {
                    return Err(anyhow::Error::new(AppError::Conflict {
                        code: "PUSH_RECONCILIATION_REQUIRED",
                        message: "Device operation requires manual reconciliation before retry"
                            .into(),
                    }));
                }
                drop(checkpoint_rows);
                tx.statement(
                    "UPDATE enrollment_device_pushes \
                     SET status='pending', error_message=NULL, started_at=NULL, completed_at=NULL \
                     WHERE id=?1",
                    params![push_id.clone()],
                )
                .await?;
                tx.statement(
                    "INSERT INTO device_operation_checkpoints \
                     (operation_key, operation, state, updated_at) \
                     VALUES (?1, 'enrollment_push', 'prepared', unixepoch())",
                    params![checkpoint_key],
                )
                .await?;
                let response = RetryResponse {
                    enrollment_id: enrollment_id_owned.clone(),
                    device_id: device_id_owned,
                    status: "pending".into(),
                };
                tx.after_commit(move || {
                    if let Err(error) = dispatcher.enqueue(AuthorizedDispatchCommand {
                        enrollment_id: enrollment_id_owned,
                        face_id,
                        photo_bytes,
                        employee_id,
                        employee_name,
                        targets: vec![AuthorizedDispatchTarget {
                            push_id,
                            device,
                            attempt: AuthorizedAttempt::committed(),
                        }],
                    }) {
                        *callback_error
                            .lock()
                            .expect("dispatch receipt mutex poisoned") = Some(error.to_string());
                    }
                });
                Ok(response)
            })
        })
        .await
        .map_err(AppError::from)?;
    if let Some(error) = dispatch_error
        .lock()
        .expect("dispatch receipt mutex poisoned")
        .take()
    {
        return Err(AppError::Internal(anyhow::anyhow!(
            "post-commit enrollment retry dispatch failed: {error}"
        )));
    }
    Ok(response)
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
               COUNT(*), \
               SUM(CASE WHEN status IN ('pending','in_progress') THEN 1 ELSE 0 END) \
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
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "no push rows for enrollment {}",
                enrollment_id
            ))
        })?;

    let success: i64 = row
        .get::<Option<i64>>(0)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let failed: i64 = row
        .get::<Option<i64>>(1)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let total: i64 = row
        .get::<Option<i64>>(2)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let nonterminal: i64 = row
        .get::<Option<i64>>(3)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    if nonterminal != 0 {
        return Ok(());
    }

    let final_status = if total == 0 || success == 0 {
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

pub async fn finalize_enrollment_status_queued(
    state: &AppState,
    enrollment_id: &str,
) -> Result<(), AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut rows = conn
        .query(
            "SELECT \
               SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), \
               SUM(CASE WHEN status='failed'  THEN 1 ELSE 0 END), \
               COUNT(*), \
               SUM(CASE WHEN status IN ('pending','in_progress') THEN 1 ELSE 0 END) \
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
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "no push rows for enrollment {}",
                enrollment_id
            ))
        })?;
    let success: i64 = row
        .get::<Option<i64>>(0)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let failed: i64 = row
        .get::<Option<i64>>(1)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let total: i64 = row
        .get::<Option<i64>>(2)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    let nonterminal: i64 = row
        .get::<Option<i64>>(3)
        .map_err(|e| AppError::Internal(e.into()))?
        .unwrap_or(0);
    if nonterminal != 0 {
        return Ok(());
    }
    let final_status = if total == 0 || success == 0 {
        "failed"
    } else if failed == 0 {
        "success"
    } else {
        "partial"
    };
    state
        .db_write
        .background_statement(
            "enrollments.finish",
            "UPDATE enrollments \
             SET status = ?1, completed_at = unixepoch(), version = version + 1 \
             WHERE id = ?2",
            vec![
                libsql::Value::Text(final_status.to_string()),
                libsql::Value::Text(enrollment_id.to_string()),
            ],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Count terminal push outcomes and finalize the enrollment within the same
/// queued transaction, so no observer can see a count detached from its state.
pub async fn finalize_enrollment(state: &AppState, enrollment_id: &str) -> Result<(), AppError> {
    let enrollment_id = enrollment_id.to_string();
    state
        .db_write
        .background_transact("enrollments.finalize", move |tx| {
            Box::pin(async move {
                let mut rows = tx
                    .query(
                        "SELECT \
                           SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), \
                           SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END), COUNT(*), \
                           SUM(CASE WHEN status IN ('pending','in_progress') THEN 1 ELSE 0 END) \
                         FROM enrollment_device_pushes WHERE enrollment_id=?1",
                        params![enrollment_id.clone()],
                    )
                    .await?;
                let row = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("push aggregate returned no row"))?;
                let success = row.get::<Option<i64>>(0)?.unwrap_or(0);
                let failed = row.get::<Option<i64>>(1)?.unwrap_or(0);
                let total = row.get::<i64>(2)?;
                let nonterminal = row.get::<Option<i64>>(3)?.unwrap_or(0);
                drop(rows);
                if nonterminal != 0 {
                    return Ok(());
                }
                let final_status = if total == 0 || success == 0 {
                    "failed"
                } else if failed == 0 {
                    "success"
                } else {
                    "partial"
                };
                let enrollment_updated = tx
                    .statement(
                        "UPDATE enrollments \
                     SET status=?1, completed_at=unixepoch(), version=version+1 \
                     WHERE id=?2",
                        params![final_status, enrollment_id],
                    )
                    .await?;
                if enrollment_updated != 1 {
                    anyhow::bail!("enrollment not found during finalization");
                }
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
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

pub async fn upsert_device_face_mapping_queued(
    state: &AppState,
    device_id: &str,
    face_id: &str,
    employee_id: &str,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    state
        .db_write
        .background_statement(
            "enrollments.upsert-face-mapping",
            "INSERT OR REPLACE INTO device_face_mappings \
             (id, device_id, face_id, employee_id, state, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
            vec![
                libsql::Value::Text(id),
                libsql::Value::Text(device_id.to_string()),
                libsql::Value::Text(face_id.to_string()),
                libsql::Value::Text(employee_id.to_string()),
            ],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

pub async fn complete_backfill_mapping(
    state: &AppState,
    checkpoint_key: &str,
    device_id: &str,
    face_id: &str,
    employee_id: &str,
) -> Result<(), AppError> {
    let checkpoint_key = checkpoint_key.to_string();
    let device_id = device_id.to_string();
    let face_id = face_id.to_string();
    let employee_id = employee_id.to_string();
    let mapping_id = Uuid::new_v4().to_string();
    state
        .db_write
        .background_transact("enrollments.complete-backfill", move |tx| {
            Box::pin(async move {
                tx.statement(
                    "INSERT INTO device_face_mappings \
                     (id, device_id, face_id, employee_id, state, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch()) \
                     ON CONFLICT(device_id, face_id) DO UPDATE SET \
                       employee_id=excluded.employee_id, state='active', \
                       version=device_face_mappings.version+1, updated_at=unixepoch()",
                    params![mapping_id, device_id, face_id, employee_id],
                )
                .await?;
                tx.statement(
                    "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
                    params![checkpoint_key],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
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

    if let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
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
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
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
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
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

pub async fn mark_mapping_pending_delete_queued(
    state: &AppState,
    mapping_id: &str,
) -> Result<(), AppError> {
    state
        .db_write
        .background_statement(
            "enrollments.mark-face-mapping-delete",
            "UPDATE device_face_mappings \
             SET state = 'pending_delete', version = version + 1, updated_at = unixepoch() \
             WHERE id = ?1",
            vec![libsql::Value::Text(mapping_id.to_string())],
        )
        .await
        .map_err(AppError::from)?;
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

pub async fn delete_mapping_queued(state: &AppState, mapping_id: &str) -> Result<(), AppError> {
    state
        .db_write
        .background_statement(
            "enrollments.delete-face-mapping",
            "DELETE FROM device_face_mappings WHERE id = ?1",
            vec![libsql::Value::Text(mapping_id.to_string())],
        )
        .await
        .map_err(AppError::from)?;
    Ok(())
}

pub async fn complete_purge_mapping(
    state: &AppState,
    checkpoint_key: &str,
    mapping_id: &str,
) -> Result<(), AppError> {
    let checkpoint_key = checkpoint_key.to_string();
    let mapping_id = mapping_id.to_string();
    state
        .db_write
        .background_transact("enrollments.complete-purge", move |tx| {
            Box::pin(async move {
                tx.statement(
                    "DELETE FROM device_face_mappings WHERE id=?1",
                    params![mapping_id],
                )
                .await?;
                tx.statement(
                    "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
                    params![checkpoint_key],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)
}

/// Fetch the employee's current status. Used by PurgeWorker Pitfall-10 guard.
pub async fn get_employee_status(conn: &Connection, employee_id: &str) -> Result<String, AppError> {
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
    let face_id = face_id.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("employee {} has no face_id", employee_id))
    })?;
    Ok((employee_id, face_id, name))
}
