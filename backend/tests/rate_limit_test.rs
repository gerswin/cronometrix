mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::middleware::rate_limit::{self, LOGIN_INIT_BURST_SIZE, STATUS_BURST_SIZE};
use cronometrix_api::setup;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

/// Build a test app wired the same way `main.rs` wires the rate-limited
/// public-auth routes: `/auth/login` + `/setup/init` behind the strict tier
/// (`rate_limit::login_and_init_rate_limit()`), `/setup/status` behind the
/// generous tier (`rate_limit::status_rate_limit()`). Returns (Router,
/// TempDir) per Plan 08-02 D-20: caller binds the TempDir to a local that
/// outlives every assertion.
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
        device_push_base_url: String::new(),
    });

    let (state, tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    let (login_init_layer, _login_init_limiter) = rate_limit::login_and_init_rate_limit();
    let (status_layer, _status_limiter) = rate_limit::status_rate_limit();

    let rate_limited_public_routes = Router::new()
        .route("/auth/login", post(auth::handlers::login))
        .route("/setup/init", post(setup::handlers::setup_init))
        .route_layer(login_init_layer)
        .merge(
            Router::new()
                .route("/setup/status", get(setup::handlers::setup_status))
                .route_layer(status_layer),
        );

    let app = Router::new()
        .nest("/api/v1", rate_limited_public_routes)
        .with_state(state);
    (app, tmp)
}

/// Two-hop shape matching production `X-Forwarded-For`: "<client>,
/// <nginx-appended remote_addr>". The extractor must key on the client
/// entry, not nginx's own trailing one — see
/// `middleware::rate_limit::client_ip_from_headers`.
fn xff_for(client_ip: &str) -> String {
    format!("{client_ip}, 172.20.0.5")
}

fn login_request(xff: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", xff)
        .body(Body::from(
            json!({"username": "nonexistent-user", "password": "wrong-password"}).to_string(),
        ))
        .unwrap()
}

fn status_request(xff: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri("/api/v1/setup/status")
        .header("x-forwarded-for", xff)
        .body(Body::empty())
        .unwrap()
}

/// H-12: intentos repetidos de login desde la misma IP se frenan.
#[tokio::test]
async fn repeated_failed_logins_from_one_ip_are_throttled() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let xff = xff_for("203.0.113.10");
    let mut saw_429 = false;
    for attempt in 0..(LOGIN_INIT_BURST_SIZE + 5) {
        let response = app.clone().oneshot(login_request(&xff)).await.unwrap();
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
        // Every request the limiter lets through must still reach the real
        // handler and get the normal invalid-credentials rejection — the
        // limiter must not corrupt the request path for admitted requests.
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {attempt} was neither throttled nor rejected as invalid credentials",
        );
    }
    assert!(
        saw_429,
        "expected a 429 within LOGIN_INIT_BURST_SIZE ({LOGIN_INIT_BURST_SIZE}) + 5 repeated login attempts from one IP",
    );
}

/// Un usuario legítimo no queda bloqueado por el ruido de otro.
#[tokio::test]
async fn one_noisy_ip_does_not_lock_out_a_different_one() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    let noisy_xff = xff_for("203.0.113.20");
    let mut noisy_saw_429 = false;
    for _ in 0..(LOGIN_INIT_BURST_SIZE + 5) {
        let response = app.clone().oneshot(login_request(&noisy_xff)).await.unwrap();
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            noisy_saw_429 = true;
            break;
        }
    }
    assert!(
        noisy_saw_429,
        "the noisy IP should have been throttled before this assertion runs"
    );

    // A distinct client IP, keyed separately by the same extractor, must
    // still get the normal (non-throttled) response — proving the limiter
    // is per-key, not global.
    let quiet_xff = xff_for("203.0.113.21");
    let response = app.clone().oneshot(login_request(&quiet_xff)).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "a different client IP must not be throttled by another IP's noise"
    );
}

/// L-01: el endpoint publico de estado tambien se limita.
#[tokio::test]
async fn the_public_status_endpoint_is_throttled() {
    let db = common::test_db().await;
    let (app, _tmp) = build_test_app(db).await;

    // Margin is generous (not "+5" like the login test) because this tier's
    // fast replenish rate (20 req/s, see STATUS_REPLENISH_PERIOD) can refill
    // a handful of tokens over the real wall-clock time this tight in-process
    // loop takes to run — a small margin flaked here in exactly that way.
    let xff = xff_for("203.0.113.30");
    let mut saw_429 = false;
    for attempt in 0..(STATUS_BURST_SIZE + 200) {
        let response = app.clone().oneshot(status_request(&xff)).await.unwrap();
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "attempt {attempt} was neither throttled nor a normal 200 from /setup/status",
        );
    }
    assert!(
        saw_429,
        "expected /setup/status to throttle within STATUS_BURST_SIZE ({STATUS_BURST_SIZE}) + 200 requests from one IP",
    );
}
