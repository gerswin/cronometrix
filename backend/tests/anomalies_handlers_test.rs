//! Integration tests for `anomalies::handlers::list_anomalies`. Targets the
//! 0% baseline gap from Plan 03 (08-04A bucket row 1). Covers:
//!   - 401 unauthenticated
//!   - 403 viewer (require_supervisor_or_above gate)
//!   - 200 happy path with seeded daily_records + daily_record_anomalies
//!   - filters: code, employee_id, from_date, to_date
//!   - pagination clamp (limit > 100 → 100, negative offset → 0)
//!   - empty result set returns 200 with data: [] + total: 0

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use cronometrix_api::anomalies;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use libsql::params;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use common::{
    create_test_department_with_shift, test_access_token, test_device_creds_key, TEST_JWT_SECRET,
};

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
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

fn build_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route("/anomalies", get(anomalies::handlers::list_anomalies))
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

/// Seed an employee + daily_record + N anomalies. Returns (daily_record_id, employee_id).
async fn seed_record(
    db: &libsql::Database,
    code: &str,
    anchor_date: &str,
    emp_code: &str,
) -> (String, String) {
    let conn = db.connect().expect("connect");

    // Department + employee. Use a unique department name per record so multiple
    // calls in the same test do not collide on UNIQUE departments.name.
    let dept_name = format!("Dept-{}", &Uuid::new_v4().to_string()[..8]);
    let dept_id =
        create_test_department_with_shift(db, &dept_name, "day", false, 480, "09:00", "17:00")
            .await;
    let emp_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), emp_code.to_string(), dept_id.clone()],
    )
    .await
    .expect("seed emp");

    // daily_record.
    let dr_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, \
         work_minutes, overtime_minutes, late_minutes, early_departure_minutes, is_rest_day_worked, \
         entry_at, exit_at, leave_id, computed_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'day', 0, 0, 0, 0, 0, NULL, NULL, NULL, unixepoch(), unixepoch(), unixepoch())",
        params![dr_id.clone(), emp_id.clone(), dept_id.clone(), anchor_date.to_string()],
    )
    .await
    .expect("seed dr");

    // anomaly row.
    conn.execute(
        "INSERT INTO daily_record_anomalies (id, daily_record_id, code, detail, created_at) \
         VALUES (?1, ?2, ?3, NULL, unixepoch())",
        params![Uuid::new_v4().to_string(), dr_id.clone(), code.to_string()],
    )
    .await
    .expect("seed anomaly");

    (dr_id, emp_id)
}

fn supervisor_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "supervisor")
}

fn admin_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "admin")
}

fn viewer_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "viewer")
}

// =============================================================================
// Auth gate (security-control coverage per threat model T-08-12A)
// =============================================================================

#[tokio::test]
async fn list_anomalies_401_when_no_token() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_anomalies_403_when_viewer() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies")
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

#[tokio::test]
async fn list_anomalies_200_for_supervisor_with_no_data() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies")
        .header(header::AUTHORIZATION, format!("Bearer {}", supervisor_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 0);
    assert!(body["data"].as_array().unwrap().is_empty());
    assert_eq!(body["limit"], 20);
    assert_eq!(body["offset"], 0);
}

// =============================================================================
// Happy path + filters
// =============================================================================

#[tokio::test]
async fn list_anomalies_200_returns_seeded_rows_for_admin() {
    let db = common::test_db().await;
    let (_dr_id, _emp_id) =
        seed_record(&db, "MISSING_EXIT", "2026-04-20", "E001").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    let row = &body["data"][0];
    assert_eq!(row["code"], "MISSING_EXIT");
    assert_eq!(row["anchor_date"], "2026-04-20");
    assert!(row["created_at"].is_string());
    // created_at must be ISO 8601 — sanity check format prefix.
    assert!(
        row["created_at"].as_str().unwrap().starts_with("20"),
        "ISO date prefix"
    );
}

#[tokio::test]
async fn list_anomalies_filter_by_code() {
    let db = common::test_db().await;
    seed_record(&db, "MISSING_EXIT", "2026-04-20", "E001").await;
    seed_record(&db, "MISSING_ENTRY", "2026-04-21", "E002").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies?code=MISSING_EXIT")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["code"], "MISSING_EXIT");
}

#[tokio::test]
async fn list_anomalies_filter_by_employee_id() {
    let db = common::test_db().await;
    let (_, emp_a) = seed_record(&db, "MISSING_EXIT", "2026-04-20", "EA").await;
    let (_, _emp_b) = seed_record(&db, "MISSING_ENTRY", "2026-04-21", "EB").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/anomalies?employee_id={}", emp_a))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["employee_id"], emp_a);
}

#[tokio::test]
async fn list_anomalies_filter_by_date_range() {
    let db = common::test_db().await;
    seed_record(&db, "MISSING_EXIT", "2026-04-20", "EA").await;
    seed_record(&db, "MISSING_EXIT", "2026-04-22", "EB").await;
    seed_record(&db, "MISSING_EXIT", "2026-04-25", "EC").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies?from_date=2026-04-21&to_date=2026-04-23")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["anchor_date"], "2026-04-22");
}

#[tokio::test]
async fn list_anomalies_pagination_clamps_excessive_limit() {
    let db = common::test_db().await;
    seed_record(&db, "MISSING_EXIT", "2026-04-20", "EA").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/anomalies?limit=999&offset=-5")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(
        body["limit"], 100,
        "limit must clamp to upper bound 100"
    );
    assert_eq!(body["offset"], 0, "negative offset must clamp to 0");
}

#[tokio::test]
async fn list_anomalies_combined_filters_intersect() {
    let db = common::test_db().await;
    seed_record(&db, "MISSING_EXIT", "2026-04-20", "EA").await;
    let (_, emp_b) = seed_record(&db, "MISSING_EXIT", "2026-04-22", "EB").await;
    seed_record(&db, "MISSING_ENTRY", "2026-04-22", "EC").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    // code AND employee AND date all combined → expect 1 row (the EB row).
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/anomalies?code=MISSING_EXIT&employee_id={}&from_date=2026-04-21",
            emp_b
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["employee_id"], emp_b);
    assert_eq!(body["data"][0]["code"], "MISSING_EXIT");
}
