//! Integration tests for the Reports Excel export endpoint (Plan 05-03 / PAY-03).
//!
//! Round-trips the bytes returned by `POST /api/v1/reports/excel` through
//! calamine 0.28 (read-only Excel parser, dev-dep only — never in production)
//! to assert:
//! - HTTP status, Content-Type, Content-Disposition (filename quoted per Pitfall 9).
//! - Sheet name = 'Resumen'.
//! - Branding header rows 0-2 (D-28) — title, client_name+RIF, period+generated_at.
//! - Branding header dashes when tenant_info empty (D-28 fallback).
//! - 20-column header row at row 4 (D-14).
//! - Per-dept subtotal rows + grand total row (D-27).
//! - Anomaly column data preserved (D-16 column population — tint is visual-only).
//! - RBAC: Viewer 403, Admin 200.
//! - Audit row written with `format="excel"` (D-21 reuse of compute_report's audit).
//! - Period > 366 days → 422 (W-4 / DoS guard).
//! - 1000-employee performance bench — `#[ignore]` by default; opt-in via
//!   `cargo nextest run --run-ignored only`.

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
    create_test_admin, create_test_viewer, test_access_token, test_device_creds_key,
    TEST_JWT_SECRET,
};
use seed::{
    seed_anomaly, seed_daily_record, seed_dept, seed_employee, seed_leave, set_tenant_branding,
};

// -----------------------------------------------------------------------------
// Harness — mirrors reports_test.rs but registers BOTH /reports/json and
// /reports/excel under the supervisor middleware so the same JWT works.
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
        license_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
    }
}

fn build_test_app(state: AppState) -> Router {
    let report_routes = Router::new()
        .route("/reports/json", post(reports::handlers::generate_json))
        .route("/reports/excel", post(reports::handlers::generate_excel))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));

    Router::new()
        .nest("/api/v1", report_routes)
        .with_state(state)
}

/// POST `/api/v1/reports/excel`. Returns (status, response_headers, body_bytes).
/// `body_bytes` is empty on non-200 responses — callers should branch on status.
async fn post_excel(
    app: &Router,
    token: &str,
    body_json: Value,
) -> (
    StatusCode,
    axum::http::HeaderMap,
    Vec<u8>,
) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/reports/excel")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body_json.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

/// Parse xlsx bytes and return the 'Resumen' worksheet range.
fn parse_xlsx(bytes: Vec<u8>) -> calamine::Range<calamine::Data> {
    use calamine::Reader;
    let cursor = std::io::Cursor::new(bytes);
    let mut wb: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(cursor).expect("open xlsx");
    wb.worksheet_range("Resumen").expect("Resumen sheet")
}

/// Stringify a cell (Float/Int rendered as decimal string, missing → "").
fn cell_string(range: &calamine::Range<calamine::Data>, row: u32, col: u32) -> String {
    range
        .get_value((row, col))
        .map(|v| match v {
            calamine::Data::String(s) => s.clone(),
            calamine::Data::Float(f) => f.to_string(),
            calamine::Data::Int(i) => i.to_string(),
            calamine::Data::Bool(b) => b.to_string(),
            calamine::Data::DateTime(dt) => dt.to_string(),
            calamine::Data::DateTimeIso(s) | calamine::Data::DurationIso(s) => s.clone(),
            calamine::Data::Error(e) => format!("{:?}", e),
            calamine::Data::Empty => String::new(),
        })
        .unwrap_or_default()
}

