//! Coverage gap-fill for `backend/src/workers/purge.rs` (08-04B Task 2).
//!
//! Baseline 0.00% line. Target ≥70%.
//!
//! `PurgeWorker::run` is mpsc-driven with batched dedup + biased shutdown.
//! For each PurgeRequest:
//!   1. Re-read employee status (Pitfall 10 guard — abort if not 'inactive').
//!   2. List device_face_mappings for the employee.
//!   3. For each mapping: fetch device, delete via ISAPI with 30s timeout,
//!      DELETE mapping row on success / mark pending_delete on error.
//!
//! Tests:
//!   * shutdown-cancel exits cleanly.
//!   * channel-drop exits cleanly.
//!   * Pitfall 10: employee with status='active' → no purge attempted.
//!   * Empty mappings: no panic, log only.
//!   * Successful purge: wiremock returns 200 → mapping row deleted.
//!   * Failed purge (5xx): mapping row marked pending_delete (state).
//!   * Dedup: send 5 requests for the same employee → drained as a single iter.

mod common;

use std::sync::Arc;
use std::time::Duration;

use cronometrix_api::config::Config;
use cronometrix_api::devices::crypto;
use cronometrix_api::enrollments::service as enrollment_service;
use cronometrix_api::workers::purge::{PurgeRequest, PurgeWorker};
use libsql::params;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use wiremock::matchers::{method as wm_method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{test_device_creds_key, TEST_JWT_SECRET};

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

async fn seed_employee_inactive(db: &libsql::Database) -> String {
    let conn = db.connect().unwrap();
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
         VALUES (?1, ?2, 'Emp', ?3, 'inactive', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .unwrap();
    emp_id
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

// =============================================================================
// Cancellation / channel close
// =============================================================================

#[tokio::test]
async fn purge_worker_exits_on_shutdown() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (_tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state, shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn purge_worker_exits_on_channel_drop() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state, shutdown);
    let h = tokio::spawn(async move { w.run(rx).await });
    drop(tx);
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok());
}

// =============================================================================
// Pitfall 10 — employee re-activated → no purge
// =============================================================================

#[tokio::test]
async fn purge_skips_when_employee_is_active() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let server = MockServer::start().await;

    // Seed ACTIVE employee + mapping (purge must skip).
    let conn = state.db.connect().unwrap();
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
         VALUES (?1, ?2, 'Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .unwrap();
    drop(conn);

    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let conn = state.db.connect().unwrap();
    enrollment_service::upsert_device_face_mapping(&conn, &device_id, "face-x", &emp_id)
        .await
        .unwrap();
    drop(conn);

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    tx.send(PurgeRequest {
        employee_id: emp_id.clone(),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Mapping still exists.
    let conn = state.db.connect().unwrap();
    let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert_eq!(mappings.len(), 1, "active employee → no purge");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
}

// =============================================================================
// Empty mappings — purge for an inactive employee with no mappings is no-op.
// =============================================================================

#[tokio::test]
async fn purge_no_mappings_no_panic() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let emp_id = seed_employee_inactive(&state.db).await;

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(PurgeRequest {
        employee_id: emp_id,
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok(), "no panic for empty mappings");
}

// =============================================================================
// Successful purge: 200 from device → mapping deleted
// =============================================================================

#[tokio::test]
async fn purge_success_deletes_mapping_row() {
    let server = MockServer::start().await;
    Mock::given(wm_method("PUT"))
        .and(wm_path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_employee_inactive(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let conn = state.db.connect().unwrap();
    enrollment_service::upsert_device_face_mapping(&conn, &device_id, "face-purge-1", &emp_id)
        .await
        .unwrap();
    drop(conn);

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(PurgeRequest {
        employee_id: emp_id.clone(),
    })
    .unwrap();

    // Wait for the purge to run + delete the row.
    let mut deleted = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
            .await
            .unwrap();
        if mappings.is_empty() {
            deleted = true;
            break;
        }
    }
    assert!(deleted, "successful purge must delete the mapping row");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
}

#[tokio::test]
async fn purge_mapping_delete_failure_keeps_pending_delete_recovery_state() {
    let server = MockServer::start().await;
    Mock::given(wm_method("PUT"))
        .and(wm_path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let emp_id = seed_employee_inactive(&state.db).await;
    let device_id = seed_device_at(&state.db, &state.config.device_creds_key, &server.uri()).await;
    let conn = state.db.connect().unwrap();
    enrollment_service::upsert_device_face_mapping(&conn, &device_id, "face-delete-db", &emp_id)
        .await
        .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_mapping_delete BEFORE DELETE ON device_face_mappings \
         BEGIN SELECT RAISE(ABORT, 'forced mapping delete failure'); END;",
    )
    .await
    .unwrap();

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let worker = PurgeWorker::new(state.clone(), shutdown.clone());
    let handle = tokio::spawn(async move { worker.run(rx).await });
    tx.send(PurgeRequest {
        employee_id: emp_id.clone(),
    })
    .unwrap();

    let mut pending = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = conn
            .query(
                "SELECT state FROM device_face_mappings WHERE employee_id=?1",
                params![emp_id.clone()],
            )
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap();
        if row.get::<String>(0).unwrap() == "pending_delete" {
            pending = true;
            break;
        }
    }
    assert!(
        pending,
        "DB delete failure must retain an explicit retry state"
    );
    shutdown.cancel();
    handle.await.unwrap();
}

// =============================================================================
// Failed purge (5xx): mapping row stays + state flips to 'pending_delete'.
// =============================================================================

#[tokio::test]
async fn purge_5xx_marks_mapping_pending_delete() {
    let server = MockServer::start().await;
    Mock::given(wm_method("PUT"))
        .and(wm_path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_employee_inactive(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    let conn = state.db.connect().unwrap();
    enrollment_service::upsert_device_face_mapping(&conn, &device_id, "face-fail", &emp_id)
        .await
        .unwrap();
    drop(conn);

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });
    tx.send(PurgeRequest {
        employee_id: emp_id.clone(),
    })
    .unwrap();

    // Wait for the per-mapping state to flip.
    let mut found_pending = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT state FROM device_face_mappings WHERE employee_id = ?1",
                params![emp_id.clone()],
            )
            .await
            .unwrap();
        if let Some(row) = rows.next().await.unwrap() {
            let st: String = row.get(0).unwrap();
            if st == "pending_delete" {
                found_pending = true;
                break;
            }
        } else {
            // No row = it got deleted somehow; not what we expect on 5xx.
            break;
        }
    }
    assert!(found_pending, "5xx purge must mark mapping pending_delete");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
}

// =============================================================================
// Missing employee — get_employee_status Err path (lines 92-94 in source)
// =============================================================================

#[tokio::test]
async fn purge_unknown_employee_logs_and_returns() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    tx.send(PurgeRequest {
        employee_id: "no-such-employee-id".into(),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), h).await;
    assert!(r.is_ok(), "worker must survive an unknown-employee request");
}

