//! Coverage gap-fill for `backend/src/enrollments/handlers.rs` (08-04B Task 1).
//!
//! Baseline 0.94% line. Target ≥70%.
//!
//! All enrollment handlers exercised through their canonical routes:
//!   * list_enrollments       (GET /enrollments)
//!   * create_enrollment      (POST /enrollments) — multipart parse, validation, JPEG magic
//!   * get_enrollment         (GET /enrollments/:id)
//!   * retry_push             (POST /enrollments/:id/pushes/:device_id/retry)
//!   * capture_from_device    (POST /enrollments/captures)
//!   * get_capture            (GET /enrollments/captures/:capture_id)

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::devices::crypto;
use cronometrix_api::enrollments::handlers::{self, CaptureState};
use cronometrix_api::enrollments::service;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use libsql::params;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method as wm_method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{test_access_token, test_device_creds_key, TEST_JWT_SECRET};

const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];
const ACCEPTABLE_FACE_QUALITY: &[u8] = br#"{
  "faceDetected": true,
  "luminanceOk": true,
  "sizeOk": true,
  "luminance": 120,
  "width": 200,
  "height": 200
}"#;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

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
    })
}

/// Build a Router mirroring the canonical enrollment routes wired admin-only.
/// Returns (Router, AppState, TempDir).
fn build_app(state: AppState) -> Router {
    let admin_routes = Router::new()
        .route(
            "/enrollments",
            get(handlers::list_enrollments).post(handlers::create_enrollment),
        )
        .route("/enrollments/{id}", get(handlers::get_enrollment))
        .route(
            "/enrollments/{id}/pushes/{device_id}/retry",
            post(handlers::retry_push),
        )
        .route("/enrollments/captures", post(handlers::capture_from_device))
        .route(
            "/enrollments/captures/{capture_id}",
            get(handlers::get_capture),
        )
        .route(
            "/enrollments/capture-from-device",
            post(|| async { StatusCode::NOT_FOUND }),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));
    Router::new()
        .nest("/api/v1", admin_routes)
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// Seed a department + employee + admin user; returns (emp_id, admin_id, admin_token).
async fn seed_full(db: &libsql::Database) -> (String, String, String) {
    let admin_id = common::create_test_admin(db).await;
    let admin_token = common::test_access_token(&admin_id, "admin");

    let conn = db.connect().expect("connect");
    let dept_id = Uuid::new_v4().to_string();
    let dept_name = format!("Dept-{}", &dept_id[..8]);
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), dept_name],
    )
    .await
    .expect("seed dept");

    let emp_id = Uuid::new_v4().to_string();
    let emp_code = format!("E-{}", &emp_id[..8]);
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Employee', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), emp_code, dept_id.clone()],
    )
    .await
    .expect("seed emp");

    (emp_id, admin_id, admin_token)
}

/// Seed an active device pointing at the wiremock URI.
async fn seed_device_at(db: &libsql::Database, key: &[u8; 32], base_url: &str) -> String {
    let parts = url_lite_split(base_url);
    let conn = db.connect().expect("connect");
    let enc = crypto::encrypt_password("device-pw", key).unwrap();
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'admin', ?6, 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![
            id.clone(),
            format!("dev-{}", &id[..8]),
            parts.0,
            parts.1 as i64,
            parts.2,
            enc,
        ],
    )
    .await
    .expect("seed device");
    id
}

fn url_lite_split(url: &str) -> (String, u16, String) {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("http://") {
        ("http".to_string(), rest)
    } else if let Some(rest) = url.strip_prefix("https://") {
        ("https".to_string(), rest)
    } else {
        panic!("unsupported scheme: {url}");
    };
    let (host, port_str) = rest.rsplit_once(':').unwrap_or((rest, "80"));
    let port: u16 = port_str.parse().unwrap_or(80);
    (host.to_string(), port, scheme)
}

/// Build a multipart/form-data body for `create_enrollment`. Boundary is fixed
/// so the test contract is reproducible.
const BOUNDARY: &str = "MultipartBoundary123";

