//! Coverage gap-fill for `backend/src/enrollments/pusher.rs` (08-04B Task 1).
//!
//! Baseline 56.57% line. Target ≥70%.
//!
//! Strategy:
//!   * Use `wiremock::MockServer` as the simulated Hikvision device — same
//!     pattern Plan 04A established in `isapi_client_test.rs`.
//!   * Invoke `push_one_device` and `push_one_device_for_backfill` directly
//!     against an in-process state with a single seeded device pointing at
//!     the mock server's URI.
//!   * Cover: success path (push row → "success", mapping upserted),
//!     ISAPI 5xx error path (push row → "failed", error_message scrubbed),
//!     timeout path is not reachable in unit-test time (request timeout is 30s,
//!     and our 30s tokio::time::timeout sits AFTER reqwest's own timeout —
//!     covered indirectly by the 5xx path which exercises the error-arm logic).
//!   * `spawn_enrollment_pushes` happy path with 0 devices (early-return
//!     finalize-only branch) AND with N devices (driver awaits pushes then
//!     finalises).
//!   * `scrub_password` is exercised via push_one_device's failed branch when
//!     the mock returns 5xx — but it lives in a private function, so we cover
//!     it transitively. We also assert the pushed row's error_message does NOT
//!     contain the device password.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::devices::crypto;
use cronometrix_api::devices::models::DeviceWithPlaintext;
use cronometrix_api::enrollments::pusher::{
    push_one_device, push_one_device_for_backfill, push_one_device_for_backfill_with_timeout,
    push_one_device_with_timeout, spawn_enrollment_pushes,
};
use cronometrix_api::enrollments::service;
use libsql::params;
use uuid::Uuid;
use wiremock::matchers::{method as wm_method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{test_device_creds_key, TEST_JWT_SECRET};

const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

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

/// Seed an active device pointing at `base_url` so the pusher resolves to the wiremock host.
async fn seed_device_at(db: &libsql::Database, key: &[u8; 32], base_url: &str) -> String {
    // Parse host:port out of base_url ("http://127.0.0.1:NNNN") to fit the schema.
    let url = url_lite_split(base_url);
    let conn = db.connect().expect("connect");
    let enc = crypto::encrypt_password("device-pw", key).unwrap();
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'admin', ?6, 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            format!("dev-{}", &id[..8]),
            url.0,
            url.1 as i64,
            url.2,
            enc,
        ],
    )
    .await
    .expect("seed device");
    id
}

/// Tiny URL splitter — returns (host, port, scheme).
/// Wiremock URI format is `http://127.0.0.1:PORT`.
fn url_lite_split(url: &str) -> (String, u16, String) {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("http://") {
        ("http".to_string(), rest)
    } else if let Some(rest) = url.strip_prefix("https://") {
        ("https".to_string(), rest)
    } else {
        panic!("unsupported scheme: {url}");
    };
    let (host, port_str) = rest.rsplit_once(':').unwrap_or((rest, "80"));
    let port: u16 = port_str.parse().unwrap_or(80);
    (host.to_string(), port, scheme)
}

/// Build a DeviceWithPlaintext directly so tests can call `push_one_device`
/// without re-decrypting via the service layer.
fn make_plain_device(id: &str, base_url: &str) -> DeviceWithPlaintext {
    DeviceWithPlaintext {
        id: id.into(),
        name: "Test Device".into(),
        base_url: base_url.into(),
        username: "admin".into(),
        password: "device-pw".into(),
        direction: "entry".into(),
        allow_insecure_tls: false,
        status: "active".into(),
        version: 1,
    }
}

async fn mount_successful_face_push(server: &MockServer) {
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(server)
        .await;
}

