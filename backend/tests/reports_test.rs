//! Integration tests for the Reports calculation API (Plan 05-02 / PAY-01..02).
//!
//! Coverage:
//! - Period preset payload mapping (weekly / biweekly_first / biweekly_second / monthly / custom).
//! - Override merge takes precedence over engine work_minutes (Pitfall 3).
//! - Leave overlay treatment per leave_type (medical / vacation / unpaid / manual).
//! - W-5: full-week leaves with no daily_records counted via secondary leaves aggregation.
//! - W-5: leaves overlay attached to a daily_record does NOT double-count.
//! - W-6: night premium gates on daily_records.shift_type, NOT departments.shift_type.
//! - Anomaly column population, anomaly_count == codes.len().
//! - Department subtotals + grand total reconcile with constituent rows.
//! - RBAC: admin + supervisor 200, viewer 403.
//! - Audit log row written on success (operation='REPORT_EXPORT'), NOT on failure (Pitfall 7).
//! - Period-too-long returns HTTP 422 UNPROCESSABLE_ENTITY (matches errors.rs:88-89).
//! - department_ids filter scopes the result set.
//! - Rest-day surcharge gates on daily_records.is_rest_day_worked.
//! - días_trabajados / días_ausentes definitions per D-34.

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

use common::{
    create_test_admin, create_test_supervisor, create_test_viewer, test_access_token,
    test_device_creds_key, TEST_JWT_SECRET,
};
use seed::{
    seed_anomaly, seed_daily_record, seed_dept, seed_employee, seed_inactive_employee,
    seed_leave, seed_override, set_tenant_branding,
};

// -----------------------------------------------------------------------------
// Harness
// -----------------------------------------------------------------------------

fn make_state(db: libsql::Database) -> AppState {
    AppState {
        db: Arc::new(db),
        config: Arc::new(Config {
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
        }),
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
    }
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

/// Fetch the row from the `rows[]` array for a given employee_id. Panics if absent.
fn row_for(payload: &Value, employee_id: &str) -> Value {
    payload["rows"]
        .as_array()
        .expect("rows[] should be an array")
        .iter()
        .find(|r| r["employee_id"] == employee_id)
        .cloned()
        .unwrap_or_else(|| panic!("no row for employee {} in payload {:?}", employee_id, payload))
}

/// Count audit rows for `REPORT_EXPORT` operation, optionally filtered by actor_id.
async fn count_export_audit(db: &libsql::Database, actor_id: Option<&str>) -> i64 {
    let conn = db.connect().expect("connect");
    let (sql, has_actor) = if actor_id.is_some() {
        (
            "SELECT COUNT(*) FROM audit_log WHERE operation = 'REPORT_EXPORT' AND actor_id = ?1",
            true,
        )
    } else {
        (
            "SELECT COUNT(*) FROM audit_log WHERE operation = 'REPORT_EXPORT'",
            false,
        )
    };
    let mut rows = if has_actor {
        conn.query(sql, libsql::params![actor_id.unwrap().to_string()])
            .await
            .unwrap()
    } else {
        conn.query(sql, ()).await.unwrap()
    };
    let row = rows.next().await.unwrap().unwrap();
    row.get::<i64>(0).unwrap()
}

// -----------------------------------------------------------------------------
// Tests — period presets
// -----------------------------------------------------------------------------

/// Verifies all four preset types resolve to the expected `(from_date, to_date)`
/// in the response payload.header. Each sub-block exercises one preset.
#[tokio::test]
async fn period_presets_in_payload() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E001", "Alice", &dept, "Dev").await;
    set_tenant_branding(&db, "Acme", "J-12345").await;

    let app = build_test_app(make_state(db));

    // Weekly: from 2026-04-25 (Sat) → ISO Mon = 2026-04-20, Sun = 2026-04-26.
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "weekly",
            "from_date": "2026-04-25",
            "to_date": "2026-04-25",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "weekly POST should 200: {:?}", body);
    assert_eq!(body["header"]["from_date"], "2026-04-20");
    assert_eq!(body["header"]["to_date"], "2026-04-26");

    // BiweeklyFirst on 2026-04-10 → (2026-04-01, 2026-04-15).
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "biweekly_first",
            "from_date": "2026-04-10",
            "to_date": "2026-04-10",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["header"]["from_date"], "2026-04-01");
    assert_eq!(body["header"]["to_date"], "2026-04-15");

    // BiweeklySecond Feb 2024 (leap) → (2024-02-16, 2024-02-29).
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "biweekly_second",
            "from_date": "2024-02-20",
            "to_date": "2024-02-20",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["header"]["from_date"], "2024-02-16");
    assert_eq!(body["header"]["to_date"], "2024-02-29");

    // Monthly April 2026 → (2026-04-01, 2026-04-30).
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-01",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["header"]["from_date"], "2026-04-01");
    assert_eq!(body["header"]["to_date"], "2026-04-30");
    assert_eq!(body["header"]["client_name"], "Acme");
    assert_eq!(body["header"]["client_rif"], "J-12345");
}

