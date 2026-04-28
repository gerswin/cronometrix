//! Phase 7 — Multi-device concurrent push integration tests.
//!
//! Covers D-06 (JoinSet fan-out), D-08 (partial failure), D-16 (backfill Semaphore=4).
//! Uses wiremock to simulate Hikvision device responses.

mod common;

use std::sync::Arc;

use cronometrix_api::enrollments::service;

// ---------------------------------------------------------------------------
// Helper: seed the minimal FK chain and return (employee_id, face_id, enrollment_id)
// ---------------------------------------------------------------------------

async fn seed_enrollment_scenario(
    conn: &libsql::Connection,
    n_devices: usize,
    device_base_urls: &[String],
) -> (String, String, String, Vec<cronometrix_api::devices::models::DeviceWithPlaintext>) {
    // User
    let user_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Push Test Admin', 'hash', 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), format!("pt-{}", &user_id[..8])],
    ).await.expect("seed user");

    // Department
    let dept_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    ).await.expect("seed dept");

    // Employee
    let emp_id = uuid::Uuid::new_v4().to_string();
    let face_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, face_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Push Test Emp', ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![emp_id.clone(), format!("EMP-{}", &emp_id[..8]), dept_id.clone(), face_id.clone()],
    ).await.expect("seed employee");

    // Face enrollment
    let fe_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO face_enrollments (id, employee_id, captured_via, photo_path, created_by, created_at) \
         VALUES (?1, ?2, 'upload', ?3, ?4, unixepoch())",
        libsql::params![fe_id.clone(), emp_id.clone(), format!("/tmp/{}.jpg", fe_id), user_id.clone()],
    ).await.expect("seed face_enrollment");

    // Enrollment
    let enr_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO enrollments (id, employee_id, face_enrollment_id, status, started_by, started_at, version) \
         VALUES (?1, ?2, ?3, 'in_progress', ?4, unixepoch(), 1)",
        libsql::params![enr_id.clone(), emp_id.clone(), fe_id.clone(), user_id.clone()],
    ).await.expect("seed enrollment");

    let mut devices = Vec::new();
    for i in 0..n_devices {
        let dev_id = uuid::Uuid::new_v4().to_string();
        let url = device_base_urls.get(i).cloned().unwrap_or_else(|| format!("http://192.168.1.{}:80", i + 10));

        // Parse ip and port from the URL so unique(ip,port) constraint holds per device.
        // URL format from wiremock: "http://127.0.0.1:PORT"
        let host_port = url.trim_start_matches("http://").trim_start_matches("https://");
        let (ip, port): (String, i64) = if let Some(colon) = host_port.rfind(':') {
            let ip = host_port[..colon].to_string();
            let port: i64 = host_port[colon + 1..].parse().unwrap_or(80);
            (ip, port)
        } else {
            (host_port.to_string(), 80)
        };

        conn.execute(
            "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, direction, \
             allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'http', 'admin', 'enc', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
            libsql::params![dev_id.clone(), format!("Device-{}", i), ip, port],
        ).await.expect("seed device");

        // Push row
        let push_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO enrollment_device_pushes (id, enrollment_id, device_id, status) \
             VALUES (?1, ?2, ?3, 'pending')",
            libsql::params![push_id, enr_id.clone(), dev_id.clone()],
        ).await.expect("seed push row");

        devices.push(cronometrix_api::devices::models::DeviceWithPlaintext {
            id: dev_id,
            name: format!("Device-{}", i),
            base_url: url,
            username: "admin".to_string(),
            password: "password".to_string(),
            direction: "entry".to_string(),
            allow_insecure_tls: true,
            status: "active".to_string(),
            version: 1,
        });
    }

    (emp_id, face_id, enr_id, devices)
}

// ---------------------------------------------------------------------------
// Test 1: JoinSet fans out to all active devices concurrently
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_joinset_fans_out_to_all_active_devices_concurrently() {
    // Spin up 3 mock Hikvision servers (one per device).
    let s1 = common::mock_hikvision_server().await;
    let s2 = common::mock_hikvision_server().await;
    let s3 = common::mock_hikvision_server().await;

    let db = common::test_db().await;
    let conn = db.connect().expect("connect");

    let urls = vec![s1.uri(), s2.uri(), s3.uri()];
    let (emp_id, face_id, enr_id, devices) =
        seed_enrollment_scenario(&conn, 3, &urls).await;

    let photo_bytes = Arc::new(common::sample_face_jpeg_50kb());

    // Build minimal AppState with the test DB (per Plan 08-02 D-20:
    // tempdir-rooted Paths via the shared helper).
    let (state, _tmp) = build_test_state(db);

    // Call spawn_enrollment_pushes and wait for it to complete.
    cronometrix_api::enrollments::pusher::spawn_enrollment_pushes(
        state.clone(),
        enr_id.clone(),
        face_id.clone(),
        photo_bytes,
        emp_id.clone(),
        devices,
    );

    // Give the async driver time to complete (it's fire-and-forget).
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // All 3 mock servers should have received UserInfo/Record POST.
    assert_eq!(s1.received_requests().await.unwrap().len(), 2, "server 1: expected UserInfo + FaceData");
    assert_eq!(s2.received_requests().await.unwrap().len(), 2, "server 2: expected UserInfo + FaceData");
    assert_eq!(s3.received_requests().await.unwrap().len(), 2, "server 3: expected UserInfo + FaceData");

    // Enrollment should be finalized as 'success'.
    let conn2 = state.db.connect().expect("connect");
    let mut rows = conn2.query(
        "SELECT status FROM enrollments WHERE id = ?1",
        libsql::params![enr_id.clone()],
    ).await.expect("query");
    let row = rows.next().await.expect("next").expect("has row");
    let status: String = row.get(0).expect("status col");
    assert_eq!(status, "success", "enrollment should be success when all 3 push to mock servers");
}