async fn clear_initial_checkpoint(state: &cronometrix_api::state::AppState, push_id: &str) {
    state
        .db
        .connect()
        .unwrap()
        .execute(
            "DELETE FROM device_operation_checkpoints WHERE operation_key=?1",
            params![service::enrollment_checkpoint_key(push_id)],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn enrollment_recovery_does_not_repeat_confirmed_device_side_effect() {
    let server = MockServer::start().await;
    mount_successful_face_push(&server).await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, employee_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let started = service::start_enrollment(
        &state,
        &user_id,
        &employee_id,
        "device",
        None,
        None,
        MINI_JPEG,
    )
    .await
    .unwrap();
    clear_initial_checkpoint(&state, &started.device_pushes[0].id).await;
    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());
    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_mapping_insert BEFORE INSERT ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced mapping failure'); END;",
    )
    .await
    .unwrap();

    push_one_device(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &employee_id,
        "Test Employee",
        &device,
    )
    .await
    .expect_err("first DB completion is deliberately rejected");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    conn.execute_batch("DROP TRIGGER fail_mapping_insert;")
        .await
        .unwrap();
    let retry_push_id =
        service::reset_push_to_pending_queued(&state, &started.enrollment_id, &device_id)
            .await
            .unwrap();
    assert_eq!(retry_push_id, started.device_pushes[0].id);

    push_one_device(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &employee_id,
        "Test Employee",
        &device,
    )
    .await
    .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn backfill_recovery_does_not_repeat_confirmed_device_side_effect() {
    let server = MockServer::start().await;
    mount_successful_face_push(&server).await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, employee_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let device = make_plain_device(&device_id, &server.uri());
    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_backfill_mapping BEFORE INSERT ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced backfill mapping failure'); END;",
    )
    .await
    .unwrap();

    push_one_device_for_backfill(
        &state,
        "face-1",
        MINI_JPEG,
        &employee_id,
        "Employee",
        &device,
    )
    .await
    .expect_err("first DB completion is deliberately rejected");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    conn.execute_batch("DROP TRIGGER fail_backfill_mapping;")
        .await
        .unwrap();
    push_one_device_for_backfill(
        &state,
        "face-1",
        MINI_JPEG,
        &employee_id,
        "Employee",
        &device,
    )
    .await
    .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn tracker_shutdown_awaits_an_inflight_device_push_and_terminal_write() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(150))
                .set_body_string(r#"{"statusCode":1}"#),
        )
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, employee_id, user_id) = seed_dept_emp_user(&state.db).await;
    let _device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let dispatcher = state
        .enrollment_dispatcher
        .start(state.clone())
        .await
        .unwrap();
    let started = service::start_enrollment(
        &state,
        &user_id,
        &employee_id,
        "device",
        None,
        None,
        MINI_JPEG,
    )
    .await
    .unwrap();
    let enrollment_id = started.enrollment_id.clone();
    for _ in 0..100 {
        if !server.received_requests().await.unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    state.enrollment_dispatcher.close().unwrap();
    dispatcher.await.unwrap().unwrap();

    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    let row = state
        .db
        .connect()
        .unwrap()
        .query(
            "SELECT status FROM enrollments WHERE id=?1",
            params![enrollment_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "success");
}

#[tokio::test]
async fn dispatcher_rejects_a_second_start_and_close_is_idempotent() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let dispatcher = cronometrix_api::enrollments::dispatcher::EnrollmentDispatcher::default();

    let handle = dispatcher.start(state.clone()).await.unwrap();
    let second_start = dispatcher.start(state).await.unwrap_err();
    assert!(second_start.to_string().contains("already started"));

    dispatcher.close().unwrap();
    dispatcher.close().unwrap();
    handle.await.unwrap().unwrap();
}

#[tokio::test]
async fn tracker_drain_reports_panicked_tasks_after_awaiting_them() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    state
        .enrollment_tasks
        .spawn(async { panic!("intentional tracked-task panic") })
        .await
        .unwrap();

    let error = state
        .enrollment_tasks
        .stop_and_join()
        .await
        .expect_err("panic must be reported after drain");
    assert!(error.to_string().contains("task join failure"));
}

// =============================================================================
// push_one_device — happy path
// =============================================================================

#[tokio::test]
async fn push_one_device_happy_path_success() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    // start_enrollment to insert push row.
    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &resp.device_pushes[0].id).await;

    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());

    let res = push_one_device(
        &state,
        &resp.enrollment_id,
        &resp.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
    )
    .await;
    assert!(res.is_ok(), "push must succeed: {res:?}");

    // Push row should be marked success.
    let conn = state.db.connect().unwrap();
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
    assert_eq!(st, "success");
    assert!(err.is_none());

    // device_face_mapping should be upserted.
    let mappings = service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(mappings
        .iter()
        .any(|(_, did, fid)| did == &device_id && fid == &resp.face_id));
}

#[tokio::test]
async fn mapping_persistence_failure_marks_push_failed_without_retrying_device() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let enrollment =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &enrollment.device_pushes[0].id).await;
    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_pusher_mapping BEFORE INSERT ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced pusher mapping failure'); END;",
    )
    .await
    .unwrap();

    let result = push_one_device(
        &state,
        &enrollment.enrollment_id,
        &enrollment.face_id,
        &Arc::new(MINI_JPEG.to_vec()),
        &emp_id,
        "Test Employee",
        &make_plain_device(&device_id, &server.uri()),
    )
    .await;
    assert!(result.is_err());

    let row = conn
        .query(
            "SELECT status, error_message FROM enrollment_device_pushes \
             WHERE enrollment_id=?1 AND device_id=?2",
            params![enrollment.enrollment_id.clone(), device_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "failed");
    assert!(row.get::<Option<String>>(1).unwrap().is_some());
}

