//! Integration tests for Phase 6 license backend.
//! Coverage: LIC-02 (fingerprint), LIC-03 (DO Functions activation),
//! LIC-04 (cached JWT), LIC-05 (anti-cloning), DEPL-04 (offline operation).
//!
//! Wave 0 baseline: scaffolding tests so subsequent tasks add behavior tests
//! without restructuring this file.

mod common;

use cronometrix_api::errors::AppError;
use cronometrix_api::license;

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
