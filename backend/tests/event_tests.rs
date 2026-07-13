//! Integration tests for the events read API (Plan 02-02 Task 2).
//!
//! Covers:
//!   - GET /api/v1/events: list + filters (employee_id, device_id, from, to)
//!     + pagination clamp + viewer access + unauthenticated 401
//!   - GET /api/v1/events/:id: 404 on unknown id
//!   - GET /api/v1/events/:id/photo: JPEG bytes, content-type, 404 paths,
//!     and path-traversal rejection (T-2-06 defense in depth)
//!
//! These tests use an in-process Router (no network bind) and a per-test
//! `TempDir` injected via `Paths::for_test` (Plan 08-02 D-20). No env-var
//! mutation; tests run cleanly in parallel under cargo nextest / cargo-llvm-cov.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::events;
use cronometrix_api::events::models::NewAttendanceEvent;
use cronometrix_api::events::service::{
    build_sse_payload, persist_attendance_event, publish_sse_event,
};
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use libsql::params;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

/// Build (Router, AppState, TempDir) for the events read API. The TempDir is
/// returned so callers bind it to a local that outlives every assertion (see
/// Pitfall 1 in 08-RESEARCH.md). `state.paths.events_root` points inside the
/// TempDir, so any photo-related assertion uses that path directly.
async fn build_test_app(db: libsql::Database) -> (Router, AppState, TempDir) {
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
    });

    let (state, tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    let viewer_routes = Router::new()
        .route("/events", get(events::handlers::list_events))
        .route("/events/{id}", get(events::handlers::get_event))
        .route("/events/{id}/photo", get(events::handlers::get_event_photo))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let app = Router::new()
        .nest("/api/v1", viewer_routes)
        .with_state(state.clone());
    (app, state, tmp)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn body_to_bytes(body: Body) -> Vec<u8> {
    body.collect().await.unwrap().to_bytes().to_vec()
}

fn viewer_token() -> String {
    common::test_access_token(&uuid::Uuid::new_v4().to_string(), "viewer")
}

fn admin_token() -> String {
    common::test_access_token(&uuid::Uuid::new_v4().to_string(), "admin")
}

// =============================================================================
// Test data seeding helpers
// =============================================================================

async fn seed_device(conn: &libsql::Connection, id: &str, ip: &str, port: i64) {
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'https', 'admin', 'ct', 'entry', 0, 'offline', 'active', 1, \
         unixepoch(), unixepoch())",
        params![id.to_string(), format!("dev-{}", id), ip.to_string(), port],
    )
    .await
    .expect("seed device");
}

async fn seed_employee(conn: &libsql::Connection, id: &str, code: &str) {
    let dept_id = format!("dept-{}", id);
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, \
         shift_end_time, lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '09:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept {}", id)],
    )
    .await
    .expect("seed dept");
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        params![
            id.to_string(),
            code.to_string(),
            format!("Emp {}", id),
            dept_id
        ],
    )
    .await
    .expect("seed employee");
}

/// Insert an event via the production persist helper so the dedup path and
/// photo-write path are exercised the same way as in prod. `events_root` is
/// the per-test tempdir-rooted JPEG directory (Plan 08-02 D-20) — the caller
/// passes `state.paths.events_root` to keep the JPEG inside the test's TempDir.
async fn seed_event(
    conn: &libsql::Connection,
    events_root: &std::path::Path,
    id: &str,
    employee_id: Option<&str>,
    device_id: &str,
    captured_at: i64,
    photo_bytes: Option<Vec<u8>>,
) {
    let ev = NewAttendanceEvent {
        id: id.to_string(),
        employee_id: employee_id.map(str::to_string),
        device_id: device_id.to_string(),
        direction: "entry".to_string(),
        captured_at,
        is_unknown: employee_id.is_none(),
        face_id: Some("42".to_string()),
        employee_no_string: Some("EMP001".to_string()),
        raw_xml: "<EventNotificationAlert/>".to_string(),
        photo_bytes,
    };
    persist_attendance_event(conn, events_root, ev)
        .await
        .expect("persist");
}

fn sse_event(id: &str, employee_id: Option<&str>, with_photo: bool) -> NewAttendanceEvent {
    NewAttendanceEvent {
        id: id.to_string(),
        employee_id: employee_id.map(str::to_string),
        device_id: "d-sse".to_string(),
        direction: "entry".to_string(),
        captured_at: 1_700_000_000,
        is_unknown: employee_id.is_none(),
        face_id: None,
        employee_no_string: None,
        raw_xml: "<EventNotificationAlert/>".to_string(),
        photo_bytes: with_photo.then(|| vec![0xff, 0xd8, 0xff]),
    }
}