// -----------------------------------------------------------------------------
// Tests — override + leave money treatment
// -----------------------------------------------------------------------------

#[tokio::test]
async fn override_takes_precedence() {
    // Engine wrote work_minutes=120 (2h). Operator override sets it to 480 (8h).
    // Report should reflect 480 (not 120).
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let dr = seed_daily_record(&db, &emp, &dept, "2026-04-15", "day", 120, 0, 0, 0, None).await;
    seed_override(&db, &dr, 480, &admin).await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["work_min"], 480, "override should win, got {:?}", row);
    assert_eq!(row["work_pay_cents"], 100_000, "$1000 full-day pay");
}

#[tokio::test]
async fn medical_leave_excluded() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    // Leave that overlays the daily_record.
    let leave = seed_leave(&db, &emp, "medical", "2026-04-15", "2026-04-15", &admin).await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "day",
        0,
        0,
        0,
        0,
        Some(&leave),
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["total_a_pagar_cents"], 0, "medical → no pay (IVSS)");
    assert_eq!(row["work_pay_cents"], 0);
    assert_eq!(row["days_ivss"], 1, "1 medical-leave day in window");
}

#[tokio::test]
async fn vacation_paid_full() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let leave = seed_leave(&db, &emp, "vacation", "2026-04-15", "2026-04-15", &admin).await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "day",
        0,
        0,
        0,
        0,
        Some(&leave),
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["work_pay_cents"], 100_000, "vacation pays full day");
    assert_eq!(row["total_a_pagar_cents"], 100_000);
    assert_eq!(row["days_vacation"], 1);
}

#[tokio::test]
async fn unpaid_leave_zero_pay() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let leave = seed_leave(&db, &emp, "unpaid", "2026-04-15", "2026-04-15", &admin).await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "day",
        0,
        0,
        0,
        0,
        Some(&leave),
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["total_a_pagar_cents"], 0);
    assert_eq!(row["days_unpaid"], 1);
}

#[tokio::test]
async fn manual_leave_zero_pay() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let leave = seed_leave(&db, &emp, "manual", "2026-04-15", "2026-04-15", &admin).await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "day",
        0,
        0,
        0,
        0,
        Some(&leave),
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["total_a_pagar_cents"], 0);
    assert_eq!(row["days_permission"], 1);
}

// -----------------------------------------------------------------------------
// Tests — W-5 leave-day counting
// -----------------------------------------------------------------------------

/// W-5 critical path: full-week vacation with NO daily_records must surface in
/// the report with days_vacation = 5 (Mon-Fri) and days_absent = 0 (the days
/// are accounted for via leave_dates exclusion).
#[tokio::test]
async fn full_week_vacation_no_captures_counts_correctly() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    // Mon 2026-04-20 → Fri 2026-04-24 (5 weekdays). NO daily_records.
    let _ = seed_leave(&db, &emp, "vacation", "2026-04-20", "2026-04-24", &admin).await;

    let app = build_test_app(make_state(db));
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-20",
            "to_date": "2026-04-26",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(
        row["days_vacation"], 5,
        "5 vacation days Mon-Fri, got: {:?}",
        row
    );
    assert_eq!(
        row["days_absent"], 0,
        "leave_dates should suppress absent count"
    );
    assert_eq!(row["days_worked"], 0);
    assert_eq!(
        row["total_a_pagar_cents"], 0,
        "vacation-without-overlay = no pay (v1 limitation)"
    );
}

