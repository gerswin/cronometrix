//! Integration tests for `audit::handlers::list_audit` (read-only paginated
//! audit log endpoint). Covers:
//!
//!   - Test 1  — Viewer denied (403 from require_supervisor_or_above)
//!   - Test 2  — Unauthenticated denied (401)
//!   - Test 3  — Admin reads list (200, 5 rows seeded)
//!   - Test 4  — Supervisor also reads list (200, 5 rows seeded)
//!   - Test 5  — Pagination (100 rows, limit=10 & offset=20)
//!   - Test 6  — Filter by actor_id
//!   - Test 7  — Filter by table_name
//!   - Test 8  — Filter by date range (from_ts + to_ts)
//!   - Test 9  — old_data and new_data parse to JSON Value (not raw string)
//!   - Test 10 — limit clamping (500 → 200, 0 → 1)
//!
//! RBAC contract is load-bearing per threat model T-09-03:
//!   Admin + Supervisor → 200; Viewer → 403; Anonymous → 401.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use cronometrix_api::audit;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use common::{test_access_token, test_device_creds_key, TEST_JWT_SECRET};

// =============================================================================
// Test helpers
// =============================================================================

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
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

fn build_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route("/audit", get(audit::handlers::list_audit))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));
    Router::new().nest("/api/v1", routes).with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
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

/// Insert N deterministic audit_log rows directly into the DB.
/// `actor_id`, `table_name`, `operation`, `old_data`, `new_data` are all
/// caller-supplied. `created_at` is base_ts + row_index so rows are
/// deterministically ordered newest-first.
// Test fixture keeps explicit SQL fields readable; collapsing them would hide
// which audit column each case is exercising.
#[allow(clippy::too_many_arguments)]
async fn seed_audit_rows(
    db: &libsql::Database,
    count: usize,
    actor_id: Option<&str>,
    table_name: &str,
    operation: &str,
    new_data: Option<&str>,
    old_data: Option<&str>,
    base_ts: i64,
) -> Vec<String> {
    let conn = db.connect().expect("connect");
    let mut ids = Vec::new();
    for i in 0..count {
        let id = Uuid::new_v4().to_string();
        let ts = base_ts + i as i64;
        let actor_val = match actor_id {
            Some(a) => format!("'{}'", a),
            None => "NULL".to_string(),
        };
        let new_val = match new_data {
            Some(v) => format!("'{}'", v),
            None => "NULL".to_string(),
        };
        let old_val = match old_data {
            Some(v) => format!("'{}'", v),
            None => "NULL".to_string(),
        };
        conn.execute(
            &format!(
                "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
                 VALUES ('{}', '{}', '{}', '{}', {}, {}, {}, {})",
                id, table_name, Uuid::new_v4(), operation, old_val, new_val, actor_val, ts
            ),
            (),
        )
        .await
        .expect("seed audit row");
        ids.push(id);
    }
    ids
}

// Base timestamp used across tests: 2026-04-01 00:00:00 UTC = 1743465600
const BASE_TS: i64 = 1743465600;

// =============================================================================
// Test 1 — Viewer denied (RBAC gate — T-09-03)
// =============================================================================

#[tokio::test]
async fn audit_403_when_viewer() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Viewer must be rejected by require_supervisor_or_above"
    );
}

// =============================================================================
// Test 2 — Unauthenticated denied
// =============================================================================

#[tokio::test]
async fn audit_401_when_unauthenticated() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Missing Authorization header must be rejected"
    );
}

// =============================================================================
// Test 3 — Admin reads list (200, 5 rows, correct shape)
// =============================================================================

#[tokio::test]
async fn audit_200_admin_reads_5_rows() {
    let db = common::test_db().await;
    seed_audit_rows(
        &db,
        5,
        Some("actor-1"),
        "employees",
        "INSERT",
        None,
        None,
        BASE_TS,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 5, "total must equal number of seeded rows");
    assert_eq!(
        body["data"].as_array().unwrap().len(),
        5,
        "data array length must equal total when < limit"
    );
    assert_eq!(body["limit"], 50, "default limit is 50");
    assert_eq!(body["offset"], 0, "default offset is 0");
    // Each row has required fields
    let row = &body["data"][0];
    assert!(row["id"].is_string());
    assert!(row["table_name"].is_string());
    assert!(row["record_id"].is_string());
    assert!(row["operation"].is_string());
    assert!(row["created_at"].is_number(), "created_at is epoch i64");
}

// =============================================================================
// Test 4 — Supervisor also reads list (200)
// =============================================================================

#[tokio::test]
async fn audit_200_supervisor_reads_list() {
    let db = common::test_db().await;
    seed_audit_rows(
        &db,
        3,
        Some("actor-2"),
        "departments",
        "UPDATE",
        None,
        None,
        BASE_TS,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", supervisor_token()),
        )
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["data"].as_array().unwrap().len(), 3);
}

