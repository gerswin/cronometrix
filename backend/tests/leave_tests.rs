//! Integration tests for leave management (LEAVE-01..04, Plan 03-03).
//!
//! Coverage:
//! - LEAVE-01: medical leave with evidence upload
//! - LEAVE-02: manual leave without evidence (justification mandatory)
//! - LEAVE-03: leave overlay suppresses work_minutes (D-16)
//! - LEAVE-04: medical flag preserved via leave_id FK → leaves.leave_type JOIN
//! - T-3-14: overlap check rejects second overlapping leave with 409
//! - T-3-15: evidence path traversal rejected (`..` / absolute paths)
//! - T-3-19: supervisor and viewer forbidden from POST/DELETE
//! - Optimistic concurrency: stale version on cancel → 409

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{delete, get, post};
use axum::Router;
use chrono::{NaiveDate, TimeZone};
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::daily_records::service as dr_service;
use cronometrix_api::leaves;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use libsql::params;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use common::{
    create_test_admin, create_test_department_with_shift, create_test_leave,
    create_test_supervisor, create_test_viewer, test_access_token, test_device_creds_key,
    TEST_JWT_SECRET,
};

// -----------------------------------------------------------------------------
// Harness
// -----------------------------------------------------------------------------

/// Build (AppState, TempDir) for a leave test. The TempDir is returned so the
/// caller binds it to a local variable that outlives every assertion — see
/// Pitfall 1 in 08-RESEARCH.md. Per D-20 (Plan 08-02), tests pass a tempdir
/// path via Paths::for_test instead of mutating CRONOMETRIX_LEAVES_ROOT.
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
    let viewer_routes = Router::new()
        .route("/leaves", get(leaves::handlers::list_leaves))
        .route("/leaves/{id}", get(leaves::handlers::get_leave))
        .route(
            "/leaves/{id}/evidence",
            get(leaves::handlers::get_leave_evidence),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let admin_routes = Router::new()
        .route("/leaves", post(leaves::handlers::create_leave))
        .route("/leaves/{id}", delete(leaves::handlers::cancel_leave))
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

/// Build a multipart/form-data body with the given text fields plus an
/// optional evidence part. Boundary is "MIME_boundary" for reproducibility.
fn build_leave_multipart(
    fields: &[(&str, &str)],
    evidence: Option<(&str, &[u8])>, // (content_type, bytes)
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
            b"Content-Disposition: form-data; name=\"evidence\"; filename=\"evidence.bin\"\r\n",
        );
        out.extend_from_slice(format!("Content-Type: {}\r\n\r\n", ct).as_bytes());
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (out, content_type)
}

async fn seed_employee(db: &libsql::Database, dept_id: &str, code: &str) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![id.clone(), code.to_string(), dept_id.to_string()],
    )
    .await
    .expect("seed employee");
    id
}

async fn seed_device(db: &libsql::Database, id: &str) {
    let conn = db.connect().expect("connect");
    let hash: u32 = id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, b| acc.wrapping_mul(131).wrapping_add(*b as u32));
    let port = 1024 + (hash % 60000) as i64;
    let ip = format!("10.0.{}.{}", (hash >> 8) & 0xFF, hash & 0xFF);
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'https', 'admin', 'ct', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), format!("dev-{}", id), ip, port],
    )
    .await
    .expect("seed device");
}

async fn seed_event(
    db: &libsql::Database,
    employee_id: &str,
    device_id: &str,
    direction: &str,
    captured_at: i64,
) {
    let conn = db.connect().expect("connect");
    let bucket = captured_at / 30;
    conn.execute(
        "INSERT INTO attendance_events (id, employee_id, device_id, direction, captured_at, \
         bucket_30s, is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, '<x/>', NULL, unixepoch())",
        params![
            Uuid::new_v4().to_string(),
            employee_id.to_string(),
            device_id.to_string(),
            direction.to_string(),
            captured_at,
            bucket
        ],
    )
    .await
    .expect("seed event");
}

async fn ensure_global_rules(db: &libsql::Database) {
    let conn = db.connect().expect("connect");
    conn.execute(
        "INSERT OR IGNORE INTO global_rules \
         (id, late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, \
          effective_from, version, updated_at) \
         VALUES ('singleton', 10, 10, 0, unixepoch(), 1, unixepoch())",
        (),
    )
    .await
    .expect("seed global_rules");
}