#[tokio::test]
async fn sse_payload_enriches_known_employee_department_and_photo_flag() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "d-sse", "10.9.0.1", 8443).await;
    seed_employee(&conn, "e-sse", "EMP-SSE").await;

    let payload = build_sse_payload(&conn, &sse_event("evt-sse", Some("e-sse"), true), true)
        .await
        .expect("enrichment succeeds");

    assert_eq!(payload.employee_id.as_deref(), Some("e-sse"));
    assert_eq!(payload.employee_name.as_deref(), Some("Emp e-sse"));
    assert_eq!(payload.department.as_deref(), Some("Dept e-sse"));
    assert!(payload.has_photo);
}

#[tokio::test]
async fn sse_payload_keeps_unknown_employee_fields_null() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    let payload = build_sse_payload(&conn, &sse_event("evt-unknown", None, false), false)
        .await
        .expect("unknown event still builds a payload");

    assert!(payload.employee_id.is_none());
    assert!(payload.employee_name.is_none());
    assert!(payload.department.is_none());
    assert!(!payload.has_photo);
}

#[tokio::test]
async fn sse_enrichment_failure_broadcasts_fallback_without_undoing_persisted_event() {
    let db = common::test_db().await;
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
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
    });
    let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
    let conn = state.db.connect().unwrap();
    seed_device(&conn, "d-sse", "10.9.0.2", 8443).await;
    seed_employee(&conn, "e-sse", "EMP-SSE").await;
    let event = sse_event("evt-fallback", Some("e-sse"), false);
    let outcome = persist_attendance_event(
        &conn,
        &state.paths.events_root,
        sse_event("evt-fallback", Some("e-sse"), false),
    )
    .await
    .expect("attendance persistence succeeds first");
    assert!(matches!(
        outcome,
        events::models::PersistOutcome::Inserted { .. }
    ));

    conn.execute(
        "ALTER TABLE departments RENAME TO unavailable_departments",
        (),
    )
    .await
    .expect("force enrichment query failure");
    let (tx, mut rx) = tokio::sync::broadcast::channel(4);
    state.event_broadcast = Some(tx);

    publish_sse_event(&state, &event, &None).await;
    let payload = rx.recv().await.expect("fallback is still broadcast");
    assert_eq!(payload.employee_id.as_deref(), Some("e-sse"));
    assert!(payload.employee_name.is_none());
    assert!(payload.department.is_none());

    let persisted: i64 = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE id = 'evt-fallback'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(
        persisted, 1,
        "enrichment failure cannot compensate the insert"
    );
}

// =============================================================================
// GET /api/v1/events — list behaviors
// =============================================================================

#[tokio::test]
async fn list_events_empty_returns_empty_array() {
    let db = common::test_db().await;
    let (app, _state, _tmp) = build_test_app(db).await;
    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 0);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_events_pagination_clamps_limit() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.0.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        // seed 3 events in distinct buckets
        seed_event(&conn, &events_root, "evt-1", Some("e1"), "d1", 1000, None).await;
        seed_event(&conn, &events_root, "evt-2", Some("e1"), "d1", 2000, None).await;
        seed_event(&conn, &events_root, "evt-3", Some("e1"), "d1", 3000, None).await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events?limit=500")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["limit"], 100, "limit must be clamped to 100");
    assert_eq!(body["total"], 3);
}

#[tokio::test]
async fn list_events_filters_by_employee_id() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.1.1", 8443).await;
        seed_employee(&conn, "eA", "EMPA").await;
        seed_employee(&conn, "eB", "EMPB").await;
        seed_event(&conn, &events_root, "evt-1", Some("eA"), "d1", 1000, None).await;
        seed_event(&conn, &events_root, "evt-2", Some("eA"), "d1", 2000, None).await;
        seed_event(&conn, &events_root, "evt-3", Some("eB"), "d1", 3000, None).await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events?employee_id=eA")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 2);
    for item in body["data"].as_array().unwrap() {
        assert_eq!(item["employee_id"], "eA");
    }
}

#[tokio::test]
async fn list_events_filters_by_device_id() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "dA", "10.1.2.1", 8443).await;
        seed_device(&conn, "dB", "10.1.2.2", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_event(&conn, &events_root, "evt-1", Some("e1"), "dA", 1000, None).await;
        seed_event(&conn, &events_root, "evt-2", Some("e1"), "dB", 2000, None).await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events?device_id=dA")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["device_id"], "dA");
}

