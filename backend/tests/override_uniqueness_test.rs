//! C-04: only one active `daily_record_overrides` row per `daily_record_id`.
//!
//! Before this fix, `idx_overrides_record` (migration 009) had no UNIQUE
//! constraint and filtered on `deleted_at IS NULL` instead of `status`, so
//! several rows could be `active` for the same daily record at once. The
//! `LEFT JOIN ... ON dro.status = 'active'` in `reports/service.rs` then
//! multiplied the report row for every extra active override, duplicating
//! days, minutes, and amounts.
//!
//! Migration 025 adds a UNIQUE partial index on
//! `(daily_record_id) WHERE status = 'active' AND deleted_at IS NULL`, and
//! `daily_records::handlers::create_override` now revokes any existing
//! active override before inserting the new one, in the same transaction —
//! otherwise a legitimate second override would fail the unique constraint,
//! or (if revoke and insert were separate operations) leave a window with no
//! active override at all.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::daily_records;
use cronometrix_api::state::AppState;
use libsql::params;
use tower::ServiceExt;
use uuid::Uuid;

use common::{create_test_department_with_shift, test_access_token, test_device_creds_key, TEST_JWT_SECRET};

const MINI_PDF: &[u8] = b"%PDF-1.4\n%fake\n";

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
        .nest("/api/v1", admin_routes)
        .with_state(state)
}

fn build_multipart(justification: &str) -> (Vec<u8>, String) {
    let boundary = "MIME_boundary";
    let mut out = Vec::new();
    out.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    out.extend_from_slice(
        b"Content-Disposition: form-data; name=\"justification\"\r\n\r\n",
    );
    out.extend_from_slice(justification.as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    out.extend_from_slice(
        b"Content-Disposition: form-data; name=\"evidence\"; filename=\"e.pdf\"\r\n",
    );
    out.extend_from_slice(b"Content-Type: application/pdf\r\n\r\n");
    out.extend_from_slice(MINI_PDF);
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    (out, format!("multipart/form-data; boundary={}", boundary))
}

async fn admin_user_with_token(db: &libsql::Database) -> (String, String) {
    let id = common::create_test_admin(db).await;
    let token = test_access_token(&id, "admin");
    (id, token)
}

/// Seed (department, employee, daily_record) and return all three ids.
async fn seed_dr(db: &libsql::Database, anchor_date: &str) -> (String, String, String) {
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
    (dept_id, emp_id, dr_id)
}

async fn post_override(app: &Router, dr_id: &str, token: &str, justification: &str) -> StatusCode {
    let (body, ct) = build_multipart(justification);
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/daily-records/{}/overrides", dr_id))
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    resp.status()
}

/// C-04: two overrides on the same daily record. The second must revoke the
/// first (in the same transaction as the insert) so the unique partial index
/// (idx_overrides_one_active_per_record) never sees two active rows for the
/// same daily_record_id — which is exactly what let the reports LEFT JOIN
/// duplicate the row.
#[tokio::test]
async fn a_second_override_revokes_the_first() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-20").await;
    let (_admin_id, token) = admin_user_with_token(&db).await;
    let (state, _tmp) = make_state(db);
    let conn = state.db.connect().unwrap();
    let app = build_test_app(state);

    let status_a = post_override(&app, &dr_id, &token, "override A").await;
    assert_eq!(status_a, StatusCode::CREATED);

    let status_b = post_override(&app, &dr_id, &token, "override B").await;
    assert_eq!(
        status_b,
        StatusCode::CREATED,
        "the second, legitimate override must succeed by revoking the first, \
         not fail against the unique index"
    );

    let active_count: i64 = conn
        .query(
            "SELECT COUNT(*) FROM daily_record_overrides WHERE daily_record_id = ?1 AND status = 'active'",
            params![dr_id.clone()],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(
        active_count, 1,
        "exactly one active override must remain per daily_record_id"
    );

    let active_justification: String = conn
        .query(
            "SELECT justification FROM daily_record_overrides WHERE daily_record_id = ?1 AND status = 'active'",
            params![dr_id.clone()],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(
        active_justification, "override B",
        "the active override must be the newest one (B), not the first (A)"
    );
}

/// Overrides are evidence and the audit obligations require keeping them —
/// revoking must never DELETE. After B replaces A, A's row must still exist
/// with status = 'revoked'. The existing `daily_record_overrides` UPDATE
/// audit trigger (migration 020) also means the revoke itself produces an
/// immutable audit-log entry automatically.
#[tokio::test]
async fn the_revoked_override_is_kept_for_audit() {
    let db = common::test_db().await;
    let (_, _, dr_id) = seed_dr(&db, "2026-04-21").await;
    let (_admin_id, token) = admin_user_with_token(&db).await;
    let (state, _tmp) = make_state(db);
    let conn = state.db.connect().unwrap();
    let app = build_test_app(state);

    assert_eq!(
        post_override(&app, &dr_id, &token, "override A").await,
        StatusCode::CREATED
    );
    assert_eq!(
        post_override(&app, &dr_id, &token, "override B").await,
        StatusCode::CREATED
    );

    let mut rows = conn
        .query(
            "SELECT status FROM daily_record_overrides WHERE daily_record_id = ?1 AND justification = 'override A'",
            params![dr_id.clone()],
        )
        .await
        .unwrap();
    let row = rows
        .next()
        .await
        .unwrap()
        .expect("revoked override row must still exist — never DELETE evidence");
    let status: String = row.get(0).unwrap();
    assert_eq!(status, "revoked", "revoke must set status, not remove the row");
    assert!(
        rows.next().await.unwrap().is_none(),
        "only one row should exist for override A"
    );

    // The pre-existing audit_daily_record_overrides_update trigger (migration
    // 020) fires on every UPDATE, so the revoke is captured in audit_log with
    // no extra code needed in the handler.
    let audit_count: i64 = conn
        .query(
            "SELECT COUNT(*) FROM audit_log \
             WHERE table_name = 'daily_record_overrides' AND operation = 'UPDATE' \
               AND json_extract(old_data, '$.status') = 'active' \
               AND json_extract(new_data, '$.status') = 'revoked' \
               AND json_extract(old_data, '$.justification') = 'override A'",
            params![],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(
        audit_count, 1,
        "revoking override A must produce exactly one immutable audit-log entry"
    );
}