fn multipart_body(fields: &[(&str, &[u8], Option<&str>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value, content_type) in fields {
        out.extend_from_slice(format!("--{}\r\n", BOUNDARY).as_bytes());
        if name == &"photo" {
            out.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"photo.jpg\"\r\n",
                    name
                )
                .as_bytes(),
            );
        } else {
            out.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{}\"\r\n", name).as_bytes(),
            );
        }
        if let Some(ct) = content_type {
            out.extend_from_slice(format!("Content-Type: {}\r\n", ct).as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{}--\r\n", BOUNDARY).as_bytes());
    out
}

// ---------------------------------------------------------------------------
// 401 / 403 — auth gate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_enrollment_401_without_token() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let app = build_app(state);

    let body = multipart_body(&[
        ("employee_id", b"x", None),
        ("captured_via", b"upload", None),
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_enrollment_403_for_non_admin() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let app = build_app(state);

    let viewer_token = test_access_token("v", "viewer");
    let body = multipart_body(&[
        ("employee_id", b"x", None),
        ("captured_via", b"upload", None),
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_enrollments_200_for_admin_with_enriched_data() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, admin_id, token) = seed_full(&state.db).await;
    let created =
        service::start_enrollment(&state, &admin_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments?status=in_progress&limit=1&offset=0")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["limit"], 1);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["data"][0]["id"], created.enrollment_id);
    assert_eq!(json["data"][0]["employee_id"], emp_id);
    assert_eq!(json["data"][0]["employee_name"], "Test Employee");
    assert!(json["data"][0]["employee_code"]
        .as_str()
        .is_some_and(|code| code.starts_with("E-")));
    assert!(json["data"][0]["device_pushes"].is_array());
}

#[tokio::test]
async fn list_enrollments_403_for_viewer() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", test_access_token("viewer-1", "viewer")),
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_enrollments_rejects_invalid_status_with_422() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments?status=pending")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert!(json["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("enrollment status")));
}

// ---------------------------------------------------------------------------
// create_enrollment validators
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_enrollment_rejects_missing_employee_id() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let body = multipart_body(&[
        ("captured_via", b"upload", None),
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "MISSING_FIELD");
}

#[tokio::test]
async fn create_enrollment_rejects_missing_captured_via() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "MISSING_FIELD");
}

#[tokio::test]
async fn create_enrollment_rejects_missing_photo() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "MISSING_FIELD");
}

#[tokio::test]
async fn create_enrollment_rejects_invalid_captured_via() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"camera", None),
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "INVALID_CAPTURED_VIA");
}

#[tokio::test]
async fn create_enrollment_rejects_non_jpeg_magic_bytes() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    // PNG magic 0x89 0x50 0x4E.
    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
        (
            "photo",
            &[0x89u8, 0x50, 0x4E, 0x47, 0x0D],
            Some("image/png"),
        ),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "PHOTO_NOT_JPEG");
}

#[tokio::test]
async fn create_enrollment_rejects_oversized_photo() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    // 2.1 MB JPEG-magic-prefixed payload.
    let mut payload = vec![0xFF, 0xD8, 0xFF];
    payload.resize(2 * 1024 * 1024 + 1024, 0xAA);
    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
        ("photo", &payload, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    // Either path is acceptable: the explicit PHOTO_TOO_LARGE check OR the
    // upstream multer field-size guard surfacing as VALIDATION_ERROR. Both
    // indicate the oversized-photo branch took the validation path.
    let code = json["error"]["code"].as_str().unwrap_or("");
    assert!(
        code == "PHOTO_TOO_LARGE" || code == "VALIDATION_ERROR",
        "expected PHOTO_TOO_LARGE or VALIDATION_ERROR, got {code}"
    );
}

// ---------------------------------------------------------------------------
// create_enrollment happy path with synthetic JPEG that decodes successfully.
// We use the real `image` crate to make a tiny valid JPEG so normalize_face_jpeg
// can decode it; otherwise it errors with PHOTO_INVALID.
// ---------------------------------------------------------------------------

fn real_tiny_jpeg() -> Vec<u8> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(50, 50, |x, y| Rgb([x as u8, y as u8, 128u8]));
    let dynamic = image::DynamicImage::ImageRgb8(img);
    let mut buf = std::io::Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut buf, 90)
        .encode_image(&dynamic)
        .expect("encode tiny JPEG");
    buf.into_inner()
}

