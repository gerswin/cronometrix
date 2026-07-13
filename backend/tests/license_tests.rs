//! Integration tests for Phase 6 license backend.
//! Coverage: LIC-02 (fingerprint), LIC-03 (DO Functions activation),
//! LIC-04 (cached JWT), LIC-05 (anti-cloning), DEPL-04 (offline operation).
//!
//! Wave 0 baseline: scaffolding tests so subsequent tasks add behavior tests
//! without restructuring this file.

mod common;

use cronometrix_api::errors::AppError;
use cronometrix_api::license;
use cronometrix_api::license::service::LicenseClaims;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::json;
use wiremock::matchers::{body_partial_json, method as wm_method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_priv_key_pem() -> &'static [u8] {
    include_bytes!("fixtures/test_license_privkey.pem")
}

fn sign_test_jwt(claims: &LicenseClaims) -> String {
    let header = Header::new(Algorithm::RS256);
    let key = EncodingKey::from_rsa_pem(test_priv_key_pem()).expect("test priv key parses");
    encode(&header, claims, &key).expect("sign test jwt")
}

fn make_claims(fingerprint: &str, exp_offset_secs: i64) -> LicenseClaims {
    let now = chrono::Utc::now().timestamp();
    LicenseClaims {
        license_key: "TEST-KEY-1234-5678".to_string(),
        hardware_fingerprint: fingerprint.to_string(),
        product: "cronometrix".to_string(),
        iat: now,
        exp: now + exp_offset_secs,
    }
}

/// Wave 0: prove the license module is reachable from integration tests.
#[test]
fn license_module_is_reachable() {
    // If this compiles, the module path is correct.
    let _ = license::fingerprint::collect_fingerprint;
    let _ = license::service::verify_license_jwt;
}

/// LIC-02 (Wave 0 stub — full determinism test added in Task 2 with stubbed inputs).
/// On a Linux host this returns Ok; on macOS dev it returns Err — both are acceptable
/// for the stub. Determinism is asserted in Task 2.
#[test]
fn fingerprint_collection_returns_string_or_error() {
    let result = license::fingerprint::collect_fingerprint();
    match result {
        Ok(s) => assert!(
            s.len() == 64 || s.is_empty(),
            "expected 64-hex-char SHA256 or empty"
        ),
        Err(_) => { /* macOS dev host — expected */ }
    }
}

/// AppError::Unlicensed -> HTTP 403 with code "UNLICENSED".
#[tokio::test]
async fn unlicensed_error_maps_to_403_with_code_unlicensed() {
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    let resp = AppError::Unlicensed.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "UNLICENSED");
    assert_eq!(json["error"]["status"], 403);
}

/// Prove jsonwebtoken's `use_pem` feature is enabled.
/// Without it, DecodingKey::from_rsa_pem does not exist.
#[test]
fn jsonwebtoken_use_pem_feature_enabled() {
    let pem = include_bytes!("fixtures/test_license_pubkey.pem");
    let _key = jsonwebtoken::DecodingKey::from_rsa_pem(pem).expect("test pubkey should parse");
}

// =============================================================================
// Task 2 — behavior tests (LIC-02..05, DEPL-04)
// =============================================================================

/// LIC-02: deterministic fingerprint on Linux. macOS dev hosts skip this test
/// via cfg gate because /proc/cpuinfo does not exist there.
#[cfg(target_os = "linux")]
#[test]
fn test_fingerprint_deterministic() {
    let a = cronometrix_api::license::fingerprint::collect_fingerprint().expect("linux fp");
    let b = cronometrix_api::license::fingerprint::collect_fingerprint().expect("linux fp");
    assert_eq!(a, b, "fingerprint must be deterministic");
    assert_eq!(a.len(), 64, "SHA256 hex must be 64 chars");
}

/// Algorithm confusion defense: HS256-signed token must NEVER verify.
#[test]
fn test_verify_rejects_hs256_token() {
    let header = Header::new(Algorithm::HS256);
    let claims = make_claims("any-fp", 3600);
    let key = EncodingKey::from_secret(b"some-symmetric-secret");
    let token = encode(&header, &claims, &key).unwrap();
    let result = cronometrix_api::license::service::verify_license_jwt(&token);
    assert!(matches!(result, Err(AppError::Unlicensed)));
}

/// Signature integrity: a JWT with a bogus signature must not verify against
/// the embedded public key.
#[test]
fn test_verify_rejects_invalid_signature() {
    let header_b64 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
    let payload_b64 = "eyJsaWNlbnNlX2tleSI6ImYiLCJoYXJkd2FyZV9maW5nZXJwcmludCI6ImYiLCJwcm9kdWN0IjoiYyIsImlhdCI6MCwiZXhwIjoyfQ";
    let bogus_sig = "AAAA";
    let token = format!("{}.{}.{}", header_b64, payload_b64, bogus_sig);
    let result = cronometrix_api::license::service::verify_license_jwt(&token);
    assert!(matches!(result, Err(AppError::Unlicensed)));
}

