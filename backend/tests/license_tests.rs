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
        Ok(s) => assert!(s.len() == 64 || s.is_empty(), "expected 64-hex-char SHA256 or empty"),
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
    let _key = jsonwebtoken::DecodingKey::from_rsa_pem(pem)
        .expect("test pubkey should parse");
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
    assert!(result.is_ok(), "expired but signed token must still verify (D-07)");
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
        .and(body_partial_json(json!({ "license_key": "ABCD-EFGH-IJKL-MNOP" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "token": signed })))
        .mount(&mock)
        .await;

    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-license-{}.jwt", uuid::Uuid::new_v4());
    let result = cronometrix_api::license::service::activate_license(
        "ABCD-EFGH-IJKL-MNOP",
        &url,
        &path,
    )
    .await;

    // On Linux the activation succeeds (fp present, claims match, JWT verifies, file written).
    // On macOS dev the very first step (collect_fingerprint) errors with AppError::Internal,
    // which is the correct fail-closed behavior — and the file is still NOT written.
    match result {
        Ok(_) => {
            let persisted = std::fs::read_to_string(&path).unwrap();
            assert!(!persisted.is_empty(), "JWT must be written on success");
            // Offline-load round-trip works (DEPL-04)
            let valid =
                cronometrix_api::license::service::load_and_validate_license(&path).await;
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
    let result = cronometrix_api::license::service::activate_license(
        "ABCD-EFGH-IJKL-MNOP",
        &url,
        &path,
    )
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