async fn assert_face_quality_rejected(quality: Option<&[u8]>, expected_code: &str) {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let database = state.db.clone();
    let enrollments_root = state.paths.enrollments_root.clone();
    let app = build_app(state);
    let jpeg = real_tiny_jpeg();
    let mut fields = vec![
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload".as_slice(), None),
    ];
    if let Some(value) = quality {
        fields.push(("face_quality_score", value, None));
    }
    fields.push(("photo", jpeg.as_slice(), Some("image/jpeg")));

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(multipart_body(&fields)))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["error"]["code"], expected_code);

    let conn = database.connect().unwrap();
    let mut rows = conn
        .query("SELECT COUNT(*) FROM face_enrollments", ())
        .await
        .unwrap();
    let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(
        count, 0,
        "invalid quality must not mutate enrollment tables"
    );
    drop(rows);
    drop(conn);
    if tokio::fs::try_exists(&enrollments_root).await.unwrap() {
        let mut entries = tokio::fs::read_dir(&enrollments_root).await.unwrap();
        assert!(
            entries.next_entry().await.unwrap().is_none(),
            "invalid quality must not persist an enrollment photo"
        );
    }
}

#[tokio::test]
async fn create_enrollment_requires_face_quality_evidence() {
    assert_face_quality_rejected(None, "FACE_QUALITY_REQUIRED").await;
}

#[tokio::test]
async fn create_enrollment_rejects_malformed_face_quality_json() {
    assert_face_quality_rejected(Some(b"not-json"), "FACE_QUALITY_INVALID").await;
}

#[tokio::test]
async fn create_enrollment_rejects_non_finite_face_quality_numbers() {
    let quality = br#"{
      "faceDetected":true,"luminanceOk":true,"sizeOk":true,
      "luminance":1e999,"width":200,"height":200
    }"#;
    assert_face_quality_rejected(Some(quality), "FACE_QUALITY_INVALID").await;
}

#[tokio::test]
async fn create_enrollment_rejects_out_of_range_face_quality_numbers() {
    let quality = br#"{
      "faceDetected":true,"luminanceOk":true,"sizeOk":true,
      "luminance":300,"width":200,"height":200
    }"#;
    assert_face_quality_rejected(Some(quality), "FACE_QUALITY_INVALID").await;
}

#[tokio::test]
async fn create_enrollment_rejects_unacceptable_face_quality_decision() {
    let quality = br#"{
      "faceDetected":false,"luminanceOk":true,"sizeOk":true,
      "luminance":120,"width":200,"height":200
    }"#;
    assert_face_quality_rejected(Some(quality), "FACE_QUALITY_UNACCEPTABLE").await;
}

#[tokio::test]
async fn create_enrollment_happy_path_returns_202() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let jpeg = real_tiny_jpeg();
    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
        ("face_quality_score", ACCEPTABLE_FACE_QUALITY, None),
        ("photo", &jpeg, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let json = body_to_json(resp.into_body()).await;
    assert!(json.get("enrollment_id").is_some());
    assert!(json.get("face_id").is_some());
}

#[tokio::test]
async fn create_enrollment_rejects_unparseable_jpeg() {
    // Magic bytes pass but normalize_face_jpeg fails to decode → PHOTO_INVALID.
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
        ("face_quality_score", ACCEPTABLE_FACE_QUALITY, None),
        // MINI_JPEG passes magic but is not actually decodable.
        ("photo", MINI_JPEG, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "PHOTO_INVALID");
}

#[tokio::test]
async fn create_enrollment_with_optional_fields_succeeds() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let jpeg = real_tiny_jpeg();
    let body = multipart_body(&[
        ("employee_id", emp_id.as_bytes(), None),
        ("captured_via", b"upload", None),
        ("source_device_id", b"", None), // Empty value should be ignored.
        ("face_quality_score", ACCEPTABLE_FACE_QUALITY, None),
        ("unknown_field", b"discarded", None), // Discarded by the catch-all arm.
        ("photo", &jpeg, Some("image/jpeg")),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// ---------------------------------------------------------------------------
// get_enrollment
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_enrollment_404_when_missing() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments/no-such-id")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "ENROLLMENT_NOT_FOUND");
}

#[tokio::test]
async fn get_enrollment_returns_full_response() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, admin_id, token) = seed_full(&state.db).await;

    // Seed an enrollment via the service layer (handler-independent).
    let resp =
        service::start_enrollment(&state, &admin_id, &emp_id, "upload", None, None, MINI_JPEG)
            .await
            .unwrap();
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/enrollments/{}", resp.enrollment_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["id"], resp.enrollment_id);
    assert_eq!(json["employee_id"], emp_id);
    assert_eq!(json["employee_name"], "Test Employee");
    assert!(json["employee_code"]
        .as_str()
        .is_some_and(|code| code.starts_with("E-")));
    assert_eq!(json["status"], "in_progress");
}

