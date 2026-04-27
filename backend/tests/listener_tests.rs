//! Integration tests for the alertStream listener + parser + persist pipeline
//! (Plan 02-03 Task 1).
//!
//! These tests drive canned multipart bytes through a tokio TCP mock that
//! mimics a Hikvision DS-K1T3xx, call `connect_and_stream` against it, and
//! assert the resulting DB + filesystem state.

mod common;

use std::sync::Arc;
use std::sync::Mutex;

use cronometrix_api::config::Config;
use cronometrix_api::isapi::stream::{connect_and_stream, DeviceConfig};
use cronometrix_api::state::AppState;
use libsql::params;
use tempfile::TempDir;
use uuid::Uuid;

use common::mock_hikvision::{
    spawn_mock_hikvision_401, spawn_mock_hikvision_digest, spawn_mock_hikvision_plain,
};
use common::{build_multipart_fixture, k1t341_event_xml, test_device_creds_key, MINI_JPEG};

// Serialize tests that mutate CRONOMETRIX_EVENTS_ROOT — std::env is process-
// global and integration tests run in the same process.
static ENV_GUARD: Mutex<()> = Mutex::new(());

struct EventsRootGuard<'a> {
    _lock: std::sync::MutexGuard<'a, ()>,
    _dir: TempDir,
    prev: Option<String>,
}

impl<'a> EventsRootGuard<'a> {
    fn new() -> Self {
        let lock = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().expect("temp dir");
        let prev = std::env::var("CRONOMETRIX_EVENTS_ROOT").ok();
        std::env::set_var("CRONOMETRIX_EVENTS_ROOT", dir.path());
        Self {
            _lock: lock,
            _dir: dir,
            prev,
        }
    }
}

impl<'a> Drop for EventsRootGuard<'a> {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => std::env::set_var("CRONOMETRIX_EVENTS_ROOT", v),
            None => std::env::remove_var("CRONOMETRIX_EVENTS_ROOT"),
        }
    }
}

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: "test-secret-key-at-least-32-characters-long!!".into(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
    })
}

fn make_state(db: libsql::Database) -> AppState {
    AppState {
        db: Arc::new(db),
        config: make_config(),
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
    }
}

/// Seed an active device row. The port value stored here is only for display —
/// the stream layer uses `cfg.base_url` directly, so we can use any valid
/// port. We derive from the id hash to avoid collisions with the partial
/// UNIQUE(ip, port) index on active rows.
async fn seed_device(conn: &libsql::Connection, id: &str, _hint_port: u16) {
    let hash: u32 = id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, b| acc.wrapping_mul(131).wrapping_add(*b as u32));
    let port = 1024 + (hash % 60000) as i64;
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, \
         created_at, updated_at) \
         VALUES (?1, ?2, '127.0.0.1', ?3, 'http', 'admin', 'ciphertext', \
         'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![
            id.to_string(),
            format!("dev-{}", id),
            port
        ],
    )
    .await
    .expect("seed device");
}

async fn seed_employee(conn: &libsql::Connection, emp_id: &str, emp_code: &str) {
    let dept_id = format!("dept-{}", emp_id);
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '09:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept {}", emp_id)],
    )
    .await
    .expect("seed dept");
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        params![
            emp_id.to_string(),
            emp_code.to_string(),
            format!("Emp {}", emp_id),
            dept_id
        ],
    )
    .await
    .expect("seed employee");
}

async fn seed_face_mapping(conn: &libsql::Connection, device_id: &str, face_id: &str, emp_id: &str) {
    conn.execute(
        "INSERT INTO device_face_mappings (id, device_id, face_id, employee_id, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 1, unixepoch(), unixepoch())",
        params![
            Uuid::new_v4().to_string(),
            device_id.to_string(),
            face_id.to_string(),
            emp_id.to_string()
        ],
    )
    .await
    .expect("seed face mapping");
}

fn device_cfg(id: &str, addr: std::net::SocketAddr) -> DeviceConfig {
    DeviceConfig {
        id: id.to_string(),
        base_url: format!("http://{}", addr),
        username: "admin".into(),
        password: "secret".into(),
        direction_default: "entry".into(),
        allow_insecure_tls: false,
    }
}

#[tokio::test]
async fn connect_and_stream_persists_one_event() {
    let _guard = EventsRootGuard::new();
    let db = common::test_db().await;

    {
        let conn = db.connect().unwrap();
        seed_device(&conn, "d1", 0).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_face_mapping(&conn, "d1", "42", "e1").await;
    }

    let body = build_multipart_fixture(&k1t341_event_xml(), Some(MINI_JPEG));
    let addr = spawn_mock_hikvision_plain(body, "MIME_boundary").await;

    let state = make_state(db);
    let cfg = device_cfg("d1", addr);

    connect_and_stream(&cfg, &state)
        .await
        .expect("stream should complete cleanly");

    // Assert exactly one row persisted with the expected employee_id
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT employee_id, direction, raw_xml, photo_path, is_unknown FROM attendance_events WHERE device_id = 'd1'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("must have at least one row");
    let employee_id: Option<String> = row.get(0).unwrap();
    let direction: String = row.get(1).unwrap();
    let raw_xml: String = row.get(2).unwrap();
    let photo_path: Option<String> = row.get(3).unwrap();
    let is_unknown: i64 = row.get(4).unwrap();

    assert_eq!(employee_id.as_deref(), Some("e1"));
    assert_eq!(direction, "entry");
    assert!(raw_xml.contains("<EventNotificationAlert"));
    assert_eq!(is_unknown, 0);
    let relpath = photo_path.expect("photo_path populated");
    let root = cronometrix_api::events::service::events_root();
    let on_disk = root.join(&relpath);
    assert!(on_disk.exists(), "photo jpeg must be on disk at {:?}", on_disk);

    // No additional rows.
    let next = rows.next().await.unwrap();
    assert!(next.is_none(), "should persist exactly one event");
}

