//! Extra integration tests for `events::handlers`. Targets the 55.68% baseline
//! gap from Plan 03 (08-04A bucket row 12). Existing `event_tests.rs` covers
//! list/get/photo happy paths and most filters; this file targets:
//!   - `events_stream` (SSE): 401 invalid token, 500 / Internal when broadcast
//!     channel is missing, happy path with broadcast subscriber smoke
//!   - `get_event_photo` traversal rejection (defense in depth)
//!   - `get_event_photo` 404 when on-disk file missing

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::events;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use libsql::params;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn make_state(db: libsql::Database) -> (AppState, TempDir) {
    let config = Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

fn build_app(state: AppState) -> Router {
    let viewer_routes = Router::new()
        .route("/events", get(events::handlers::list_events))
        .route("/events/{id}", get(events::handlers::get_event))
        .route("/events/{id}/photo", get(events::handlers::get_event_photo))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));
    // events_stream is registered without auth middleware (mirrors main.rs;
    // the handler validates the JWT itself from query param).
    let public_routes = Router::new().route("/events/stream", get(events::handlers::events_stream));
    Router::new()
        .nest("/api/v1", viewer_routes.merge(public_routes))
        .with_state(state)
}

async fn seed_device(conn: &libsql::Connection, id: &str) {
    let port = 8000 + (id.len() as i64);
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?2, '10.0.0.1', ?3, 'https', 'admin', 'ct', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), format!("dev-{}", id), port],
    )
    .await
    .unwrap();
}

async fn seed_event_with_photo(conn: &libsql::Connection, device_id: &str, photo_path: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let captured_at: i64 = 1_700_000_000;
    let bucket = captured_at / 30;
    conn.execute(
        "INSERT INTO attendance_events (id, employee_id, device_id, direction, captured_at, \
         bucket_30s, is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
         VALUES (?1, NULL, ?2, 'entry', ?3, ?4, 0, NULL, NULL, '<x/>', ?5, unixepoch())",
        params![
            id.clone(),
            device_id.to_string(),
            captured_at,
            bucket,
            photo_path.to_string()
        ],
    )
    .await
    .unwrap();
    id
}

fn viewer_token() -> String {
    common::test_access_token(&Uuid::new_v4().to_string(), "viewer")
}

// =============================================================================
// SSE stream: events_stream auth + initialisation branches
// =============================================================================

#[tokio::test]
async fn events_stream_401_when_token_invalid() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/stream?token=not-a-jwt")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn events_stream_401_when_token_missing() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    // Missing query param `token` → axum's Query extractor fails; this surfaces
    // as 400 (Bad Request) from axum-the-framework before our handler runs.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/stream")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(
        resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "missing token should be a 4xx, got {:?}",
        resp.status()
    );
}

#[tokio::test]
async fn events_stream_500_when_broadcast_channel_missing() {
    // AppState built via test_state_with_tmpdir has event_broadcast = None;
    // events_stream should return Internal (500) — matches the handler's
    // ok_or_else(|| AppError::Internal(...)).
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    // Forge a valid access JWT for any user.
    let token = common::test_access_token(&Uuid::new_v4().to_string(), "viewer");

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/events/stream?token={}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "no broadcast channel wired → Internal error"
    );
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "INTERNAL_ERROR");
}

#[tokio::test]
async fn events_stream_returns_sse_when_broadcast_present() {
    // Wire a broadcast channel into AppState then hit the stream handler
    // and read the response headers (text/event-stream).
    let db = common::test_db().await;
    let (mut state, _tmp) = make_state(db);
    let (tx, _rx) = tokio::sync::broadcast::channel(8);
    state.event_broadcast = Some(tx);
    let app = build_app(state);

    let token = common::test_access_token(&Uuid::new_v4().to_string(), "viewer");
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/events/stream?token={}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        ct.starts_with("text/event-stream"),
        "Content-Type should be SSE, got: {ct}"
    );
}

// =============================================================================
// get_event_photo defense-in-depth branches (T-2-06)
// =============================================================================

#[tokio::test]
async fn get_event_photo_404_when_path_traversal_in_db() {
    // Defense-in-depth: even if photo_path was somehow set to `../etc/passwd`,
    // the handler must reject it without reaching the filesystem.
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "dev-trav").await;
    let event_id = seed_event_with_photo(&conn, "dev-trav", "../../etc/passwd").await;

    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/events/{}/photo", event_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_PHOTO_NOT_FOUND");
}

#[tokio::test]
async fn get_event_photo_404_when_path_starts_with_slash() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "dev-abs").await;
    let event_id = seed_event_with_photo(&conn, "dev-abs", "/etc/shadow").await;

    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/events/{}/photo", event_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_PHOTO_NOT_FOUND");
}

#[tokio::test]
async fn get_event_photo_404_when_file_missing_on_disk() {
    // events_root canonicalize fails because the directory doesn't exist
    // → 404, not 500. The directory is only auto-created when something
    // writes through write_photo_atomic; with no events on disk in this
    // tempdir, the canonicalize step rejects.
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "dev-miss").await;
    let event_id = seed_event_with_photo(&conn, "dev-miss", "2026/04/abc.jpg").await;

    let (state, _tmp) = make_state(db);
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/events/{}/photo", event_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