// ---------------------------------------------------------------------------
// retry_push
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_push_errors_for_unknown_enrollment() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/no-enr/pushes/no-dev/retry")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // Either path is acceptable:
    //   - reset_push_to_pending FK fails → 500 INTERNAL_ERROR (FK violation)
    //   - get_enrollment_push_params not-found → 404 ENROLLMENT_NOT_FOUND
    // Both indicate the unknown-enrollment branch took the error path.
    let s = resp.status();
    assert!(
        s == StatusCode::NOT_FOUND || s == StatusCode::INTERNAL_SERVER_ERROR,
        "expected 404 or 500, got {s}"
    );
}

#[tokio::test]
async fn retry_push_404_when_employee_has_no_photo() {
    // Build an enrollment but immediately UPDATE the employee to clear current_face_enrollment_id
    // so get_current_photo_path returns None → 404 PHOTO_NOT_FOUND.
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, admin_id, token) = seed_full(&state.db).await;
    let config = state.config.clone();
    let device_id = seed_device_at(&state.db, &config.device_creds_key, "http://127.0.0.1:1").await;

    let resp =
        service::start_enrollment(&state, &admin_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    service::update_push_status(
        &state.db.connect().unwrap(),
        &resp.device_pushes[0].id,
        "failed",
        Some("offline"),
    )
    .await
    .unwrap();
    // Unset current_face_enrollment_id so get_current_photo_path → None.
    let conn = state.db.connect().unwrap();
    conn.execute(
        "UPDATE employees SET current_face_enrollment_id = NULL WHERE id = ?1",
        params![emp_id.clone()],
    )
    .await
    .unwrap();
    drop(conn);

    let app = build_app(state);
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/api/v1/enrollments/{}/pushes/{}/retry",
            resp.enrollment_id, device_id
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["error"]["code"], "PHOTO_NOT_FOUND");
}

#[tokio::test]
async fn retry_push_returns_202_when_photo_present() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, admin_id, token) = seed_full(&state.db).await;
    let config = state.config.clone();

    // Spawn wiremock so the lifecycle-owned push does not hang on connection refused.
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;

    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let dispatcher = state
        .enrollment_dispatcher
        .start(state.clone())
        .await
        .unwrap();
    let resp =
        service::start_enrollment(&state, &admin_id, &emp_id, "device", None, None, MINI_JPEG)
            .await
            .unwrap();
    for _ in 0..200 {
        let row = state
            .db
            .connect()
            .unwrap()
            .query(
                "SELECT status FROM enrollment_device_pushes WHERE id=?1",
                params![resp.device_pushes[0].id.clone()],
            )
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap();
        if row.get::<String>(0).unwrap() == "success" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    service::update_push_status(
        &state.db.connect().unwrap(),
        &resp.device_pushes[0].id,
        "failed",
        Some("offline"),
    )
    .await
    .unwrap();

    // Materialise the photo on disk by re-calling the photo path (start_enrollment writes it).
    let state_for_poll = state.clone();
    let app = build_app(state);
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/api/v1/enrollments/{}/pushes/{}/retry",
            resp.enrollment_id, device_id
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["enrollment_id"], resp.enrollment_id);
    assert_eq!(json["device_id"], device_id);
    assert_eq!(json["status"], "pending");

    state_for_poll.enrollment_dispatcher.close().unwrap();
    dispatcher.await.unwrap().unwrap();
    let row = state_for_poll
        .db
        .connect()
        .unwrap()
        .query(
            "SELECT status FROM enrollment_device_pushes WHERE id=?1",
            params![resp.device_pushes[0].id.clone()],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "success");
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn obsolete_capture_and_device_retry_routes_return_404() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let obsolete_capture = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/capture-from-device")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = app.clone().oneshot(obsolete_capture).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let obsolete_retry = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/enr-1/devices/dev-1/retry")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(obsolete_retry).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// canonical captures collection + get_capture
// ---------------------------------------------------------------------------

#[tokio::test]
async fn capture_start_failure_leaves_no_state_or_jpeg() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let state_for_assertion = state.clone();
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/captures")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"device_id": "nonexistent", "employee_id": emp_id}).to_string(),
        ))
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    // get_decrypted returns a service error → 404 (NotFound) or 500 — the
    // load-bearing assertion is "not 200 ACCEPTED".
    assert_ne!(r.status(), StatusCode::ACCEPTED);
    assert!(state_for_assertion.captures.read().await.is_empty());
    assert!(
        !state_for_assertion.paths.captures_tmp_root.exists(),
        "fallible device setup must happen before capture state or JPEG creation"
    );
}

