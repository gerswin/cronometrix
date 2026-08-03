//! C-10, part 1: the raw push body must be durable in `device_push_inbox`
//! before the handler answers the reader. See `backend/src/devices/push.rs`
//! for why an unconditional 200 without that guarantee silently drops
//! attendance markings.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use libsql::params;
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use common::{test_device_creds_key, TEST_JWT_SECRET};

const DEVICE_ID: &str = "dev-push-inbox";
const TOKEN: &str = "9a1c3e5b7d2f4a8c0e6b9d1f3a5c7e91";

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
        device_push_base_url: String::new(),
    })
}

fn app(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/v1/devices/{device_id}/push/{token}",
            post(cronometrix_api::devices::push::receive_push),
        )
        .with_state(state)
}

async fn seed(conn: &libsql::Connection, token: Option<&str>) {
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at, \
          ingest_mode, push_token) \
         VALUES (?1, 'Push Inbox Device', '10.0.0.10', 80, 'http', 'admin', 'ciphertext', 'entry', \
                 0, 'offline', 'active', 1, unixepoch(), unixepoch(), 'push', ?2)",
        params![DEVICE_ID, token],
    )
    .await
    .unwrap();
}

fn recognition_event() -> String {
    r#"{"dateTime":"2026-08-02T10:15:30-04:00","eventType":"AccessControllerEvent",
        "AccessControllerEvent":{"majorEventType":5,"subEventType":75,
        "name":"Gerswin Pineda","employeeNoString":"EMP-PUSH-INBOX","attendanceStatus":"checkIn"}}"#
        .to_string()
}

async fn post_event(state: &AppState, token: &str, body: String) -> StatusCode {
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/devices/{DEVICE_ID}/push/{token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    app(state.clone()).oneshot(request).await.unwrap().status()
}

async fn inbox_row_count(state: &AppState) -> i64 {
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM device_push_inbox WHERE device_id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

/// C-10: the raw body has to be on disk BEFORE the handler acknowledges it.
/// Before this change the handler processed, swallowed errors, and answered
/// 200 regardless — a busy write queue lost the event with no trace.
#[tokio::test]
async fn a_push_is_persisted_before_it_is_acknowledged() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let body = recognition_event();
    let status = post_event(&state, TOKEN, body.clone()).await;
    assert_eq!(status, StatusCode::OK);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT device_id, content_type, body, body_sha256, status, attempts \
             FROM device_push_inbox WHERE device_id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("inbox row must exist");

    let device_id: String = row.get(0).unwrap();
    let content_type: String = row.get(1).unwrap();
    let stored_body: Vec<u8> = row.get(2).unwrap();
    let stored_sha256: String = row.get(3).unwrap();
    let status_col: String = row.get(4).unwrap();
    let attempts: i64 = row.get(5).unwrap();

    assert_eq!(device_id, DEVICE_ID);
    assert_eq!(content_type, "application/json");
    // Byte-for-byte: the inbox must hold the exact bytes the reader sent, not
    // a lossily re-encoded String.
    assert_eq!(stored_body, body.as_bytes());
    assert_eq!(stored_sha256, format!("{:x}", Sha256::digest(body.as_bytes())));
    assert_eq!(status_col, "pending");
    assert_eq!(attempts, 0);

    assert_eq!(inbox_row_count(&state).await, 1);
}

/// Two pushes with an IDENTICAL body produce TWO rows. The inbox must not
/// deduplicate: heartbeats routinely repeat their body verbatim, and a false
/// dedup positive would silently drop a real marking — the exact failure this
/// table exists to prevent. Deduplication already lives downstream, in
/// `bucket_30s` on `attendance_events`.
#[tokio::test]
async fn identical_bodies_are_both_stored() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let body = recognition_event();
    assert_eq!(post_event(&state, TOKEN, body.clone()).await, StatusCode::OK);
    assert_eq!(post_event(&state, TOKEN, body).await, StatusCode::OK);

    assert_eq!(inbox_row_count(&state).await, 2);
}

/// A bad token must not write anything — not to `attendance_events`, and not
/// to the durability inbox either.
#[tokio::test]
async fn an_unauthorized_push_stores_nothing() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    assert_eq!(
        post_event(&state, "wrong-token", recognition_event()).await,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(inbox_row_count(&state).await, 0);
}