fn caracas_epoch(date: NaiveDate, hh: u32, mm: u32) -> i64 {
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let naive = date.and_time(chrono::NaiveTime::from_hms_opt(hh, mm, 0).unwrap());
    tz.from_local_datetime(&naive).single().unwrap().timestamp()
}

// -----------------------------------------------------------------------------
// LEAVE-01: medical leave with evidence upload
// -----------------------------------------------------------------------------

#[tokio::test]
async fn create_leave_medical_with_evidence() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state.clone());

    let token = test_access_token(&admin, "admin");
    // Mini JPEG magic bytes (SOI + JFIF APP0 + EOI).
    let jpeg: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
    ];
    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-20"),
            ("leave_type", "medical"),
            ("justification", "Doctor's note attached"),
        ],
        Some(("image/jpeg", jpeg)),
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "medical leave POST should 201"
    );
    let body_json = body_to_json(resp.into_body()).await;
    assert_eq!(body_json["leave_type"], "medical");
    assert_eq!(body_json["employee_id"], emp);
    assert_eq!(body_json["justification"], "Doctor's note attached");
    assert!(
        body_json["evidence_path"].is_string(),
        "evidence_path must be Some for medical leave; got {:?}",
        body_json["evidence_path"]
    );

    // Evidence file must exist at state.paths.leaves_root/{evidence_path}.
    let relpath = body_json["evidence_path"].as_str().unwrap();
    let full = state.paths.leaves_root.join(relpath);
    assert!(
        full.exists(),
        "evidence file should exist on disk at {:?}",
        full
    );
}

#[tokio::test]
async fn create_leave_medical_without_evidence_rejected() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&admin, "admin");

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-20"),
            ("leave_type", "medical"),
            ("justification", "no evidence attached"),
        ],
        None,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "medical without evidence must fail validation"
    );
}

// -----------------------------------------------------------------------------
// LEAVE-02: manual leave without evidence
// -----------------------------------------------------------------------------

#[tokio::test]
async fn create_leave_manual_without_evidence() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&admin, "admin");

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-22"),
            ("to_date", "2026-04-22"),
            ("leave_type", "manual"),
            ("justification", "Permiso especial autorizado"),
        ],
        None,
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
    let body_json = body_to_json(resp.into_body()).await;
    assert_eq!(body_json["leave_type"], "manual");
    assert_eq!(body_json["justification"], "Permiso especial autorizado");
    assert!(
        body_json["evidence_path"].is_null(),
        "manual leave without evidence should have null evidence_path"
    );
}

// -----------------------------------------------------------------------------
// T-3-14: overlap check returns 409 LeaveConflict
// -----------------------------------------------------------------------------

#[tokio::test]
async fn create_leave_overlap_returns_conflict() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    // Seed an existing leave 2026-04-20 to 2026-04-22.
    let _seeded =
        create_test_leave(&db, &emp, "vacation", "2026-04-20", "2026-04-22", &admin).await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&admin, "admin");

    // Attempt a second leave overlapping (2026-04-21 to 2026-04-25).
    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-21"),
            ("to_date", "2026-04-25"),
            ("leave_type", "manual"),
            ("justification", "This should conflict"),
        ],
        None,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "overlap must return 409"
    );
    let body_json = body_to_json(resp.into_body()).await;
    assert_eq!(body_json["error"]["code"], "LEAVE_OVERLAP");
}