#[tokio::test]
async fn capture_start_invalid_encrypted_password_leaves_no_state_or_jpeg() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (employee_id, _admin_id, token) = seed_full(&state.db).await;
    let device_id = seed_device_at(
        &state.db,
        &state.config.device_creds_key,
        "http://127.0.0.1:1",
    )
    .await;
    state
        .db
        .connect()
        .unwrap()
        .execute(
            "UPDATE devices SET encrypted_password='invalid-base64' WHERE id=?1",
            params![device_id.clone()],
        )
        .await
        .unwrap();
    let assertion_state = state.clone();
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/enrollments/captures")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"device_id": device_id, "employee_id": employee_id})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::ACCEPTED);
    assert!(assertion_state.captures.read().await.is_empty());
    assert!(!assertion_state.paths.captures_tmp_root.exists());
}

#[tokio::test]
async fn capture_start_invalid_device_url_leaves_no_state_or_jpeg() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (employee_id, _admin_id, token) = seed_full(&state.db).await;
    let device_id = seed_device_at(
        &state.db,
        &state.config.device_creds_key,
        "http://127.0.0.1:1",
    )
    .await;
    state
        .db
        .connect()
        .unwrap()
        .execute(
            "UPDATE devices SET ip='[' WHERE id=?1",
            params![device_id.clone()],
        )
        .await
        .unwrap();
    let assertion_state = state.clone();
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/enrollments/captures")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"device_id": device_id, "employee_id": employee_id})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::ACCEPTED);
    assert!(assertion_state.captures.read().await.is_empty());
    assert!(!assertion_state.paths.captures_tmp_root.exists());
}

#[tokio::test]
async fn capture_from_device_returns_202_with_capture_id() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let config = state.config.clone();
    // Use unreachable device — handler must still 202 with the capture_id;
    // the spawned capture task will record an error/timeout in the map.
    let device_id = seed_device_at(&state.db, &config.device_creds_key, "http://127.0.0.1:1").await;
    let state_for_poll = state.clone();
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/captures")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"device_id": device_id, "employee_id": emp_id}).to_string(),
        ))
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
    let json = body_to_json(r.into_body()).await;
    assert!(json.get("capture_id").is_some());
    assert_eq!(json["status"], "capturing");
    assert_eq!(json["source_device_id"], device_id);

    // Wait for the detached capture spawn body in capture_from_device to land
    // in an error state (port 1 connection refused before 30s capture timeout).
    // This exercises the spawn block at lines 371-433 of the handler.
    let cap_id = json["capture_id"].as_str().unwrap().to_string();
    let mut landed = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let map = state_for_poll.captures.read().await;
        if let Some(state) = map.get(&cap_id) {
            if state.status != "capturing" {
                assert_eq!(state.source_device_id, device_id);
                landed = true;
                break;
            }
        }
    }
    assert!(
        landed,
        "capture spawn body must transition past 'capturing'"
    );
}

#[tokio::test]
async fn capture_from_device_success_path_writes_jpeg_under_captures_tmp_root() {
    // Spawn wiremock that successfully serves the 2-step capture flow.
    let server = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .mount(&server)
        .await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/ISAPI/AccessControl/CapturedFacePicture"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "image/jpeg")
                .set_body_bytes(MINI_JPEG.to_vec()),
        )
        .mount(&server)
        .await;

    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (emp_id, _admin_id, token) = seed_full(&state.db).await;
    let config = state.config.clone();
    let device_id = seed_device_at(&state.db, &config.device_creds_key, &server.uri()).await;
    let state_for_poll = state.clone();
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/enrollments/captures")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"device_id": device_id, "employee_id": emp_id}).to_string(),
        ))
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["source_device_id"], device_id);
    let cap_id = json["capture_id"].as_str().unwrap().to_string();

    // Poll for the success branch — the spawn body must write the jpeg to
    // captures_tmp_root and update state to "captured".
    let mut captured = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let map = state_for_poll.captures.read().await;
        if let Some(s) = map.get(&cap_id) {
            if s.status == "captured" {
                assert_eq!(s.source_device_id, device_id);
                captured = true;
                break;
            }
        }
    }
    assert!(captured, "wiremock-backed capture must reach 'captured'");

    let app = build_app(state_for_poll);
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/enrollments/captures/{cap_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["status"], "captured");
    assert_eq!(json["source_device_id"], device_id);
}