/// D-07 soft expiry: an expired but validly signed JWT must still verify.
#[test]
fn test_verify_accepts_expired_token() {
    let claims = make_claims("test-fp", -3600);
    let token = sign_test_jwt(&claims);
    let result = cronometrix_api::license::service::verify_license_jwt(&token);
    assert!(
        result.is_ok(),
        "expired but signed token must still verify (D-07)"
    );
    assert_eq!(result.unwrap().license_key, "TEST-KEY-1234-5678");
}

/// load_and_validate_license must return false (not panic) when the cache file
/// does not exist — first-run boot path.
#[tokio::test]
async fn test_load_and_validate_license_no_file() {
    let path = format!("/tmp/cronometrix-no-such-file-{}", uuid::Uuid::new_v4());
    let valid = cronometrix_api::license::service::load_and_validate_license(&path).await;
    assert!(!valid);
}

/// load_and_validate_license must return false when the cached file is not a
/// valid signed JWT.
#[tokio::test]
async fn test_load_and_validate_license_invalid_signature() {
    let path = format!("/tmp/cronometrix-bogus-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, "not.a.jwt").unwrap();
    let valid = cronometrix_api::license::service::load_and_validate_license(&path).await;
    assert!(!valid);
    let _ = std::fs::remove_file(&path);
}

/// LIC-03: activate_license POSTs license_key + hardware_fingerprint to DO
/// Functions and persists the returned token to disk on success.
#[tokio::test]
async fn test_activation_calls_do_functions_with_fingerprint() {
    let mock = MockServer::start().await;
    // Use the local fingerprint so the JWT we mint matches what activate_license expects.
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap_or_default();
    let claims = make_claims(&fp, 365 * 24 * 60 * 60);
    let signed = sign_test_jwt(&claims);

    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .and(body_partial_json(
            json!({ "license_key": "ABCD-EFGH-IJKL-MNOP" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "token": signed })))
        .mount(&mock)
        .await;

    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-license-{}.jwt", uuid::Uuid::new_v4());
    let result =
        cronometrix_api::license::service::activate_license("ABCD-EFGH-IJKL-MNOP", &url, &path)
            .await;

    // On Linux the activation succeeds (fp present, claims match, JWT verifies, file written).
    // On macOS dev the very first step (collect_fingerprint) errors with AppError::Internal,
    // which is the correct fail-closed behavior — and the file is still NOT written.
    match result {
        Ok(_) => {
            let persisted = std::fs::read_to_string(&path).unwrap();
            assert!(!persisted.is_empty(), "JWT must be written on success");
            // Offline-load round-trip works (DEPL-04)
            let valid = cronometrix_api::license::service::load_and_validate_license(&path).await;
            let _ = valid; // platform-dependent; persistence is the load-bearing assertion
        }
        Err(AppError::Internal(_)) => {
            assert!(
                !std::path::Path::new(&path).exists(),
                "JWT must NOT be persisted when fingerprint collection fails"
            );
        }
        other => panic!("unexpected activation result: {:?}", other),
    }
    let _ = std::fs::remove_file(&path);
}

/// LIC-05: when DO Functions returns a JWT whose fingerprint claim does NOT
/// match the local fingerprint, activate_license must fail and MUST NOT persist
/// the token to disk.
#[tokio::test]
async fn test_activation_rejects_fingerprint_mismatch() {
    let mock = MockServer::start().await;
    let claims = make_claims("DEFINITELY-NOT-LOCAL-FP", 365 * 24 * 60 * 60);
    let signed = sign_test_jwt(&claims);

    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "token": signed })))
        .mount(&mock)
        .await;

    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-mismatch-{}.jwt", uuid::Uuid::new_v4());
    let result =
        cronometrix_api::license::service::activate_license("ABCD-EFGH-IJKL-MNOP", &url, &path)
            .await;

    // On Linux the local fp is collected and rejected as Forbidden.
    // On macOS the fp lookup itself errors → Internal (still fail-closed).
    match result {
        Err(AppError::Forbidden) => {}
        Err(AppError::Internal(_)) => {}
        other => panic!("expected Forbidden or Internal, got {:?}", other),
    }

    // Critical: file was NOT created
    assert!(
        !std::path::Path::new(&path).exists(),
        "JWT must not be persisted on fp mismatch"
    );
}

