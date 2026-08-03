//! I5 + I7: the report's universe must be bounded to employment that
//! OVERLAPS the requested period, and one bad row must never fail the
//! report for everyone else.
//!
//! I5 background: C-05 (see `reports_employment_window_test.rs`) correctly
//! removed `e.status = 'active'` as a report gate — that filter was making a
//! mid-period leaver (and their pending pay) disappear entirely. But
//! `status` was also implicitly bounding the report's universe to *some*
//! set of employees. Once it was removed with nothing replacing that bound,
//! ANY employee ever created — including one terminated years before the
//! requested period — produces an all-zeros row in every report forever.
//! The fix bounds the universe to employment that overlaps `[from..to]`:
//! `e.terminated_on IS NULL OR e.terminated_on >= from` AND
//! `e.hire_date IS NULL OR e.hire_date <= to`. Both predicates read
//! `e.*` only, so they live in the WHERE clause without degrading the
//! `daily_records` LEFT JOIN (unlike `shift_type`, which had to move into
//! the JOIN's ON clause for exactly that reason — see Important 1).
//!
//! I7 background: `require_salary_kind` used to return `Result` and get
//! `?`-propagated, so ONE employee with a NULL `salary_kind` produced a 500
//! for the whole report — every OTHER employee became uncomputable because
//! of one unrelated row. It now returns `Option`; a missing salary_kind
//! degrades to a per-row `SALARY_KIND_MISSING` anomaly with that row's
//! money left at zero (never assumed `Daily` — that's the H-08 defect
//! verbatim), exactly like `HIRE_DATE_MISSING` already does for a missing
//! hire date.

mod common;

#[path = "fixtures/reports/seed.rs"]
mod seed;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::reports;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use common::{create_test_admin, test_access_token, test_device_creds_key, TEST_JWT_SECRET};
use seed::{seed_daily_record, seed_dept, seed_employee, seed_employee_with_window};

// -----------------------------------------------------------------------------
// Harness (mirrors tests/reports_test.rs / reports_employment_window_test.rs
// — integration tests are separate binaries and cannot share code across
// files other than through `mod`).
// -----------------------------------------------------------------------------

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
    let report_routes = Router::new()
        .route("/reports/json", post(reports::handlers::generate_json))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));

    Router::new()
        .nest("/api/v1", report_routes)
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn post_report(app: &Router, token: &str, body_json: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/reports/json")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body_json.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = body_to_json(resp.into_body()).await;
    (status, body)
}

/// Fetch the row from the `rows[]` array for a given employee_id, if present.
fn find_row(payload: &Value, employee_id: &str) -> Option<Value> {
    payload["rows"]
        .as_array()
        .expect("rows[] should be an array")
        .iter()
        .find(|r| r["employee_id"] == employee_id)
        .cloned()
}

/// Same as `find_row` but panics if absent — used once the test has already
/// asserted presence via `find_row`.
fn row_for(payload: &Value, employee_id: &str) -> Value {
    find_row(payload, employee_id).unwrap_or_else(|| {
        panic!(
            "no row for employee {} in payload {:?}",
            employee_id, payload
        )
    })
}

// -----------------------------------------------------------------------------
// I5 — universe bound to employment overlapping the period
// -----------------------------------------------------------------------------

/// I5: someone terminated well before the period starts must not appear at
/// all — this is the exact defect: an employee deactivated in 2024 was
/// producing an all-zeros row in every 2026 report.
#[tokio::test]
async fn an_employee_terminated_before_the_period_does_not_appear() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Hired in 2020, terminated in 2024 — nowhere near the 2026 report
    // period requested below.
    let gone = seed_employee_with_window(
        &db,
        "G1",
        "Gone",
        &dept,
        "Dev",
        "inactive",
        Some("2020-01-01"),
        Some("2024-06-30"),
    )
    .await;
    let still_here = seed_employee(&db, "S1", "Still", &dept, "Dev").await;
    let _ = seed_daily_record(&db, &still_here, &dept, "2026-04-15", "day", 480, 0, 0, 0, None)
        .await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    assert!(
        find_row(&body, &gone).is_none(),
        "I5: an employee terminated in 2024 must not appear in a 2026 \
         report at all — not even as an all-zeros row: {:?}",
        body
    );
    assert!(
        find_row(&body, &still_here).is_some(),
        "sanity: a currently-employed worker with events in the period \
         must still appear: {:?}",
        body
    );
}