// =============================================================================
// Test 5 — Pagination (100 rows, limit=10, offset=20, sort DESC)
// =============================================================================

#[tokio::test]
async fn audit_pagination_limit_offset() {
    let db = common::test_db().await;
    // Seed 100 rows with deterministic created_at values (BASE_TS + 0..99)
    seed_audit_rows(&db, 100, None, "events", "INSERT", None, None, BASE_TS).await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit?limit=10&offset=20")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 100, "total must reflect all rows");
    assert_eq!(
        body["data"].as_array().unwrap().len(),
        10,
        "page size must equal requested limit"
    );
    assert_eq!(body["limit"], 10);
    assert_eq!(body["offset"], 20);

    // Verify DESC order: first item in result must have higher created_at than last
    let arr = body["data"].as_array().unwrap();
    let first_ts = arr[0]["created_at"].as_i64().unwrap();
    let last_ts = arr[arr.len() - 1]["created_at"].as_i64().unwrap();
    assert!(
        first_ts >= last_ts,
        "sort order must be created_at DESC: first={} last={}",
        first_ts,
        last_ts
    );
}

// =============================================================================
// Test 6 — Filter by actor_id
// =============================================================================

#[tokio::test]
async fn audit_filter_by_actor_id() {
    let db = common::test_db().await;
    let actor_a = Uuid::new_v4().to_string();
    let actor_b = Uuid::new_v4().to_string();
    seed_audit_rows(
        &db,
        3,
        Some(&actor_a),
        "employees",
        "INSERT",
        None,
        None,
        BASE_TS,
    )
    .await;
    seed_audit_rows(
        &db,
        2,
        Some(&actor_b),
        "employees",
        "INSERT",
        None,
        None,
        BASE_TS + 10,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/audit?actor_id={}", actor_a))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 3, "only actor_a rows returned");
    let arr = body["data"].as_array().unwrap();
    for row in arr {
        assert_eq!(
            row["actor_id"].as_str().unwrap(),
            actor_a,
            "actor_id filter must be applied"
        );
    }
}

// =============================================================================
// Test 7 — Filter by table_name
// =============================================================================

#[tokio::test]
async fn audit_filter_by_table_name() {
    let db = common::test_db().await;
    seed_audit_rows(&db, 4, None, "employees", "INSERT", None, None, BASE_TS).await;
    seed_audit_rows(
        &db,
        2,
        None,
        "departments",
        "INSERT",
        None,
        None,
        BASE_TS + 10,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit?table_name=employees")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 4, "only employees rows returned");
    for row in body["data"].as_array().unwrap() {
        assert_eq!(row["table_name"], "employees");
    }
}

#[tokio::test]
async fn audit_filters_by_record_and_operation_together() {
    let db = common::test_db().await;
    let target_record = Uuid::new_v4().to_string();
    let other_record = Uuid::new_v4().to_string();
    let conn = db.connect().expect("connect");
    for (record_id, operation) in [
        (target_record.as_str(), "UPDATE"),
        (target_record.as_str(), "INSERT"),
        (other_record.as_str(), "UPDATE"),
    ] {
        conn.execute(
            "INSERT INTO audit_log (id, table_name, record_id, operation, created_at) \
             VALUES (?1, 'employees', ?2, ?3, ?4)",
            libsql::params![Uuid::new_v4().to_string(), record_id, operation, BASE_TS],
        )
        .await
        .expect("seed audit row");
    }
    drop(conn);

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/audit?record_id={target_record}&operation=UPDATE"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["record_id"], target_record);
    assert_eq!(body["data"][0]["operation"], "UPDATE");
}

// =============================================================================
// Test 8 — Filter by date range (from_ts + to_ts)
// =============================================================================

#[tokio::test]
async fn audit_filter_by_date_range() {
    let db = common::test_db().await;
    // Seed 3 batches at different timestamps
    // Batch A: ts = BASE_TS (2026-04-01)
    seed_audit_rows(&db, 2, None, "employees", "INSERT", None, None, BASE_TS).await;
    // Batch B: ts = BASE_TS + 100 (inside range)
    seed_audit_rows(
        &db,
        3,
        None,
        "employees",
        "UPDATE",
        None,
        None,
        BASE_TS + 100,
    )
    .await;
    // Batch C: ts = BASE_TS + 1000 (after range)
    seed_audit_rows(
        &db,
        1,
        None,
        "employees",
        "DELETE",
        None,
        None,
        BASE_TS + 1000,
    )
    .await;

    let from_ts = BASE_TS + 50;
    let to_ts = BASE_TS + 200;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/audit?from_ts={}&to_ts={}", from_ts, to_ts))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    // Batch B rows (BASE_TS+100 .. BASE_TS+102) fall in [from_ts, to_ts]
    assert_eq!(
        body["total"], 3,
        "only rows in [from_ts, to_ts] range returned"
    );
}

// =============================================================================
// Test 9 — old_data and new_data parse to JSON Value (not raw string)
// =============================================================================

#[tokio::test]
async fn audit_json_data_fields_deserialize_to_objects() {
    let db = common::test_db().await;
    let new_data_json = r#"{"name":"Ana","department_id":"dept-1"}"#;
    let old_data_json = r#"{"name":"Ana Antigua","department_id":"dept-1"}"#;
    seed_audit_rows(
        &db,
        1,
        Some("actor-x"),
        "employees",
        "UPDATE",
        Some(new_data_json),
        Some(old_data_json),
        BASE_TS,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    let row = &body["data"][0];
    // new_data must be a JSON object, not a string
    assert!(
        row["new_data"].is_object(),
        "new_data must deserialize to an object, got: {}",
        row["new_data"]
    );
    assert_eq!(
        row["new_data"]["name"], "Ana",
        "new_data.name must equal the value from JSON"
    );
    // old_data must also be a JSON object
    assert!(
        row["old_data"].is_object(),
        "old_data must deserialize to an object, got: {}",
        row["old_data"]
    );
    assert_eq!(row["old_data"]["name"], "Ana Antigua");
}

// =============================================================================
// Test 10 — limit clamping (500 → 200, 0 → 1)
// =============================================================================

#[tokio::test]
async fn audit_limit_clamped_to_200_and_1() {
    let db = common::test_db().await;
    seed_audit_rows(&db, 5, None, "leaves", "INSERT", None, None, BASE_TS).await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state.clone());

    // limit=500 → clamp to 200
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit?limit=500")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["limit"], 200, "limit=500 must be clamped to max 200");

    // limit=0 → clamp to 1
    let req2 = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit?limit=0")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = body_to_json(resp2.into_body()).await;
    assert_eq!(body2["limit"], 1, "limit=0 must be clamped to min 1");
}

// =============================================================================
// Helpers for /audit/actors tests
// =============================================================================

fn build_actors_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route("/audit/actors", get(audit::handlers::list_actors))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));
    Router::new().nest("/api/v1", routes).with_state(state)
}

