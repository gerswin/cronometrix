mod common;

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::config::Config;
use cronometrix_api::errors::AppError;
use cronometrix_api::users::handlers::{self, DeactivateQuery};
use cronometrix_api::users::models::{CreateUserRequest, UpdateUserRequest, UserListQuery};
use cronometrix_api::users::service;

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
    })
}

fn request(username: &str, role: &str) -> CreateUserRequest {
    CreateUserRequest {
        username: username.to_string(),
        full_name: format!("{username} User"),
        role: role.to_string(),
        password: "correct-horse-battery-staple".to_string(),
    }
}

fn assert_code(error: AppError, expected: &str) {
    let actual = match error {
        AppError::Validation { code, .. }
        | AppError::Conflict { code, .. }
        | AppError::NotFound { code, .. } => code,
        other => panic!("unexpected error variant: {other:?}"),
    };
    assert_eq!(actual, expected);
}

fn admin_claims(sub: &str) -> Claims {
    Claims {
        sub: sub.to_string(),
        role: Role::Admin,
        exp: chrono::Utc::now().timestamp() + 3600,
        iat: chrono::Utc::now().timestamp(),
        jti: uuid::Uuid::new_v4().to_string(),
        token_type: "access".to_string(),
    }
}

#[tokio::test]
async fn handlers_validate_and_execute_the_complete_user_lifecycle() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db, config());

    let invalid_create = handlers::create_user(
        State(state.clone()),
        Json(CreateUserRequest {
            username: String::new(),
            full_name: "Invalid User".to_string(),
            role: "viewer".to_string(),
            password: "secure-password".to_string(),
        }),
    )
    .await
    .unwrap_err();
    assert_code(invalid_create, "VALIDATION_ERROR");

    let (status, Json(created)) = handlers::create_user(
        State(state.clone()),
        Json(request("handler-user", "viewer")),
    )
    .await
    .unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);

    let Json(page) = handlers::list_users(
        State(state.clone()),
        Query(UserListQuery {
            limit: Some(10),
            offset: Some(0),
            status: Some("active".to_string()),
            role: Some("viewer".to_string()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, created.id);

    let Json(fetched) = handlers::get_user(State(state.clone()), Path(created.id.clone()))
        .await
        .unwrap();
    assert_eq!(fetched.username, "handler-user");

    let invalid_update = handlers::update_user(
        State(state.clone()),
        Extension(admin_claims("different-admin")),
        Path(created.id.clone()),
        Json(UpdateUserRequest {
            full_name: None,
            role: None,
            password: Some("short".to_string()),
            status: None,
            version: 1,
        }),
    )
    .await
    .unwrap_err();
    assert_code(invalid_update, "VALIDATION_ERROR");

    let Json(updated) = handlers::update_user(
        State(state.clone()),
        Extension(admin_claims("different-admin")),
        Path(created.id.clone()),
        Json(UpdateUserRequest {
            full_name: Some("Updated Handler User".to_string()),
            role: Some("supervisor".to_string()),
            password: None,
            status: None,
            version: 1,
        }),
    )
    .await
    .unwrap();
    assert_eq!(updated.full_name, "Updated Handler User");
    assert_eq!(updated.role, "supervisor");

    let status = handlers::deactivate_user(
        State(state.clone()),
        Extension(admin_claims("different-admin")),
        Path(created.id),
        Query(DeactivateQuery {
            version: updated.version,
        }),
    )
    .await
    .unwrap();
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_get_list_and_validation_paths() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), config());

    assert_code(
        service::create(&state, request("invalid", "owner"))
            .await
            .unwrap_err(),
        "INVALID_ROLE",
    );
    let viewer = service::create(&state, request("viewer-a", "viewer"))
        .await
        .unwrap();
    assert_eq!(viewer.status, "active");
    assert_eq!(
        service::get_by_id(&db.connect().unwrap(), &viewer.id)
            .await
            .unwrap()
            .id,
        viewer.id
    );
    assert_code(
        service::get_by_id(&db.connect().unwrap(), "missing-user")
            .await
            .unwrap_err(),
        "USER_NOT_FOUND",
    );
    assert_code(
        service::create(&state, request("viewer-a", "admin"))
            .await
            .unwrap_err(),
        "USERNAME_EXISTS",
    );

    let page = service::list(
        &db.connect().unwrap(),
        UserListQuery {
            limit: Some(500),
            offset: Some(-5),
            status: None,
            role: Some("viewer".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.limit, 100);
    assert_eq!(page.offset, 0);
    assert_code(
        service::list(
            &db.connect().unwrap(),
            UserListQuery {
                limit: None,
                offset: None,
                status: Some("pending".into()),
                role: None,
            },
        )
        .await
        .unwrap_err(),
        "INVALID_STATUS",
    );
    assert_code(
        service::list(
            &db.connect().unwrap(),
            UserListQuery {
                limit: None,
                offset: None,
                status: None,
                role: Some("owner".into()),
            },
        )
        .await
        .unwrap_err(),
        "INVALID_ROLE",
    );
}

#[tokio::test]
async fn update_changes_all_mutable_fields_and_rotates_credentials() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), config());
    let user = service::create(&state, request("mutable", "viewer"))
        .await
        .unwrap();
    db.connect()
        .unwrap()
        .execute(
            "UPDATE users SET refresh_token_hash='old-session' WHERE id=?1",
            libsql::params![user.id.clone()],
        )
        .await
        .unwrap();

    let updated = service::update(
        &state,
        "different-admin",
        &user.id,
        UpdateUserRequest {
            full_name: Some("Updated Name".into()),
            role: Some("supervisor".into()),
            password: Some("new-secure-password".into()),
            status: Some("inactive".into()),
            version: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.full_name, "Updated Name");
    assert_eq!(updated.role, "supervisor");
    assert_eq!(updated.status, "inactive");
    assert_eq!(updated.version, 2);

    let row = db
        .connect()
        .unwrap()
        .query(
            "SELECT password_hash, refresh_token_hash FROM users WHERE id=?1",
            libsql::params![user.id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let hash: String = row.get(0).unwrap();
    let refresh: Option<String> = row.get(1).unwrap();
    cronometrix_api::auth::service::verify_password("new-secure-password", &hash).unwrap();
    assert!(refresh.is_none());
}

#[tokio::test]
async fn update_enforces_self_protection_and_noop_semantics() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db, config());
    let user = service::create(&state, request("self-admin", "admin"))
        .await
        .unwrap();

    assert_code(
        service::update(
            &state,
            &user.id,
            &user.id,
            UpdateUserRequest {
                full_name: None,
                role: Some("viewer".into()),
                password: None,
                status: None,
                version: 1,
            },
        )
        .await
        .unwrap_err(),
        "CANNOT_CHANGE_OWN_ROLE",
    );
    assert_code(
        service::update(
            &state,
            &user.id,
            &user.id,
            UpdateUserRequest {
                full_name: None,
                role: None,
                password: None,
                status: Some("inactive".into()),
                version: 1,
            },
        )
        .await
        .unwrap_err(),
        "CANNOT_DEACTIVATE_SELF",
    );
    assert_code(
        service::update(
            &state,
            "other-admin",
            &user.id,
            UpdateUserRequest {
                full_name: None,
                role: None,
                password: None,
                status: Some("pending".into()),
                version: 1,
            },
        )
        .await
        .unwrap_err(),
        "INVALID_STATUS",
    );

    let unchanged = service::update(
        &state,
        "other-admin",
        &user.id,
        UpdateUserRequest {
            full_name: None,
            role: None,
            password: None,
            status: None,
            version: 99,
        },
    )
    .await
    .unwrap();
    assert_eq!(unchanged.version, 1);
}