/// I5: someone hired after the period ends hasn't started yet and must not
/// appear either — the employment window bound is symmetric.
#[tokio::test]
async fn an_employee_hired_after_the_period_does_not_appear() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Hired in May 2026 — after the April 2026 period requested below.
    let future = seed_employee_with_window(
        &db,
        "F1",
        "Future",
        &dept,
        "Dev",
        "active",
        Some("2026-05-01"),
        None,
    )
    .await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    assert!(
        find_row(&body, &future).is_none(),
        "I5: an employee hired after the period ends must not appear: {:?}",
        body
    );
}

/// I5: the case that must not break. An employee whose employment window
/// OVERLAPS the period — terminated partway through it — must still appear,
/// exactly as C-05 already guarantees. Bounding the universe must not
/// re-narrow it back to the C-05 defect.
#[tokio::test]
async fn an_employee_whose_employment_overlaps_the_period_appears() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Hired well before the period, terminated partway through the
    // requested April 2026 period — employment clearly overlaps.
    let mid_leaver = seed_employee_with_window(
        &db,
        "M1",
        "MidLeaver",
        &dept,
        "Dev",
        "inactive",
        Some("2020-01-01"),
        Some("2026-04-10"),
    )
    .await;
    let _ = seed_daily_record(&db, &mid_leaver, &dept, "2026-04-06", "day", 480, 0, 0, 0, None)
        .await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    let row = row_for(&body, &mid_leaver);
    assert_eq!(
        row["days_worked"], 1,
        "the day worked before termination must still be paid: {:?}",
        row
    );
    assert!(
        row["total_a_pagar_cents"].as_i64().unwrap() > 0,
        "worked day before mid-period termination must still be paid: {:?}",
        row
    );
}

// -----------------------------------------------------------------------------
// I7 — one bad row must not fail the whole report
// -----------------------------------------------------------------------------

/// I7: an employee with a NULL salary_kind must not 500 the report for
/// everyone else. It degrades to a per-row anomaly — money stays at zero for
/// THAT row, and every other employee's payroll computes normally.
#[tokio::test]
async fn one_employee_without_salary_kind_does_not_break_the_whole_report() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let good = seed_employee(&db, "OK1", "Okay", &dept, "Dev").await;
    let bad = seed_employee(&db, "BAD1", "Bad", &dept, "Dev").await;
    db.connect()
        .unwrap()
        .execute(
            "UPDATE employees SET salary_kind = NULL WHERE id = ?1",
            libsql::params![bad.clone()],
        )
        .await
        .unwrap();

    let _ = seed_daily_record(&db, &good, &dept, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &bad, &dept, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-15",
            "to_date": "2026-04-15",
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "I7: one employee's missing salary_kind must not fail the report \
         for everyone else: {:?}",
        body
    );

    let good_row = row_for(&body, &good);
    assert!(
        good_row["total_a_pagar_cents"].as_i64().unwrap() > 0,
        "the OTHER employee's payroll must compute normally: {:?}",
        good_row
    );

    let bad_row = row_for(&body, &bad);
    assert_eq!(
        bad_row["total_a_pagar_cents"], 0,
        "I7: a row with no salary_kind never invents a unit to keep going \
         (that is H-08 verbatim) — money stays at zero: {:?}",
        bad_row
    );
    assert_eq!(
        bad_row["days_worked"], 1,
        "time-based facts are not invented data and still accumulate — \
         only the money math is skipped: {:?}",
        bad_row
    );
    let bad_codes: Vec<String> = bad_row["anomaly_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        bad_codes.contains(&"SALARY_KIND_MISSING".to_string()),
        "missing salary_kind must be a visible anomaly, got: {:?}",
        bad_codes
    );
}
