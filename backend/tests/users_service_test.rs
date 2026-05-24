mod common;

use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use cronometrix_api::users::models::{CreateUserRequest, UpdateUserRequest, UserListQuery};
use cronometrix_api::users::service;
use std::sync::Arc;
use tempfile::TempDir;

fn test_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
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

async fn make_state() -> (AppState, TempDir) {
    let db = common::test_db().await;
    common::test_state_with_tmpdir(Arc::new(db), test_config())
}

fn new_user_req(username: &str, role: &str) -> CreateUserRequest {
    CreateUserRequest {
        username: username.to_string(),
        full_name: format!("Full {}", username),
        role: role.to_string(),
        password: "password123".to_string(),
    }
}

fn empty_update(version: i64) -> UpdateUserRequest {
    UpdateUserRequest {
        full_name: None,
        role: None,
        password: None,
        status: None,
        version,
    }
}

#[tokio::test]
async fn create_persists_active_user_with_hashed_password() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("alice", "admin"))
        .await
        .expect("create");
    assert_eq!(user.username, "alice");
    assert_eq!(user.role, "admin");
    assert_eq!(user.status, "active");
    assert_eq!(user.version, 1);

    // password_hash must not be the plaintext and must verify.
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT password_hash FROM users WHERE id = ?1",
            libsql::params![user.id.clone()],
        )
        .await
        .unwrap();
    let hash: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_ne!(hash, "password123");
    assert!(hash.starts_with("$argon2"));
}

#[tokio::test]
async fn create_rejects_invalid_role() {
    let (state, _tmp) = make_state().await;
    let err = service::create(&state, new_user_req("bob", "superuser"))
        .await
        .unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::Validation { code, .. } if code == "INVALID_ROLE"));
}

#[tokio::test]
async fn create_duplicate_username_returns_conflict() {
    let (state, _tmp) = make_state().await;
    service::create(&state, new_user_req("carol", "viewer"))
        .await
        .expect("first create");
    let err = service::create(&state, new_user_req("carol", "admin"))
        .await
        .unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::Conflict { code, .. } if code == "USERNAME_EXISTS"));
}

#[tokio::test]
async fn get_by_id_found_and_not_found() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("dave", "supervisor"))
        .await
        .unwrap();
    let conn = state.db.connect().unwrap();
    let fetched = service::get_by_id(&conn, &user.id).await.unwrap();
    assert_eq!(fetched.username, "dave");

    let err = service::get_by_id(&conn, "missing-id").await.unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::NotFound { code, .. } if code == "USER_NOT_FOUND"));
}

#[tokio::test]
async fn list_defaults_to_active_and_filters_by_role() {
    let (state, _tmp) = make_state().await;
    service::create(&state, new_user_req("admin1", "admin")).await.unwrap();
    service::create(&state, new_user_req("sup1", "supervisor")).await.unwrap();
    let viewer = service::create(&state, new_user_req("view1", "viewer")).await.unwrap();
    // Deactivate the viewer so the default (active) list excludes it.
    service::deactivate(&state, "actor", &viewer.id, viewer.version)
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();

    // Default status = active → admin1 + sup1 (view1 is inactive).
    let active = service::list(&conn, UserListQuery { limit: None, offset: None, status: None, role: None })
        .await
        .unwrap();
    assert_eq!(active.total, 2);

    // Filter by role = admin.
    let admins = service::list(
        &conn,
        UserListQuery { limit: None, offset: None, status: None, role: Some("admin".to_string()) },
    )
    .await
    .unwrap();
    assert_eq!(admins.total, 1);
    assert_eq!(admins.data[0].username, "admin1");

    // status = inactive → view1 only.
    let inactive = service::list(
        &conn,
        UserListQuery { limit: None, offset: None, status: Some("inactive".to_string()), role: None },
    )
    .await
    .unwrap();
    assert_eq!(inactive.total, 1);
    assert_eq!(inactive.data[0].username, "view1");
}

#[tokio::test]
async fn list_rejects_invalid_status_and_role() {
    let (state, _tmp) = make_state().await;
    let conn = state.db.connect().unwrap();

    let bad_status = service::list(
        &conn,
        UserListQuery { limit: None, offset: None, status: Some("deleted".to_string()), role: None },
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_status, cronometrix_api::errors::AppError::Validation { code, .. } if code == "INVALID_STATUS"));

    let bad_role = service::list(
        &conn,
        UserListQuery { limit: None, offset: None, status: None, role: Some("root".to_string()) },
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_role, cronometrix_api::errors::AppError::Validation { code, .. } if code == "INVALID_ROLE"));
}

#[tokio::test]
async fn list_clamps_pagination() {
    let (state, _tmp) = make_state().await;
    service::create(&state, new_user_req("p1", "viewer")).await.unwrap();
    let conn = state.db.connect().unwrap();
    let res = service::list(
        &conn,
        UserListQuery { limit: Some(9999), offset: Some(-5), status: None, role: None },
    )
    .await
    .unwrap();
    assert_eq!(res.limit, 100); // clamped from 9999
    assert_eq!(res.offset, 0); // clamped from -5
}

#[tokio::test]
async fn update_full_name_only_bumps_version() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("erin", "viewer")).await.unwrap();
    let updated = service::update(
        &state,
        "some-admin",
        &user.id,
        UpdateUserRequest {
            full_name: Some("Erin Updated".to_string()),
            role: None,
            password: None,
            status: None,
            version: user.version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.full_name, "Erin Updated");
    assert_eq!(updated.version, user.version + 1);
}

