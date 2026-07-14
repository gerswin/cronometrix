//! Service-layer tests for `leaves::service`. Targets the 69.87% baseline gap
//! from Plan 03 (08-04A bucket row 15) and the 46.56% gap on `leaves::handlers`
//! (row 14, branched paths complemented by leaves_handlers_extra_test.rs).
//! Existing `leave_tests.rs` covers the multipart create + cancel flows; this
//! file targets pure service-function branches:
//!   - create_leave: invalid leave_type, medical-without-evidence, malformed
//!     date, from > to, overlap detection
//!   - get_by_id 404 path
//!   - list filters (status, leave_type, employee_id, from/to overlap)
//!   - cancel: 404 / 409 / 204 happy / second cancel 409
//!   - fetch_active_leave_for_date: Some / None / cancelled-leave-skipped

mod common;

use std::sync::Arc;

use chrono::NaiveDate;
use cronometrix_api::config::Config;
use cronometrix_api::errors::AppError;
use cronometrix_api::leaves::models::{CreateLeaveRequest, LeaveListQuery};
use cronometrix_api::leaves::service as ls;

use common::create_test_admin;

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.into(),
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
    })
}

async fn seed_dept(db: &libsql::Database, name: &str) -> String {
    common::create_test_department_with_shift(db, name, "day", false, 480, "09:00", "17:00").await
}

async fn seed_employee(db: &libsql::Database, dept_id: &str, code: &str) -> String {
    let conn = db.connect().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![id.clone(), code.to_string(), dept_id.to_string()],
    )
    .await
    .unwrap();
    id
}

fn req_basic(emp: &str, kind: &str, from: &str, to: &str) -> CreateLeaveRequest {
    CreateLeaveRequest {
        employee_id: emp.to_string(),
        from_date: from.to_string(),
        to_date: to.to_string(),
        leave_type: kind.to_string(),
        justification: "test".into(),
    }
}

// =============================================================================
// create_leave validation branches
// =============================================================================

#[tokio::test]
async fn create_leave_rejects_unknown_leave_type() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DInv").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "holiday", "2026-04-20", "2026-04-20"),
        None,
    )
    .await
    .expect_err("unknown leave_type must reject");
    let s = err.to_string();
    assert!(
        s.contains("validation") || s.contains("medical, vacation"),
        "err: {s}"
    );
}

#[tokio::test]
async fn create_leave_medical_without_evidence_rejects() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DMed").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "medical", "2026-04-20", "2026-04-20"),
        None,
    )
    .await
    .expect_err("medical without evidence must reject");
    let s = err.to_string();
    assert!(
        s.contains("evidence") || s.contains("validation"),
        "err: {s}"
    );
}

#[tokio::test]
async fn create_leave_rejects_malformed_from_date() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DDate").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "20-04-2026", "2026-04-22"),
        None,
    )
    .await
    .expect_err("malformed from_date must reject");
    match err {
        AppError::Validation { code, message } => {
            assert_eq!(code, "VALIDATION_ERROR");
            assert!(message.contains("from_date"), "msg: {message}");
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn create_leave_rejects_malformed_to_date() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DDate2").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "not-a-date"),
        None,
    )
    .await
    .expect_err("malformed to_date must reject");
    match err {
        AppError::Validation { code, message } => {
            assert_eq!(code, "VALIDATION_ERROR");
            assert!(message.contains("to_date"), "msg: {message}");
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn create_leave_rejects_when_from_after_to() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DRange").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-25", "2026-04-20"),
        None,
    )
    .await
    .expect_err("from > to must reject");
    match err {
        AppError::Validation { code, message } => {
            assert_eq!(code, "VALIDATION_ERROR");
            assert!(message.contains("from_date"), "msg: {message}");
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn create_leave_rejects_overlap_with_existing_active_leave() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DOverlap").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let _first = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-25"),
        None,
    )
    .await
    .unwrap();
    let err = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-22", "2026-04-28"),
        None,
    )
    .await
    .expect_err("overlap must reject");
    match err {
        AppError::LeaveConflict { code, .. } => {
            assert_eq!(code, "LEAVE_OVERLAP");
        }
        other => panic!("expected LeaveConflict, got {other:?}"),
    }
}

#[tokio::test]
async fn create_leave_succeeds_with_evidence_for_medical() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DMedOK").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let leave = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "medical", "2026-04-20", "2026-04-22"),
        Some("evidence/abc.pdf".into()),
    )
    .await
    .unwrap();
    assert_eq!(leave.leave_type, "medical");
    assert_eq!(leave.evidence_path.as_deref(), Some("evidence/abc.pdf"));
    assert_eq!(leave.status, "active");
}

// =============================================================================
// get_by_id / list
// =============================================================================

#[tokio::test]
async fn get_by_id_404() {
    let db = Arc::new(common::test_db().await);
    let (_state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let err = ls::get_by_id(&conn, "no-such-id").await.expect_err("404");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn list_filter_by_leave_type() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DListType").await;
    let emp1 = seed_employee(&db, &dept, "E1").await;
    let emp2 = seed_employee(&db, &dept, "E2").await;
    ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp1, "vacation", "2026-04-20", "2026-04-22"),
        None,
    )
    .await
    .unwrap();
    ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp2, "manual", "2026-04-22", "2026-04-22"),
        None,
    )
    .await
    .unwrap();

    let result = ls::list(
        &conn,
        LeaveListQuery {
            limit: None,
            offset: None,
            status: None, // default 'active'
            employee_id: None,
            leave_type: Some("vacation".into()),
            from_date: None,
            to_date: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0].leave_type, "vacation");
}