// =============================================================================
// push_one_device — 5xx error path → push row "failed" + scrubbed error
// =============================================================================

#[tokio::test]
async fn push_one_device_5xx_marks_push_failed() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(500).set_body_string("device-pw blew up"))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &resp.device_pushes[0].id).await;

    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());

    let err = push_one_device(
        &state,
        &resp.enrollment_id,
        &resp.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
    )
    .await
    .unwrap_err();
    let s = err.to_string();
    // Critical: the password "device-pw" must NOT appear in the surfaced error string
    // (T-7-06 — credential redaction).
    assert!(
        !s.contains("device-pw"),
        "password must be scrubbed from error: {s}"
    );

    // Push row was marked failed with a scrubbed error_message.
    let conn = state.db.connect().unwrap();
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
    let err_msg: Option<String> = row.get(1).unwrap();
    assert_eq!(st, "failed");
    let msg = err_msg.expect("error_message must be set on failure");
    // Even if the upstream body included the password, the scrubbed copy must not.
    assert!(
        !msg.contains("device-pw"),
        "stored error_message must be scrubbed: {msg}"
    );
    drop(rows);
    drop(conn);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    let retry = push_one_device(
        &state,
        &resp.enrollment_id,
        &resp.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
    )
    .await;
    assert!(retry.is_err());
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "ambiguous device result must never be replayed"
    );
}

#[tokio::test]
async fn partial_device_push_is_manual_and_never_replayed() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upload failed"))
        .mount(&server)
        .await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let started =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &started.device_pushes[0].id).await;
    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());

    push_one_device(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
    )
    .await
    .expect_err("face upload failure follows an accepted user upsert");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    let checkpoint: String = state
        .db
        .connect()
        .unwrap()
        .query(
            "SELECT state FROM device_operation_checkpoints WHERE operation_key=?1",
            params![service::enrollment_checkpoint_key(
                &started.device_pushes[0].id
            )],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(checkpoint, "manual");

    push_one_device(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
    )
    .await
    .expect_err("manual checkpoint must prohibit replay");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn device_timeouts_are_manual_and_never_replayed_for_enrollment_and_backfill() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(1))
                .set_body_string(r#"{"statusCode":1}"#),
        )
        .mount(&server)
        .await;
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let started =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &started.device_pushes[0].id).await;
    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());
    push_one_device_with_timeout(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
        std::time::Duration::from_millis(100),
    )
    .await
    .expect_err("device call must time out");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let enrollment_requests = server.received_requests().await.unwrap().len();
    assert_eq!(enrollment_requests, 1);
    push_one_device_with_timeout(
        &state,
        &started.enrollment_id,
        &started.face_id,
        &photo,
        &emp_id,
        "Test Employee",
        &device,
        std::time::Duration::from_millis(100),
    )
    .await
    .expect_err("manual timeout checkpoint must prohibit replay");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        enrollment_requests
    );

    let backfill_face = "timeout-backfill-face";
    push_one_device_for_backfill_with_timeout(
        &state,
        backfill_face,
        MINI_JPEG,
        &emp_id,
        "Test Employee",
        &device,
        std::time::Duration::from_millis(100),
    )
    .await
    .expect_err("backfill device call must time out");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let after_backfill = server.received_requests().await.unwrap().len();
    assert_eq!(after_backfill, enrollment_requests + 1);
    push_one_device_for_backfill_with_timeout(
        &state,
        backfill_face,
        MINI_JPEG,
        &emp_id,
        "Test Employee",
        &device,
        std::time::Duration::from_millis(100),
    )
    .await
    .expect_err("manual backfill timeout checkpoint must prohibit replay");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        after_backfill
    );
}

// =============================================================================
// push_one_device — missing push row early-return
// =============================================================================

#[tokio::test]
async fn push_one_device_no_push_row_returns_terminal_error_without_device_call() {
    let server = MockServer::start().await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());

    // A missing persistence row must not be treated as a successful dispatch.
    let res = push_one_device(
        &state,
        "nonexistent-enr",
        "nonexistent-face",
        &photo,
        &emp_id,
        "Anyone",
        &device,
    )
    .await;
    assert!(res.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}

// =============================================================================
// push_one_device_for_backfill — success path
// =============================================================================