#[tokio::test]
async fn concurrent_overlapping_leave_creates_yield_one_success_and_one_conflict() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-concurrent", "day", false, 480, "09:00", "17:00")
            .await;
    let emp = seed_employee(&db, &dept, "E-CONCURRENT").await;
    let (state, _tmp) = make_state(db);

    // Hold the single writer so both requests are pending together behind the
    // same serialized boundary. This makes the concurrency case deterministic.
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let blocker_queue = state.db_write.clone();
    let blocker = tokio::spawn(async move {
        blocker_queue
            .job("test.block-leave-writer", move |_| {
                Box::pin(async move {
                    release_rx.await.expect("release writer");
                    Ok(())
                })
            })
            .await
    });
    while state.db_write.stats().accepted < 1 {
        tokio::task::yield_now().await;
    }

    let first_state = state.clone();
    let first_admin = admin.clone();
    let first_emp = emp.clone();
    let first = tokio::spawn(async move {
        leaves::service::create_leave_queued(
            &first_state,
            &first_admin,
            leaves::models::CreateLeaveRequest {
                employee_id: first_emp,
                from_date: "2026-04-20".into(),
                to_date: "2026-04-22".into(),
                leave_type: "vacation".into(),
                justification: "first concurrent request".into(),
            },
            None,
        )
        .await
    });
    let second_state = state.clone();
    let second = tokio::spawn(async move {
        leaves::service::create_leave_queued(
            &second_state,
            &admin,
            leaves::models::CreateLeaveRequest {
                employee_id: emp,
                from_date: "2026-04-21".into(),
                to_date: "2026-04-23".into(),
                leave_type: "manual".into(),
                justification: "second concurrent request".into(),
            },
            None,
        )
        .await
    });

    while state.db_write.stats().accepted < 3 {
        tokio::task::yield_now().await;
    }
    release_tx.send(()).expect("release blocker");
    blocker.await.unwrap().unwrap();

    let results = [first.await.unwrap(), second.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let conflicts: Vec<_> = results
        .iter()
        .filter_map(|result| match result {
            Err(cronometrix_api::errors::AppError::LeaveConflict { code, .. }) => Some(*code),
            _ => None,
        })
        .collect();
    assert_eq!(conflicts, vec!["LEAVE_OVERLAP"]);
}

#[tokio::test]
async fn closed_queue_after_leave_evidence_write_leaves_no_row_or_file() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-closed", "day", false, 480, "09:00", "17:00")
            .await;
    let emp = seed_employee(&db, &dept, "E-CLOSED").await;
    let (state, _tmp) = make_state(db);
    let leaves_root = state.paths.leaves_root.clone();
    state.db_write.close_and_flush().await.unwrap();
    let app = build_test_app(state.clone());

    let jpeg: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0xFF, 0xD9];
    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-05-01"),
            ("to_date", "2026-05-01"),
            ("leave_type", "medical"),
            ("justification", "queue closure compensation"),
        ],
        Some(("image/jpeg", jpeg)),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", test_access_token(&admin, "admin")),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let conn = state.db.connect().unwrap();
    let count: i64 = conn
        .query(
            "SELECT COUNT(*) FROM leaves WHERE employee_id = ?1",
            params![emp],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(count, 0);
    let files = std::fs::read_dir(&leaves_root)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(files, 0, "failed queued write must remove leave evidence");
}

#[tokio::test]
async fn cancelled_request_after_leave_job_admission_keeps_committed_evidence() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept = create_test_department_with_shift(
        &db,
        "D-cancelled-request",
        "day",
        false,
        480,
        "09:00",
        "17:00",
    )
    .await;
    let emp = seed_employee(&db, &dept, "E-CANCELLED-REQUEST").await;
    let (state, _tmp) = make_state(db);
    let leaves_root = state.paths.leaves_root.clone();
    let app = build_test_app(state.clone());

    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let blocker_queue = state.db_write.clone();
    let blocker = tokio::spawn(async move {
        blocker_queue
            .job("test.block-cancelled-leave", move |_| {
                Box::pin(async move {
                    release_rx.await.expect("release writer");
                    Ok(())
                })
            })
            .await
    });
    while state.db_write.stats().accepted < 1 {
        tokio::task::yield_now().await;
    }

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-05-03"),
            ("to_date", "2026-05-03"),
            ("leave_type", "medical"),
            ("justification", "request cancellation must not orphan row"),
        ],
        Some(("image/jpeg", &[0xFF, 0xD8, 0xFF, 0xE0][..])),
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", test_access_token(&admin, "admin")),
        )
        .body(Body::from(body))
        .unwrap();
    let request_task = tokio::spawn(app.oneshot(request));
    while state.db_write.stats().accepted < 2 {
        tokio::task::yield_now().await;
    }
    request_task.abort();
    assert!(request_task.await.unwrap_err().is_cancelled());
    release_tx.send(()).expect("release blocker");
    blocker.await.unwrap().unwrap();
    while state.db_write.stats().completed < 2 {
        tokio::task::yield_now().await;
    }

    let conn = state.db.connect().unwrap();
    let evidence_path: String = conn
        .query(
            "SELECT evidence_path FROM leaves WHERE employee_id = ?1",
            params![emp],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .expect("accepted leave transaction committed")
        .get(0)
        .unwrap();
    assert!(
        leaves_root.join(evidence_path).exists(),
        "committed leave row must retain its evidence after request cancellation"
    );
}

