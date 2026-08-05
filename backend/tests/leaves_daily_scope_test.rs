//! Bloque 4 (H-11, Task 4b): leaves and daily records are scoped to the actor's
//! department. A scoped supervisor/viewer sees and reads only records of
//! employees in its department; other departments are invisible (list) and 404
//! (detail). An admin is unscoped.

mod common;

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::auth::rbac::AuthUser;
use cronometrix_api::config::Config;
use cronometrix_api::errors::AppError;
use libsql::{params, Connection};

use cronometrix_api::daily_records::handlers as dr_handlers;
use cronometrix_api::daily_records::models::DailyRecordListQuery;
use cronometrix_api::leaves::handlers as leave_handlers;
use cronometrix_api::leaves::models::LeaveListQuery;

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

fn actor(c: &Claims) -> AuthUser {
    AuthUser(c.clone())
}

async fn seed_employee(conn: &Connection, id: &str, dept: &str) {
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?1, ?1, ?2, 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), dept.to_string()],
    )
    .await
    .expect("seed employee");
}

async fn seed_user(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?1, ?1, 'hash', 'admin', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string()],
    )
    .await
    .expect("seed user");
}

async fn seed_leave(conn: &Connection, id: &str, employee_id: &str) {
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, justification, \
         evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, '2026-01-01', '2026-01-02', 'vacation', 'reason', NULL, 'admin', \
         'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), employee_id.to_string()],
    )
    .await
    .expect("seed leave");
}

async fn seed_daily_record(conn: &Connection, id: &str, employee_id: &str, dept: &str, anchor: &str) {
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, \
         work_minutes, overtime_minutes, late_minutes, early_departure_minutes, \
         is_rest_day_worked, computed_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'day', 480, 0, 0, 0, 0, unixepoch(), unixepoch(), unixepoch())",
        params![
            id.to_string(),
            employee_id.to_string(),
            dept.to_string(),
            anchor.to_string()
        ],
    )
    .await
    .expect("seed daily record");
}

fn leave_query() -> LeaveListQuery {
    LeaveListQuery {
        limit: None,
        offset: None,
        employee_id: None,
        leave_type: None,
        status: None,
        from_date: None,
        to_date: None,
    }
}

fn dr_query() -> DailyRecordListQuery {
    DailyRecordListQuery {
        limit: None,
        offset: None,
        employee_id: None,
        department_id: None,
        from_date: None,
        to_date: None,
    }
}

fn assert_not_found(err: AppError, expected: &str) {
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, expected),
        other => panic!("expected NotFound {expected}, got {other:?}"),
    }
}

#[tokio::test]
async fn leaves_and_daily_records_are_scoped_to_the_actors_department() {
    let db = common::test_db().await;
    let dept_a = common::create_test_department_with_shift(
        &db, "Dept-A", "day", false, 480, "08:00", "17:00",
    )
    .await;
    let dept_b = common::create_test_department_with_shift(
        &db, "Dept-B", "day", false, 480, "08:00", "17:00",
    )
    .await;
    {
        let conn = db.connect().unwrap();
        seed_user(&conn, "admin").await;
        seed_employee(&conn, "emp-a", &dept_a).await;
        seed_employee(&conn, "emp-b", &dept_b).await;
        seed_leave(&conn, "leave-a", "emp-a").await;
        seed_leave(&conn, "leave-b", "emp-b").await;
        seed_daily_record(&conn, "dr-a", "emp-a", &dept_a, "2026-01-01").await;
        seed_daily_record(&conn, "dr-b", "emp-b", &dept_b, "2026-01-01").await;
    }
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());

    let sup_a = claims(Role::Supervisor, Some(&dept_a));
    let admin = claims(Role::Admin, None);

    // --- leaves ---
    let listed = leave_handlers::list_leaves(State(state.clone()), actor(&sup_a), Query(leave_query()))
        .await
        .unwrap();
    let ids: Vec<String> = listed.0.data.iter().map(|l| l.id.clone()).collect();
    assert_eq!(ids, vec!["leave-a".to_string()]);

    leave_handlers::get_leave(State(state.clone()), actor(&sup_a), Path("leave-a".into()))
        .await
        .unwrap();
    let err = leave_handlers::get_leave(State(state.clone()), actor(&sup_a), Path("leave-b".into()))
        .await
        .unwrap_err();
    assert_not_found(err, "LEAVE_NOT_FOUND");

    let listed = leave_handlers::list_leaves(State(state.clone()), actor(&admin), Query(leave_query()))
        .await
        .unwrap();
    assert_eq!(listed.0.total, 2);

    // D2: medical evidence of an out-of-department leave is 404 for a scoped
    // actor (the health file is never served across departments).
    let err =
        leave_handlers::get_leave_evidence(State(state.clone()), actor(&sup_a), Path("leave-b".into()))
            .await
            .unwrap_err();
    assert_not_found(err, "LEAVE_EVIDENCE_NOT_FOUND");

    // --- daily records ---
    let listed =
        dr_handlers::list_daily_records(State(state.clone()), actor(&sup_a), Query(dr_query()))
            .await
            .unwrap();
    let ids: Vec<String> = listed.0.data.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, vec!["dr-a".to_string()]);

    dr_handlers::get_daily_record(State(state.clone()), actor(&sup_a), Path("dr-a".into()))
        .await
        .unwrap();
    let err =
        dr_handlers::get_daily_record(State(state.clone()), actor(&sup_a), Path("dr-b".into()))
            .await
            .unwrap_err();
    assert_not_found(err, "DAILY_RECORD_NOT_FOUND");

    let listed =
        dr_handlers::list_daily_records(State(state.clone()), actor(&admin), Query(dr_query()))
            .await
            .unwrap();
    assert_eq!(listed.0.total, 2);
}