/// W-5 medical variant: same shape with leave_type='medical'.
#[tokio::test]
async fn full_week_medical_leave_no_captures() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let _ = seed_leave(&db, &emp, "medical", "2026-04-20", "2026-04-24", &admin).await;

    let app = build_test_app(make_state(db));
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-20",
            "to_date": "2026-04-26",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["days_ivss"], 5);
    assert_eq!(row["days_absent"], 0);
}

/// W-5 no-double-count: a leave overlay attached to a daily_record AND the
/// leave row spanning Mon-Fri must produce days_vacation = 5 (not 6 = 5 from
/// secondary aggregation + 1 from overlay branch). The daily_records branch
/// MUST NOT increment leave-day counters.
#[tokio::test]
async fn leave_overlay_does_not_double_count() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let leave = seed_leave(&db, &emp, "vacation", "2026-04-20", "2026-04-24", &admin).await;
    // One overlay on Monday only. The leave row spans Mon-Fri.
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-20",
        "day",
        0,
        0,
        0,
        0,
        Some(&leave),
    )
    .await;

    let app = build_test_app(make_state(db));
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-20",
            "to_date": "2026-04-26",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(
        row["days_vacation"], 5,
        "5 (not 6) — daily_records branch must not double-count"
    );
}

// -----------------------------------------------------------------------------
// Tests — W-6 shift_type source
// -----------------------------------------------------------------------------

/// W-6 critical path: dr.shift_type drives night premium, NOT departments.shift_type.
/// Sub-case A: dept policy = 'day' but daily_record = 'night' → premium > 0.
/// Sub-case B: dept policy = 'night' but daily_record = 'day' → premium == 0.
#[tokio::test]
async fn night_premium_uses_daily_record_shift_type() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    // Sub-case A: dept = 'day', daily_record = 'night' → premium APPLIED.
    let dept_a = seed_dept(&db, "Day Dept", 100_000, 480, "day").await;
    let emp_a = seed_employee(&db, "A1", "Alice", &dept_a, "Dev").await;
    let _dr_a = seed_daily_record(
        &db,
        &emp_a,
        &dept_a,
        "2026-04-15",
        "night", // ← per-day shift, the W-6 source
        480,
        0,
        0,
        0,
        None,
    )
    .await;

    // Sub-case B: dept = 'night', daily_record = 'day' → premium NOT applied.
    let dept_b = seed_dept(&db, "Night Dept", 100_000, 480, "night").await;
    let emp_b = seed_employee(&db, "B1", "Bob", &dept_b, "Dev").await;
    let _dr_b = seed_daily_record(
        &db,
        &emp_b,
        &dept_b,
        "2026-04-15",
        "day", // ← per-day shift wins
        480,
        0,
        0,
        0,
        None,
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);

    let row_a = row_for(&body, &emp_a);
    assert_eq!(
        row_a["night_premium_cents"], 30_000,
        "dr.shift_type='night' → +30% premium ($300), got {:?}",
        row_a
    );

    let row_b = row_for(&body, &emp_b);
    assert_eq!(
        row_b["night_premium_cents"], 0,
        "dr.shift_type='day' → no premium even with dept='night', got {:?}",
        row_b
    );
}

/// Existing variant: positive case where dept and dr both say 'night'.
#[tokio::test]
async fn shift_type_night_premium_applied() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Night Dept", 100_000, 480, "night").await;
    let emp = seed_employee(&db, "N1", "Nina", &dept, "Sec").await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "night",
        480,
        0,
        0,
        0,
        None,
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["night_premium_cents"], 30_000);
}

/// Existing variant: dr.shift_type='day' even with work > 0 → no premium.
#[tokio::test]
async fn shift_type_day_no_night_premium() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Day Dept", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "D1", "Dan", &dept, "Dev").await;
    let _dr = seed_daily_record(
        &db,
        &emp,
        &dept,
        "2026-04-15",
        "day",
        480,
        0,
        0,
        0,
        None,
    )
    .await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    assert_eq!(row["night_premium_cents"], 0);
}

// -----------------------------------------------------------------------------
// Tests — anomaly column, subtotals, grand total
// -----------------------------------------------------------------------------