/// Count audit rows where operation='REPORT_EXPORT' AND new_data->>'$.format' = format_str.
async fn count_export_audit_with_format(db: &libsql::Database, format_str: &str) -> i64 {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM audit_log WHERE operation = 'REPORT_EXPORT' \
             AND json_extract(new_data, '$.format') = ?1",
            libsql::params![format_str.to_string()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    row.get::<i64>(0).unwrap()
}

// -----------------------------------------------------------------------------
// 1. Response headers
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_response_headers() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;

    let app = build_test_app(make_state(db));
    let (status, headers, _bytes) = post_excel(
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
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or(""),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    let cd = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    // The handler emits the request `from_date`/`to_date` verbatim; period
    // monthly anchored at 2026-04-01 → filename embeds the request dates.
    assert!(
        cd.starts_with("attachment; filename=\"prenomina_"),
        "unexpected Content-Disposition: {}",
        cd
    );
    assert!(
        cd.ends_with(".xlsx\""),
        "filename must end with .xlsx and be quoted (Pitfall 9): {}",
        cd
    );
}

// -----------------------------------------------------------------------------
// 2. Round-trip parse via calamine
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_round_trip() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    use calamine::Reader;
    let cursor = std::io::Cursor::new(bytes);
    let mut wb: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(cursor).expect("open xlsx");
    let sheet_names = wb.sheet_names();
    assert!(sheet_names.iter().any(|s| s == "Resumen"));
    let range = wb.worksheet_range("Resumen").expect("Resumen sheet");
    // At minimum: branding header (rows 0-2), spacer (row 3), column header (row 4).
    assert!(
        range.get_size().0 >= 5,
        "expected ≥5 rows, got {:?}",
        range.get_size()
    );
}

// -----------------------------------------------------------------------------
// 3. Branding header rows present (D-28)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_branding_header_present() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    set_tenant_branding(&db, "Acme Industria CA", "J-12345678-9").await;

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    let range = parse_xlsx(bytes);
    // Row 0 (top-left of merged title row).
    let title = cell_string(&range, 0, 0);
    assert!(
        title.contains("Reporte Pre-Nómina"),
        "expected title row, got: {:?}",
        title
    );
    // Row 1 — client_name + RIF.
    let meta = cell_string(&range, 1, 0);
    assert!(
        meta.contains("Acme Industria CA"),
        "row 1 missing client_name: {:?}",
        meta
    );
    assert!(
        meta.contains("J-12345678-9"),
        "row 1 missing RIF: {:?}",
        meta
    );
    // Row 2 — period + generated.
    let period_row = cell_string(&range, 2, 0);
    assert!(
        period_row.contains("Período: "),
        "row 2 missing 'Período: ': {:?}",
        period_row
    );
    assert!(
        period_row.contains("Generado: "),
        "row 2 missing 'Generado: ': {:?}",
        period_row
    );
}

// -----------------------------------------------------------------------------
// 4. Branding header dashes when tenant_info empty (D-28 fallback)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_branding_header_dashes_when_empty() {
    // Default seed leaves client_name + client_rif blank → must render as '—'.
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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
    let range = parse_xlsx(bytes);
    let meta = cell_string(&range, 1, 0);
    // Both client_name and RIF are empty → dash on each side of the meta string.
    let dash_count = meta.matches('—').count();
    assert!(
        dash_count >= 2,
        "expected at least 2 '—' in row 1 (empty client_name + RIF), got {} dashes in: {:?}",
        dash_count,
        meta
    );
}

// -----------------------------------------------------------------------------
// 5. Column headers row 4 (D-14)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_column_headers_present() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let _emp = seed_employee(&db, "E1", "Bob", &dept, "Dev").await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    let range = parse_xlsx(bytes);
    let expected = [
        "Cédula",
        "Nombre",
        "Departamento",
        "Cargo",
        "Min Trab",
        "Min Extra",
        "Min Retraso",
        "Días Trab",
        "Días Aus",
        "Pago Base",
        "Pago Extra",
        "Prima Nocturna",
        "Recargo Domingo",
        "Descuento Retraso",
        "Total a Pagar",
        "Días IVSS",
        "Días Vacación",
        "Días Permiso",
        "Días No Remunerado",
        "Anomalías",
    ];
    for (i, label) in expected.iter().enumerate() {
        let cell = cell_string(&range, 4, i as u32);
        assert_eq!(cell, *label, "column {} mismatch: got {:?}", i, cell);
    }
}

