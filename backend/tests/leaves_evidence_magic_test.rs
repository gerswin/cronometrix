//! M-07: `leaves::handlers::create_leave` must not trust the client-supplied
//! multipart `Content-Type` header for evidence uploads. The authoritative
//! type — and the extension the file is persisted under — must come from the
//! file's own magic bytes, mirroring the CR-03 mitigation already applied to
//! `daily_records::handlers::create_override`
//! (`infer_evidence_ext_from_magic`, shared via
//! `cronometrix_api::storage::evidence_magic`).

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::leaves;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use common::{create_test_admin, create_test_department_with_shift, test_access_token};

fn make_state(db: libsql::Database) -> (AppState, tempfile::TempDir) {
    let config = Arc::new(Config {
        database_path: "test.db".into(),
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
        device_push_base_url: String::new(),
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

fn build_app(state: AppState) -> Router {
    let admin = Router::new()
        .route("/leaves", post(leaves::handlers::create_leave))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));
    Router::new().nest("/api/v1", admin).with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

fn build_multipart(fields: &[(&str, &str)], evidence: Option<(&str, &[u8])>) -> (Vec<u8>, String) {
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
            b"Content-Disposition: form-data; name=\"evidence\"; filename=\"x\"\r\n",
        );
        out.extend_from_slice(format!("Content-Type: {}\r\n\r\n", ct).as_bytes());
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (out, content_type)
}

async fn seed_employee(db: &libsql::Database, code: &str) -> String {
    let dept_name = format!("Dept-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let dept_id =
        create_test_department_with_shift(db, &dept_name, "day", false, 480, "09:00", "17:00")
            .await;
    let conn = db.connect().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test', ?3, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![id.clone(), code.to_string(), dept_id],
    )
    .await
    .unwrap();
    id
}

const REAL_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

const REAL_PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-a-real-png-payload";

/// M-07: an HTML payload disguised as a JPEG (via the client-controlled
/// Content-Type header) must not reach the leaves evidence directory. The
/// header is orientative, not authoritative.
#[tokio::test]
async fn html_declared_as_jpeg_is_rejected() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let emp = seed_employee(&db, "MHTML").await;
    let (state, _tmp) = make_state(db);
    let leaves_root = state.paths.leaves_root.clone();
    let app = build_app(state);
    let token = test_access_token(&admin, "admin");

    let html_payload: &[u8] = b"<html><body><script>alert(1)</script></body></html>";
    let (body, ct) = build_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-22"),
            ("leave_type", "medical"),
            ("justification", "test"),
        ],
        Some(("image/jpeg", html_payload)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let b = body_to_json(resp.into_body()).await;
    assert_eq!(b["error"]["code"], "VALIDATION_ERROR");

    // Nothing must have been written to disk under the evidence root.
    let nothing_on_disk = !leaves_root.exists()
        || std::fs::read_dir(&leaves_root)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);
    assert!(
        nothing_on_disk,
        "rejected upload must not leave a file in {}",
        leaves_root.display()
    );
}

/// A real JPEG (correct magic bytes + correct header) still uploads — the
/// fix must not break the normal path.
#[tokio::test]
async fn a_real_jpeg_is_accepted() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let emp = seed_employee(&db, "MJPG").await;
    let (state, _tmp) = make_state(db);
    let leaves_root = state.paths.leaves_root.clone();
    let app = build_app(state);
    let token = test_access_token(&admin, "admin");

    let (body, ct) = build_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-22"),
            ("leave_type", "medical"),
            ("justification", "test"),
        ],
        Some(("image/jpeg", REAL_JPEG)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let has_file = std::fs::read_dir(&leaves_root)
        .map(|entries| entries.count() > 0)
        .unwrap_or(false);
    assert!(has_file, "a real JPEG must be persisted to disk");
}

/// The stored extension is derived from the bytes, not from the client
/// header or filename: a real PNG declared as `application/pdf` must be
/// stored under a `.png` path, never `.pdf`.
#[tokio::test]
async fn the_stored_extension_comes_from_the_bytes() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let emp = seed_employee(&db, "MPNG").await;
    let (state, _tmp) = make_state(db);
    let leaves_root = state.paths.leaves_root.clone();
    let app = build_app(state);
    let token = test_access_token(&admin, "admin");

    let (body, ct) = build_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-22"),
            ("leave_type", "medical"),
            ("justification", "test"),
        ],
        // Declared as PDF via the header, but the bytes are a real PNG.
        Some(("application/pdf", REAL_PNG)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let entries: Vec<_> = std::fs::read_dir(&leaves_root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one evidence file expected");
    assert!(
        entries[0].ends_with(".png"),
        "extension must be derived from the real PNG bytes, got '{}'",
        entries[0]
    );
    assert!(
        !entries[0].ends_with(".pdf"),
        "must not trust the client-declared 'application/pdf' header"
    );
}