#[tokio::test]
async fn anomaly_codes_in_payload() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let dr = seed_daily_record(&db, &emp, &dept, "2026-04-15", "day", 240, 0, 0, 0, None).await;
    seed_anomaly(&db, &dr, "MISSING_EXIT").await;
    seed_anomaly(&db, &dr, "OT_CAP_EXCEEDED_DAILY").await;

    let app = build_test_app(make_state(db));
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
    assert_eq!(status, StatusCode::OK);
    let row = row_for(&body, &emp);
    let codes: Vec<String> = row["anomaly_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(codes.contains(&"MISSING_EXIT".to_string()));
    assert!(codes.contains(&"OT_CAP_EXCEEDED_DAILY".to_string()));
    assert_eq!(row["anomaly_count"], codes.len() as i64);
}

#[tokio::test]
async fn subtotals_match_constituents() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let e1 = seed_employee(&db, "E1", "Alice", &dept, "Dev").await;
    let e2 = seed_employee(&db, "E2", "Bob", &dept, "Dev").await;
    let _ = seed_daily_record(&db, &e1, &dept, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &e2, &dept, "2026-04-15", "day", 240, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    let subtotals = body["dept_subtotals"].as_array().unwrap();
    assert_eq!(subtotals.len(), 1);
    let sub = &subtotals[0];
    let row1 = row_for(&body, &e1);
    let row2 = row_for(&body, &e2);
    let sum_work_min = row1["work_min"].as_i64().unwrap() + row2["work_min"].as_i64().unwrap();
    let sum_pay = row1["work_pay_cents"].as_i64().unwrap()
        + row2["work_pay_cents"].as_i64().unwrap();
    assert_eq!(sub["aggregates"]["work_min"], sum_work_min);
    assert_eq!(sub["aggregates"]["work_pay_cents"], sum_pay);
}

#[tokio::test]
async fn grand_total_matches_subtotals() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept_a = seed_dept(&db, "Alpha", 100_000, 480, "day").await;
    let dept_b = seed_dept(&db, "Beta", 200_000, 480, "day").await;
    let ea = seed_employee(&db, "A1", "Alice", &dept_a, "Dev").await;
    let eb = seed_employee(&db, "B1", "Bob", &dept_b, "Dev").await;
    let _ = seed_daily_record(&db, &ea, &dept_a, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &eb, &dept_b, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    let subtotals = body["dept_subtotals"].as_array().unwrap();
    let sum_subtotal_pay: i64 = subtotals
        .iter()
        .map(|s| s["aggregates"]["work_pay_cents"].as_i64().unwrap())
        .sum();
    assert_eq!(body["grand_total"]["work_pay_cents"], sum_subtotal_pay);
    assert_eq!(body["grand_total"]["work_pay_cents"], 100_000 + 200_000);
}

// -----------------------------------------------------------------------------
// Tests — RBAC
// -----------------------------------------------------------------------------

#[tokio::test]
async fn admin_can_export() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let app = build_test_app(make_state(db));
    let (status, _) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn supervisor_can_export() {
    let db = common::test_db().await;
    let sup = create_test_supervisor(&db).await;
    let token = test_access_token(&sup, "supervisor");
    let app = build_test_app(make_state(db));
    let (status, _) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn viewer_blocked_on_export() {
    let db = common::test_db().await;
    let v = create_test_viewer(&db).await;
    let token = test_access_token(&v, "viewer");
    let app = build_test_app(make_state(db));
    let (status, _) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// -----------------------------------------------------------------------------
// Tests — audit log
// -----------------------------------------------------------------------------

#[tokio::test]
async fn audit_entry_on_export() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let state = make_state(db);
    // Clone the Arc<Database> so we can verify audit_log after the POST.
    let state_db = state.db.clone();
    let app = build_test_app(state);

    let before = count_export_audit(&state_db, Some(&admin)).await;
    let (status, _) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let after = count_export_audit(&state_db, Some(&admin)).await;
    assert_eq!(after - before, 1, "exactly one REPORT_EXPORT row written");
}

#[tokio::test]
async fn no_audit_on_failure() {
    // Pitfall 7: failed report (period > 366d) must NOT write an audit row.
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let state = make_state(db);
    let state_db = state.db.clone();
    let app = build_test_app(state);

    let before = count_export_audit(&state_db, None).await;
    let (status, _) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2024-01-01",
            "to_date": "2025-12-31",
        }),
    )
    .await;
    assert!(status.is_client_error(), "should reject, got {}", status);
    let after = count_export_audit(&state_db, None).await;
    assert_eq!(after, before, "no audit row on failure");
}

