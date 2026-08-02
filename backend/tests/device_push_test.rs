//! The inbound device webhook is the only route that writes attendance data
//! without a JWT. Its whole defence is a per-device secret in the URL path, so
//! these tests are about who gets rejected as much as what gets stored.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use libsql::params;
use tower::ServiceExt;

use common::{test_device_creds_key, TEST_JWT_SECRET};

const DEVICE_ID: &str = "dev-push";
const TOKEN: &str = "7f3c9a1e5b2d4f8a0c6e9b1d3f5a7c90";

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

async fn seed(conn: &libsql::Connection, ingest_mode: &str, token: Option<&str>) {
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at, \
          ingest_mode, push_token) \
         VALUES (?1, 'Push Device', '10.0.0.9', 80, 'http', 'admin', 'ciphertext', 'entry', \
                 0, 'offline', 'active', 1, unixepoch(), unixepoch(), ?2, ?3)",
        params![DEVICE_ID, ingest_mode, token],
    )
    .await
    .unwrap();
}

/// Verbatim shape of a DS-K1T341CMFW recognition event.
fn recognition_event() -> String {
    r#"{"dateTime":"2026-08-02T10:15:30-04:00","eventType":"AccessControllerEvent",
        "AccessControllerEvent":{"majorEventType":5,"subEventType":75,
        "name":"Gerswin Pineda","employeeNoString":"EMP-PUSH","attendanceStatus":"checkIn"}}"#
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

async fn event_count(state: &AppState) -> i64 {
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE device_id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

#[tokio::test]
async fn push_persists_a_marking_and_resolves_the_employee() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES ('d-push', 'Push', 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES ('emp-push', 'EMP-PUSH', 'Gerswin Pineda', 'd-push', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    assert_eq!(
        post_event(&state, TOKEN, recognition_event()).await,
        StatusCode::OK
    );

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT employee_id, direction, is_unknown FROM attendance_events WHERE device_id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("marking persisted");
    assert_eq!(
        row.get::<Option<String>>(0).unwrap().as_deref(),
        Some("emp-push")
    );
    // checkIn from the payload wins over the device's `entry` default.
    assert_eq!(row.get::<String>(1).unwrap(), "entry");
    assert_eq!(row.get::<i64>(2).unwrap(), 0);

    // The webhook is a push-mode device's only liveness signal.
    let mut rows = conn
        .query(
            "SELECT connection_state FROM devices WHERE id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    let state_now: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(state_now, "online");
}

/// A wrong token must not write, and must not reveal whether the device exists.
#[tokio::test]
async fn push_rejects_a_bad_token_without_persisting() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    for attempt in [
        "wrong",
        // Shares a prefix with the real token: a short-circuiting compare would
        // leak that through timing.
        "7f3c9a1e5b2d4f8a0c6e9b1d3f5a7c91",
        // Right length, wrong content — rules out a length-only check.
        "0000000000000000000000000000000f",
    ] {
        assert_eq!(
            post_event(&state, attempt, recognition_event()).await,
            StatusCode::UNAUTHORIZED,
            "token {attempt:?} must be rejected"
        );
    }

    // An empty token cannot match the route at all, so this is a 404 from the
    // router rather than a 401 from the handler. Still reveals nothing about
    // whether the device exists, and still writes nothing.
    assert_eq!(
        post_event(&state, "", recognition_event()).await,
        StatusCode::NOT_FOUND
    );

    assert_eq!(event_count(&state).await, 0);
}

/// A stream-mode device must not accept pushes: honouring both transports would
/// store every marking twice, and the dedup index cannot catch it for unknown
/// faces because it keys on a NULL employee_id.
#[tokio::test]
async fn push_is_refused_for_a_stream_mode_device() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "stream", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    assert_eq!(
        post_event(&state, TOKEN, recognition_event()).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(event_count(&state).await, 0);
}

/// A device switched to push before a token was minted must not be a wildcard.
#[tokio::test]
async fn push_is_refused_when_the_device_has_no_token() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", None).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    assert_eq!(
        post_event(&state, "anything", recognition_event()).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(event_count(&state).await, 0);
}

/// Door and tamper notifications share the event envelope but name nobody.
///
/// The device must still get 200 — not merely "not an error". DS-K1T341CMFW
/// treats a 204 as non-delivery and re-sends the identical event forever, so the
/// queue head never advances and the face recognitions behind it never arrive.
#[tokio::test]
async fn push_accepts_but_does_not_store_an_event_without_an_identity() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let door = r#"{"dateTime":"2026-08-02T10:15:29-04:00","eventType":"AccessControllerEvent",
        "AccessControllerEvent":{"majorEventType":5,"subEventType":22,"doorNo":1,
        "attendanceStatus":"undefined"}}"#
        .to_string();

    assert_eq!(
        post_event(&state, TOKEN, door).await,
        StatusCode::OK
    );
    assert_eq!(event_count(&state).await, 0);
}

/// Garbage must not 500: an error response makes the reader re-queue and retry
/// the same payload indefinitely.
#[tokio::test]
async fn push_swallows_an_unparseable_payload() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    assert_eq!(
        post_event(&state, TOKEN, "{not json at all".to_string()).await,
        StatusCode::OK
    );
    assert_eq!(event_count(&state).await, 0);
}

/// The reader posts the captured face alongside the alert once picture upload is
/// provisioned; the JPEG must land on disk, not be dropped.
#[tokio::test]
async fn push_stores_the_captured_face_from_a_multipart_body() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let body = [
        b"--BND\r\nContent-Type: application/json\r\n\r\n".as_slice(),
        recognition_event().as_bytes(),
        b"\r\n--BND\r\nContent-Type: image/jpeg\r\n\r\n",
        common::MINI_JPEG,
        b"\r\n--BND--\r\n",
    ]
    .concat();

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/devices/{DEVICE_ID}/push/{TOKEN}"))
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=BND")
        .body(Body::from(body))
        .unwrap();
    let response = app(state.clone()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT photo_path FROM attendance_events WHERE device_id = ?1",
            params![DEVICE_ID],
        )
        .await
        .unwrap();
    let photo: Option<String> = rows.next().await.unwrap().expect("row").get(0).unwrap();
    let photo = photo.expect("the captured face must be stored");
    assert!(
        state.paths.events_root.join(&photo).exists(),
        "photo must exist on disk at {photo}"
    );
}

/// The acknowledgement status is load-bearing, so it is asserted exactly rather
/// than through `is_success()`.
///
/// DS-K1T341CMFW advances its send queue only on a 200. Measured on hardware:
/// answering 204 produced the same `serialNo` on four consecutive pushes, while
/// 200 advanced it every time. A 204 therefore does not merely look untidy — it
/// wedges the device permanently on whatever event happens to be at the head.
#[tokio::test]
async fn push_acknowledges_with_200_so_the_device_queue_advances() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "push", Some(TOKEN)).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    // An accepted marking, an ignored door event and unparseable garbage must
    // all acknowledge identically: any of them left unacknowledged stalls the
    // queue just as effectively.
    for body in [
        recognition_event(),
        r#"{"eventType":"AccessControllerEvent","AccessControllerEvent":{"subEventType":22}}"#
            .to_string(),
        "{not json at all".to_string(),
    ] {
        assert_eq!(
            post_event(&state, TOKEN, body).await,
            StatusCode::OK,
            "every accepted or ignored push must answer exactly 200"
        );
    }
}