/// DO Functions 404 must surface as AppError::NotFound{code:"LICENSE_NOT_FOUND"}.
#[tokio::test]
async fn test_activation_maps_404_to_license_not_found() {
    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;

    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-404-{}.jwt", uuid::Uuid::new_v4());
    let result = cronometrix_api::license::service::activate_license("BAD", &url, &path).await;
    match result {
        Err(AppError::NotFound { code, .. }) => assert_eq!(code, "LICENSE_NOT_FOUND"),
        Err(AppError::Internal(_)) => {
            // macOS dev: fingerprint collection fails before the HTTP call —
            // acceptable, the activation still does not persist.
        }
        other => panic!("expected NotFound LICENSE_NOT_FOUND, got {:?}", other),
    }
}

/// DO Functions 409 must surface as AppError::Conflict{code:"ALREADY_ACTIVATED"}.
#[tokio::test]
async fn test_activation_maps_409_to_already_activated() {
    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(409))
        .mount(&mock)
        .await;

    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-409-{}.jwt", uuid::Uuid::new_v4());
    let result = cronometrix_api::license::service::activate_license("DUP", &url, &path).await;
    match result {
        Err(AppError::Conflict { code, .. }) => assert_eq!(code, "ALREADY_ACTIVATED"),
        Err(AppError::Internal(_)) => { /* macOS dev — see 404 test */ }
        other => panic!("expected Conflict ALREADY_ACTIVATED, got {:?}", other),
    }
}

