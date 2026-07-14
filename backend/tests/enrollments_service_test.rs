//! Coverage gap-fill for `backend/src/enrollments/service.rs` (08-04B Task 1).
//!
//! Baseline 23.17% line. Target ≥70%.
//!
//! The service exposes pure DB operations (no HTTP). We exercise them directly
//! over a fresh per-test libSQL database via `common::test_state_with_tmpdir`.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::db::write_queue::{run_write_worker, DbWriteQueue, DbWriteQueueConfig};
use cronometrix_api::enrollments::models::EnrollmentListQuery;
use cronometrix_api::enrollments::service;
use cronometrix_api::errors::AppError;
use libsql::params;
use tokio::sync::Notify;
use uuid::Uuid;

use common::{test_device_creds_key, TEST_JWT_SECRET};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
    })
}

/// Seed a department + employee returning (dept_id, emp_id, user_id).
async fn seed_dept_emp_user(db: &libsql::Database) -> (String, String, String) {
    let conn = db.connect().expect("connect");
    let user_id = common::create_test_admin(db).await;
    let dept_id = Uuid::new_v4().to_string();
    let dept_name = format!("Dept-{}", &dept_id[..8]);
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), dept_name],
    )
    .await
    .expect("seed dept");

    let emp_id = Uuid::new_v4().to_string();
    let emp_code = format!("E-{}", &emp_id[..8]);
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Employee', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), emp_code, dept_id.clone()],
    )
    .await
    .expect("seed emp");

    (dept_id, emp_id, user_id)
}

/// Seed a device with encrypted credentials. Returns the device id.
async fn seed_device(db: &libsql::Database, key: &[u8; 32]) -> String {
    let conn = db.connect().expect("connect");
    use cronometrix_api::devices::crypto;
    let enc = crypto::encrypt_password("secret", key).unwrap();
    let id = Uuid::new_v4().to_string();
    // Hash all bytes so two seeded devices in one test do not collide on the
    // partial UNIQUE(ip, port) index.
    let hash: u32 = id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, b| acc.wrapping_mul(131).wrapping_add(*b as u32));
    let port = 30000 + (hash % 30000) as i64;
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, \
         created_at, updated_at) \
         VALUES (?1, ?2, '127.0.0.1', ?3, 'http', 'admin', ?4, 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.clone(), format!("dev-{}", &id[..8]), port, enc],
    )
    .await
    .expect("seed device");
    id
}

// ---------------------------------------------------------------------------
// start_enrollment
// ---------------------------------------------------------------------------

const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

#[tokio::test]
async fn start_enrollment_writes_rows_and_photo_no_devices() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .expect("start_enrollment");

    assert!(!resp.enrollment_id.is_empty());
    assert!(!resp.face_id.is_empty());
    // No active devices seeded → no push rows emitted.
    assert!(resp.device_pushes.is_empty());

    // Assert photo file was written under enrollments_root.
    let photo_path = state.paths.enrollments_root.join(&emp_id);
    assert!(photo_path.exists(), "employee enrollment dir should exist");

    // Assert the enrollments row landed.
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT employee_id, status FROM enrollments WHERE id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("row");
    let emp: String = row.get(0).unwrap();
    let status: String = row.get(1).unwrap();
    assert_eq!(emp, emp_id);
    assert_eq!(status, "in_progress");

    // Employee must have face_id and current_face_enrollment_id set.
    let mut rows = conn
        .query(
            "SELECT face_id, current_face_enrollment_id FROM employees WHERE id = ?1",
            params![emp_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("emp row");
    let face_id: Option<String> = row.get(0).unwrap();
    let cfe: Option<String> = row.get(1).unwrap();
    assert!(face_id.is_some());
    assert!(cfe.is_some());
}

#[tokio::test]
async fn start_enrollment_emits_push_row_per_device() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let d1 = seed_device(&state.db, &config.device_creds_key).await;
    let _d2 = seed_device(&state.db, &config.device_creds_key).await;

    let resp = service::start_enrollment(
        &state,
        &user_id,
        &emp_id,
        "device",
        Some(&d1),
        Some("0.95"),
        MINI_JPEG,
    )
    .await
    .expect("start_enrollment with devices");

    assert_eq!(resp.device_pushes.len(), 2);
    for p in &resp.device_pushes {
        assert_eq!(p.status, "pending");
    }
}

