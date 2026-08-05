//! Bloque 4 (H-11, Task 1): department scope enters the identity chain.
//!
//! The token carries the scope and round-trips; a user persists its scope on
//! create; an unknown department is rejected cleanly; and the PATCH is
//! tri-state so an admin can both assign and clear a scope back to org-wide.

mod common;

use std::sync::Arc;

use cronometrix_api::auth::models::Role;
use cronometrix_api::auth::service as auth_service;
use cronometrix_api::config::Config;
use cronometrix_api::users::models::{CreateUserRequest, UpdateUserRequest};
use cronometrix_api::users::service as users_service;
use libsql::params;

use common::{create_test_department_with_shift, test_device_creds_key, TEST_JWT_SECRET};

fn make_config() -> Arc<Config> {
    Arc::new(Config {
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
    })
}

async fn read_department(conn: &libsql::Connection, user_id: &str) -> Option<String> {
    let mut rows = conn
        .query(
            "SELECT department_id FROM users WHERE id = ?1",
            params![user_id.to_string()],
        )
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

fn new_user(username: &str, role: &str, department_id: Option<String>) -> CreateUserRequest {
    CreateUserRequest {
        username: username.into(),
        full_name: format!("Full {username}"),
        role: role.into(),
        password: "password123".into(),
        department_id,
    }
}

fn patch_department(department_id: Option<Option<String>>, version: i64) -> UpdateUserRequest {
    UpdateUserRequest {
        full_name: None,
        role: None,
        password: None,
        status: None,
        department_id,
        version,
    }
}

/// The access token carries the department scope and decodes it back; an
/// unscoped user's token carries None.
#[test]
fn an_access_token_carries_and_recovers_the_department_scope() {
    let secret = TEST_JWT_SECRET.as_bytes();

    let scoped =
        auth_service::issue_access_token("user-1", &Role::Supervisor, Some("dept-A"), secret)
            .unwrap();
    let claims = auth_service::verify_access_token(&scoped, secret).unwrap();
    assert_eq!(claims.department_id.as_deref(), Some("dept-A"));

    let unscoped =
        auth_service::issue_access_token("admin-1", &Role::Admin, None, secret).unwrap();
    let claims = auth_service::verify_access_token(&unscoped, secret).unwrap();
    assert_eq!(claims.department_id, None);
}

/// Creating a user with a department persists it; without one it is NULL.
#[tokio::test]
async fn creating_a_user_persists_its_department_scope() {
    let db = common::test_db().await;
    let dept =
        create_test_department_with_shift(&db, "Dept-A", "day", false, 480, "08:00", "17:00").await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let scoped = users_service::create(&state, new_user("sup-a", "supervisor", Some(dept.clone())))
        .await
        .unwrap();
    let unscoped = users_service::create(&state, new_user("admin-x", "admin", None))
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();
    assert_eq!(read_department(&conn, &scoped.id).await, Some(dept));
    assert_eq!(read_department(&conn, &unscoped.id).await, None);
}

/// An unknown department is a clean validation error, not a raw FK failure.
#[tokio::test]
async fn creating_a_user_with_an_unknown_department_is_rejected() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let result =
        users_service::create(&state, new_user("x", "viewer", Some("does-not-exist".into()))).await;
    assert!(result.is_err(), "an unknown department must be rejected");
}

/// PATCH is tri-state: an omitted field leaves the scope, an explicit null
/// clears it back to org-wide (the case a plain Option<String> could not do).
#[tokio::test]
async fn updating_department_scope_is_tri_state() {
    let db = common::test_db().await;
    let dept =
        create_test_department_with_shift(&db, "Dept-A", "day", false, 480, "08:00", "17:00").await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let actor = "some-other-admin-id";

    // Omission leaves the scope untouched.
    let keep = users_service::create(&state, new_user("keep", "supervisor", Some(dept.clone())))
        .await
        .unwrap();
    users_service::update(
        &state,
        actor,
        &keep.id,
        UpdateUserRequest {
            full_name: Some("Renamed".into()),
            role: None,
            password: None,
            status: None,
            department_id: None, // omitted
            version: 1,
        },
    )
    .await
    .unwrap();

    // Explicit null clears the scope to org-wide.
    let clear = users_service::create(&state, new_user("clear", "supervisor", Some(dept.clone())))
        .await
        .unwrap();
    users_service::update(&state, actor, &clear.id, patch_department(Some(None), 1))
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();
    assert_eq!(
        read_department(&conn, &keep.id).await,
        Some(dept),
        "an omitted department_id leaves the scope untouched"
    );
    assert_eq!(
        read_department(&conn, &clear.id).await,
        None,
        "an explicit null clears the scope back to org-wide"
    );
}
