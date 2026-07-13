mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::setup;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// Build a test app with all auth + setup routes wired up. Returns
/// (Router, TempDir) per Plan 08-02 D-20: caller binds the TempDir to a
/// local that outlives every assertion (Pitfall 1 in 08-RESEARCH.md).
async fn build_test_app(db: libsql::Database) -> (Router, tempfile::TempDir) {
    let config = Arc::new(Config {
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
    });

    let (state, tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

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

    let app = Router::new()
        .nest(
            "/api/v1",
            public_routes.merge(cookie_auth_routes).merge(admin_routes),
        )
        .with_state(state);
    (app, tmp)
}

/// Helper: collect response body bytes into a serde_json::Value
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn seed_auth_user(
    db: &libsql::Database,
    username: &str,
    full_name: &str,
    password: &str,
) -> String {
    let conn = db.connect().unwrap();
    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = cronometrix_api::auth::service::hash_password(password).unwrap();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![
            user_id.clone(),
            username.to_string(),
            full_name.to_string(),
            password_hash
        ],
    )
    .await
    .unwrap();
    user_id
}

async fn login_tokens(app: &Router, username: &str, password: &str) -> (String, String) {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": username, "password": password}).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let refresh_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("login must set refresh cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_string();
    let body = body_to_json(response.into_body()).await;
    let access_token = body["access_token"]
        .as_str()
        .expect("login must return access token")
        .to_string();
    (access_token, refresh_cookie)
}

async fn refresh_with_cookie(app: &Router, cookie: &str) -> axum::response::Response {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap()
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
    assert!(
        result.is_err(),
        "verify_password should fail with wrong password"
    );
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

    let (app, _tmp) = build_test_app(db).await;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": "password123"}).to_string(),
        ))
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
    let (app, _tmp) = build_test_app(db).await;

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
    seed_auth_user(&db, "refreshadmin", "Refresh Admin", "password123").await;

    let (app, _tmp) = build_test_app(db).await;
    let (login_access_token, login_refresh_cookie) =
        login_tokens(&app, "refreshadmin", "password123").await;

    let refresh_response = refresh_with_cookie(&app, &login_refresh_cookie).await;
    assert_eq!(
        refresh_response.status(),
        StatusCode::OK,
        "Refresh should return 200"
    );
    let replacement_refresh_cookie = refresh_response
        .headers()
        .get(header::SET_COOKIE)
        .expect("refresh must set replacement cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_string();
    let body = body_to_json(refresh_response.into_body()).await;
    let replacement_access_token = body["access_token"]
        .as_str()
        .expect("refresh must return replacement access token");
    assert_ne!(
        replacement_access_token, login_access_token,
        "immediate refresh must mint a different access token"
    );
    assert_ne!(
        replacement_refresh_cookie, login_refresh_cookie,
        "immediate refresh must mint a different refresh cookie"
    );
}

#[tokio::test]
async fn jwt_refresh_rejects_replayed_cookie_without_set_cookie() {
    let db = common::test_db().await;
    seed_auth_user(&db, "replayadmin", "Replay Admin", "password123").await;
    let (app, _tmp) = build_test_app(db).await;
    let (_, old_refresh_cookie) = login_tokens(&app, "replayadmin", "password123").await;

    let first_response = refresh_with_cookie(&app, &old_refresh_cookie).await;
    assert_eq!(first_response.status(), StatusCode::OK);

    let replay_response = refresh_with_cookie(&app, &old_refresh_cookie).await;
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        replay_response.headers().get(header::SET_COOKIE).is_none(),
        "replay rejection must not set or delete a cookie"
    );
}

#[tokio::test]
async fn concurrent_refresh_allows_one_rotation_without_loser_cookie() {
    let db = common::test_db().await;
    seed_auth_user(&db, "concurrentadmin", "Concurrent Admin", "password123").await;
    let (app, _tmp) = build_test_app(db).await;
    let (_, old_refresh_cookie) = login_tokens(&app, "concurrentadmin", "password123").await;

    let (first, second) = tokio::join!(
        refresh_with_cookie(&app, &old_refresh_cookie),
        refresh_with_cookie(&app, &old_refresh_cookie)
    );
    let responses = [first, second];
    let success_count = responses
        .iter()
        .filter(|response| response.status() == StatusCode::OK)
        .count();
    let unauthorized_count = responses
        .iter()
        .filter(|response| response.status() == StatusCode::UNAUTHORIZED)
        .count();
    assert_eq!(success_count, 1, "exactly one refresh must win the CAS");
    assert_eq!(
        unauthorized_count, 1,
        "exactly one concurrent refresh must lose the CAS"
    );

    let winner = responses
        .iter()
        .find(|response| response.status() == StatusCode::OK)
        .unwrap();
    let winner_cookie = winner
        .headers()
        .get(header::SET_COOKIE)
        .expect("winning refresh must set its replacement cookie")
        .to_str()
        .unwrap();
    assert!(
        !winner_cookie.starts_with(&old_refresh_cookie),
        "winning refresh must replace the original cookie"
    );

    let loser = responses
        .iter()
        .find(|response| response.status() == StatusCode::UNAUTHORIZED)
        .unwrap();
    assert!(
        loser.headers().get(header::SET_COOKIE).is_none(),
        "losing refresh must not clear or overwrite the winning cookie"
    );
}

#[tokio::test]
async fn setup_wizard_creates_admin() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

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
