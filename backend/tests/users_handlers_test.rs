mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::users;
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

    let admin_routes = Router::new()
        .route("/users", post(users::handlers::create_user))
        .route("/users", get(users::handlers::list_users))
        .route("/users/{id}", get(users::handlers::get_user))
        .route("/users/{id}", patch(users::handlers::update_user))
        .route("/users/{id}", delete(users::handlers::deactivate_user))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    let app = Router::new().nest("/api/v1", admin_routes).with_state(state);
    (app, tmp)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

fn admin_token() -> String {
    common::test_access_token("actor-admin-id", "admin")
}

fn auth_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn create_user_returns_201_then_validation_422() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let resp = app
        .clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "newuser", "full_name": "New User", "role": "viewer", "password": "password123"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["username"], "newuser");
    assert_eq!(body["status"], "active");

    // Short password → 422 VALIDATION_ERROR.
    let resp = app
        .clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "x", "full_name": "X", "role": "viewer", "password": "short"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn list_users_returns_200_with_pagination_envelope() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;
    app.clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "listed", "full_name": "Listed", "role": "admin", "password": "password123"}),
        ))
        .await
        .unwrap();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_json(resp.into_body()).await;
    assert!(body["total"].as_i64().unwrap() >= 1);
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn get_user_200_and_404() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;
    let created = app
        .clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "gettable", "full_name": "Get", "role": "supervisor", "password": "password123"}),
        ))
        .await
        .unwrap();
    let id = body_to_json(created.into_body()).await["id"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/users/{}", id))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/users/does-not-exist")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_user_200_and_invalid_body_422() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;
    let created = app
        .clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "updatable", "full_name": "Up", "role": "viewer", "password": "password123"}),
        ))
        .await
        .unwrap();
    let body = body_to_json(created.into_body()).await;
    let id = body["id"].as_str().unwrap().to_string();
    let version = body["version"].as_i64().unwrap();

    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/users/{}", id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(json!({"full_name": "Updated Name", "version": version}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated = body_to_json(resp.into_body()).await;
    assert_eq!(updated["full_name"], "Updated Name");

    // Invalid: password too short → 422.
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/users/{}", id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::from(json!({"password": "no", "version": version + 1}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn deactivate_user_returns_204() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;
    let created = app
        .clone()
        .oneshot(auth_post(
            "/api/v1/users",
            json!({"username": "deactivatable", "full_name": "De", "role": "viewer", "password": "password123"}),
        ))
        .await
        .unwrap();
    let body = body_to_json(created.into_body()).await;
    let id = body["id"].as_str().unwrap().to_string();
    let version = body["version"].as_i64().unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/users/{}?version={}", id, version))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
