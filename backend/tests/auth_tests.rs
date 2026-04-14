mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use axum::routing::{get, post};
use cronometrix_api::auth;
use cronometrix_api::setup;
use cronometrix_api::state::AppState;
use cronometrix_api::config::Config;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use http_body_util::BodyExt;

/// Build a test app with all auth + setup routes wired up.
async fn build_test_app(db: libsql::Database) -> Router {
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
    });

    let state = AppState {
        db: Arc::new(db),
        config,
    };

    let public_routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/login", post(auth::handlers::login))
        .route("/setup/status", get(setup::handlers::setup_status))
        .route("/setup/init", post(setup::handlers::setup_init));

    // Cookie-authenticated routes (use refresh cookie, not Bearer token)
    let cookie_auth_routes = Router::new()
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/auth/logout", post(auth::handlers::logout));

    // Admin-only route for RBAC test (requires Bearer token + Admin role)
    let admin_routes = Router::new()
        .route("/admin/test", post(|| async { "admin only" }))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest("/api/v1", public_routes.merge(cookie_auth_routes).merge(admin_routes))
        .with_state(state)
}

/// Helper: collect response body bytes into a serde_json::Value
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

#[tokio::test]
async fn password_hashing_uses_argon2id() {
    let hash = cronometrix_api::auth::service::hash_password("testpass").unwrap();
    assert!(
        hash.starts_with("$argon2id$"),
        "Expected argon2id hash, got: {}",
        hash
    );
    cronometrix_api::auth::service::verify_password("testpass", &hash)
        .expect("verify_password should succeed with correct password");

    let result = cronometrix_api::auth::service::verify_password("wrongpass", &hash);
    assert!(result.is_err(), "verify_password should fail with wrong password");
}

#[tokio::test]
async fn auth_login_returns_jwt() {
    let db = common::test_db().await;

    // Insert a real admin with a properly hashed password
    let conn = db.connect().unwrap();
    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = cronometrix_api::auth::service::hash_password("password123").unwrap();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, 'admin', 'Admin User', ?2, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id, password_hash],
    )
    .await
    .unwrap();

    let app = build_test_app(db).await;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"username": "admin", "password": "password123"}).to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_json(response.into_body()).await;
    assert!(
        body["access_token"].is_string(),
        "Expected access_token in response, got: {:?}",
        body
    );
    assert_eq!(body["user"]["username"], "admin");
}

#[tokio::test]
async fn rbac_middleware_blocks_unauthorized() {
    let db = common::test_db().await;
    let app = build_test_app(db).await;

    // Create a viewer token
    let viewer_id = uuid::Uuid::new_v4().to_string();
    let viewer_token = common::test_access_token(&viewer_id, "viewer");

    // Attempt to POST to admin-only endpoint with viewer token — expect 403
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/admin/test")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Viewer token should be rejected with 403 on admin-only route"
    );
}

#[tokio::test]
async fn jwt_refresh_rotates_tokens() {
    let db = common::test_db().await;

    // Insert a real admin with a properly hashed password
    let conn = db.connect().unwrap();
    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = cronometrix_api::auth::service::hash_password("password123").unwrap();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, 'refreshadmin', 'Refresh Admin', ?2, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id, password_hash],
    )
    .await
    .unwrap();

    let app = build_test_app(db).await;

    // Login first to get refresh cookie
    let login_request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"username": "refreshadmin", "password": "password123"}).to_string()))
        .unwrap();

    let login_response = app.clone().oneshot(login_request).await.unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);

    // Extract refresh cookie from Set-Cookie header
    let cookie_header = login_response
        .headers()
        .get(header::SET_COOKIE)
        .expect("Should have Set-Cookie header")
        .to_str()
        .unwrap()
        .to_string();

    // Extract just the cookie name=value part
    let cookie_value = cookie_header
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_string();

    // Use refresh token to get new access token
    let refresh_request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, cookie_value)
        .body(Body::empty())
        .unwrap();

    let refresh_response = app.oneshot(refresh_request).await.unwrap();
    assert_eq!(
        refresh_response.status(),
        StatusCode::OK,
        "Refresh should return 200"
    );

    let body = body_to_json(refresh_response.into_body()).await;
    assert!(
        body["access_token"].is_string(),
        "Refresh should return new access_token, got: {:?}",
        body
    );
}

#[tokio::test]
async fn setup_wizard_creates_admin() {
    let db = common::test_db().await;
    let app = build_test_app(db).await;

    // First call — should succeed with 201
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/setup/init")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "full_name": "System Admin",
                "username": "sysadmin",
                "password": "securepassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "First setup/init should return 201"
    );

    // Second call — should return 409 Conflict
    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/setup/init")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "full_name": "Another Admin",
                "username": "otheradmin",
                "password": "anotherpassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();
    assert_eq!(
        response2.status(),
        StatusCode::CONFLICT,
        "Second setup/init should return 409"
    );

    let body2 = body_to_json(response2.into_body()).await;
    assert_eq!(
        body2["error"]["code"], "SETUP_ALREADY_COMPLETE",
        "Should return SETUP_ALREADY_COMPLETE error code"
    );
}