#[tokio::test]
async fn update_password_clears_refresh_token() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("frank", "admin")).await.unwrap();
    // Seed a refresh token to prove it gets cleared.
    state
        .db_write
        .execute(
            "UPDATE users SET refresh_token_hash = 'sometoken' WHERE id = ?1",
            vec![libsql::Value::Text(user.id.clone())],
        )
        .await
        .unwrap();

    // version bumped by the manual UPDATE above? No — that UPDATE didn't touch version.
    service::update(
        &state,
        "some-admin",
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: None,
            password: Some("newpassword456".to_string()),
            status: None,
            version: user.version,
        },
    )
    .await
    .unwrap();

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT refresh_token_hash FROM users WHERE id = ?1",
            libsql::params![user.id.clone()],
        )
        .await
        .unwrap();
    let token: Option<String> = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert!(token.is_none(), "refresh token must be cleared on password change");
}

#[tokio::test]
async fn update_role_and_status_for_other_user() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("grace", "viewer")).await.unwrap();
    let updated = service::update(
        &state,
        "some-admin",
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: Some("supervisor".to_string()),
            password: None,
            status: Some("inactive".to_string()),
            version: user.version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.role, "supervisor");
    assert_eq!(updated.status, "inactive");
}

#[tokio::test]
async fn update_cannot_change_own_role() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("heidi", "admin")).await.unwrap();
    let err = service::update(
        &state,
        &user.id, // actor == target
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: Some("viewer".to_string()),
            password: None,
            status: None,
            version: user.version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::Validation { code, .. } if code == "CANNOT_CHANGE_OWN_ROLE"));
}

#[tokio::test]
async fn update_cannot_deactivate_self() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("ivan", "admin")).await.unwrap();
    let err = service::update(
        &state,
        &user.id,
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: None,
            password: None,
            status: Some("inactive".to_string()),
            version: user.version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::Validation { code, .. } if code == "CANNOT_DEACTIVATE_SELF"));
}

#[tokio::test]
async fn update_rejects_invalid_role_and_status() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("judy", "viewer")).await.unwrap();

    let bad_role = service::update(
        &state,
        "admin",
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: Some("wizard".to_string()),
            password: None,
            status: None,
            version: user.version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_role, cronometrix_api::errors::AppError::Validation { code, .. } if code == "INVALID_ROLE"));

    let bad_status = service::update(
        &state,
        "admin",
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: None,
            password: None,
            status: Some("banned".to_string()),
            version: user.version,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_status, cronometrix_api::errors::AppError::Validation { code, .. } if code == "INVALID_STATUS"));
}

#[tokio::test]
async fn update_with_no_fields_returns_current_unchanged() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("ken", "viewer")).await.unwrap();
    let same = service::update(&state, "admin", &user.id, empty_update(user.version))
        .await
        .unwrap();
    assert_eq!(same.version, user.version); // no bump
    assert_eq!(same.username, "ken");
}

#[tokio::test]
async fn update_stale_version_conflicts_and_missing_not_found() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("laura", "viewer")).await.unwrap();

    let conflict = service::update(
        &state,
        "admin",
        &user.id,
        UpdateUserRequest {
            full_name: Some("Stale".to_string()),
            role: None,
            password: None,
            status: None,
            version: 999,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(conflict, cronometrix_api::errors::AppError::Conflict { code, .. } if code == "VERSION_CONFLICT"));

    let missing = service::update(
        &state,
        "admin",
        "no-such-user",
        UpdateUserRequest {
            full_name: Some("X".to_string()),
            role: None,
            password: None,
            status: None,
            version: 1,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(missing, cronometrix_api::errors::AppError::NotFound { code, .. } if code == "USER_NOT_FOUND"));
}

#[tokio::test]
async fn deactivate_soft_deletes_and_sets_deleted_at() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("mallory", "supervisor")).await.unwrap();
    let result = service::deactivate(&state, "admin", &user.id, user.version)
        .await
        .unwrap();
    assert_eq!(result.status, "inactive");
    assert!(result.deleted_at.is_some());
    assert_eq!(result.version, user.version + 1);
}

#[tokio::test]
async fn deactivate_self_forbidden() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("nina", "admin")).await.unwrap();
    let err = service::deactivate(&state, &user.id, &user.id, user.version)
        .await
        .unwrap_err();
    assert!(matches!(err, cronometrix_api::errors::AppError::Validation { code, .. } if code == "CANNOT_DEACTIVATE_SELF"));
}

#[tokio::test]
async fn deactivate_stale_version_conflicts_and_missing_not_found() {
    let (state, _tmp) = make_state().await;
    let user = service::create(&state, new_user_req("oscar", "viewer")).await.unwrap();

    let conflict = service::deactivate(&state, "admin", &user.id, 999)
        .await
        .unwrap_err();
    assert!(matches!(conflict, cronometrix_api::errors::AppError::Conflict { code, .. } if code == "VERSION_CONFLICT"));

    let missing = service::deactivate(&state, "admin", "ghost", 1)
        .await
        .unwrap_err();
    assert!(matches!(missing, cronometrix_api::errors::AppError::NotFound { code, .. } if code == "USER_NOT_FOUND"));
}
