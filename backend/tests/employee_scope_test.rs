//! Bloque 4 (H-11, Task 3): employee reads and writes are confined to the
//! actor's department. A scoped supervisor sees and edits only its own
//! department; an out-of-scope employee is 404 (not 403, so its existence is
//! not leaked); creating or moving outside the scope is denied; an admin is
//! unscoped.

mod common;

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::auth::rbac::AuthUser;
use cronometrix_api::config::Config;
use cronometrix_api::employees::handlers;
use cronometrix_api::employees::models::{
    CreateEmployeeRequest, EmployeeListQuery, SalaryKind, UpdateEmployeeRequest,
};
use cronometrix_api::employees::service;
use cronometrix_api::errors::AppError;
use cronometrix_api::state::AppState;

fn config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
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
        device_push_base_url: String::new(),
    })
}

fn claims(role: Role, department_id: Option<&str>) -> Claims {
    Claims {
        sub: "actor".into(),
        role,
        department_id: department_id.map(String::from),
        exp: chrono::Utc::now().timestamp() + 3600,
        iat: chrono::Utc::now().timestamp(),
        jti: "jti".into(),
        token_type: "access".into(),
    }
}

/// `AuthUser` is not `Clone`; each handler call takes it by value, so mint a
/// fresh wrapper around the (cloneable) claims per call.
fn actor(c: &Claims) -> AuthUser {
    AuthUser(c.clone())
}

fn new_employee(code: &str, name: &str, dept: &str) -> CreateEmployeeRequest {
    CreateEmployeeRequest {
        employee_code: code.into(),
        name: name.into(),
        department_id: dept.into(),
        position: None,
        hire_date: None,
        base_salary_cents: Some(100_000),
        salary_kind: Some(SalaryKind::Monthly),
    }
}

fn empty_query() -> EmployeeListQuery {
    EmployeeListQuery {
        limit: None,
        offset: None,
        name: None,
        department_id: None,
        status: None,
    }
}

fn rename(name: &str, version: i64) -> UpdateEmployeeRequest {
    UpdateEmployeeRequest {
        name: Some(name.into()),
        department_id: None,
        position: None,
        hire_date: None,
        base_salary_cents: None,
        salary_kind: None,
        version,
    }
}

fn assert_not_found(err: AppError) {
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, "EMPLOYEE_NOT_FOUND"),
        other => panic!("expected EMPLOYEE_NOT_FOUND, got {other:?}"),
    }
}

/// Seed two departments and one active employee in each.
async fn two_departments() -> (
    AppState,
    tempfile::TempDir,
    String,
    String,
    (String, i64),
    (String, i64),
) {
    let db = common::test_db().await;
    let dept_a = common::create_test_department_with_shift(
        &db, "Dept-A", "day", false, 480, "08:00", "17:00",
    )
    .await;
    let dept_b = common::create_test_department_with_shift(
        &db, "Dept-B", "day", false, 480, "08:00", "17:00",
    )
    .await;
    let (state, tmp) = common::test_state_with_tmpdir(Arc::new(db), config());

    let emp_a = service::create_queued(&state, new_employee("A-1", "Alice", &dept_a))
        .await
        .unwrap();
    let emp_b = service::create_queued(&state, new_employee("B-1", "Bob", &dept_b))
        .await
        .unwrap();

    (
        state,
        tmp,
        dept_a,
        dept_b,
        (emp_a.id, emp_a.version),
        (emp_b.id, emp_b.version),
    )
}

#[tokio::test]
async fn a_scoped_supervisor_only_sees_and_edits_its_own_department() {
    let (state, _tmp, dept_a, _dept_b, (a_id, a_ver), (b_id, b_ver)) = two_departments().await;
    let sup_a = claims(Role::Supervisor, Some(&dept_a));

    // list: only department A
    let listed = handlers::list_employees(State(state.clone()), actor(&sup_a), Query(empty_query()))
        .await
        .unwrap();
    assert_eq!(listed.0.data.len(), 1);
    assert_eq!(listed.0.data[0].id, a_id);

    // get own -> ok
    let got = handlers::get_employee(State(state.clone()), actor(&sup_a), Path(a_id.clone()))
        .await
        .unwrap();
    assert_eq!(got.0.id, a_id);

    // get other department -> 404
    let err = handlers::get_employee(State(state.clone()), actor(&sup_a), Path(b_id.clone()))
        .await
        .unwrap_err();
    assert_not_found(err);

    // patch own -> ok
    let patched = handlers::update_employee(
        State(state.clone()),
        actor(&sup_a),
        Path(a_id.clone()),
        Json(rename("Alice R", a_ver)),
    )
    .await
    .unwrap();
    assert_eq!(patched.0.name, "Alice R");

    // patch other department -> 404
    let err = handlers::update_employee(
        State(state.clone()),
        actor(&sup_a),
        Path(b_id.clone()),
        Json(rename("hax", b_ver)),
    )
    .await
    .unwrap_err();
    assert_not_found(err);
}

#[tokio::test]
async fn a_scoped_supervisor_cannot_create_or_move_outside_its_department() {
    let (state, _tmp, dept_a, dept_b, (a_id, a_ver), _b) = two_departments().await;
    let sup_a = claims(Role::Supervisor, Some(&dept_a));

    // create in B -> forbidden
    let err = handlers::create_employee(
        State(state.clone()),
        actor(&sup_a),
        Json(new_employee("X-1", "Mallory", &dept_b)),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, AppError::Forbidden),
        "create outside scope must be forbidden"
    );

    // create in A -> created
    let (status, _created) = handlers::create_employee(
        State(state.clone()),
        actor(&sup_a),
        Json(new_employee("A-2", "Anna", &dept_a)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);

    // moving an in-scope employee into B -> forbidden
    let err = handlers::update_employee(
        State(state.clone()),
        actor(&sup_a),
        Path(a_id.clone()),
        Json(UpdateEmployeeRequest {
            name: None,
            department_id: Some(dept_b.clone()),
            position: None,
            hire_date: None,
            base_salary_cents: None,
            salary_kind: None,
            version: a_ver,
        }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, AppError::Forbidden),
        "moving out of scope must be forbidden"
    );
}

#[tokio::test]
async fn an_admin_is_unscoped_and_reaches_every_department() {
    let (state, _tmp, _dept_a, _dept_b, (a_id, _), (b_id, b_ver)) = two_departments().await;
    let admin = claims(Role::Admin, None);

    // list: both departments
    let listed = handlers::list_employees(State(state.clone()), actor(&admin), Query(empty_query()))
        .await
        .unwrap();
    assert_eq!(listed.0.data.len(), 2);

    // admin reads and edits department B freely
    let got = handlers::get_employee(State(state.clone()), actor(&admin), Path(b_id.clone()))
        .await
        .unwrap();
    assert_eq!(got.0.id, b_id);
    let patched = handlers::update_employee(
        State(state.clone()),
        actor(&admin),
        Path(b_id.clone()),
        Json(rename("Bob R", b_ver)),
    )
    .await
    .unwrap();
    assert_eq!(patched.0.name, "Bob R");

    assert_ne!(a_id, b_id);
}