#[tokio::test]
async fn leave_filesystem_failure_precedes_queue_admission() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-fs", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E-FS").await;
    let (mut state, tmp) = make_state(db);
    let unusable_root = tmp.path().join("leaves-is-a-file");
    std::fs::write(&unusable_root, b"not a directory").unwrap();
    let mut paths = (*state.paths).clone();
    paths.leaves_root = unusable_root;
    state.paths = Arc::new(paths);
    let app = build_test_app(state.clone());

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-05-02"),
            ("to_date", "2026-05-02"),
            ("leave_type", "medical"),
            ("justification", "filesystem failure"),
        ],
        Some(("image/jpeg", &[0xFF, 0xD8, 0xFF][..])),
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", test_access_token(&admin, "admin")),
        )
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(state.db_write.stats().accepted, 0);
}

// -----------------------------------------------------------------------------
// Optimistic concurrency on cancel: 409 with stale version, 204 with correct
// -----------------------------------------------------------------------------

#[tokio::test]
async fn cancel_leave_optimistic_concurrency() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let leave_id =
        create_test_leave(&db, &emp, "vacation", "2026-04-20", "2026-04-20", &admin).await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state.clone());
    let token = test_access_token(&admin, "admin");

    // Stale version (42) → 409.
    let stale_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/leaves/{}?version=42", leave_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(stale_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "stale version must return 409"
    );

    // Correct version (1) → 204.
    let ok_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/leaves/{}?version=1", leave_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(ok_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "correct version must return 204"
    );

    // Verify row is soft-deleted via direct DB access (state.db is Arc, clonable).
    let conn = state.db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT status, deleted_at FROM leaves WHERE id = ?1",
            params![leave_id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("row");
    let status: String = row.get(0).unwrap();
    let deleted_at: Option<i64> = row.get(1).unwrap();
    assert_eq!(status, "cancelled");
    assert!(deleted_at.is_some());
}

// -----------------------------------------------------------------------------
// LEAVE-03: overlay suppresses work_minutes + EVENTS_ON_LEAVE_DAY
// -----------------------------------------------------------------------------

#[tokio::test]
async fn leave_overlay_suppresses_work_minutes() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    seed_device(&db, "dev-1").await;

    // Seed leave covering 2026-04-20 AND events on that day.
    let _leave_id =
        create_test_leave(&db, &emp, "medical", "2026-04-20", "2026-04-20", &admin).await;
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    seed_event(&db, &emp, "dev-1", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp, "dev-1", "exit", caracas_epoch(anchor, 17, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp, anchor)
        .await
        .expect("recompute");

    // Assert DailyRecord shape: overlay suppresses everything.
    let conn = state.db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT work_minutes, overtime_minutes, late_minutes, early_departure_minutes, leave_id \
             FROM daily_records WHERE employee_id = ?1",
            params![emp.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("row");
    let work: i64 = row.get(0).unwrap();
    let ot: i64 = row.get(1).unwrap();
    let late: i64 = row.get(2).unwrap();
    let early: i64 = row.get(3).unwrap();
    let leave_id: Option<String> = row.get(4).unwrap();
    assert_eq!(work, 0, "leave overlay must zero work_minutes");
    assert_eq!(ot, 0, "leave overlay must zero overtime_minutes");
    assert_eq!(late, 0, "leave overlay must zero late_minutes");
    assert_eq!(early, 0, "leave overlay must zero early_departure_minutes");
    assert!(
        leave_id.is_some(),
        "leave_id must be set on the DailyRecord"
    );

    // EVENTS_ON_LEAVE_DAY anomaly must be present.
    let mut arows = conn
        .query(
            "SELECT COUNT(*) FROM daily_record_anomalies dra \
             JOIN daily_records dr ON dr.id = dra.daily_record_id \
             WHERE dr.employee_id = ?1 AND dra.code = 'EVENTS_ON_LEAVE_DAY'",
            params![emp.clone()],
        )
        .await
        .unwrap();
    let anom_count: i64 = arows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(anom_count, 1, "EVENTS_ON_LEAVE_DAY anomaly must be raised");

    // Events must STILL exist in attendance_events (D-16: overlay does not
    // purge raw events; append-only invariant preserved).
    let mut erows = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE employee_id = ?1",
            params![emp],
        )
        .await
        .unwrap();
    let event_count: i64 = erows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(
        event_count, 2,
        "raw events must remain in event store (append-only)"
    );
}

