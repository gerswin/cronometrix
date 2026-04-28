//! Integration tests for `daily_records::handlers`. Targets the 0% baseline gap
//! from Plan 03 (08-04A bucket row 6). Covers:
//!   - GET /daily-records: 401, 200 happy path, filters, pagination
//!   - GET /daily-records/{id}: 200 happy path with anomalies, 404 unknown id
//!   - POST /daily-records/{id}/overrides: admin-only multipart create
//!     - 401 / 403 RBAC gates
//!     - 422 missing justification, missing evidence, bad date pair
//!     - 422 unsupported content-type, 422 magic-byte mismatch
//!     - 404 daily_record not found
//!     - 201 happy path: file lands in state.paths.overrides_root,
//!       override row inserted

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::daily_records;
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
    let viewer_routes = Router::new()
        .route(
            "/daily-records",
            get(daily_records::handlers::list_daily_records),
        )
        .route(
            "/daily-records/{id}",
            get(daily_records::handlers::get_daily_record),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let admin_routes = Router::new()
        .route(
            "/daily-records/{id}/overrides",
            post(daily_records::handlers::create_override),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest("/api/v1", viewer_routes.merge(admin_routes))
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

fn build_multipart(
    fields: &[(&str, &str)],
    evidence: Option<(&str, &[u8])>,
) -> (Vec<u8>, String) {
    let boundary = "MIME_boundary";
    let mut out = Vec::new();
    for (name, value) in fields {
        out.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        out.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        out.extend_from_slice(value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if let Some((ct, bytes)) = evidence {
        out.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        out.extend_from_slice(
            b"Content-Disposition: form-data; name=\"evidence\"; filename=\"e.bin\"\r\n",
        );
        out.extend_from_slice(format!("Content-Type: {}\r\n\r\n", ct).as_bytes());
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (out, content_type)
}

const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

const MINI_PDF: &[u8] = b"%PDF-1.4\n%fake\n";
const MINI_PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

fn admin_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "admin")
}

fn supervisor_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "supervisor")
}

fn viewer_token() -> String {
    test_access_token(&Uuid::new_v4().to_string(), "viewer")
}

/// Insert an admin user row in the users table and return (user_id, access_token).
/// Required by the create_override happy-path tests because daily_record_overrides.overridden_by
/// is a FK to users(id) — using a stranger UUID in the JWT triggers a FK violation 500.
async fn admin_user_with_token(db: &libsql::Database) -> (String, String) {
    let id = common::create_test_admin(db).await;
    let token = test_access_token(&id, "admin");
    (id, token)
}

/// Seed (department, employee, daily_record) and return all three ids.
async fn seed_dr(
    db: &libsql::Database,
    anchor_date: &str,
) -> (String, String, String) {
    let dept_name = format!("Dept-{}", &Uuid::new_v4().to_string()[..8]);
    let dept_id =
        create_test_department_with_shift(db, &dept_name, "day", false, 480, "09:00", "17:00")
            .await;
    let emp_id = Uuid::new_v4().to_string();
    let dr_id = Uuid::new_v4().to_string();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &Uuid::new_v4().to_string()[..6]), dept_id.clone()],
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, \
         work_minutes, overtime_minutes, late_minutes, early_departure_minutes, is_rest_day_worked, \
         entry_at, exit_at, leave_id, computed_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'day', 420, 0, 5, 0, 0, NULL, NULL, NULL, unixepoch(), unixepoch(), unixepoch())",
        params![dr_id.clone(), emp_id.clone(), dept_id.clone(), anchor_date.to_string()],
    )
    .await
    .unwrap();
    // Seed an anomaly so get_daily_record exercises the anomalies-attach path.
    conn.execute(
        "INSERT INTO daily_record_anomalies (id, daily_record_id, code, detail, created_at) \
         VALUES (?1, ?2, 'MISSING_EXIT', NULL, unixepoch())",
        params![Uuid::new_v4().to_string(), dr_id.clone()],
    )
    .await
    .unwrap();
    (dept_id, emp_id, dr_id)
}

// =============================================================================
// LIST / GET (viewer routes)
// =============================================================================

#[tokio::test]
async fn list_daily_records_401_without_token() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/daily-records")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_daily_records_200_for_viewer_with_data() {
    let db = common::test_db().await;
    let (_dept, _emp, _dr) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/daily-records")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    let row = &body["data"][0];
    assert_eq!(row["anchor_date"], "2026-04-20");
    // anomalies array must be present and non-empty per the seeded MISSING_EXIT.
    let anoms = row["anomalies"].as_array().unwrap();
    assert!(
        anoms.iter().any(|c| c == "MISSING_EXIT"),
        "anomalies must include seeded MISSING_EXIT, got: {:?}",
        anoms
    );
}

#[tokio::test]
async fn list_daily_records_filter_by_employee_and_date_range() {
    let db = common::test_db().await;
    let (_, emp_a, _) = seed_dr(&db, "2026-04-20").await;
    let (_, _emp_b, _) = seed_dr(&db, "2026-04-25").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/daily-records?employee_id={}&from_date=2026-04-19&to_date=2026-04-21",
            emp_a
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["employee_id"], emp_a);
}