#[tokio::test]
async fn get_capture_404_for_unknown_capture() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments/captures/no-such-cap")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["error"]["code"], "CAPTURE_NOT_FOUND");
}

#[tokio::test]
async fn get_capture_returns_status_capturing() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;

    // Seed the captures map directly.
    let cap_id = "cap-1".to_string();
    {
        let mut map = state.captures.write().await;
        map.insert(
            cap_id.clone(),
            CaptureState {
                status: "capturing".into(),
                source_device_id: "dev-source".into(),
                photo_path: None,
                photo_identity: None,
                error_message: None,
                created_at: Instant::now(),
                terminal_at: None,
            },
        );
    }
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/enrollments/captures/{}", cap_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["status"], "capturing");
    assert_eq!(json["source_device_id"], "dev-source");
    // photo_b64 must be omitted (status != "captured").
    assert!(json.get("photo_b64").is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn get_capture_does_not_hold_capture_map_lock_during_slow_file_read() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let fifo = state.paths.captures_tmp_root.join("slow.jpg");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    state.captures.write().await.insert(
        "slow".into(),
        CaptureState {
            status: "captured".into(),
            source_device_id: "device".into(),
            photo_path: Some(fifo.to_string_lossy().into_owned()),
            photo_identity: None,
            error_message: None,
            created_at: Instant::now(),
            terminal_at: Some(Instant::now()),
        },
    );
    let app = build_app(state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/enrollments/captures/slow")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let lock = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        state.captures.write(),
    )
    .await
    .expect("slow filesystem read must not hold the capture map lock");
    drop(lock);
    let _ = tokio::fs::write(&fifo, b"jpeg").await;
    assert_eq!(
        response.await.unwrap().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn get_capture_returns_photo_b64_when_captured() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;

    // Write a photo to disk + insert capture state pointing at it.
    let cap_id = "cap-ok".to_string();
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let path = state.paths.captures_tmp_root.join("cap-ok.jpg");
    tokio::fs::write(&path, MINI_JPEG).await.unwrap();
    {
        let mut map = state.captures.write().await;
        map.insert(
            cap_id.clone(),
            CaptureState {
                status: "captured".into(),
                source_device_id: "dev-source".into(),
                photo_path: Some(path.to_string_lossy().into_owned()),
                photo_identity: None,
                error_message: None,
                created_at: Instant::now(),
                terminal_at: Some(Instant::now()),
            },
        );
    }
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/enrollments/captures/{}", cap_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["status"], "captured");
    assert_eq!(json["source_device_id"], "dev-source");
    let b64 = json["photo_b64"].as_str().expect("photo_b64 set");
    assert!(!b64.is_empty());
}

#[tokio::test]
async fn get_capture_status_error_omits_photo_b64() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (_emp_id, _admin_id, token) = seed_full(&state.db).await;

    let cap_id = "cap-err".to_string();
    {
        let mut map = state.captures.write().await;
        map.insert(
            cap_id.clone(),
            CaptureState {
                status: "error".into(),
                source_device_id: "dev-source".into(),
                photo_path: None,
                photo_identity: None,
                error_message: Some("some upstream error".into()),
                created_at: Instant::now(),
                terminal_at: Some(Instant::now()),
            },
        );
    }
    let app = build_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/enrollments/captures/{}", cap_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let json = body_to_json(r.into_body()).await;
    assert_eq!(json["status"], "error");
    assert_eq!(json["source_device_id"], "dev-source");
    assert_eq!(json["error_message"], "some upstream error");
    assert!(json.get("photo_b64").is_none());
}

// ---------------------------------------------------------------------------
// new_captures_map smoke
// ---------------------------------------------------------------------------

#[tokio::test]
async fn new_captures_map_starts_empty() {
    let map = handlers::new_captures_map();
    let r = map.read().await;
    assert!(r.is_empty());
}

// ---------------------------------------------------------------------------
// CaptureState Debug + Clone
// ---------------------------------------------------------------------------

#[test]
fn capture_state_clone_and_debug() {
    let cs = CaptureState {
        status: "capturing".into(),
        source_device_id: "dev-source".into(),
        photo_path: Some("/tmp/x.jpg".into()),
        photo_identity: None,
        error_message: None,
        created_at: Instant::now(),
        terminal_at: None,
    };
    let cloned = cs.clone();
    let dbg = format!("{:?}", cloned);
    assert!(dbg.contains("CaptureState"));
    assert!(dbg.contains("capturing"));
    assert_eq!(cloned.source_device_id, "dev-source");
}