#[tokio::test]
async fn start_enrollment_preserves_face_id_on_re_enrollment() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    let r1 = service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
        .await
        .unwrap();
    let r2 = service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
        .await
        .unwrap();

    // D-10: face_id must be stable across re-enrollment.
    assert_eq!(r1.face_id, r2.face_id);
    assert_ne!(r1.enrollment_id, r2.enrollment_id);
}

#[tokio::test]
async fn start_enrollment_rolls_back_every_row_and_photo_when_push_insert_fails() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    seed_device(&state.db, &config.device_creds_key).await;

    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_enrollment_push_insert \
         BEFORE INSERT ON enrollment_device_pushes \
         BEGIN SELECT RAISE(ABORT, 'forced push insert failure'); END;",
    )
    .await
    .unwrap();

    let error =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .expect_err("trigger must abort the queued enrollment transaction");
    assert!(error.to_string().contains("forced push insert failure"));

    for table in [
        "face_enrollments",
        "enrollments",
        "enrollment_device_pushes",
    ] {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = conn
            .query(&sql, ())
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap()
            .get(0)
            .unwrap();
        assert_eq!(count, 0, "{table} must roll back");
    }
    let employee: (Option<String>, Option<String>) = {
        let row = conn
            .query(
                "SELECT face_id, current_face_enrollment_id FROM employees WHERE id=?1",
                params![emp_id.clone()],
            )
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap();
        (row.get(0).unwrap(), row.get(1).unwrap())
    };
    assert_eq!(employee, (None, None));
    let employee_photo_dir = state.paths.enrollments_root.join(&emp_id);
    if employee_photo_dir.exists() {
        assert_eq!(
            std::fs::read_dir(employee_photo_dir).unwrap().count(),
            0,
            "AtomicFileGuard must compensate the JPEG after rollback"
        );
    }
}