// -----------------------------------------------------------------------------
// LEAVE-04: medical flag preserved via JOIN
// -----------------------------------------------------------------------------

#[tokio::test]
async fn leave_overlay_medical_flag_preserved() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;

    let _leave_id =
        create_test_leave(&db, &emp, "medical", "2026-04-20", "2026-04-20", &admin).await;
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp, anchor)
        .await
        .expect("recompute");

    // JOIN daily_records → leaves and read leaves.leave_type — LEAVE-04
    // guarantees the medical flag is recoverable for Phase 5 IVSS reporting.
    let conn = state.db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT l.leave_type FROM daily_records dr \
             JOIN leaves l ON l.id = dr.leave_id \
             WHERE dr.employee_id = ?1",
            params![emp],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("join row");
    let lt: String = row.get(0).unwrap();
    assert_eq!(
        lt, "medical",
        "leaves.leave_type must be 'medical' when resolved via leave_id FK"
    );
}

// -----------------------------------------------------------------------------
// T-3-19: RBAC — supervisor/viewer cannot create or cancel leaves
// -----------------------------------------------------------------------------

#[tokio::test]
async fn create_leave_forbidden_for_supervisor() {
    let db = common::test_db().await;
    let supervisor = create_test_supervisor(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&supervisor, "supervisor");

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-20"),
            ("leave_type", "manual"),
            ("justification", "Supervisor shouldn't be able to do this"),
        ],
        None,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "supervisor must NOT be able to create leaves (require_admin)"
    );
}

#[tokio::test]
async fn create_leave_forbidden_for_viewer() {
    let db = common::test_db().await;
    let viewer = create_test_viewer(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&viewer, "viewer");

    let (body, ct) = build_leave_multipart(
        &[
            ("employee_id", &emp),
            ("from_date", "2026-04-20"),
            ("to_date", "2026-04-20"),
            ("leave_type", "manual"),
            ("justification", "Viewer shouldn't be able to do this"),
        ],
        None,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/leaves")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// -----------------------------------------------------------------------------
// T-3-15: evidence path traversal defence
// -----------------------------------------------------------------------------

#[tokio::test]
async fn evidence_path_traversal_rejected() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;

    // Manually seed a leave row with a CRAFTED evidence_path containing `..`
    // to simulate a tampered DB record. The GET /evidence handler must reject
    // it at the traversal-guard before canonicalize runs.
    let malicious_id = Uuid::new_v4().to_string();
    let conn = db.connect().expect("connect");
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
         justification, evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, '2026-04-20', '2026-04-20', 'medical', 'tampered', \
                 '../../../../etc/passwd', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![malicious_id.clone(), emp, admin.clone()],
    )
    .await
    .unwrap();

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&admin, "admin");

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/leaves/{}/evidence", malicious_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "traversal attempt must be rejected as 404 (never reveal server paths)"
    );
    let body_json = body_to_json(resp.into_body()).await;
    assert_eq!(body_json["error"]["code"], "LEAVE_EVIDENCE_NOT_FOUND");
}

// -----------------------------------------------------------------------------
// Read-side: all 3 roles can GET /leaves (Phase 1 D-09)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn list_leaves_accessible_to_viewer() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let viewer = create_test_viewer(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let _leave_id =
        create_test_leave(&db, &emp, "vacation", "2026-04-20", "2026-04-22", &admin).await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let token = test_access_token(&viewer, "viewer");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/leaves")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_json = body_to_json(resp.into_body()).await;
    assert_eq!(body_json["total"], 1);
    let data = body_json["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["leave_type"], "vacation");
}