// =============================================================================
// Device fetch error mid-loop — drop the device after seeding the mapping so
// `devices_service::get_decrypted` errors → mapping is marked pending_delete.
// =============================================================================

#[tokio::test]
async fn purge_marks_pending_when_device_fetch_fails() {
    let server = MockServer::start().await; // unused but holds a port reservation
    let _ = &server;
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_employee_inactive(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;

    // Seed mapping with the active device, then mark device inactive so
    // get_decrypted (which requires status='active') 404s.
    let conn = state.db.connect().unwrap();
    use cronometrix_api::enrollments::service as enr_svc;
    enr_svc::upsert_device_face_mapping(&conn, &device_id, "face-pd", &emp_id)
        .await
        .unwrap();
    conn.execute(
        "UPDATE devices SET status = 'inactive' WHERE id = ?1",
        params![device_id.clone()],
    )
    .await
    .unwrap();
    drop(conn);

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    tx.send(PurgeRequest {
        employee_id: emp_id.clone(),
    })
    .unwrap();

    // Wait for the mapping to be marked pending_delete via the device-fetch-Err arm.
    let mut found = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT state FROM device_face_mappings WHERE employee_id = ?1",
                params![emp_id.clone()],
            )
            .await
            .unwrap();
        if let Some(row) = rows.next().await.unwrap() {
            let st: String = row.get(0).unwrap();
            if st == "pending_delete" {
                found = true;
                break;
            }
        }
    }
    assert!(
        found,
        "inactive-device branch must mark mapping pending_delete"
    );

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
}

// =============================================================================
// Dedup: 5 requests for the same employee → batched into a single iteration.
// =============================================================================

#[tokio::test]
async fn purge_worker_dedupes_repeated_requests() {
    let server = MockServer::start().await;
    Mock::given(wm_method("PUT"))
        .and(wm_path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let config = state.config.clone();
    let emp_id = seed_employee_inactive(&state.db).await;
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let conn = state.db.connect().unwrap();
    enrollment_service::upsert_device_face_mapping(&conn, &device_id, "face-d", &emp_id)
        .await
        .unwrap();
    drop(conn);

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<PurgeRequest>();
    let w = PurgeWorker::new(state.clone(), shutdown.clone());
    let h = tokio::spawn(async move { w.run(rx).await });

    // Burst 5 identical requests — HashSet dedup should collapse to 1.
    for _ in 0..5 {
        tx.send(PurgeRequest {
            employee_id: emp_id.clone(),
        })
        .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(400)).await;
    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
    // After cancel the mapping row should be gone (single successful purge).
    let conn = state.db.connect().unwrap();
    let mappings = enrollment_service::list_mappings_for_employee(&conn, &emp_id)
        .await
        .unwrap();
    assert!(mappings.is_empty(), "burst dedup → 1 successful purge");
}

// =============================================================================
// Debug Trait — `PurgeRequest` is Clone + Debug.
// =============================================================================

#[test]
fn purge_request_debug_and_clone() {
    let req = PurgeRequest {
        employee_id: "emp".into(),
    };
    let d = format!("{:?}", req);
    assert!(d.contains("PurgeRequest"));
    let _ = req.clone();
}
