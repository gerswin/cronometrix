mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use axum::routing::{get, patch};
use cronometrix_api::auth;
use cronometrix_api::rules;
use cronometrix_api::config::Config;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use http_body_util::BodyExt;

/// Build a test app with rules routes only. Returns (Router, TempDir) per
/// Plan 08-02 D-20: caller binds the TempDir to a local that outlives every
/// assertion (Pitfall 1 in 08-RESEARCH.md).
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

    // GET rules: any authenticated role
    let viewer_routes = Router::new()
        .route("/rules", get(rules::handlers::get_rules))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    // PATCH rules: admin only
    let admin_routes = Router::new()
        .route("/rules", patch(rules::handlers::update_rules))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    let app = Router::new()
        .nest("/api/v1", viewer_routes.merge(admin_routes))
        .with_state(state);
    (app, tmp)
}

/// Collect response body into JSON.
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

#[tokio::test]
async fn rules_tolerance_endpoint() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (app, _tmp) = build_test_app(db).await;

    // GET /rules — verify default singleton values (seeded in migration: 10/10)
    let get_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK, "GET /rules should return 200");
    let rules = body_to_json(get_resp.into_body()).await;

    assert_eq!(
        rules["late_arrival_tolerance_min"], 10,
        "Default late_arrival_tolerance_min should be 10"
    );
    assert_eq!(
        rules["early_departure_tolerance_min"], 10,
        "Default early_departure_tolerance_min should be 10"
    );
    assert!(rules["effective_from"].is_string(), "effective_from should be ISO 8601 string");
    assert!(rules["updated_at"].is_string(), "updated_at should be ISO 8601 string");

    let version = rules["version"].as_i64().unwrap();

    // PATCH — update tolerances
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/rules")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "late_arrival_tolerance_min": 15,
                "early_departure_tolerance_min": 5,
                "version": version
            })
            .to_string(),
        ))
        .unwrap();

    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK, "PATCH /rules should return 200");
    let updated = body_to_json(patch_resp.into_body()).await;

    assert_eq!(updated["late_arrival_tolerance_min"], 15, "late_arrival_tolerance_min should be updated to 15");
    assert_eq!(updated["early_departure_tolerance_min"], 5, "early_departure_tolerance_min should be updated to 5");
    assert_eq!(updated["version"], version + 1, "Version should increment");
}

#[tokio::test]
async fn rules_bonus_minutes_config() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (app, _tmp) = build_test_app(db).await;

    // GET to fetch current version
    let get_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let current = body_to_json(get_resp.into_body()).await;
    let version = current["version"].as_i64().unwrap();

    // PATCH bonus_minutes
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/rules")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "bonus_minutes": 15,
                "version": version
            })
            .to_string(),
        ))
        .unwrap();

    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK, "PATCH should return 200");
    let patched = body_to_json(patch_resp.into_body()).await;
    assert_eq!(patched["bonus_minutes"], 15, "bonus_minutes should be 15 after PATCH");

    // GET again to verify persistence
    let get_req2 = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp2 = app.clone().oneshot(get_req2).await.unwrap();
    assert_eq!(get_resp2.status(), StatusCode::OK);
    let verified = body_to_json(get_resp2.into_body()).await;
    assert_eq!(verified["bonus_minutes"], 15, "bonus_minutes should persist after GET");
}

#[tokio::test]
async fn rules_effective_from_updates_on_change() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (app, _tmp) = build_test_app(db).await;

    // GET to capture initial effective_from and version
    let get_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let initial = body_to_json(get_resp.into_body()).await;
    let initial_effective_from = initial["effective_from"].as_str().unwrap().to_string();
    let version = initial["version"].as_i64().unwrap();

    // Wait 1 second so the timestamp will differ
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // PATCH rules — should update effective_from per RULE-03
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/rules")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "late_arrival_tolerance_min": 20,
                "version": version
            })
            .to_string(),
        ))
        .unwrap();

    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK, "PATCH should return 200");

    // GET rules again to verify effective_from changed
    let get_req2 = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp2 = app.clone().oneshot(get_req2).await.unwrap();
    assert_eq!(get_resp2.status(), StatusCode::OK);
    let updated = body_to_json(get_resp2.into_body()).await;
    let new_effective_from = updated["effective_from"].as_str().unwrap().to_string();

    assert_ne!(
        new_effective_from, initial_effective_from,
        "effective_from should be updated to a newer timestamp after PATCH (RULE-03)"
    );
    assert_eq!(updated["late_arrival_tolerance_min"], 20, "tolerance should be updated");
}
