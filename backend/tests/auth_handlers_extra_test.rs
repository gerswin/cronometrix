//! Extra integration tests for `auth::handlers` to close the 67.39% baseline
//! gap from Plan 03 (08-04A bucket row 2). Existing `auth_tests.rs` already
//! covers the login + refresh + setup happy paths; this file covers:
//!
//!   - login: 401 invalid password, 401 unknown username, 422 missing fields
//!   - refresh: 401 missing cookie, 401 expired/invalid token, 401 wrong
//!     token_type (access token in refresh slot), 401 mismatched stored hash
//!   - logout: 401 missing cookie, 200 happy path with cookie clear
//!
//! Per threat model T-08-12A, negative-path coverage of auth controls IS the
//! security control under test.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

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

    let routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/login", post(auth::handlers::login))
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/auth/logout", post(auth::handlers::logout));

    let app = Router::new().nest("/api/v1", routes).with_state(state);
    (app, tmp)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn seed_admin(db: &libsql::Database, username: &str, password: &str) -> String {
    let conn = db.connect().unwrap();
    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = cronometrix_api::auth::service::hash_password(password).unwrap();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Admin', ?3, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), username.to_string(), password_hash],
    )
    .await
    .unwrap();
    user_id
}

// =============================================================================
// LOGIN — fail-closed branches (T-08-12A)
// =============================================================================

#[tokio::test]
async fn login_401_when_password_wrong() {
    let db = common::test_db().await;
    let _uid = seed_admin(&db, "alice", "correct-password").await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "alice", "password": "wrong-password"}).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "wrong password must return 401"
    );
}

#[tokio::test]
async fn login_401_when_username_unknown() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "nobody", "password": "x"}).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "unknown user must return 401 (no enumeration)"
    );
}

#[tokio::test]
async fn login_422_when_username_blank() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "", "password": "x"}).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty username must fail validation"
    );
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn login_422_when_password_blank() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "alice", "password": ""}).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// =============================================================================
// REFRESH — fail-closed branches
// =============================================================================

#[tokio::test]
async fn refresh_401_when_no_cookie() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "missing refresh cookie must 401"
    );
}

#[tokio::test]
async fn refresh_401_when_token_garbage() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, "refresh_token=not-a-real-jwt")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "malformed JWT in refresh cookie must 401"
    );
}

#[tokio::test]
async fn refresh_401_when_access_token_used_as_refresh() {
    // T-01-10 / token_type-confusion: an access token (token_type = "access")
    // must NEVER be accepted by /auth/refresh.
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    // Forge an access token.
    let access = common::test_access_token(&uuid::Uuid::new_v4().to_string(), "admin");

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, format!("refresh_token={}", access))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "access token in refresh slot must be rejected (token_type guard)"
    );
}

#[tokio::test]
async fn refresh_401_when_stored_hash_does_not_match() {
    // The user's refresh_token_hash in DB no longer matches the cookie.
    // Simulates the post-rotation / post-password-change / post-logout state.
    let db = common::test_db().await;
    seed_admin(&db, "bob", "bobpass1234").await;

    // Login to get a real refresh cookie via a temporary app handle. The DB
    // is shared across both apps because libsql::Database is Arc-cloned in
    // common::test_state.
    let db_arc = std::sync::Arc::new(db);
    let config = std::sync::Arc::new(Config {
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
    let (state, _tmp) = common::test_state_with_tmpdir(db_arc.clone(), config);
    let routes = Router::new()
        .route("/auth/login", post(auth::handlers::login))
        .route("/auth/refresh", post(auth::handlers::refresh));
    let app = Router::new().nest("/api/v1", routes).with_state(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "bob", "password": "bobpass1234"}).to_string(),
        ))
        .unwrap();
    let login_resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let cookie_header = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("Set-Cookie present")
        .to_str()
        .unwrap()
        .to_string();
    let cookie_value = cookie_header.split(';').next().unwrap().trim().to_string();

    // Simulate post-logout / post-password-change: NULL out the refresh hash
    // in the DB. The cookie itself is still a valid JWT (signature + exp +
    // token_type all check out) but the DB row no longer has a matching hash.
    let conn = db_arc.connect().unwrap();
    conn.execute(
        "UPDATE users SET refresh_token_hash = NULL WHERE username = 'bob'",
        (),
    )
    .await
    .unwrap();

    // Refresh now must 401 because the stored hash mismatches the cookie's hash.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, cookie_value)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "cookie with mismatched stored hash must 401 (T-01-10)"
    );
}

// =============================================================================
// LOGOUT — covers the unauthenticated branch + happy path
// =============================================================================

#[tokio::test]
async fn logout_401_when_no_cookie() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_200_clears_refresh_cookie_after_login() {
    let db = common::test_db().await;
    seed_admin(&db, "carol", "carolpass1234").await;
    let (app, _tmp) = build_test_app(db).await;

    // Login.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "carol", "password": "carolpass1234"}).to_string(),
        ))
        .unwrap();
    let login_resp = app.clone().oneshot(req).await.unwrap();
    let cookie_header = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cookie_value = cookie_header.split(';').next().unwrap().trim().to_string();

    // Logout.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header(header::COOKIE, &cookie_value)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Set-Cookie on logout must include a Max-Age=0 (expired) refresh cookie.
    let set_cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("logout sets a clearing cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        set_cookie.contains("refresh_token="),
        "logout must clear refresh_token cookie, got: {set_cookie}"
    );
    // Either Max-Age=0 or an Expires in the past — accept Max-Age=0 form.
    assert!(
        set_cookie.contains("Max-Age=0") || set_cookie.contains("max-age=0"),
        "logout cookie must have Max-Age=0, got: {set_cookie}"
    );

    // The post-logout refresh attempt with the same (now invalidated) cookie 401s.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header(header::COOKIE, cookie_value)
        .body(Body::empty())
        .unwrap();
    let r = app.oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_401_when_token_invalid() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header(header::COOKIE, "refresh_token=not-a-real-jwt")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "logout requires a valid refresh JWT"
    );
}
