//! Locks the destructive E2E reset capability without process-global env races.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use cronometrix_api::config::Config;
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

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

fn build_router(
    mut state: cronometrix_api::state::AppState,
    e2e_enabled: bool,
    reset_enabled: bool,
) -> Router {
    state.e2e_enabled = e2e_enabled;
    state.test_reset_enabled = reset_enabled;
    let mut api = Router::new();
    if e2e_enabled && reset_enabled {
        api = api.route(
            "/__test_reset",
            post(cronometrix_api::test_reset::test_reset),
        );
    }
    Router::new().nest("/api/v1", api).with_state(state)
}

async fn post_reset(e2e_enabled: bool, reset_enabled: bool) -> axum::response::Response {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), test_config());
    build_router(state, e2e_enabled, reset_enabled)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/__test_reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn test_reset_returns_404_without_e2e_capability() {
    assert_eq!(
        post_reset(false, false).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_reset_returns_404_when_only_reset_capability_is_enabled() {
    assert_eq!(
        post_reset(false, true).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn handler_defense_in_depth_checks_each_capability() {
    for (e2e_enabled, reset_enabled) in [(false, false), (false, true), (true, false)] {
        let db = common::test_db().await;
        let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), test_config());
        state.e2e_enabled = e2e_enabled;
        state.test_reset_enabled = reset_enabled;

        assert_eq!(
            cronometrix_api::test_reset::test_reset(axum::extract::State(state))
                .await
                .unwrap_err(),
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn test_reset_returns_404_when_reset_capability_is_disabled() {
    assert_eq!(
        post_reset(true, false).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_reset_returns_200_when_both_capabilities_are_enabled() {
    let response = post_reset(true, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body, serde_json::json!({"reset": true}));
}

#[tokio::test]
async fn test_reset_preserves_existing_audit_evidence() {
    let db = Arc::new(common::test_db().await);
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO audit_log (id, table_name, record_id, operation, new_data, created_at) \
         VALUES ('reset-proof', 'employees', 'employee-proof', 'INSERT', '{}', 1770000000)",
        (),
    )
    .await
    .unwrap();
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), test_config());

    let response = build_router(state, true, true)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/__test_reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let count: i64 = conn
        .query(
            "SELECT COUNT(*) FROM audit_log WHERE id = 'reset-proof'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(count, 1, "E2E reset must never erase legal audit evidence");
}
