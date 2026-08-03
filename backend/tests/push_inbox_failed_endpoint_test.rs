//! `GET /api/v1/devices/push-inbox/failed` — the C-10 part 2 dead-letter
//! view. RBAC (min role supervisor) and the "never returns the raw body"
//! contract are both load-bearing: the raw body can carry a face JPEG, and
//! this is a diagnostics route, not a biometrics one.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use cronometrix_api::{auth, devices};
use http_body_util::BodyExt;
use libsql::params;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use common::{test_access_token, test_device_creds_key, TEST_JWT_SECRET};

fn make_state(db: libsql::Database) -> (AppState, tempfile::TempDir) {
    let config = Arc::new(Config {
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
        device_push_base_url: String::new(),
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

fn build_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route(
            "/devices/push-inbox/failed",
            get(devices::handlers::list_push_inbox_failed),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));
    Router::new().nest("/api/v1", routes).with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn admin_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "admin")
}

fn supervisor_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "supervisor")
}

fn viewer_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "viewer")
}

async fn seed_device(conn: &libsql::Connection, device_id: &str) {
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at, \
          ingest_mode, push_token) \
         VALUES (?1, 'Endpoint Device', '10.0.0.30', 80, 'http', 'admin', 'ciphertext', 'entry', \
                 0, 'offline', 'active', 1, unixepoch(), unixepoch(), 'push', 'tok')",
        params![device_id],
    )
    .await
    .unwrap();
}

/// Seed a `device_push_inbox` row directly in the `failed` state, WITH a
/// distinctive body — so a test can assert the body never reaches the
/// response.
async fn seed_failed_row(conn: &libsql::Connection, device_id: &str, last_error: &str) -> String {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO device_push_inbox \
           (id, device_id, content_type, body, body_sha256, received_at, status, attempts, \
            last_error, processed_at) \
         VALUES (?1, ?2, 'application/json', ?3, 'sha', unixepoch(), 'failed', 3, ?4, unixepoch())",
        params![
            id.clone(),
            device_id,
            b"SECRET_FACE_JPEG_BYTES_MUST_NEVER_LEAK".to_vec(),
            last_error,
        ],
    )
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn viewer_is_forbidden() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/devices/push-inbox/failed")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn anonymous_is_unauthorized() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/devices/push-inbox/failed")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// The core contract: supervisor can read it, the expected fields are
/// present, and the raw `body` — which can carry a face JPEG — never
/// appears anywhere in the response.
#[tokio::test]
async fn supervisor_reads_the_dead_letter_list_without_the_raw_body() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-endpoint-failed";
    seed_device(&conn, device_id).await;
    let inbox_id = seed_failed_row(&conn, device_id, "payload could not be parsed").await;
    drop(conn);
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/devices/push-inbox/failed")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", supervisor_token()),
        )
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let raw_text = String::from_utf8_lossy(&bytes);

    // The dead giveaway that the body leaked would be its bytes appearing
    // anywhere in the wire payload — assert on the raw text, not just the
    // parsed JSON shape, so a future field addition can't reintroduce it.
    assert!(
        !raw_text.contains("SECRET_FACE_JPEG_BYTES_MUST_NEVER_LEAK"),
        "the raw push body must never appear in the dead-letter response"
    );
    assert!(
        !raw_text.contains("\"body\""),
        "no field named body may appear in the response at all"
    );

    let json: Value = serde_json::from_str(&raw_text).unwrap();
    let entries = json.as_array().expect("response must be a JSON array");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry["id"], inbox_id);
    assert_eq!(entry["device_id"], device_id);
    assert_eq!(entry["attempts"], 3);
    assert_eq!(entry["last_error"], "payload could not be parsed");
    assert!(
        entry["received_at"].is_string(),
        "received_at must be ISO 8601, not a raw epoch"
    );
}

#[tokio::test]
async fn admin_can_also_read_it() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-endpoint-admin";
    seed_device(&conn, device_id).await;
    seed_failed_row(&conn, device_id, "device no longer exists").await;
    drop(conn);
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/devices/push-inbox/failed")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json.as_array().unwrap().len(), 1);
}

/// Only `failed` rows are dead letters — a `pending` or `done` row must not
/// appear in this view.
#[tokio::test]
async fn pending_and_done_rows_are_excluded() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-endpoint-mixed";
    seed_device(&conn, device_id).await;
    conn.execute(
        "INSERT INTO device_push_inbox \
           (id, device_id, content_type, body, body_sha256, received_at, status, attempts) \
         VALUES (?1, ?2, 'application/json', ?3, 'sha', unixepoch(), 'pending', 0)",
        params![
            Uuid::new_v4().to_string(),
            device_id,
            b"pending body".to_vec()
        ],
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO device_push_inbox \
           (id, device_id, content_type, body, body_sha256, received_at, status, attempts, processed_at) \
         VALUES (?1, ?2, 'application/json', ?3, 'sha', unixepoch(), 'done', 0, unixepoch())",
        params![Uuid::new_v4().to_string(), device_id, b"done body".to_vec()],
    )
    .await
    .unwrap();
    seed_failed_row(&conn, device_id, "genuinely failed").await;
    drop(conn);
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/devices/push-inbox/failed")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", supervisor_token()),
        )
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 1, "only the failed row belongs in this view");
    assert_eq!(entries[0]["last_error"], "genuinely failed");
}