#[tokio::test]
async fn update_distinguishes_missing_rows_from_stale_versions() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db, config());
    let user = service::create(&state, request("versioned", "viewer"))
        .await
        .unwrap();
    let change = |version| UpdateUserRequest {
        full_name: Some("Changed".into()),
        role: None,
        password: None,
        status: None,
        version,
    };

    assert_code(
        service::update(&state, "admin", &user.id, change(99))
            .await
            .unwrap_err(),
        "VERSION_CONFLICT",
    );
    assert_code(
        service::update(&state, "admin", "missing-user", change(1))
            .await
            .unwrap_err(),
        "USER_NOT_FOUND",
    );
}

#[tokio::test]
async fn deactivate_covers_success_self_missing_and_version_conflict() {
    let db = Arc::new(common::test_db().await);
    let (state, _tmp) = common::test_state_with_tmpdir(db, config());
    let user = service::create(&state, request("deactivate-me", "viewer"))
        .await
        .unwrap();

    assert_code(
        service::deactivate(&state, &user.id, &user.id, 1)
            .await
            .unwrap_err(),
        "CANNOT_DEACTIVATE_SELF",
    );
    assert_code(
        service::deactivate(&state, "admin", &user.id, 99)
            .await
            .unwrap_err(),
        "VERSION_CONFLICT",
    );
    assert_code(
        service::deactivate(&state, "admin", "missing-user", 1)
            .await
            .unwrap_err(),
        "USER_NOT_FOUND",
    );
    let inactive = service::deactivate(&state, "admin", &user.id, 1)
        .await
        .unwrap();
    assert_eq!(inactive.status, "inactive");
    assert!(inactive.deleted_at.is_some());
    assert_eq!(inactive.version, 2);
}