#[tokio::test]
async fn list_daily_records_filter_by_department() {
    let db = common::test_db().await;
    let (dept_a, _, _) = seed_dr(&db, "2026-04-20").await;
    let (_dept_b, _, _) = seed_dr(&db, "2026-04-21").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/daily-records?department_id={}", dept_a))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["data"][0]["department_id"], dept_a);
}

#[tokio::test]
async fn list_daily_records_pagination_clamps() {
    let db = common::test_db().await;
    seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/daily-records?limit=999&offset=-3")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["limit"], 100);
    assert_eq!(body["offset"], 0);
}

#[tokio::test]
async fn get_daily_record_404_unknown() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/daily-records/nonexistent-id")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "DAILY_RECORD_NOT_FOUND");
}

#[tokio::test]
async fn get_daily_record_200_with_anomalies_attached() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/daily-records/{}", dr_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["id"], dr_id);
    let anoms = body["anomalies"].as_array().unwrap();
    assert!(anoms.iter().any(|c| c == "MISSING_EXIT"));
}

// =============================================================================
// CREATE OVERRIDE (admin route)
// =============================================================================

#[tokio::test]
async fn create_override_401_without_token() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(&[("justification", "x")], Some(("application/pdf", MINI_PDF)));
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_override_403_for_supervisor() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(&[("justification", "x")], Some(("application/pdf", MINI_PDF)));
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", supervisor_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_override_422_when_justification_missing() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(&[("override_work_minutes", "30")], Some(("application/pdf", MINI_PDF)));
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let b = body_to_json(resp.into_body()).await;
    assert_eq!(b["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_override_422_when_evidence_missing() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(&[("justification", "valid reason")], None);
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_override_422_when_justification_blank() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "   ")],
        Some(("application/pdf", MINI_PDF)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_override_422_unsupported_content_type() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "valid reason")],
        Some(("application/octet-stream", MINI_PDF)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let b = body_to_json(resp.into_body()).await;
    assert!(
        b["error"]["message"]
            .as_str()
            .unwrap()
            .contains("PDF, JPEG, or PNG"),
        "should mention allowed types: {b:?}"
    );
}

#[tokio::test]
async fn create_override_422_magic_byte_mismatch() {
    // Declared image/jpeg but bytes are not a JPEG magic prefix.
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "spoofed mime")],
        Some(("image/jpeg", b"NOTAJPEG")),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let b = body_to_json(resp.into_body()).await;
    assert!(b["error"]["message"]
        .as_str()
        .unwrap()
        .contains("supported file type"));
}

#[tokio::test]
async fn create_override_422_when_exit_not_after_entry() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    // override_exit_at <= override_entry_at — must reject (WR-06).
    let (body, ct) = build_multipart(
        &[
            ("justification", "valid"),
            ("override_entry_at", "2026-04-20T13:00:00Z"),
            ("override_exit_at", "2026-04-20T13:00:00Z"),
        ],
        Some(("application/pdf", MINI_PDF)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_override_404_when_daily_record_missing() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "valid")],
        Some(("application/pdf", MINI_PDF)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/daily-records/no-such-id/overrides")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let b = body_to_json(resp.into_body()).await;
    assert_eq!(b["error"]["code"], "DAILY_RECORD_NOT_FOUND");
}

#[tokio::test]
async fn create_override_201_pdf_writes_to_overrides_root() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (_admin_id, token) = admin_user_with_token(&db).await;
    let (state, _tmp) = make_state(db);
    let overrides_root = state.paths.overrides_root.clone();
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[
            ("justification", "Doctor's note"),
            ("override_work_minutes", "420"),
            ("override_entry_at", "2026-04-20T09:00:00Z"),
            ("override_exit_at", "2026-04-20T17:00:00Z"),
        ],
        Some(("application/pdf", MINI_PDF)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let b = body_to_json(resp.into_body()).await;
    assert_eq!(b["daily_record_id"], dr_id);
    assert_eq!(b["override_work_minutes"], 420);
    let evidence = b["evidence_path"].as_str().expect("evidence_path string");
    assert!(evidence.ends_with(".pdf"), "ext from magic bytes: {evidence}");
    let full = overrides_root.join(evidence);
    assert!(full.exists(), "evidence file must land at {:?}", full);
}

#[tokio::test]
async fn create_override_201_jpeg_extension() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (_admin_id, token) = admin_user_with_token(&db).await;
    let (state, _tmp) = make_state(db);
    let overrides_root = state.paths.overrides_root.clone();
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "valid")],
        Some(("image/jpeg", MINI_JPEG)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let b = body_to_json(resp.into_body()).await;
    let evidence = b["evidence_path"].as_str().unwrap();
    assert!(evidence.ends_with(".jpg"));
    assert!(overrides_root.join(evidence).exists());
}

#[tokio::test]
async fn create_override_201_png_extension() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (_admin_id, token) = admin_user_with_token(&db).await;
    let (state, _tmp) = make_state(db);
    let overrides_root = state.paths.overrides_root.clone();
    let app = build_test_app(state);

    let (body, ct) = build_multipart(
        &[("justification", "valid")],
        Some(("image/png", MINI_PNG)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let b = body_to_json(resp.into_body()).await;
    let evidence = b["evidence_path"].as_str().unwrap();
    assert!(evidence.ends_with(".png"));
    assert!(overrides_root.join(evidence).exists());
}