// -----------------------------------------------------------------------------
// 6. Per-dept subtotal rows (D-27)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_dept_subtotals_present() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept_p = seed_dept(&db, "Producción", 100_000, 480, "day").await;
    let dept_a = seed_dept(&db, "Administración", 100_000, 480, "day").await;
    let p1 = seed_employee(&db, "P1", "Pedro", &dept_p, "Dev").await;
    let p2 = seed_employee(&db, "P2", "Pablo", &dept_p, "Dev").await;
    let a1 = seed_employee(&db, "A1", "Ana", &dept_a, "Dev").await;
    let a2 = seed_employee(&db, "A2", "Alicia", &dept_a, "Dev").await;
    let _ = seed_daily_record(&db, &p1, &dept_p, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &p2, &dept_p, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &a1, &dept_a, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &a2, &dept_a, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    let range = parse_xlsx(bytes);
    let (n_rows, _) = range.get_size();
    let mut subtotal_labels: Vec<String> = Vec::new();
    for r in 5..(n_rows as u32) {
        let label = cell_string(&range, r, 1);
        if label.starts_with("Total ") && label != "Total General" {
            subtotal_labels.push(label);
        }
    }
    assert!(
        subtotal_labels.len() >= 2,
        "expected ≥2 dept subtotal rows, got: {:?}",
        subtotal_labels
    );
    assert!(
        subtotal_labels.iter().any(|s| s == "Total Producción"),
        "missing Total Producción in {:?}",
        subtotal_labels
    );
    assert!(
        subtotal_labels.iter().any(|s| s == "Total Administración"),
        "missing Total Administración in {:?}",
        subtotal_labels
    );
}

// -----------------------------------------------------------------------------
// 7. Grand total row (D-27)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_grand_total_present() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept_p = seed_dept(&db, "Producción", 100_000, 480, "day").await;
    let dept_a = seed_dept(&db, "Administración", 100_000, 480, "day").await;
    let p1 = seed_employee(&db, "P1", "Pedro", &dept_p, "Dev").await;
    let a1 = seed_employee(&db, "A1", "Ana", &dept_a, "Dev").await;
    let _ = seed_daily_record(&db, &p1, &dept_p, "2026-04-15", "day", 480, 0, 0, 0, None).await;
    let _ = seed_daily_record(&db, &a1, &dept_a, "2026-04-15", "day", 480, 0, 0, 0, None).await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    let range = parse_xlsx(bytes);
    let (n_rows, _) = range.get_size();
    let mut found_grand = false;
    for r in 5..(n_rows as u32) {
        if cell_string(&range, r, 1) == "Total General" {
            found_grand = true;
            break;
        }
    }
    assert!(found_grand, "Total General row missing");
}

// -----------------------------------------------------------------------------
// 8. Anomaly column data preserved (D-16 column population)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn excel_anomaly_data_present() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    let emp = seed_employee(&db, "E1", "Bob Anomaly", &dept, "Dev").await;
    let dr = seed_daily_record(&db, &emp, &dept, "2026-04-15", "day", 240, 0, 0, 0, None).await;
    seed_anomaly(&db, &dr, "MISSING_ENTRY").await;
    seed_anomaly(&db, &dr, "OT_CAP_EXCEEDED_DAILY").await;

    let app = build_test_app(make_state(db));
    let (status, _, bytes) = post_excel(
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

    let range = parse_xlsx(bytes);
    let (n_rows, _) = range.get_size();
    let mut found = false;
    for r in 5..(n_rows as u32) {
        if cell_string(&range, r, 1) == "Bob Anomaly" {
            let codes = cell_string(&range, r, 19);
            assert!(
                codes.contains("MISSING_ENTRY"),
                "missing MISSING_ENTRY in: {:?}",
                codes
            );
            assert!(
                codes.contains("OT_CAP_EXCEEDED_DAILY"),
                "missing OT_CAP_EXCEEDED_DAILY in: {:?}",
                codes
            );
            found = true;
            break;
        }
    }
    assert!(found, "anomaly employee row not found in xlsx body");
}

