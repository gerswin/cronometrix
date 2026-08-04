//! M-04: two related report/engine defects around leave handling.
//!
//! 1. The `shift_type` report filter is applied to the main `daily_records`
//!    query (see W-6 / Important-1 fixes in `reports/service.rs`) but was
//!    NOT applied to the secondary `leaves` aggregation query (W-5). A report
//!    filtered to a single shift therefore silently pulled in leave days from
//!    every other shift. This file locks the fix.
//!
//! 2. `backend/src/calc/engine.rs:15-37` zeroes the whole day the moment a
//!    leave overlay is present — there is no concept of a half-day leave.
//!    We looked at the schema before committing to that half of the task:
//!    `leaves` (migration `010_leaves.sql`) is explicitly "Full-day only
//!    (D-14)" — `from_date`/`to_date` are whole calendar dates, not
//!    timestamps, and that constraint is echoed in `leaves/mod.rs` (D-14) and
//!    baked into evidence upload / approval flow around it. Supporting a
//!    half-day leave needs a schema migration (an interval column, e.g.
//!    `half` or `from_time`/`to_time`), a DTO change, an engine overlay
//!    rewrite (partial-minute zeroing instead of whole-day), and UI work in
//!    the leave request form — a different, larger task than "roughly a
//!    one-line fix." Per the task brief's explicit escape hatch, that half is
//!    deferred; no test for it is written here since committing a test for
//!    functionality that intentionally isn't implemented would just be a
//!    permanently-failing/ignored test, not a lock on real behavior.

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
use seed::{seed_dept, seed_employee, seed_leave};

// -----------------------------------------------------------------------------
// Harness (mirrors tests/reports_employment_window_test.rs — integration
// tests are separate binaries and cannot share code across files other than
// through `mod`).
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

fn find_row(payload: &Value, employee_id: &str) -> Option<Value> {
    payload["rows"]
        .as_array()
        .expect("rows[] should be an array")
        .iter()
        .find(|r| r["employee_id"] == employee_id)
        .cloned()
}

fn row_for(payload: &Value, employee_id: &str) -> Value {
    find_row(payload, employee_id).unwrap_or_else(|| {
        panic!(
            "no row for employee {} in payload {:?}",
            employee_id, payload
        )
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

/// M-04: the `shift_type` filter must apply to the `leaves` secondary
/// aggregation the same way it applies to the main `daily_records` JOIN.
///
/// Two employees, no daily_records for either (pure leave-only week, the W-5
/// case), one in a "day" department and one in a "night" department. Both
/// take a full-week vacation with zero biometric captures.
///
/// Unfiltered, both must show 5 days_vacation / 0 days_absent (W-5 baseline).
/// Filtered to `shift_type: "night"`, the day-shift employee's leave must
/// stop being counted as vacation — those days fall back to days_absent,
/// exactly the same degrade the main query already applies to worked days
/// under a shift mismatch. Before this fix, the leave query ignored
/// `shift_type` entirely and both employees kept their 5 days_vacation
/// regardless of the filter.
#[tokio::test]
async fn the_shift_filter_applies_to_the_leave_query_too() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept_day = seed_dept(&db, "Eng Day", 100_000, 480, "day").await;
    let dept_night = seed_dept(&db, "Eng Night", 100_000, 480, "night").await;
    let day_employee = seed_employee(&db, "D1", "Diana", &dept_day, "Dev").await;
    let night_employee = seed_employee(&db, "N1", "Nico", &dept_night, "Dev").await;

    // Mon 2026-04-13 .. Fri 2026-04-17 — 5 weekdays, no weekend to reason
    // about. Both employees are on vacation the whole period, zero captures.
    seed_leave(
        &db,
        &day_employee,
        "vacation",
        "2026-04-13",
        "2026-04-17",
        &admin,
    )
    .await;
    seed_leave(
        &db,
        &night_employee,
        "vacation",
        "2026-04-13",
        "2026-04-17",
        &admin,
    )
    .await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    // Sanity check (unfiltered): both employees show a full week of vacation.
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-13",
            "to_date": "2026-04-17",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    let diana = row_for(&body, &day_employee);
    assert_eq!(diana["days_vacation"], 5, "sanity unfiltered: {:?}", diana);
    assert_eq!(diana["days_absent"], 0, "sanity unfiltered: {:?}", diana);

    let nico = row_for(&body, &night_employee);
    assert_eq!(nico["days_vacation"], 5, "sanity unfiltered: {:?}", nico);
    assert_eq!(nico["days_absent"], 0, "sanity unfiltered: {:?}", nico);

    // Filtered to night: the day-shift employee's leave must NOT be counted
    // as vacation anymore — this is the defect. Before the fix, the leaves
    // secondary query had no shift_type predicate at all, so `diana` kept
    // days_vacation == 5 under a "night" filter despite being a day-shift
    // employee.
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-13",
            "to_date": "2026-04-17",
            "shift_type": "night",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    let diana = row_for(&body, &day_employee);
    assert_eq!(
        diana["days_vacation"], 0,
        "M-04: shift_type=night must exclude the day-shift employee's leave \
         from the vacation count: {:?}",
        diana
    );
    assert_eq!(
        diana["days_absent"], 5,
        "M-04: days excluded from the leave count by the shift filter must \
         fall back to days_absent, mirroring how the main query already \
         treats a shift mismatch on worked days: {:?}",
        diana
    );

    let nico = row_for(&body, &night_employee);
    assert_eq!(
        nico["days_vacation"], 5,
        "the matching-shift employee's leave must remain counted: {:?}",
        nico
    );
    assert_eq!(nico["days_absent"], 0, "unaffected by the filter: {:?}", nico);
}