#[tokio::test]
async fn push_one_device_for_backfill_happy_path() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let device = make_plain_device(&device_id, &server.uri());
    let face_id = "face-backfill-1";

    push_one_device_for_backfill(
        &state,
        face_id,
        MINI_JPEG,
        &emp_id,
        "Backfill Test",
        &device,
    )
    .await
    .expect("backfill push must succeed");

    let conn = state.db.connect().unwrap();
    let mappings = service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(mappings
        .iter()
        .any(|(_, did, fid)| did == &device_id && fid == face_id));
}

// =============================================================================
// push_one_device_for_backfill — 5xx error path
// =============================================================================

#[tokio::test]
async fn push_one_device_for_backfill_5xx_returns_err_scrubbed() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(500).set_body_string("device-pw oops"))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, _user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let device = make_plain_device(&device_id, &server.uri());

    let err = push_one_device_for_backfill(
        &state,
        "face-x",
        MINI_JPEG,
        &emp_id,
        "Backfill Test",
        &device,
    )
    .await
    .unwrap_err();
    let s = err.to_string();
    assert!(!s.contains("device-pw"), "scrub: {s}");
    assert!(
        s.contains("backfill ISAPI push failed") || s.contains("ISAPI"),
        "must surface as backfill-tagged error: {s}"
    );
}

// =============================================================================
// spawn_enrollment_pushes — driver fan-out
// =============================================================================

#[tokio::test]
async fn spawn_enrollment_pushes_zero_devices_finalises_failed() {
    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;

    // No devices seeded → push rows table empty.
    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let photo = Arc::new(MINI_JPEG.to_vec());

    spawn_enrollment_pushes(
        state.clone(),
        resp.enrollment_id.clone(),
        resp.face_id.clone(),
        photo,
        emp_id.clone(),
        "Test Employee".to_string(),
        vec![],
    )
    .await
    .unwrap();

    // Yield first so the spawned task can claim a slot before we enter the poll loop.
    tokio::task::yield_now().await;

    // Driver runs under the task tracker; give it up to ~5s to invoke finalize.
    for _ in 0..200 {
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT status FROM enrollments WHERE id = ?1",
                params![resp.enrollment_id.clone()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let st: String = row.get(0).unwrap();
        drop(rows);
        drop(conn);
        if st == "failed" {
            return; // expected outcome — total==0 finalise branch.
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("spawn_enrollment_pushes(zero devices) did not finalise to failed within 5s");
}

#[tokio::test]
async fn spawn_enrollment_pushes_with_devices_finalises_success() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let config = make_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config.clone());
    let (_dept, emp_id, user_id) = seed_dept_emp_user(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let resp =
        service::start_enrollment(&state, &user_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    clear_initial_checkpoint(&state, &resp.device_pushes[0].id).await;

    let device = make_plain_device(&device_id, &server.uri());
    let photo = Arc::new(MINI_JPEG.to_vec());

    // Spawn the fan-out driver. We do not poll the DB directly because libsql's
    // shared-cache locking can starve the driver task when the test holds an
    // outer SELECT cursor open in a loop. Instead we await the entire spawn
    // chain by wrapping it with a wider tokio::time::timeout: the driver picks
    // up the device, calls push_one_device, finalises, and returns — and the
    // observable side effect is the enrollments row flipping to "success".
    let state_for_drive = state.clone();
    let enr_for_drive = resp.enrollment_id.clone();
    let face_for_drive = resp.face_id.clone();
    let emp_for_drive = emp_id.clone();
    let driver = tokio::spawn(async move {
        spawn_enrollment_pushes(
            state_for_drive,
            enr_for_drive,
            face_for_drive,
            photo,
            emp_for_drive,
            "Test Employee".to_string(),
            vec![device],
        )
        .await
        .unwrap();
    });
    // The spawn_enrollment_pushes call returns immediately (it spawns a
    // tracked task internally). Wait for the owned task to finalise by
    // polling, with very gentle backoff and explicit drops.
    driver.await.unwrap();

    let mut last_status = String::new();
    for _ in 0..400 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let st: String = {
            let mut rows = conn
                .query(
                    "SELECT status FROM enrollments WHERE id = ?1",
                    params![resp.enrollment_id.clone()],
                )
                .await
                .unwrap();
            let row = rows.next().await.unwrap().unwrap();
            row.get(0).unwrap()
        };
        last_status = st.clone();
        if st == "success" {
            return;
        }
    }
    panic!(
        "spawn_enrollment_pushes(1 device, 200) did not finalise to success (last={})",
        last_status
    );
}