// -----------------------------------------------------------------------------
// 9. RBAC: viewer blocked
// -----------------------------------------------------------------------------

#[tokio::test]
async fn viewer_blocked_on_excel() {
    let db = common::test_db().await;
    let viewer = create_test_viewer(&db).await;
    let token = test_access_token(&viewer, "viewer");
    let app = build_test_app(make_state(db));
    let (status, _, _) = post_excel(
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
// 10. Audit row written with format=excel (D-21)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn audit_entry_on_excel_export() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let state = make_state(db);
    let state_db = state.db.clone();
    let app = build_test_app(state);

    let before = count_export_audit_with_format(&state_db, "excel").await;
    let (status, _, _) = post_excel(
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
    let after = count_export_audit_with_format(&state_db, "excel").await;
    assert_eq!(
        after - before,
        1,
        "expected exactly one REPORT_EXPORT row with format='excel'"
    );
}

// -----------------------------------------------------------------------------
// 11. Period > 366 days → 422 (W-4 / DoS guard)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn period_too_long_rejected_excel() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");
    let app = build_test_app(make_state(db));

    let (status, _, body) = post_excel(
        &app,
        &token,
        json!({
            "period_type": "custom",
            "from_date": "2025-01-01",
            "to_date": "2026-12-31",
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "AppError::Validation maps to 422 per errors.rs:88-89"
    );
    // Error body is JSON for the validation error path.
    let body_str = String::from_utf8_lossy(&body);
    let v: Value = serde_json::from_str(&body_str).unwrap_or(json!(null));
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
}

// -----------------------------------------------------------------------------
// 12. 1000-employee perf bench (#[ignore] — opt-in via --run-ignored)
// -----------------------------------------------------------------------------

/// Performance gate per D-22: 1000-employee monthly report must finish in <5s.
/// Marked `#[ignore]` because seed time alone (~30k INSERTs) dominates the
/// non-perf test budget. Run with:
///   cargo nextest run --test reports_excel_test --run-ignored only -- bench_1000_employees_under_5s
#[tokio::test]
#[ignore]
async fn bench_1000_employees_under_5s() {
    use std::time::{Duration, Instant};

    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let token = test_access_token(&admin, "admin");

    let dept = seed_dept(&db, "Eng", 100_000, 480, "day").await;
    // Seed 1000 employees, each with one daily_record per weekday (≈22) in
    // April 2026 — that's ~22k rows. Reduce per-emp to keep seed time
    // tractable; the 1000-row dimension is what matters for the bench.
    for i in 0..1000 {
        let emp = seed_employee(
            &db,
            &format!("E{:04}", i),
            &format!("Emp {}", i),
            &dept,
            "Dev",
        )
        .await;
        for d in 1..=22 {
            let _ = seed_daily_record(
                &db,
                &emp,
                &dept,
                &format!("2026-04-{:02}", d),
                "day",
                480,
                0,
                0,
                0,
                None,
            )
            .await;
        }
    }

    let app = build_test_app(make_state(db));
    let start = Instant::now();
    let (status, _, _bytes) = post_excel(
        &app,
        &token,
        json!({
            "period_type": "monthly",
            "from_date": "2026-04-01",
            "to_date": "2026-04-30",
        }),
    )
    .await;
    let elapsed = start.elapsed();
    assert_eq!(status, StatusCode::OK);
    assert!(
        elapsed < Duration::from_secs(5),
        "1000-employee report took {:?}, expected <5s (D-22)",
        elapsed
    );
}

// -----------------------------------------------------------------------------
// Suppress dead-code warning when only a subset of seed/common helpers is used
// in this test binary.
// -----------------------------------------------------------------------------
#[allow(dead_code)]
fn _silence_unused() {
    // Compile-time guard: ensure the seed helpers we don't use here remain
    // discoverable by name when extending the suite.
    let _ = seed_leave;
}