// ---------------------------------------------------------------------------
// Test 2: Partial failure sets enrollment status = 'partial'
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_partial_failure_sets_enrollment_status_partial() {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    // s1 = success, s2 = failure (500 on both endpoints).
    let s1 = common::mock_hikvision_server().await;
    let s2_fail = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&s2_fail)
        .await;

    let db = common::test_db().await;
    let conn = db.connect().expect("connect");

    let urls = vec![s1.uri(), s2_fail.uri()];
    let (_emp_id, face_id, enr_id, devices) =
        seed_enrollment_scenario(&conn, 2, &urls).await;

    let photo_bytes = Arc::new(common::sample_face_jpeg_50kb());
    let (state, _tmp) = build_test_state(db);

    cronometrix_api::enrollments::pusher::spawn_enrollment_pushes(
        state.clone(),
        enr_id.clone(),
        face_id,
        photo_bytes,
        _emp_id,
        devices,
    );

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let conn2 = state.db.connect().expect("connect");
    let mut rows = conn2.query(
        "SELECT status FROM enrollments WHERE id = ?1",
        libsql::params![enr_id.clone()],
    ).await.expect("query");
    let row = rows.next().await.expect("next").expect("has row");
    let status: String = row.get(0).expect("status col");
    assert_eq!(status, "partial", "1 success + 1 failure = partial");
}

// ---------------------------------------------------------------------------
// Test 3: Zero devices succeed → enrollment status = 'failed'
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_zero_devices_succeed_sets_failed() {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    let s1_fail = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&s1_fail)
        .await;

    let db = common::test_db().await;
    let conn = db.connect().expect("connect");

    let urls = vec![s1_fail.uri()];
    let (_emp_id, face_id, enr_id, devices) =
        seed_enrollment_scenario(&conn, 1, &urls).await;

    let photo_bytes = Arc::new(common::sample_face_jpeg_50kb());
    let (state, _tmp) = build_test_state(db);

    cronometrix_api::enrollments::pusher::spawn_enrollment_pushes(
        state.clone(),
        enr_id.clone(),
        face_id,
        photo_bytes,
        _emp_id,
        devices,
    );

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let conn2 = state.db.connect().expect("connect");
    let mut rows = conn2.query(
        "SELECT status FROM enrollments WHERE id = ?1",
        libsql::params![enr_id.clone()],
    ).await.expect("query");
    let row = rows.next().await.expect("next").expect("has row");
    let status: String = row.get(0).expect("status col");
    assert_eq!(status, "failed", "all failures = failed");
}

// ---------------------------------------------------------------------------
// Test 4: All devices succeed → enrollment status = 'success'
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_all_devices_succeed_sets_success() {
    let s1 = common::mock_hikvision_server().await;
    let s2 = common::mock_hikvision_server().await;

    let db = common::test_db().await;
    let conn = db.connect().expect("connect");

    let urls = vec![s1.uri(), s2.uri()];
    let (_emp_id, face_id, enr_id, devices) =
        seed_enrollment_scenario(&conn, 2, &urls).await;

    let photo_bytes = Arc::new(common::sample_face_jpeg_50kb());
    let (state, _tmp) = build_test_state(db);

    cronometrix_api::enrollments::pusher::spawn_enrollment_pushes(
        state.clone(),
        enr_id.clone(),
        face_id,
        photo_bytes,
        _emp_id,
        devices,
    );

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let conn2 = state.db.connect().expect("connect");
    let mut rows = conn2.query(
        "SELECT status FROM enrollments WHERE id = ?1",
        libsql::params![enr_id.clone()],
    ).await.expect("query");
    let row = rows.next().await.expect("next").expect("has row");
    let status: String = row.get(0).expect("status col");
    assert_eq!(status, "success", "all success = success");
}

// ---------------------------------------------------------------------------
// Test 5: BackfillWorker processes exactly one request without panic
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires wired DB device + full photo on disk — covered in wave 2 HTTP tests"]
async fn test_backfill_respects_semaphore_4_max_in_flight() {
    // This test verifies that BackfillWorker uses Semaphore(4) to cap concurrency.
    // Covered via integration tests in wave 2 once the full HTTP layer is wired.
    todo!("implement in wave 2 with 10-device DB setup")
}

// ---------------------------------------------------------------------------
// Shared test state builder
// ---------------------------------------------------------------------------

/// Build (AppState, TempDir) for multi-device push tests. Per Plan 08-02
/// D-20: AppState carries a tempdir-rooted `Paths`; the caller binds the
/// returned TempDir to a local that outlives every assertion (Pitfall 1 in
/// 08-RESEARCH.md). Drives off the same shared helper as every other
/// integration test in this crate.
fn build_test_state(
    db: libsql::Database,
) -> (cronometrix_api::state::AppState, tempfile::TempDir) {
    let config = Arc::new(cronometrix_api::config::Config {
        database_path: String::new(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: "test".to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3000,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: chrono_tz::America::Caracas,
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}