// -----------------------------------------------------------------------------
// Tests — DoS guard, filters, edge cases
// -----------------------------------------------------------------------------

#[tokio::test]
async fn period_too_long_rejected() {
    // 2024-01-01 .. 2025-12-31 spans 731 days (> 366). Must return 422.
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let app = build_test_app(make_state(db));

    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2024-01-01",
            "to_date": "2025-12-31",
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "AppError::Validation maps to 422 per errors.rs:88-89"
    );
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    let msg = body["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("Period range cannot exceed 366 days"),
        "expected 366-day message, got: {}",
        msg
    );
}

#[tokio::test]
async fn department_filter_applied() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept_a = seed_dept(&db, "Alpha", 100_000, 480, "day").await;
    let dept_b = seed_dept(&db, "Beta", 100_000, 480, "day").await;
    let ea = seed_employee(&db, "A1", "Alice", &dept_a, "Dev").await;
    let eb = seed_employee(&db, "B1", "Bob", &dept_b, "Dev").await;
    let _ = seed_daily_record(&db, &ea, &dept_a, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &eb, &dept_b, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (status, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "department_ids": [dept_a],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["employee_id"], ea);
    assert!(
        rows.iter().all(|r| r["employee_id"] != Value::String(eb.clone())),
        "Beta dept employee should be excluded"
    );
}

#[tokio::test]
async fn rest_day_surcharge_only_when_flagged() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let e1 = seed_employee(&db, "E1", "Alice", &dept, "Dev").await;
    let e2 = seed_employee(&db, "E2", "Bob", &dept, "Dev").await;
    // E1 worked rest day flagged
    let _ = seed_daily_record(&db, &e1, &dept, "2026-04-19", "day", 480, 0, 0, 1, None).await;
    // E2 worked normal day (rest_day_worked=0)
    let _ = seed_daily_record(&db, &e2, &dept, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    let row1 = row_for(&body, &e1);
    let row2 = row_for(&body, &e2);
    assert_eq!(row1["rest_day_surcharge_cents"], 50_000);
    assert_eq!(row2["rest_day_surcharge_cents"], 0);
}

#[tokio::test]
async fn dias_trabajados_count() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    // 3 days with work_minutes>0, 1 day with work_minutes=0 → days_worked=3
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-13", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-14", "day", 240, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-15", "day", 60, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-16", "day", 0, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    let row = row_for(&body, &emp);
    assert_eq!(row["days_worked"], 3);
}

#[tokio::test]
async fn dias_ausentes_weekday_only() {
    // Period: 2026-04-13 (Mon) .. 2026-04-19 (Sun). 5 weekdays Mon-Fri.
    // Employee worked exactly 1 day. Expected days_absent = 4 (other 4 weekdays).
    // Saturday + Sunday must NOT count toward days_absent.
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;
    let _ = seed_daily_record(&db, &emp, &dept, "2026-04-13", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2026-04-13",
            "to_date": "2026-04-19",
        }),
    )
    .await;
    let row = row_for(&body, &emp);
    assert_eq!(
        row["days_absent"], 4,
        "5 weekdays - 1 worked = 4 absent (weekend excluded), got: {:?}",
        row
    );
}

/// Sanity check: include_inactive=false (default) excludes inactive employees
/// even if they have daily_records in the period. include_inactive=true brings
/// them back. Acts as a guard that the predicate is parameterized correctly.
#[tokio::test]
async fn include_inactive_filter_works() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let active = seed_employee(&db, "A1", "Active", &dept, "Dev").await;
    let inactive = seed_inactive_employee(&db, "I1", "Inactive", &dept).await;
    let _ = seed_daily_record(&db, &active, &dept, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(
        &db, &inactive, &dept, "2026-04-15", "day", 480, 0, 0, 0, None,
    )
    .await;

    let app = build_test_app(make_state(db));
    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "include_inactive": false,
        }),
    )
    .await;
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "default excludes inactive");
    assert_eq!(rows[0]["employee_id"], active);

    let (_, body) = post_report(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
            "include_inactive": true,
        }),
    )
    .await;
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "include_inactive=true brings inactive back");
}