#[tokio::test]
async fn accepted_enrollment_survives_request_cancellation_with_photo_owned() {
    let db = common::test_db().await;
    let config = make_config();
    let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    let (queue, receiver) = DbWriteQueue::channel(DbWriteQueueConfig {
        capacity: 4,
        ..Default::default()
    });
    state.db_write = queue.clone();
    let worker = tokio::spawn(run_write_worker(state.db.clone(), receiver));
    let blocker_started = Arc::new(Notify::new());
    let release_blocker = Arc::new(Notify::new());
    let blocker = tokio::spawn({
        let queue = queue.clone();
        let blocker_started = blocker_started.clone();
        let release_blocker = release_blocker.clone();
        async move {
            queue
                .job("test.block-writer", move |_conn| {
                    Box::pin(async move {
                        blocker_started.notify_one();
                        release_blocker.notified().await;
                        Ok(())
                    })
                })
                .await
        }
    });
    blocker_started.notified().await;

    let request = tokio::spawn({
        let state = state.clone();
        let emp_id = emp_id.clone();
        async move {
            service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
                .await
        }
    });
    while queue.stats().accepted < 2 {
        tokio::task::yield_now().await;
    }
    request.abort();
    release_blocker.notify_one();
    blocker.await.unwrap().unwrap();
    queue.flush().await.unwrap();

    let conn = state.db.connect().unwrap();
    let row = conn
        .query(
            "SELECT fe.photo_path FROM enrollments e \
             JOIN face_enrollments fe ON fe.id=e.face_enrollment_id \
             WHERE e.employee_id=?1",
            params![emp_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .expect("accepted enrollment committed after caller cancellation");
    let photo_path: String = row.get(0).unwrap();
    assert!(state.paths.enrollments_root.join(photo_path).exists());

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

// ---------------------------------------------------------------------------
// get_enrollment_with_pushes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_enrollment_with_pushes_returns_full_response() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let d1 = seed_device(&state.db, &config.device_creds_key).await;
    let _d2 = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();

    let conn = state.db.connect().unwrap();
    conn.execute(
        "UPDATE enrollment_device_pushes \
         SET status = 'failed', error_message = 'camera rejected face', \
             started_at = 1700000000, completed_at = 1700000030 \
         WHERE enrollment_id = ?1 AND device_id = ?2",
        params![resp.enrollment_id.clone(), d1.clone()],
    )
    .await
    .unwrap();
    let got = service::get_enrollment_with_pushes(&conn, &resp.enrollment_id)
        .await
        .unwrap();
    assert_eq!(got.id, resp.enrollment_id);
    assert_eq!(got.employee_id, emp_id);
    assert_eq!(got.employee_name, "Test Employee");
    assert_eq!(got.employee_code, format!("E-{}", &got.employee_id[..8]));
    assert_eq!(got.status, "in_progress");
    assert_eq!(got.device_pushes.len(), 2);
    assert!(got.completed_at.is_none());
    assert_eq!(got.version, 1);

    let failed = got
        .device_pushes
        .iter()
        .find(|push| push.device_id == d1)
        .expect("failed push row present");
    assert!(failed.device_name.starts_with("dev-"));
    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_message.as_deref(),
        Some("camera rejected face")
    );
    assert_eq!(
        failed.started_at.as_deref(),
        Some("2023-11-14T22:13:20+00:00")
    );
    assert_eq!(
        failed.completed_at.as_deref(),
        Some("2023-11-14T22:13:50+00:00")
    );
}

#[tokio::test]
async fn get_enrollment_with_pushes_404_when_missing() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    let conn = state.db.connect().unwrap();
    let err = service::get_enrollment_with_pushes(&conn, "no-such-id")
        .await
        .unwrap_err();
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, "ENROLLMENT_NOT_FOUND"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// list_enrollments
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_enrollments_filters_orders_and_paginates_before_loading_pushes() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let d1 = seed_device(&state.db, &config.device_creds_key).await;
    let d2 = seed_device(&state.db, &config.device_creds_key).await;

    let first =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let second =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let terminal =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();

    let conn = state.db.connect().unwrap();
    conn.execute(
        "UPDATE enrollments SET started_at = 1000 WHERE id IN (?1, ?2)",
        params![first.enrollment_id.clone(), second.enrollment_id.clone()],
    )
    .await
    .unwrap();
    conn.execute(
        "UPDATE enrollments \
         SET status = 'failed', started_at = 2000, completed_at = 2010, version = 2 \
         WHERE id = ?1",
        params![terminal.enrollment_id.clone()],
    )
    .await
    .unwrap();

    let mut expected_in_progress = vec![first.enrollment_id.clone(), second.enrollment_id.clone()];
    expected_in_progress.sort();

    let first_page = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: Some("in_progress".into()),
            limit: Some(1),
            offset: Some(0),
        },
    )
    .await
    .unwrap();
    assert_eq!(first_page.total, 2, "push joins must not inflate total");
    assert_eq!(first_page.limit, 1);
    assert_eq!(first_page.offset, 0);
    assert_eq!(first_page.data.len(), 1);
    assert_eq!(first_page.data[0].id, expected_in_progress[0]);
    assert_eq!(first_page.data[0].employee_name, "Test Employee");
    assert_eq!(
        first_page.data[0].employee_code,
        format!("E-{}", &emp_id[..8])
    );
    assert_eq!(first_page.data[0].device_pushes.len(), 2);

    let push_device_ids: std::collections::BTreeSet<_> = first_page.data[0]
        .device_pushes
        .iter()
        .map(|push| push.device_id.as_str())
        .collect();
    assert_eq!(
        push_device_ids,
        [d1.as_str(), d2.as_str()].into_iter().collect()
    );
    assert!(first_page.data[0]
        .device_pushes
        .iter()
        .all(|push| !push.device_name.is_empty()));
    let push_ids: Vec<_> = first_page.data[0]
        .device_pushes
        .iter()
        .map(|push| push.id.clone())
        .collect();
    let mut sorted_push_ids = push_ids.clone();
    sorted_push_ids.sort();
    assert_eq!(push_ids, sorted_push_ids, "push tie-breaker must be stable");

    let second_page = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: Some("in_progress".into()),
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(second_page.total, 2);
    assert_eq!(second_page.data.len(), 1);
    assert_eq!(second_page.data[0].id, expected_in_progress[1]);
    assert_eq!(second_page.data[0].device_pushes.len(), 2);

    let all_states = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: None,
            limit: Some(100),
            offset: Some(0),
        },
    )
    .await
    .unwrap();
    assert_eq!(all_states.total, 3);
    assert_eq!(all_states.data[0].id, terminal.enrollment_id);
    assert_eq!(all_states.data[0].status, "failed");
    assert_eq!(all_states.data[0].version, 2);
    assert_eq!(
        all_states.data[1..]
            .iter()
            .map(|enrollment| enrollment.id.clone())
            .collect::<Vec<_>>(),
        expected_in_progress
    );
}