#[tokio::test]
async fn list_events_filters_by_time_range() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.3.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        // Buckets: 1000 -> bkt 33 ; 2000 -> bkt 66 ; 3000 -> bkt 100
        seed_event(&conn, &events_root, "evt-1", Some("e1"), "d1", 1000, None).await;
        seed_event(&conn, &events_root, "evt-2", Some("e1"), "d1", 2000, None).await;
        seed_event(&conn, &events_root, "evt-3", Some("e1"), "d1", 3000, None).await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events?from=1500&to=2500")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(
        body["total"], 1,
        "only evt-2 has captured_at in [1500, 2500)"
    );
}

#[tokio::test]
async fn list_events_viewer_can_read() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.4.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_event(&conn, &events_root, "evt-1", Some("e1"), "d1", 1000, None).await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "viewer must be able to list events (D-15)"
    );
}

#[tokio::test]
async fn list_events_unauthenticated_401() {
    let db = common::test_db().await;
    let (app, _state, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// GET /api/v1/events/:id — single-event behaviors
// =============================================================================

#[tokio::test]
async fn get_event_by_id_404_if_missing() {
    let db = common::test_db().await;
    let (app, _state, _tmp) = build_test_app(db).await;
    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/does-not-exist")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_NOT_FOUND");
}

// =============================================================================
// GET /api/v1/events/:id/photo — photo streaming + 404 + traversal defense
// =============================================================================

#[tokio::test]
async fn get_event_photo_returns_jpeg_bytes() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    let photo_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, b'J', b'F', b'I', b'F', 0xFF, 0xD9];
    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.5.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_event(
            &conn,
            &events_root,
            "evt-photo-1",
            Some("e1"),
            "d1",
            1_700_000_000,
            Some(photo_bytes.clone()),
        )
        .await;
    }
    // Sanity: file must be on disk under the per-test events root.
    let expected = events_root.join("2023-11-14/evt-photo-1.jpg");
    assert!(
        expected.exists(),
        "photo file must be on disk: {:?}",
        expected
    );

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/evt-photo-1/photo")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg"
    );
    let bytes = body_to_bytes(resp.into_body()).await;
    assert_eq!(bytes, photo_bytes);
}

#[tokio::test]
async fn get_event_photo_404_if_no_photo_path() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.6.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_event(
            &conn,
            &events_root,
            "evt-no-photo",
            Some("e1"),
            "d1",
            1000,
            None,
        )
        .await;
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/evt-no-photo/photo")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_PHOTO_NOT_FOUND");
}

#[tokio::test]
async fn get_event_photo_404_if_file_missing() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;
    let events_root = state.paths.events_root.clone();

    let photo_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.7.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        seed_event(
            &conn,
            &events_root,
            "evt-missing-file",
            Some("e1"),
            "d1",
            1_700_000_000,
            Some(photo_bytes.clone()),
        )
        .await;
    }
    // Delete the on-disk file but keep the DB row pointing at it.
    let victim = events_root.join("2023-11-14/evt-missing-file.jpg");
    assert!(
        victim.exists(),
        "file should have been written by seed_event"
    );
    std::fs::remove_file(&victim).expect("remove photo file");

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/evt-missing-file/photo")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "missing file must be 404 not 500"
    );
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_PHOTO_NOT_FOUND");
}

#[tokio::test]
async fn get_event_photo_rejects_path_traversal() {
    let db = common::test_db().await;
    let (app, state, _tmp) = build_test_app(db).await;

    {
        let conn = state.db.connect().unwrap();
        seed_device(&conn, "d1", "10.1.8.1", 8443).await;
        seed_employee(&conn, "e1", "EMP001").await;
        // Insert the row WITHOUT going through persist_attendance_event so we can
        // plant a malicious photo_path that the persist helper would never emit.
        conn.execute(
            "INSERT INTO attendance_events \
             (id, employee_id, device_id, direction, captured_at, bucket_30s, \
              is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
             VALUES (?1, ?2, ?3, 'entry', ?4, ?5, 0, NULL, NULL, '<xml/>', ?6, unixepoch())",
            params![
                "evt-traversal".to_string(),
                "e1".to_string(),
                "d1".to_string(),
                1000_i64,
                33_i64,
                "../../../etc/passwd".to_string()
            ],
        )
        .await
        .expect("insert tampered row");
    }

    let token = viewer_token();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/events/evt-traversal/photo")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "path-traversal payload must be rejected (404, not served)"
    );
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "EVENT_PHOTO_NOT_FOUND");
}

// Admin token construct kept available for future write-side tests (02-03
// supervisor will likely need it). Mark it with an explicit use to silence
// dead-code warnings if no test currently invokes admin_token().
#[allow(dead_code)]
fn _keep_admin_token_alive() {
    let _ = admin_token;
}
