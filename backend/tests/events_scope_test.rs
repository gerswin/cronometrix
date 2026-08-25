//! Bloque 4 (H-11, Task 4a): attendance events are scoped to the actor's
//! department. A scoped supervisor/viewer sees only events of employees in its
//! department; other departments and unknown-face events are invisible (list)
//! and 404 (detail). An admin is unscoped and sees everything, unknowns
//! included.

mod common;

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::auth::rbac::AuthUser;
use cronometrix_api::config::Config;
use cronometrix_api::errors::AppError;
use cronometrix_api::events::handlers;
use cronometrix_api::events::models::{EventListQuery, NewAttendanceEvent};
use cronometrix_api::events::service;
use cronometrix_api::state::AppState;
use libsql::{params, Connection};

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

fn list_query() -> EventListQuery {
    EventListQuery {
        limit: None,
        offset: None,
        employee_id: None,
        device_id: None,
        from: None,
        to: None,
        include_unknown: None,
    }
}

async fn seed_device(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?1, '10.0.0.1', 8000, 'https', 'admin', 'ciphertext', 'entry', 0, \
         'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string()],
    )
    .await
    .expect("seed device");
}

async fn seed_employee(conn: &Connection, id: &str, code: &str, dept: &str) {
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), code.to_string(), format!("Emp {code}"), dept.to_string()],
    )
    .await
    .expect("seed employee");
}

async fn persist(state: &AppState, root: &std::path::Path, id: &str, employee_id: Option<&str>, captured_at: i64, unknown: bool) {
    let event = NewAttendanceEvent {
        id: id.to_string(),
        employee_id: employee_id.map(String::from),
        device_id: "dev-1".to_string(),
        direction: "entry".to_string(),
        captured_at,
        is_unknown: unknown,
        face_id: Some("1".to_string()),
        employee_no_string: None,
        raw_payload: "{}".to_string(),
        photo_bytes: None,
    };
    service::persist_attendance_event_queued(state, root, event)
        .await
        .expect("persist event");
}

fn assert_not_found(err: AppError, expected: &str) {
    match err {
        AppError::NotFound { code, .. } => assert_eq!(code, expected),
        other => panic!("expected NotFound {expected}, got {other:?}"),
    }
}

#[tokio::test]
async fn events_are_scoped_to_the_actors_department() {
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
        seed_device(&conn, "dev-1").await;
        seed_employee(&conn, "emp-a", "A-1", &dept_a).await;
        seed_employee(&conn, "emp-b", "B-1", &dept_b).await;
    }
    let (state, tmp) = common::test_state_with_tmpdir(Arc::new(db), config());

    persist(&state, tmp.path(), "ev-a", Some("emp-a"), 1_700_000_000, false).await;
    persist(&state, tmp.path(), "ev-b", Some("emp-b"), 1_700_000_100, false).await;
    persist(&state, tmp.path(), "ev-u", None, 1_700_000_200, true).await;

    let sup_a = claims(Role::Supervisor, Some(&dept_a));
    let admin = claims(Role::Admin, None);

    // scoped list: only department A's event
    let listed = handlers::list_events(State(state.clone()), actor(&sup_a), Query(list_query()))
        .await
        .unwrap();
    let ids: Vec<String> = listed.0.data.iter().map(|e| e.id.clone()).collect();
    assert_eq!(ids, vec!["ev-a".to_string()]);
    assert_eq!(listed.0.total, 1);

    // scoped get own -> ok
    let got = handlers::get_event(State(state.clone()), actor(&sup_a), Path("ev-a".into()))
        .await
        .unwrap();
    assert_eq!(got.0.id, "ev-a");

    // scoped get other department -> 404
    let err = handlers::get_event(State(state.clone()), actor(&sup_a), Path("ev-b".into()))
        .await
        .unwrap_err();
    assert_not_found(err, "EVENT_NOT_FOUND");

    // scoped get unknown-face event -> 404 (no department)
    let err = handlers::get_event(State(state.clone()), actor(&sup_a), Path("ev-u".into()))
        .await
        .unwrap_err();
    assert_not_found(err, "EVENT_NOT_FOUND");

    // admin is unscoped: sees all three, unknown included
    let listed = handlers::list_events(State(state.clone()), actor(&admin), Query(list_query()))
        .await
        .unwrap();
    assert_eq!(listed.0.total, 3);
    let got = handlers::get_event(State(state.clone()), actor(&admin), Path("ev-u".into()))
        .await
        .unwrap();
    assert_eq!(got.0.id, "ev-u");

    // D2: the face photo of an out-of-department event is 404 for a scoped
    // actor (the biometric file is never served across departments).
    let err = handlers::get_event_photo(State(state.clone()), actor(&sup_a), Path("ev-b".into()))
        .await
        .unwrap_err();
    assert_not_found(err, "EVENT_PHOTO_NOT_FOUND");
}
