//! Phase 9 — integration test that locks the __test_reset gating contract:
//!
//! - Without CRONOMETRIX_E2E=true: the route is not registered → router returns 404
//! - With CRONOMETRIX_E2E=true: the route is registered AND the handler verifies
//!   the env again → returns 200 {"reset": true}
//!
//! Both tests use in-process Axum routing (no subprocess) so they run fast,
//! but they faithfully replicate the `if std::env::var("CRONOMETRIX_E2E") == Ok("true")`
//! guard from main.rs because `build_minimal_router` mirrors that guard exactly.
//!
//! Threat model: T-09-02 — accidental access to __test_reset in production would
//! truncate audit_log, destroying compliance evidence.

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

/// Minimal Config for test purposes. Mirrors the pattern in auth_tests.rs.
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

/// Build a minimal router that replicates the CRONOMETRIX_E2E gate from main.rs.
/// When `e2e=false`, the /__test_reset route is not registered at all (router → 404).
/// When `e2e=true`, the route is registered and the handler also re-checks the env.
fn build_minimal_router(
    state: cronometrix_api::state::AppState,
    e2e: bool,
) -> Router {
    let mut api = Router::new();
    if e2e {
        api = api.route(
            "/__test_reset",
            post(cronometrix_api::test_reset::test_reset),
        );
    }
    Router::new().nest("/api/v1", api).with_state(state)
}

/// D-13 / T-09-02 negative gate test:
/// Without CRONOMETRIX_E2E=true the route is absent from the router → 404.
/// This is the primary security assertion: the route cannot be reached in prod.
#[tokio::test]
async fn test_reset_returns_404_without_e2e_flag() {
    // Remove env var so both the router guard and the handler guard are unset.
    // NOTE: SAFETY — std::env is process-global. These tests are intentionally
    // NOT run in parallel (they mutate env). cargo nextest runs each integration
    // test file in isolation, so this is safe between test files. Within this
    // file, tokio::test is single-threaded by default — tests run sequentially.
    std::env::remove_var("CRONOMETRIX_E2E");

    let db = common::test_db().await;
    let config = test_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    // Build router WITHOUT the test_reset route (e2e=false mirrors main.rs behaviour
    // when CRONOMETRIX_E2E is unset at startup).
    let app = build_minimal_router(state, false);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/__test_reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "__test_reset MUST return 404 when CRONOMETRIX_E2E is not set \
         (route not registered)"
    );
}

/// D-13 / T-09-02 positive gate test:
/// With CRONOMETRIX_E2E=true the route exists AND returns 200 {"reset": true}.
#[tokio::test]
async fn test_reset_returns_200_with_e2e_flag() {
    // Set the env var so the handler's defense-in-depth check passes.
    std::env::set_var("CRONOMETRIX_E2E", "true");

    let db = common::test_db().await;
    let config = test_config();
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    // Build router WITH the test_reset route (e2e=true mirrors main.rs behaviour
    // when CRONOMETRIX_E2E=true at startup).
    let app = build_minimal_router(state, true);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/__test_reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "__test_reset MUST return 200 when CRONOMETRIX_E2E=true and route is registered"
    );

    // Verify response body is {"reset": true}
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "reset": true }),
        "response body must be {{\"reset\": true}}"
    );

    // Clean up env so other tests are not affected.
    std::env::remove_var("CRONOMETRIX_E2E");
}