#[tokio::test]
async fn list_filter_by_employee_id() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DListEmp").await;
    let emp1 = seed_employee(&db, &dept, "E1").await;
    let emp2 = seed_employee(&db, &dept, "E2").await;
    ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp1, "vacation", "2026-04-20", "2026-04-22"),
        None,
    )
    .await
    .unwrap();
    ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp2, "vacation", "2026-04-22", "2026-04-22"),
        None,
    )
    .await
    .unwrap();
    let result = ls::list(
        &conn,
        LeaveListQuery {
            limit: None,
            offset: None,
            status: None,
            employee_id: Some(emp1.clone()),
            leave_type: None,
            from_date: None,
            to_date: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0].employee_id, emp1);
}

#[tokio::test]
async fn list_overlap_window_via_from_to() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DListWin").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-25"),
        None,
    )
    .await
    .unwrap();
    // Window ending before the leave: zero rows.
    let result = ls::list(
        &conn,
        LeaveListQuery {
            limit: None,
            offset: None,
            status: None,
            employee_id: None,
            leave_type: None,
            from_date: Some("2026-05-01".into()),
            to_date: Some("2026-05-15".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 0);
    // Window overlapping: 1 row.
    let result = ls::list(
        &conn,
        LeaveListQuery {
            limit: None,
            offset: None,
            status: None,
            employee_id: None,
            leave_type: None,
            from_date: Some("2026-04-22".into()),
            to_date: Some("2026-04-30".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
}

#[tokio::test]
async fn list_pagination_clamps() {
    let db = Arc::new(common::test_db().await);
    let (_state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let result = ls::list(
        &conn,
        LeaveListQuery {
            limit: Some(999),
            offset: Some(-2),
            status: None,
            employee_id: None,
            leave_type: None,
            from_date: None,
            to_date: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.limit, 100);
    assert_eq!(result.offset, 0);
}

// =============================================================================
// cancel
// =============================================================================

#[tokio::test]
async fn cancel_404_unknown_id() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let err = ls::cancel_queued(&state, "no-such", 1)
        .await
        .expect_err("must 404");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn cancel_409_stale_version() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DCancVer").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let leave = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-20"),
        None,
    )
    .await
    .unwrap();
    let err = ls::cancel_queued(&state, &leave.id, leave.version + 99)
        .await
        .expect_err("must 409");
    assert!(
        err.to_string().contains("modified") || err.to_string().contains("conflict"),
        "stale version → conflict; got: {}",
        err
    );
}

#[tokio::test]
async fn cancel_then_recancel_409() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let _conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DRecanc").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let leave = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-20"),
        None,
    )
    .await
    .unwrap();
    ls::cancel_queued(&state, &leave.id, leave.version)
        .await
        .unwrap();
    let err = ls::cancel_queued(&state, &leave.id, leave.version + 1)
        .await
        .expect_err("second cancel must 409");
    assert!(err.to_string().contains("conflict") || err.to_string().contains("modified"));
}

// =============================================================================
// fetch_active_leave_for_date
// =============================================================================

#[tokio::test]
async fn fetch_active_leave_returns_some_when_covering_date() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DFetchSome").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let leave = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-25"),
        None,
    )
    .await
    .unwrap();
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 22).unwrap();
    let r = ls::fetch_active_leave_for_date(&conn, &emp, anchor)
        .await
        .unwrap();
    let lr = r.expect("Some");
    assert_eq!(lr.id, leave.id);
    assert_eq!(lr.leave_type, "vacation");
    assert_eq!(lr.from_date, NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
    assert_eq!(lr.to_date, NaiveDate::from_ymd_opt(2026, 4, 25).unwrap());
}

#[tokio::test]
async fn fetch_active_leave_returns_none_outside_window() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DFetchNone").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let _ = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-25"),
        None,
    )
    .await
    .unwrap();
    let outside = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
    let r = ls::fetch_active_leave_for_date(&conn, &emp, outside)
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn fetch_active_leave_skips_cancelled_leaves() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let admin = create_test_admin(&db).await;
    let conn = db.connect().unwrap();
    let dept = seed_dept(&db, "DFetchSkip").await;
    let emp = seed_employee(&db, &dept, "E1").await;
    let leave = ls::create_leave_queued(
        &state,
        &admin,
        req_basic(&emp, "vacation", "2026-04-20", "2026-04-25"),
        None,
    )
    .await
    .unwrap();
    ls::cancel_queued(&state, &leave.id, leave.version)
        .await
        .unwrap();
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 22).unwrap();
    let r = ls::fetch_active_leave_for_date(&conn, &emp, anchor)
        .await
        .unwrap();
    assert!(
        r.is_none(),
        "cancelled leaves must not be returned by fetch_active_leave_for_date"
    );
}

#[tokio::test]
async fn fetch_active_leave_returns_none_for_unknown_employee() {
    let db = Arc::new(common::test_db().await);
    let (_state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let r = ls::fetch_active_leave_for_date(
        &conn,
        "ghost",
        NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
    )
    .await
    .unwrap();
    assert!(r.is_none());
}