/// Insert a single user row directly into the DB for actors test seeding.
/// Includes all NOT NULL columns: created_at and updated_at use a fixed epoch.
async fn seed_user_row(db: &libsql::Database, id: &str, username: &str, role: &str) {
    let conn = db.connect().expect("connect");
    conn.execute(
        &format!(
            "INSERT INTO users (id, username, full_name, password_hash, role, status, created_at, updated_at) \
             VALUES ('{}', '{}', '{} fullname', 'hash', '{}', 'active', {}, {})",
            id, username, username, role, BASE_TS, BASE_TS
        ),
        (),
    )
    .await
    .expect("seed user row");
}

// =============================================================================
// Test 11 — audit_actors_returns_200_for_admin (happy path)
// =============================================================================

#[tokio::test]
async fn audit_actors_returns_200_for_admin() {
    let db = common::test_db().await;
    seed_user_row(&db, "admin-user-1", "admin", "admin").await;
    seed_audit_rows(
        &db,
        1,
        Some("admin-user-1"),
        "employees",
        "INSERT",
        None,
        None,
        BASE_TS,
    )
    .await;
    let (state, _tmp) = make_state(db);
    let app = build_actors_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit/actors")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    let arr = body.as_array().expect("response must be a JSON array");
    // Filter to the non-null actor (audit triggers on users INSERT produce a NULL actor_id row)
    let non_null: Vec<_> = arr.iter().filter(|a| !a["actor_id"].is_null()).collect();
    assert_eq!(non_null.len(), 1, "must return exactly 1 non-null actor");
    assert_eq!(non_null[0]["actor_id"], "admin-user-1");
    assert_eq!(non_null[0]["username"], "admin");
    assert_eq!(non_null[0]["role"], "admin");
}

// =============================================================================
// Test 12 — audit_actors_viewer_returns_403 (RBAC gate)
// =============================================================================

#[tokio::test]
async fn audit_actors_viewer_returns_403() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_actors_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit/actors")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Viewer must be rejected by require_supervisor_or_above"
    );
}

// =============================================================================
// Test 13 — audit_actors_returns_empty_when_no_log
// =============================================================================

#[tokio::test]
async fn audit_actors_returns_empty_when_no_log() {
    let db = common::test_db().await;
    // No audit_log rows seeded — result must be empty array
    let (state, _tmp) = make_state(db);
    let app = build_actors_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit/actors")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    let arr = body.as_array().expect("response must be a JSON array");
    assert_eq!(arr.len(), 0, "empty audit_log must return [] not null");
}
