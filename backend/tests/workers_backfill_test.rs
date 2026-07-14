//! Coverage gap-fill for `backend/src/workers/backfill.rs` (08-04B Task 2).
//!
//! Baseline 0.00% line. Target ≥70%.
//!
//! BackfillWorker pushes all active enrolled faces to a newly-registered
//! device. Tests:
//!   * shutdown-cancel exits cleanly.
//!   * channel-drop exits cleanly.
//!   * No matching device → silent.
//!   * Empty enrollment list (no employees with face_id) → silent / no panic.
//!   * Successful backfill: wiremock 200 → device_face_mappings upserted.
//!   * Failed backfill: wiremock 5xx → no mapping written, but worker keeps going.

mod common;

use std::sync::Arc;
use std::time::Duration;

use cronometrix_api::config::Config;
use cronometrix_api::devices::crypto;
use cronometrix_api::enrollments::service as enrollment_service;
use cronometrix_api::workers::backfill::{BackfillRequest, BackfillWorker};
use libsql::params;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
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
        database_path: "test.db".into(),
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

async fn seed_device_at(db: &libsql::Database, key: &[u8; 32], base_url: &str) -> String {
    let parts = url_lite_split(base_url);
    let conn = db.connect().unwrap();
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
            parts.0,
            parts.1 as i64,
            parts.2,
            enc,
        ],
    )
    .await
    .unwrap();
    id
}

/// Seed an active employee + a face_enrollment + write the photo bytes to disk.
/// Returns the employee id.
async fn seed_active_enrolled_employee(state: &cronometrix_api::state::AppState) -> String {
    let conn = state.db.connect().unwrap();
    let admin_id = common::create_test_admin(&state.db).await;
    let dept_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    )
    .await
    .unwrap();
    let emp_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Backfill Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .unwrap();
    drop(conn);

    // Use start_enrollment to wire face_id + current_face_enrollment_id +
    // the photo file on disk.
    let _ = enrollment_service::start_enrollment(
        state, &admin_id, &emp_id, "upload", None, None, MINI_JPEG,
    )
    .await
    .unwrap();
    emp_id
}

// =============================================================================
// Cancellation / channel close
// =============================================================================

#[tokio::test]
async fn backfill_worker_exits_on_shutdown() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (_tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state, shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn backfill_worker_exits_on_channel_drop() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state, shutdown);
    let h = tokio::spawn(async move { w.run(rx).await });
    drop(tx);
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

// =============================================================================
// Unknown device → silently no-op
// =============================================================================

#[tokio::test]
async fn backfill_unknown_device_no_panic() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(BackfillRequest {
        device_id: "nonexistent".into(),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

// =============================================================================
// No employees enrolled → silent no-op
// =============================================================================

#[tokio::test]
async fn backfill_with_no_employees_to_backfill_no_panic() {
    let server = MockServer::start().await;
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(BackfillRequest {
        device_id: device_id.clone(),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());

    // Smoke: no mapping rows materialised.
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query("SELECT count(*) FROM device_face_mappings", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let n: i64 = row.get(0).unwrap();
    assert_eq!(n, 0);
}

// =============================================================================
// Successful backfill: wiremock 200 → mapping upserted
// =============================================================================

#[tokio::test]
async fn backfill_success_upserts_mapping_for_each_enrolled_employee() {
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
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_active_enrolled_employee(&state).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(BackfillRequest {
        device_id: device_id.clone(),
    })
    .unwrap();

    // Wait for the mapping row to materialise.
    let mut found = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
            .await
            .unwrap();
        if mappings.iter().any(|(_, did, _)| did == &device_id) {
            found = true;
            break;
        }
    }
    assert!(found, "successful backfill must upsert device_face_mapping");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
}

#[tokio::test]
async fn backfill_mapping_failure_does_not_retry_accepted_device_side_effects() {
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
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let emp_id = seed_active_enrolled_employee(&state).await;
    let device_id = seed_device_at(&state.db, &state.config.device_creds_key, &server.uri()).await;
    let conn = state.db.connect().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_backfill_mapping BEFORE INSERT ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced backfill mapping failure'); END;",
    )
    .await
    .unwrap();

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let worker = BackfillWorker::new(state.clone(), shutdown.clone());
    let handle = tokio::spawn(async move { worker.run(rx).await });
    tx.send(BackfillRequest {
        device_id: device_id.clone(),
    })
    .unwrap();

    for _ in 0..100 {
        if server
            .received_requests()
            .await
            .is_some_and(|requests| requests.len() >= 2)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    server.verify().await;
    let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(
        mappings.is_empty(),
        "failed mapping remains recoverable by backfill replay"
    );

    shutdown.cancel();
    handle.await.unwrap();
}

// =============================================================================
// Failed backfill (5xx): mapping is NOT written; worker keeps running.
// =============================================================================

#[tokio::test]
async fn backfill_5xx_does_not_write_mapping_and_keeps_running() {
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_active_enrolled_employee(&state).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<BackfillRequest>();
    let w = BackfillWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(BackfillRequest {
        device_id: device_id.clone(),
    })
    .unwrap();

    // Give it a generous window for the 5xx path to settle.
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Mapping must NOT have been written.
    let conn = state.db.connect().unwrap();
    let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(
        mappings.iter().all(|(_, did, _)| did != &device_id),
        "5xx must not produce a mapping row"
    );

    // Worker is still alive; cancel cleanly.
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

// =============================================================================
// Debug + Clone on BackfillRequest
// =============================================================================

#[test]
fn backfill_request_debug_and_clone() {
    let r = BackfillRequest {
        device_id: "dev".into(),
    };
    let d = format!("{:?}", r);
    assert!(d.contains("BackfillRequest"));
    let _ = r.clone();
}
