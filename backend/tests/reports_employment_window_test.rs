//! C-05: the report's universe must come from employment validity
//! (`employees` bounded by `hire_date`/`terminated_on`), not from whoever
//! happened to leave a `daily_records` row. Before this fix, the report was
//! built `FROM daily_records dr JOIN employees e` — an active employee who
//! never clocked in didn't appear at all (no absence, no deduction, no
//! alert), and was manipulable: suppress the events and the person vanishes
//! from payroll.
//!
//! Coverage:
//! - An employee with zero daily_records still appears, with zero worked
//!   minutes and full-period absences.
//! - A mid-period hire does not accrue absences before `hire_date`.
//! - A mid-period termination keeps the days actually worked and stops
//!   accruing absences afterward — this is the grave case: the report used
//!   to make a terminated employee (and their pending pay) disappear
//!   entirely once `status` flipped to 'inactive'.

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
// Harness (mirrors tests/reports_test.rs — integration tests are separate
// binaries and cannot share code across files other than through `mod`).
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
// Tests
// -----------------------------------------------------------------------------

/// C-05: an active employee with no marks at all must appear as absent.
/// Today they disappear, and with them their deduction and their alert.
#[tokio::test]
async fn an_employee_with_no_events_at_all_appears_as_absent() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let with_events = seed_employee(&db, "A1", "Alice", &dept, "Dev").await;
    let no_events = seed_employee(&db, "B1", "Bob", &dept, "Dev").await;

    // Period: Mon 2026-04-13 .. Fri 2026-04-17 — 5 weekdays, no weekend to
    // reason about. Alice works all 5; Bob has zero daily_records.
    for day in ["2026-04-13", "2026-04-14", "2026-04-15", "2026-04-16", "2026-04-17"] {
        let _ = seed_daily_record(&db, &with_events, &dept, day, "day", 480, 0, 0, 0, None).await;
    }

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
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

    // Both employees appear — this is the defect: `no_events` used to be
    // absent from `rows[]` entirely.
    let alice = row_for(&body, &with_events);
    let bob = find_row(&body, &no_events)
        .unwrap_or_else(|| panic!("C-05: employee with zero daily_records vanished from the report: {:?}", body));

    assert_eq!(alice["days_worked"], 5);
    assert_eq!(alice["days_absent"], 0);

    assert_eq!(bob["work_min"], 0, "no clock events -> zero worked minutes");
    assert_eq!(bob["days_worked"], 0);
    assert_eq!(
        bob["days_absent"], 5,
        "5 weekdays, zero worked, zero leave -> 5 absences: {:?}",
        bob
    );
    assert_eq!(
        bob["total_a_pagar_cents"], 0,
        "nothing worked -> nothing paid"
    );

    // seed_employee() leaves hire_date NULL — an unbounded employment start
    // is not computable with certainty, so it must surface as a visible
    // anomaly rather than being silently assumed to start at the period.
    let bob_codes: Vec<String> = bob["anomaly_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        bob_codes.contains(&"HIRE_DATE_MISSING".to_string()),
        "missing hire_date must be a visible anomaly, got: {:?}",
        bob_codes
    );
}

/// An employee hired mid-period must not accrue absences before their
/// hire_date.
#[tokio::test]
async fn absences_do_not_start_before_the_hire_date() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Period: Mon 2026-04-13 .. Fri 2026-04-17 (5 weekdays).
    // Hired Thu 2026-04-16 -> only Thu+Fri (2 weekdays) are inside the
    // employment window. No daily_records at all.
    let emp = seed_employee_with_window(
        &db,
        "C1",
        "Carla",
        &dept,
        "Dev",
        "active",
        Some("2026-04-16"),
        None,
    )
    .await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
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

    let row = row_for(&body, &emp);
    assert_eq!(
        row["days_absent"], 2,
        "only Thu+Fri fall on/after hire_date 2026-04-16, not the full 5-weekday period: {:?}",
        row
    );
    assert_eq!(row["days_worked"], 0);

    // hire_date IS set -> no HIRE_DATE_MISSING anomaly.
    let codes: Vec<String> = row["anomaly_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        !codes.contains(&"HIRE_DATE_MISSING".to_string()),
        "hire_date is set, must not be flagged: {:?}",
        codes
    );
}

/// A terminated employee keeps the days they worked and stops accruing
/// absences afterward. This is the grave case: filtering the report by
/// `status = 'active'` used to make a mid-period leaver — and their pending
/// pay for the days they DID work — disappear entirely.
#[tokio::test]
async fn a_terminated_employee_keeps_worked_days_and_stops_accruing_absences() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Hired well before the period; period is Mon 2026-04-13 .. Fri
    // 2026-04-17 (5 weekdays). Worked Mon+Tue, terminated Tue (last worked
    // day is the termination day, inclusive) -> status flips to 'inactive'.
    let emp = seed_employee_with_window(
        &db,
        "D1",
        "Dario",
        &dept,
        "Dev",
        "inactive",
        Some("2020-01-01"),
        Some("2026-04-14"),
    )
    .await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-13", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-14", "day", 480, 0, 0, 0, None).await;

    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-13",
            "to_date": "2026-04-17",
            // default (false): the C-05 fix means this must NOT be required
            // to see a terminated employee's worked days — employment
            // validity, not `status`, decides who is in the report.
            "include_inactive": false,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "report should 200: {:?}", body);

    let row = find_row(&body, &emp).unwrap_or_else(|| {
        panic!(
            "C-05 grave case: a terminated employee with worked days — and pending \
             pay for them — vanished from the report entirely: {:?}",
            body
        )
    });

    assert_eq!(row["days_worked"], 2, "Mon+Tue were worked: {:?}", row);
    assert_eq!(
        row["days_absent"], 0,
        "Wed/Thu/Fri are after terminated_on and must not accrue absences: {:?}",
        row
    );
    assert!(
        row["total_a_pagar_cents"].as_i64().unwrap() > 0,
        "worked days before termination must still be paid: {:?}",
        row
    );
}