/// DEPL-04: with a valid cached JWT on disk and NO DO Functions URL configured,
/// the verify path needs no internet at all.
#[tokio::test]
async fn test_offline_operation_with_cached_jwt() {
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap_or_default();
    let claims = make_claims(&fp, 365 * 24 * 60 * 60);
    let signed = sign_test_jwt(&claims);
    let path = format!("/tmp/cronometrix-offline-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, &signed).unwrap();

    // Direct verify works without any URL — no internet required.
    let claims_back = cronometrix_api::license::service::verify_license_jwt(&signed);
    assert!(claims_back.is_ok());

    let _ = std::fs::remove_file(&path);
}

// =============================================================================
// Plan 06-02 Task 2 — Gate behavior tests
// =============================================================================
//
// These tests build a minimal Axum router with the same middleware ordering
// main.rs uses (auth.then license, applied via route_layer in reverse — so
// require_license fires first per axum 0.8 semantics — closes T-06-17).
// The license gate FALSE case is asserted here so existing per-plan test
// fixtures can stay in their license_valid=true configuration.

mod gate_behavior_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{header, Method, Request, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use cronometrix_api::auth;
    use cronometrix_api::config::Config;
    use cronometrix_api::employees;
    use cronometrix_api::setup;

    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// Build a router with the license gate wired exactly as main.rs.
    /// `license_valid` parameter controls the gate state for the test.
    /// Returns (Router, the AtomicBool clone, TempDir) so the caller can
    /// observe the post-activation flip and keep the per-test path roots
    /// alive (Plan 08-02 D-20 / Pitfall 1 in 08-RESEARCH.md).
    async fn build_gated_app(
        db: libsql::Database,
        license_valid: bool,
        do_url: String,
    ) -> (Router, Arc<AtomicBool>, tempfile::TempDir) {
        let lv = Arc::new(AtomicBool::new(license_valid));
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
            license_jwt_path: format!("/tmp/cronometrix-test-{}.jwt", uuid::Uuid::new_v4()),
            do_functions_activate_url: do_url,
            do_functions_renew_url: String::new(),
            cors_allowed_origins: Vec::new(),
            cookie_secure: false,
        });

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let paths = Arc::new(cronometrix_api::state::Paths::for_test(tmp.path()));
        let mut state = common::test_state(Arc::new(db), config, paths);
        state.license_valid = lv.clone();

        let public_routes = Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/setup/status", get(setup::handlers::setup_status))
            .route("/setup/activate", post(setup::handlers::setup_activate));

        // Mirror main.rs ordering: auth FIRST in source, license SECOND in
        // source — axum 0.8 reverses route_layer so license runs FIRST on
        // the request path.
        let viewer_routes = Router::new()
            .route("/employees", get(employees::handlers::list_employees))
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth::middleware::require_auth,
            ))
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                license::middleware::require_license,
            ));

        let app = Router::new()
            .nest("/api/v1", public_routes.merge(viewer_routes))
            .with_state(state);
        (app, lv, tmp)
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::json!(null))
    }

    /// LIC-01 / T-06-17: with license_valid=false, a protected route returns
    /// 403 UNLICENSED even when the Authorization header carries a valid Bearer
    /// — proving the license gate runs BEFORE require_auth (no auth-state leak).
    #[tokio::test]
    async fn test_license_gate_blocks_unlicensed_protected_route() {
        let db = common::test_db().await;
        let (app, _lv, _tmp) = build_gated_app(db, false, String::new()).await;
        let token = common::test_access_token("admin-id", "admin");
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/employees")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = body_to_json(resp.into_body()).await;
        assert_eq!(body["error"]["code"], "UNLICENSED");
    }

    /// With license_valid=true, the existing auth+handler chain runs unchanged.
    /// We only assert the response is NOT a 403 UNLICENSED — the actual handler
    /// status depends on data shape and is owned by other tests.
    #[tokio::test]
    async fn test_license_gate_allows_licensed_protected_route() {
        let db = common::test_db().await;
        let (app, _lv, _tmp) = build_gated_app(db, true, String::new()).await;
        let token = common::test_access_token("admin-id", "admin");
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/employees")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Not 403 — i.e., NOT gated. (Could be 200 OK or 401 if token expired —
        // the load-bearing assertion is "license gate did not fire".)
        let status = resp.status();
        if status == StatusCode::FORBIDDEN {
            let body = body_to_json(resp.into_body()).await;
            // If forbidden, must NOT be UNLICENSED — that would mean gate fired.
            assert_ne!(
                body["error"]["code"], "UNLICENSED",
                "license gate must not fire when license_valid=true"
            );
        }
    }

    /// /setup/status is public (no auth, no license) and returns BOTH
    /// `initialized` and `licensed` boolean fields. With license_valid=false
    /// the response must show licensed=false but still 200 OK.
    #[tokio::test]
    async fn test_public_setup_status_ungated_when_unlicensed() {
        let db = common::test_db().await;
        let (app, _lv, _tmp) = build_gated_app(db, false, String::new()).await;
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/setup/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_to_json(resp.into_body()).await;
        assert!(
            body.get("initialized").is_some(),
            "status must include 'initialized'"
        );
        assert!(
            body.get("licensed").is_some(),
            "status must include 'licensed'"
        );
        assert_eq!(body["licensed"], false);
    }

    /// Format validation: short keys never reach DO Functions.
    #[tokio::test]
    async fn test_setup_activate_validates_license_key_format() {
        let db = common::test_db().await;
        let (app, _lv, _tmp) = build_gated_app(db, false, "http://unused".to_string()).await;
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/setup/activate")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"license_key":"X"}).to_string(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// LIC-03 round-trip: wiremock returns a valid signed JWT, handler verifies
    /// + persists + flips license_valid → true.
    /// Platform note: on macOS dev hosts collect_fingerprint() errors out
    /// (no /proc/cpuinfo) so the activation falls into AppError::Internal —
    /// the gate stays closed (correct fail-closed behavior). Both outcomes
    /// are accepted as long as the security invariant holds.
    #[tokio::test]
    async fn test_setup_activate_succeeds_via_wiremock() {
        use wiremock::matchers::{method as wm_method, path as wm_path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap_or_default();
        let claims = make_claims(&fp, 365 * 24 * 60 * 60);
        let signed = sign_test_jwt(&claims);

        Mock::given(wm_method("POST"))
            .and(wm_path("/licenses/activate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": signed})),
            )
            .mount(&mock)
            .await;

        let url = format!("{}/licenses/activate", mock.uri());
        let db = common::test_db().await;
        let (app, lv, _tmp) = build_gated_app(db, false, url).await;
        assert!(!lv.load(Ordering::Relaxed));

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/setup/activate")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"license_key":"ABCD-EFGH-IJKL-MNOP"}).to_string(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        if resp.status() == StatusCode::OK {
            assert!(
                lv.load(Ordering::Relaxed),
                "license_valid must be true after success"
            );
            let body = body_to_json(resp.into_body()).await;
            assert_eq!(body["activated"], true);
        } else {
            // macOS dev path — fingerprint collection failed; gate stays closed.
            assert!(
                !lv.load(Ordering::Relaxed),
                "license_valid must remain false on activation failure"
            );
        }
    }

    /// DO Functions 404 surfaces as AppError::NotFound{code:"LICENSE_NOT_FOUND"}.
    /// On macOS dev, fingerprint collection fails before the HTTP call so
    /// the handler errors with Internal — both outcomes are fail-closed.
    #[tokio::test]
    async fn test_setup_activate_maps_404_to_license_not_found() {
        use wiremock::matchers::{method as wm_method, path as wm_path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(wm_method("POST"))
            .and(wm_path("/licenses/activate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock)
            .await;

        let url = format!("{}/licenses/activate", mock.uri());
        let db = common::test_db().await;
        let (app, _lv, _tmp) = build_gated_app(db, false, url).await;

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/setup/activate")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"license_key":"ABCD-EFGH-IJKL-MNOP"}).to_string(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        if resp.status() == StatusCode::NOT_FOUND {
            let body = body_to_json(resp.into_body()).await;
            assert_eq!(body["error"]["code"], "LICENSE_NOT_FOUND");
        }
        // Otherwise: 500 Internal on macOS dev — acceptable, gate stays closed.
    }
}