#[tokio::test]
async fn heartbeat_updates_last_seen_at_and_does_not_persist() {
    let _guard = EventsRootGuard::new();
    let db = common::test_db().await;

    {
        let conn = db.connect().unwrap();
        seed_device(&conn, "d-hb", 0).await;
    }

    let body = build_multipart_fixture(&common::heartbeat_event_xml(), None);
    let addr = spawn_mock_hikvision_plain(body, "MIME_boundary").await;

    let state = make_state(db);
    let cfg = device_cfg("d-hb", addr);
    connect_and_stream(&cfg, &state).await.expect("stream ok");

    let conn = state.db.connect().unwrap();
    let count_row = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE device_id = 'd-hb'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let count: i64 = count_row.get(0).unwrap();
    assert_eq!(count, 0, "heartbeat must not persist an attendance event");

    let ls_row = conn
        .query(
            "SELECT last_seen_at FROM devices WHERE id = 'd-hb'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let last_seen: Option<i64> = ls_row.get(0).unwrap();
    assert!(
        last_seen.is_some() && last_seen.unwrap() > 0,
        "heartbeat must refresh last_seen_at"
    );
}

#[tokio::test]
async fn unknown_face_persists_with_is_unknown() {
    let _guard = EventsRootGuard::new();
    let db = common::test_db().await;

    {
        let conn = db.connect().unwrap();
        seed_device(&conn, "d-unk", 0).await;
    }

    let body = build_multipart_fixture(&common::unknown_face_event_xml(), Some(MINI_JPEG));
    let addr = spawn_mock_hikvision_plain(body, "MIME_boundary").await;

    let state = make_state(db);
    let cfg = device_cfg("d-unk", addr);
    connect_and_stream(&cfg, &state).await.expect("stream ok");

    let conn = state.db.connect().unwrap();
    let row = conn
        .query(
            "SELECT employee_id, is_unknown, face_id FROM attendance_events WHERE device_id = 'd-unk'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .expect("one unknown event row");
    let employee_id: Option<String> = row.get(0).unwrap();
    let is_unknown: i64 = row.get(1).unwrap();
    let face_id: Option<String> = row.get(2).unwrap();
    assert!(employee_id.is_none());
    assert_eq!(is_unknown, 1);
    assert_eq!(face_id.as_deref(), Some("9999"));
}

#[tokio::test]
async fn second_identical_event_deduplicates() {
    let _guard = EventsRootGuard::new();
    let db = common::test_db().await;
    {
        let conn = db.connect().unwrap();
        seed_device(&conn, "d-dup", 0).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_face_mapping(&conn, "d-dup", "42", "e1").await;
    }

    let body = build_multipart_fixture(&k1t341_event_xml(), Some(MINI_JPEG));

    // Two sequential connections, each serving the same fixture. The second
    // must hit the dedup branch (same employee_id, device_id, direction, bucket_30s).
    let addr1 = spawn_mock_hikvision_plain(body.clone(), "MIME_boundary").await;
    let state = make_state(db);
    let cfg1 = device_cfg("d-dup", addr1);
    connect_and_stream(&cfg1, &state).await.expect("first stream");

    let addr2 = spawn_mock_hikvision_plain(body, "MIME_boundary").await;
    let cfg2 = device_cfg("d-dup", addr2);
    connect_and_stream(&cfg2, &state).await.expect("second stream");

    let conn = state.db.connect().unwrap();
    let row = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE device_id = 'd-dup'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.get(0).unwrap();
    assert_eq!(count, 1, "dedup must keep row count at 1 on identical replay");
}

#[tokio::test]
async fn connect_and_stream_fails_cleanly_on_401() {
    let _guard = EventsRootGuard::new();
    let db = common::test_db().await;
    {
        let conn = db.connect().unwrap();
        seed_device(&conn, "d-401", 0).await;
    }

    let addr = spawn_mock_hikvision_401("admin").await;
    let state = make_state(db);
    let cfg = device_cfg("d-401", addr);
    let result = connect_and_stream(&cfg, &state).await;
    assert!(result.is_err(), "401 must bubble up as Err");

    // connection_state must remain 'offline' (or at worst not be flipped to
    // online — our implementation only flips to online AFTER a successful
    // status check).
    let conn = state.db.connect().unwrap();
    let row = conn
        .query(
            "SELECT connection_state FROM devices WHERE id = 'd-401'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let cs: String = row.get(0).unwrap();
    assert_eq!(cs, "offline");
}

#[tokio::test]
async fn digest_auth_mock_serves_body_after_challenge() {
    // Sanity check that the digest mock fixture completes the cycle. This
    // does NOT call connect_and_stream because diqwest's retry semantics
    // would couple the test to crate internals; we instead use reqwest +
    // diqwest directly here to exercise the fixture.
    let _guard = EventsRootGuard::new();

    let body = build_multipart_fixture(&k1t341_event_xml(), Some(MINI_JPEG));
    let addr = spawn_mock_hikvision_digest(body.clone(), "MIME_boundary", "admin", "secret").await;

    use diqwest::WithDigestAuth;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{}/", addr))
        .send_digest_auth(("admin", "secret"))
        .await
        .expect("digest auth completes");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let got = resp.bytes().await.unwrap();
    assert_eq!(got.as_ref(), body.as_slice());
}