#[tokio::test]
async fn list_enrollments_normalizes_pagination_and_rejects_invalid_status() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    let default_page = service::list_enrollments(&conn, EnrollmentListQuery::default())
        .await
        .unwrap();
    assert_eq!(default_page.limit, 20);
    assert_eq!(default_page.offset, 0);

    let max_page = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: None,
            limit: Some(101),
            offset: Some(-5),
        },
    )
    .await
    .unwrap();
    assert_eq!(max_page.limit, 100);
    assert_eq!(max_page.offset, 0);

    let min_page = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: None,
            limit: Some(0),
            offset: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(min_page.limit, 1);

    let err = service::list_enrollments(
        &conn,
        EnrollmentListQuery {
            status: Some("pending".into()),
            limit: None,
            offset: None,
        },
    )
    .await
    .unwrap_err();
    match err {
        AppError::Validation { message, .. } => {
            assert!(message.contains("enrollment status"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// get_push_id + reset_push_to_pending + mark_push_in_progress + update_push_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn push_status_lifecycle() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();

    // get_push_id
    let push_id = service::get_push_id(&conn, &resp.enrollment_id, &device_id)
        .await
        .unwrap();
    assert!(!push_id.is_empty());

    // mark_push_in_progress
    service::mark_push_in_progress(&conn, &push_id)
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status, started_at FROM enrollment_device_pushes WHERE id = ?1",
            params![push_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let st: String = row.get(0).unwrap();
    let started: Option<i64> = row.get(1).unwrap();
    assert_eq!(st, "in_progress");
    assert!(started.is_some());

    // update_push_status — success
    service::update_push_status(&conn, &push_id, "success", None)
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status, error_message, completed_at FROM enrollment_device_pushes WHERE id = ?1",
            params![push_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let st: String = row.get(0).unwrap();
    let err: Option<String> = row.get(1).unwrap();
    let completed: Option<i64> = row.get(2).unwrap();
    assert_eq!(st, "success");
    assert!(err.is_none());
    assert!(completed.is_some());

    // update_push_status — failed with error_message
    service::update_push_status(&conn, &push_id, "failed", Some("upstream timeout"))
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status, error_message FROM enrollment_device_pushes WHERE id = ?1",
            params![push_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let st: String = row.get(0).unwrap();
    let err: Option<String> = row.get(1).unwrap();
    assert_eq!(st, "failed");
    assert_eq!(err.as_deref(), Some("upstream timeout"));
}

#[tokio::test]
async fn complete_push_success_rolls_back_status_when_mapping_insert_fails() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device(&state.db, &config.device_creds_key).await;
    let enrollment =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let push_id = enrollment.device_pushes[0].id.clone();
    service::mark_push_in_progress_queued(&state, &push_id)
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_device_mapping_insert \
         BEFORE INSERT ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced mapping failure'); END;",
    )
    .await
    .unwrap();

    service::complete_push_success(&state, &push_id, &device_id, &enrollment.face_id, &emp_id)
        .await
        .expect_err("mapping trigger must roll back the push success update");

    let status: String = conn
        .query(
            "SELECT status FROM enrollment_device_pushes WHERE id=?1",
            params![push_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(status, "in_progress");
    let mappings: i64 = conn
        .query("SELECT COUNT(*) FROM device_face_mappings", ())
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(mappings, 0);
}

#[tokio::test]
async fn reset_push_to_pending_idempotent() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();

    // First mark failed.
    let push_id_orig = service::get_push_id(&conn, &resp.enrollment_id, &device_id)
        .await
        .unwrap();
    service::update_push_status(&conn, &push_id_orig, "failed", Some("err"))
        .await
        .unwrap();

    // Reset.
    let new_push_id = service::reset_push_to_pending(&conn, &resp.enrollment_id, &device_id)
        .await
        .unwrap();
    assert!(!new_push_id.is_empty());

    let mut rows = conn
        .query(
            "SELECT status, error_message FROM enrollment_device_pushes \
             WHERE enrollment_id = ?1 AND device_id = ?2",
            params![resp.enrollment_id.clone(), device_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let st: String = row.get(0).unwrap();
    let err: Option<String> = row.get(1).unwrap();
    assert_eq!(st, "pending");
    assert!(err.is_none());
}

#[tokio::test]
async fn get_push_id_404_when_missing() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let conn = state.db.connect().unwrap();

    let err = service::get_push_id(&conn, "no-enr", "no-dev")
        .await
        .unwrap_err();
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, "PUSH_NOT_FOUND"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// finalize_enrollment_status — every branch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn finalize_status_all_success() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let _d1 = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();
    // Mark all pushes success.
    conn.execute(
        "UPDATE enrollment_device_pushes SET status='success' WHERE enrollment_id = ?1",
        params![resp.enrollment_id.clone()],
    )
    .await
    .unwrap();
    service::finalize_enrollment_status(&conn, &resp.enrollment_id)
        .await
        .unwrap();

    let mut rows = conn
        .query(
            "SELECT status, completed_at FROM enrollments WHERE id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let st: String = row.get(0).unwrap();
    let completed: Option<i64> = row.get(1).unwrap();
    assert_eq!(st, "success");
    assert!(completed.is_some());
}

#[tokio::test]
async fn finalize_status_partial_when_some_failed() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let _d1 = seed_device(&state.db, &config.device_creds_key).await;
    let _d2 = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT id FROM enrollment_device_pushes WHERE enrollment_id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let mut ids = Vec::new();
    while let Some(r) = rows.next().await.unwrap() {
        ids.push(r.get::<String>(0).unwrap());
    }
    drop(rows);
    assert_eq!(ids.len(), 2);
    conn.execute(
        "UPDATE enrollment_device_pushes SET status='success' WHERE id = ?1",
        params![ids[0].clone()],
    )
    .await
    .unwrap();
    conn.execute(
        "UPDATE enrollment_device_pushes SET status='failed' WHERE id = ?1",
        params![ids[1].clone()],
    )
    .await
    .unwrap();
    service::finalize_enrollment_status(&conn, &resp.enrollment_id)
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status FROM enrollments WHERE id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let st: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(st, "partial");
}

#[tokio::test]
async fn finalize_status_failed_when_all_failed() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let _d1 = seed_device(&state.db, &config.device_creds_key).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();
    conn.execute(
        "UPDATE enrollment_device_pushes SET status='failed' WHERE enrollment_id = ?1",
        params![resp.enrollment_id.clone()],
    )
    .await
    .unwrap();
    service::finalize_enrollment_status(&conn, &resp.enrollment_id)
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status FROM enrollments WHERE id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let st: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(st, "failed");
}

#[tokio::test]
async fn finalize_status_failed_when_no_pushes() {
    // total == 0 branch.
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();
    // No devices seeded → 0 push rows.
    service::finalize_enrollment_status(&conn, &resp.enrollment_id)
        .await
        .unwrap();
    let mut rows = conn
        .query(
            "SELECT status FROM enrollments WHERE id = ?1",
            params![resp.enrollment_id.clone()],
        )
        .await
        .unwrap();
    let st: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(st, "failed");
}

#[tokio::test]
async fn finalize_enrollment_rolls_back_when_terminal_update_fails() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    seed_device(&state.db, &config.device_creds_key).await;
    let enrollment =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    service::complete_push_failure(&state, &enrollment.device_pushes[0].id, "offline")
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_enrollment_finalize \
         BEFORE UPDATE OF status ON enrollments \
         BEGIN SELECT RAISE(ABORT, 'forced finalize failure'); END;",
    )
    .await
    .unwrap();
    service::finalize_enrollment(&state, &enrollment.enrollment_id)
        .await
        .expect_err("finalization trigger must abort the canonical transaction");

    let row = conn
        .query(
            "SELECT status, completed_at, version FROM enrollments WHERE id=?1",
            params![enrollment.enrollment_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "in_progress");
    assert_eq!(row.get::<Option<i64>>(1).unwrap(), None);
    assert_eq!(row.get::<i64>(2).unwrap(), 1);
}

// ---------------------------------------------------------------------------
// upsert_device_face_mapping + list_mappings_for_employee + mark_pending_delete + delete_mapping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mapping_lifecycle_upsert_list_mark_delete() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device(&state.db, &config.device_creds_key).await;
    let conn = state.db.connect().unwrap();

    let face_id = "face-aaa";
    service::upsert_device_face_mapping(&conn, &device_id, face_id, &emp_id)
        .await
        .unwrap();
    // Upsert again — INSERT OR REPLACE creates a new row id under the same (device_id, face_id).
    service::upsert_device_face_mapping(&conn, &device_id, face_id, &emp_id)
        .await
        .unwrap();

    let mappings = service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(!mappings.is_empty());
    let (mapping_id, _dev, _face) = mappings[0].clone();

    // Mark pending_delete; list should still include it.
    service::mark_mapping_pending_delete(&conn, &mapping_id)
        .await
        .unwrap();
    let after = service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert_eq!(after.len(), mappings.len());

    // Delete; list should drop it.
    service::delete_mapping(&conn, &mapping_id).await.unwrap();
    let after_del = service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(after_del.iter().all(|(id, _, _)| id != &mapping_id));
}

// ---------------------------------------------------------------------------
// get_employee_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_employee_status_returns_active() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let conn = state.db.connect().unwrap();

    let st = service::get_employee_status(&conn, &emp_id).await.unwrap();
    assert_eq!(st, "active");
}

#[tokio::test]
async fn get_employee_status_404_on_missing() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let conn = state.db.connect().unwrap();

    let err = service::get_employee_status(&conn, "no-such-emp")
        .await
        .unwrap_err();
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, "EMPLOYEE_NOT_FOUND"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// list_employees_with_face + get_current_photo_path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_employees_with_face_and_get_current_photo_path() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    // Before enrollment — list should be empty (no face_id).
    let conn = state.db.connect().unwrap();
    let before = service::list_employees_with_face(&conn).await.unwrap();
    assert!(before.iter().all(|(id, _, _)| id != &emp_id));

    // Before enrollment — get_current_photo_path returns None.
    let p = service::get_current_photo_path(&conn, &emp_id)
        .await
        .unwrap();
    assert!(p.is_none());

    // After enrollment — both should produce a hit.
    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let after = service::list_employees_with_face(&conn).await.unwrap();
    assert!(after
        .iter()
        .any(|(id, fid, _)| id == &emp_id && fid == &resp.face_id));

    let p = service::get_current_photo_path(&conn, &emp_id)
        .await
        .unwrap();
    assert!(p.is_some());
    let p = p.unwrap();
    assert!(p.contains(&emp_id));
    assert!(p.ends_with(".jpg"));
}

// ---------------------------------------------------------------------------
// get_enrollment_push_params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_enrollment_push_params_returns_triple() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let conn = state.db.connect().unwrap();

    let (e, f, n) = service::get_enrollment_push_params(&conn, &resp.enrollment_id)
        .await
        .unwrap();
    assert_eq!(e, emp_id);
    assert_eq!(f, resp.face_id);
    assert_eq!(n, "Test Employee");
}

#[tokio::test]
async fn get_enrollment_push_params_404_when_missing() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let conn = state.db.connect().unwrap();

    let err = service::get_enrollment_push_params(&conn, "no-such-enr")
        .await
        .unwrap_err();
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, "ENROLLMENT_NOT_FOUND"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}
