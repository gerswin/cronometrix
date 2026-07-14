//! Service-layer tests for `departments::service`. Targets the 66.95% baseline
//! gap from Plan 03 (08-04A bucket row 9). Existing `department_tests.rs`
//! covers the handler-level happy paths; this file focuses on the service
//! function branches not exercised there:
//!   - validate_lunch error variants ("fixed" with no duration, weird mode)
//!   - create() returns Conflict on duplicate name
//!   - update() empty-PATCH returns current state (no SET clause)
//!   - update() returns 404 / 409 / VERSION_CONFLICT
//!   - list() filters by status
//!   - get_by_id() 404

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::departments::models::{
    CreateDepartmentRequest, DepartmentListQuery, UpdateDepartmentRequest,
};
use cronometrix_api::departments::service as dept_service;

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

#[tokio::test]
async fn create_department_happy_path_punch_mode() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let dept = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "Engineering".into(),
            base_salary_cents: 100_000,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .expect("punch mode requires no duration");
    assert_eq!(dept.name, "Engineering");
    assert_eq!(dept.lunch_mode, "punch");
    assert!(dept.lunch_duration_min.is_none());
    assert_eq!(dept.status, "active");
    assert_eq!(dept.version, 1);
}

#[tokio::test]
async fn create_department_fixed_mode_requires_duration() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let err = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "BadFixed".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "fixed".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .expect_err("fixed mode without duration must fail");
    let s = err.to_string();
    assert!(
        s.contains("validation") || s.contains("required") || s.contains("LUNCH"),
        "err: {s}"
    );
}

#[tokio::test]
async fn create_department_fixed_mode_zero_duration_rejected() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let err = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "ZeroDur".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "fixed".into(),
            lunch_duration_min: Some(0),
        },
    )
    .await
    .expect_err("fixed mode with 0 duration must fail");
    let _ = err;
}

#[tokio::test]
async fn create_department_fixed_mode_negative_duration_rejected() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let r = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "NegDur".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "fixed".into(),
            lunch_duration_min: Some(-30),
        },
    )
    .await;
    assert!(r.is_err(), "negative lunch duration must reject");
}

#[tokio::test]
async fn create_department_invalid_lunch_mode_rejected() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let err = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "WeirdMode".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "auto".into(), // not 'fixed' or 'punch'
            lunch_duration_min: Some(30),
        },
    )
    .await
    .expect_err("unknown lunch_mode must error");
    let s = err.to_string();
    assert!(
        s.contains("validation") || s.contains("lunch_mode"),
        "err must mention lunch_mode: {s}"
    );
}

#[tokio::test]
async fn create_department_duplicate_name_returns_conflict() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let _first = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "DupName".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap();

    let err = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "DupName".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .expect_err("duplicate name must conflict");
    let s = err.to_string();
    assert!(
        s.contains("conflict") || s.contains("DEPARTMENT_NAME_EXISTS") || s.contains("already"),
        "err: {s}"
    );
}

#[tokio::test]
async fn get_by_id_404_unknown() {
    let db = Arc::new(common::test_db().await);
    let (_state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let err = dept_service::get_by_id(&conn, "no-such-id")
        .await
        .expect_err("must 404");
    let s = err.to_string();
    assert!(s.contains("not found"), "err: {s}");
}

#[tokio::test]
async fn list_departments_default_filters_to_active() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let _ = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "Active1".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap();

    let result = dept_service::list(
        &conn,
        DepartmentListQuery {
            limit: None,
            offset: None,
            status: None, // defaults to 'active'
        },
    )
    .await
    .unwrap();
    assert!(result.total >= 1);
    for d in &result.data {
        assert_eq!(d.status, "active");
    }
}

#[tokio::test]
async fn list_pagination_clamps() {
    let db = Arc::new(common::test_db().await);
    let (_state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let conn = db.connect().unwrap();
    let result = dept_service::list(
        &conn,
        DepartmentListQuery {
            limit: Some(500),
            offset: Some(-7),
            status: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.limit, 100);
    assert_eq!(result.offset, 0);
}

#[tokio::test]
async fn update_with_no_fields_returns_current_state() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let dept = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "NoOpUpdate".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap();

    let updated = dept_service::update_queued(
        &state,
        &dept.id,
        UpdateDepartmentRequest {
            name: None,
            base_salary_cents: None,
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: None,
            lunch_duration_min: None,
            version: dept.version,
        },
    )
    .await
    .expect("no-op patch returns Ok");
    assert_eq!(updated.id, dept.id);
    assert_eq!(updated.version, dept.version, "no-op must not bump version");
}

#[tokio::test]
async fn update_404_when_id_unknown() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let err = dept_service::update_queued(
        &state,
        "no-such-id",
        UpdateDepartmentRequest {
            name: Some("New".into()),
            base_salary_cents: None,
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: None,
            lunch_duration_min: None,
            version: 1,
        },
    )
    .await
    .expect_err("must 404");
    let s = err.to_string();
    assert!(s.contains("not found"));
}

#[tokio::test]
async fn update_409_on_stale_version() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let dept = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "StaleVer".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap();

    // First patch bumps the version.
    let _ = dept_service::update_queued(
        &state,
        &dept.id,
        UpdateDepartmentRequest {
            name: Some("UpdatedOnce".into()),
            base_salary_cents: None,
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: None,
            lunch_duration_min: None,
            version: dept.version,
        },
    )
    .await
    .unwrap();

    // Second patch with the now-stale version → conflict.
    let err = dept_service::update_queued(
        &state,
        &dept.id,
        UpdateDepartmentRequest {
            name: Some("UpdatedTwice".into()),
            base_salary_cents: None,
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: None,
            lunch_duration_min: None,
            version: dept.version, // stale
        },
    )
    .await
    .expect_err("stale version must 409");
    let s = err.to_string();
    assert!(
        s.contains("conflict") || s.contains("modified"),
        "err must indicate version conflict: {s}"
    );
}

#[tokio::test]
async fn update_validates_lunch_when_changing_mode() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let dept = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "ChangeMode".into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap();

    // Try to switch to "fixed" without supplying lunch_duration_min — must reject.
    let err = dept_service::update_queued(
        &state,
        &dept.id,
        UpdateDepartmentRequest {
            name: None,
            base_salary_cents: None,
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: Some("fixed".into()),
            lunch_duration_min: None,
            version: dept.version,
        },
    )
    .await
    .expect_err("switching to fixed without duration must fail");
    let _ = err;
}

#[tokio::test]
async fn update_can_change_each_individual_field() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), make_config());
    let _conn = db.connect().unwrap();
    let dept = dept_service::create_queued(
        &state,
        CreateDepartmentRequest {
            name: "FieldByField".into(),
            base_salary_cents: 1000,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "fixed".into(),
            lunch_duration_min: Some(60),
        },
    )
    .await
    .unwrap();

    // Change name + salary, keep the rest.
    let updated = dept_service::update_queued(
        &state,
        &dept.id,
        UpdateDepartmentRequest {
            name: Some("NewName".into()),
            base_salary_cents: Some(2000),
            shift_start_time: None,
            shift_end_time: None,
            lunch_mode: None,
            lunch_duration_min: None,
            version: dept.version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name, "NewName");
    assert_eq!(updated.base_salary_cents, 2000);
    assert_eq!(updated.shift_start_time, "09:00");
    assert_eq!(updated.lunch_duration_min, Some(60));
    assert_eq!(updated.version, dept.version + 1);
}
