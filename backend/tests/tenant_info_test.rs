mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, patch};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use cronometrix_api::tenant_info;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// Build a test app with tenant_info routes only (GET viewer + PATCH admin).
async fn build_test_app(db: libsql::Database) -> Router {
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
    });

    let state = AppState {
        db: Arc::new(db),
        config,
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
    };

    let viewer_routes = Router::new()
        .route("/tenant-info", get(tenant_info::handlers::get_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let admin_routes = Router::new()
        .route("/tenant-info", patch(tenant_info::handlers::patch_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest("/api/v1", viewer_routes.merge(admin_routes))
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

/// Test 1: GET returns the seed row (empty values, version=1).
#[tokio::test]
async fn get_returns_seed_row() {
    let db = common::test_db().await;
    let viewer_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&viewer_id, "viewer");
    let app = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/tenant-info")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET should return 200");

    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["client_name"], "", "Seed client_name should be empty");
    assert_eq!(body["client_rif"], "", "Seed client_rif should be empty");
    assert_eq!(body["address"], "", "Seed address should be empty");
    assert_eq!(body["version"], 1, "Seed version should be 1");
    assert!(
        body["updated_at"].is_string(),
        "updated_at should be ISO 8601 string"
    );
}

/// Test 2: Admin PATCH succeeds; values are returned with version incremented.
#[tokio::test]
async fn admin_patch_succeeds() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/tenant-info")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "client_name": "Acme",
                "client_rif": "J-1-9",
                "address": "Caracas",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "PATCH should return 200");

    let body = body_to_json(resp.into_body()).await;
    assert_eq!(body["client_name"], "Acme", "client_name should be updated");
    assert_eq!(body["client_rif"], "J-1-9", "client_rif should be updated");
    assert_eq!(body["address"], "Caracas", "address should be updated");
    assert_eq!(body["version"], 2, "Version should increment to 2");
}

/// Test 3: Supervisor cannot PATCH (require_admin returns 403).
#[tokio::test]
async fn supervisor_blocked() {
    let db = common::test_db().await;
    let sup_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&sup_id, "supervisor");
    let app = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/tenant-info")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "client_name": "ShouldNotApply",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Supervisor PATCH should return 403"
    );
}

/// Test 4: Stale version PATCH returns 409 VERSION_CONFLICT.
#[tokio::test]
async fn version_conflict() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/tenant-info")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "client_name": "Stale",
                "version": 999
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "Stale version PATCH should return 409"
    );

    let body = body_to_json(resp.into_body()).await;
    assert_eq!(
        body["error"]["code"], "VERSION_CONFLICT",
        "Error code should be VERSION_CONFLICT, got: {:?}",
        body
    );
}

/// Test 5: After admin PATCH, the audit_tenant_info_update trigger writes a row.
#[tokio::test]
async fn audit_trigger_fires() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");

    // Hold a connection to the test DB BEFORE we hand it to the app so we can verify audit_log
    // afterwards. (We re-connect via the app's state too — both connections share the file.)
    let conn = db.connect().expect("connect");

    let app = build_test_app(db).await;

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/tenant-info")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "client_name": "AuditTest CA",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "PATCH should return 200");

    // Verify audit_log has a row for tenant_info UPDATE.
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM audit_log WHERE table_name = 'tenant_info' AND operation = 'UPDATE'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    assert!(
        count >= 1,
        "audit_log should contain at least one tenant_info UPDATE row, got {}",
        count
    );
}
