//! Coverage gap-fill for `backend/src/license/service.rs` (08-04B Task 2).
//!
//! Baseline 18.95% line. Target ≥70%.
//!
//! `license_tests.rs` already covers verify_license_jwt + activate_license
//! (happy + 404/409 + fingerprint mismatch + algorithm-confusion). What
//! remains uncovered:
//!   * activate_license — empty URL guard (BadGateway ACTIVATION_UNREACHABLE)
//!   * activate_license — non-2xx other than 404/409 (BadGateway with status code)
//!   * activate_license — malformed JSON body (BadGateway)
//!   * activate_license — body missing "token" key (BadGateway)
//!   * activate_license — unreachable URL (network error → BadGateway)
//!   * load_and_validate_license — empty file branch
//!   * load_and_validate_license — happy path with matching fingerprint
//!   * try_renew (private — exercised via renewal_task) — full chain
//!   * renewal_task — cancel-on-token shutdown branch
//!
//! All tests use `MockServer` per the 04A wiremock pattern; license PEM
//! fixtures are reused from `tests/fixtures/`.

mod common;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use cronometrix_api::errors::AppError;
#[cfg(target_os = "linux")]
use cronometrix_api::license::service::try_renew;
use cronometrix_api::license::service::{
    activate_license, load_and_validate_license, renewal_task, LicenseClaims,
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method as wm_method, path as wm_path};
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

// =============================================================================
// activate_license — empty URL guard
// =============================================================================

#[tokio::test]
async fn activate_with_empty_url_returns_bad_gateway() {
    let path = format!("/tmp/cronometrix-empty-url-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("KEY", "", &path).await;
    match result {
        Err(AppError::BadGateway { code, .. }) => assert_eq!(code, "ACTIVATION_UNREACHABLE"),
        other => panic!("expected BadGateway ACTIVATION_UNREACHABLE, got {other:?}"),
    }
    assert!(!std::path::Path::new(&path).exists());
}

// =============================================================================
// activate_license — 5xx upstream
// =============================================================================

#[tokio::test]
async fn activate_with_5xx_maps_to_bad_gateway_with_status_code() {
    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;
    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-503-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("ANY", &url, &path).await;
    match result {
        Err(AppError::BadGateway { code, message }) => {
            assert_eq!(code, "ACTIVATION_UNREACHABLE");
            assert!(message.contains("503"), "msg should include 503: {message}");
        }
        Err(AppError::Internal(_)) => {
            // macOS: fingerprint collection blew up before the HTTP call.
        }
        other => panic!("expected BadGateway, got {other:?}"),
    }
}

// =============================================================================
// activate_license — malformed JSON body
// =============================================================================

#[tokio::test]
async fn activate_with_malformed_body_maps_to_bad_gateway() {
    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json-at-all"))
        .mount(&mock)
        .await;
    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-bad-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("ANY", &url, &path).await;
    match result {
        Err(AppError::BadGateway { code, message }) => {
            assert_eq!(code, "ACTIVATION_UNREACHABLE");
            assert!(message.contains("malformed"));
        }
        Err(AppError::Internal(_)) => { /* macOS dev */ }
        other => panic!("expected BadGateway malformed, got {other:?}"),
    }
}

// =============================================================================
// activate_license — body missing "token"
// =============================================================================

#[tokio::test]
async fn activate_with_missing_token_maps_to_bad_gateway() {
    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/licenses/activate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"foo": "bar"})))
        .mount(&mock)
        .await;
    let url = format!("{}/licenses/activate", mock.uri());
    let path = format!("/tmp/cronometrix-notok-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("ANY", &url, &path).await;
    match result {
        Err(AppError::BadGateway { code, message }) => {
            assert_eq!(code, "ACTIVATION_UNREACHABLE");
            assert!(message.contains("token"));
        }
        Err(AppError::Internal(_)) => { /* macOS dev */ }
        other => panic!("expected BadGateway missing token, got {other:?}"),
    }
}

// =============================================================================
// activate_license — unreachable URL (network error)
// =============================================================================

#[tokio::test]
async fn activate_with_unreachable_url_maps_to_bad_gateway() {
    // Port 1 on loopback is reliably refused.
    let url = "http://127.0.0.1:1/licenses/activate".to_string();
    let path = format!("/tmp/cronometrix-unreach-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("ANY", &url, &path).await;
    match result {
        Err(AppError::BadGateway { code, .. }) => {
            assert_eq!(code, "ACTIVATION_UNREACHABLE");
        }
        Err(AppError::Internal(_)) => { /* macOS dev — fp lookup blew up first */ }
        other => panic!("expected BadGateway unreachable, got {other:?}"),
    }
    assert!(!std::path::Path::new(&path).exists());
}

// =============================================================================
// load_and_validate_license — empty file branch
// =============================================================================

#[tokio::test]
async fn load_and_validate_license_empty_file_returns_false() {
    let path = format!("/tmp/cronometrix-empty-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, "").unwrap();
    let valid = load_and_validate_license(&path).await;
    assert!(!valid);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn load_and_validate_license_whitespace_only_returns_false() {
    let path = format!("/tmp/cronometrix-ws-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, "   \n   ").unwrap();
    let valid = load_and_validate_license(&path).await;
    assert!(!valid);
    let _ = std::fs::remove_file(&path);
}

// =============================================================================
// load_and_validate_license — fingerprint mismatch path (signature OK, fp != local)
// =============================================================================

#[tokio::test]
async fn load_and_validate_license_fingerprint_mismatch_returns_false() {
    // Sign a valid JWT but with a fingerprint that cannot match the local box.
    let claims = make_claims("DEFINITELY-NOT-LOCAL-FP-ZZZZZZZZZZ", 365 * 24 * 60 * 60);
    let signed = sign_test_jwt(&claims);
    let path = format!("/tmp/cronometrix-fpmm-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, &signed).unwrap();

    let valid = load_and_validate_license(&path).await;
    // On Linux: fp computed → mismatch → false.
    // On macOS: fingerprint::collect_fingerprint errors → also false.
    assert!(!valid, "fingerprint mismatch must be fail-closed");
    let _ = std::fs::remove_file(&path);
}

// =============================================================================
// load_and_validate_license — happy path (valid + matching fp)
// On macOS dev hosts the fingerprint collection blows up, so this test only
// runs on Linux.
// =============================================================================

#[cfg(target_os = "linux")]
#[tokio::test]
async fn load_and_validate_license_happy_path_on_linux() {
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().expect("linux fp");
    let claims = make_claims(&fp, 365 * 24 * 60 * 60);
    let signed = sign_test_jwt(&claims);
    let path = format!("/tmp/cronometrix-ok-{}.jwt", uuid::Uuid::new_v4());
    std::fs::write(&path, &signed).unwrap();

    let valid = load_and_validate_license(&path).await;
    assert!(valid);
    let _ = std::fs::remove_file(&path);
}

// =============================================================================
// renewal_task — cancellation token shuts down the loop cleanly.
// =============================================================================

#[tokio::test]
async fn renewal_task_exits_on_cancel() {
    let lv = Arc::new(AtomicBool::new(true));
    let cancel = CancellationToken::new();
    let path = format!("/tmp/cronometrix-renewal-{}.jwt", uuid::Uuid::new_v4());

    // No URL configured → loop has nothing to do; the test only verifies that
    // the cancellation branch fires before the 24h sleep elapses.
    let task = tokio::spawn({
        let path = path.clone();
        let lv = lv.clone();
        let cancel = cancel.clone();
        async move {
            renewal_task(path, String::new(), lv, cancel).await;
        }
    });

    // Give the loop a moment to enter the select.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    cancel.cancel();

    // Must exit promptly (not wait 24h).
    let r = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
    assert!(r.is_ok(), "renewal_task must exit on cancel within 5s");
}

// =============================================================================
// try_renew + renewal_task — Linux behavior coverage
// =============================================================================

#[cfg(target_os = "linux")]
fn write_license(path: &std::path::Path, fingerprint: &str, exp_offset_secs: i64) -> String {
    let token = sign_test_jwt(&make_claims(fingerprint, exp_offset_secs));
    std::fs::write(path, &token).unwrap();
    token
}

#[cfg(target_os = "linux")]
fn assert_bad_gateway(result: Result<(), AppError>, expected_code: &str) {
    match result {
        Err(AppError::BadGateway { code, .. }) => assert_eq!(code, expected_code),
        other => panic!("expected BadGateway {expected_code}, got {other:?}"),
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn try_renew_skips_license_outside_renewal_window() {
    let mock = MockServer::start().await;
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let jwt_path = tmp.path().join("license.jwt");
    let original = write_license(&jwt_path, &fp, 60 * 24 * 60 * 60);

    try_renew(jwt_path.to_str().unwrap(), &format!("{}/renew", mock.uri()))
        .await
        .unwrap();

    assert_eq!(std::fs::read_to_string(&jwt_path).unwrap(), original);
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn try_renew_maps_transport_and_upstream_failures() {
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let jwt_path = tmp.path().join("license.jwt");
    write_license(&jwt_path, &fp, 24 * 60 * 60);

    let unreachable = try_renew(jwt_path.to_str().unwrap(), "http://127.0.0.1:1/renew").await;
    assert_bad_gateway(unreachable, "RENEWAL_UNREACHABLE");

    let mock = MockServer::start().await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/renew"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;
    let rejected = try_renew(jwt_path.to_str().unwrap(), &format!("{}/renew", mock.uri())).await;
    assert_bad_gateway(rejected, "RENEWAL_FAILED");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn try_renew_rejects_malformed_missing_and_untrusted_tokens() {
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let jwt_path = tmp.path().join("license.jwt");
    let original = write_license(&jwt_path, &fp, 24 * 60 * 60);
    let mock = MockServer::start().await;

    Mock::given(wm_method("POST"))
        .and(wm_path("/malformed"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&mock)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/missing"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&mock)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/untrusted"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "not.a.jwt"})),
        )
        .mount(&mock)
        .await;

    let malformed = try_renew(
        jwt_path.to_str().unwrap(),
        &format!("{}/malformed", mock.uri()),
    )
    .await;
    assert_bad_gateway(malformed, "RENEWAL_FAILED");
    let missing = try_renew(
        jwt_path.to_str().unwrap(),
        &format!("{}/missing", mock.uri()),
    )
    .await;
    assert_bad_gateway(missing, "RENEWAL_FAILED");
    assert!(matches!(
        try_renew(
            jwt_path.to_str().unwrap(),
            &format!("{}/untrusted", mock.uri())
        )
        .await,
        Err(AppError::Unlicensed)
    ));
    assert_eq!(std::fs::read_to_string(&jwt_path).unwrap(), original);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn try_renew_enforces_fingerprint_and_atomically_persists_success() {
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let jwt_path = tmp.path().join("license.jwt");
    let original = write_license(&jwt_path, &fp, 24 * 60 * 60);
    let mismatch = sign_test_jwt(&make_claims(
        "NOT-THE-LOCAL-FINGERPRINT",
        365 * 24 * 60 * 60,
    ));
    let renewed = sign_test_jwt(&make_claims(&fp, 365 * 24 * 60 * 60));
    let mock = MockServer::start().await;

    Mock::given(wm_method("POST"))
        .and(wm_path("/mismatch"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": mismatch})),
        )
        .mount(&mock)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/success"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": renewed})),
        )
        .mount(&mock)
        .await;

    assert!(matches!(
        try_renew(
            jwt_path.to_str().unwrap(),
            &format!("{}/mismatch", mock.uri())
        )
        .await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(std::fs::read_to_string(&jwt_path).unwrap(), original);

    try_renew(
        jwt_path.to_str().unwrap(),
        &format!("{}/success", mock.uri()),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&jwt_path).unwrap(), renewed);
    assert!(!jwt_path.with_extension("jwt.tmp").exists());
}

#[cfg(target_os = "linux")]
async fn drive_one_renewal_tick(
    jwt_path: String,
    renew_url: String,
    license_valid: Arc<AtomicBool>,
) -> (tokio::task::JoinHandle<()>, CancellationToken) {
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        renewal_task(jwt_path, renew_url, license_valid, task_cancel).await;
    });
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(24 * 60 * 60)).await;
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
    (task, cancel)
}

#[cfg(target_os = "linux")]
async fn stop_renewal_task(task: tokio::task::JoinHandle<()>, cancel: CancellationToken) {
    cancel.cancel();
    tokio::task::yield_now().await;
    task.await.unwrap();
}

#[cfg(target_os = "linux")]
async fn wait_for_request_path(mock: &MockServer, expected_path: &str) {
    let observed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if mock
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|request| request.url.path() == expected_path)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        observed.is_ok(),
        "renewal endpoint did not receive a request at {expected_path}"
    );
}

#[cfg(target_os = "linux")]
async fn wait_for_file_content(path: &std::path::Path, expected: &str) {
    let persisted = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(path).unwrap() == expected {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        persisted.is_ok(),
        "renewal task did not persist the expected JWT"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test(start_paused = true)]
async fn renewal_task_skips_when_license_is_invalid_or_url_is_empty() {
    let mock = MockServer::start().await;
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let jwt_path = tmp.path().join("license.jwt");
    let original = write_license(&jwt_path, &fp, 24 * 60 * 60);

    let (task, cancel) = drive_one_renewal_tick(
        jwt_path.to_string_lossy().into_owned(),
        format!("{}/renew", mock.uri()),
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    assert!(mock.received_requests().await.unwrap().is_empty());
    stop_renewal_task(task, cancel).await;

    let (task, cancel) = drive_one_renewal_tick(
        jwt_path.to_string_lossy().into_owned(),
        String::new(),
        Arc::new(AtomicBool::new(true)),
    )
    .await;
    assert_eq!(std::fs::read_to_string(&jwt_path).unwrap(), original);
    stop_renewal_task(task, cancel).await;
}

#[cfg(target_os = "linux")]
#[tokio::test(start_paused = true)]
async fn renewal_task_attempts_failure_and_success_without_stopping() {
    let mock = MockServer::start().await;
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let failed_path = tmp.path().join("failed.jwt");
    let success_path = tmp.path().join("success.jwt");
    let failed_original = write_license(&failed_path, &fp, 24 * 60 * 60);
    write_license(&success_path, &fp, 24 * 60 * 60);
    let renewed = sign_test_jwt(&make_claims(&fp, 365 * 24 * 60 * 60));

    Mock::given(wm_method("POST"))
        .and(wm_path("/failure"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;
    Mock::given(wm_method("POST"))
        .and(wm_path("/success"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": renewed})),
        )
        .mount(&mock)
        .await;

    let (failure_task, failure_cancel) = drive_one_renewal_tick(
        failed_path.to_string_lossy().into_owned(),
        format!("{}/failure", mock.uri()),
        Arc::new(AtomicBool::new(true)),
    )
    .await;
    // Resume real time while reqwest and wiremock complete their socket I/O.
    // Keeping Tokio time paused here can auto-advance reqwest's timeout before
    // the loopback response is processed.
    tokio::time::resume();
    wait_for_request_path(&mock, "/failure").await;
    assert_eq!(
        std::fs::read_to_string(&failed_path).unwrap(),
        failed_original
    );
    stop_renewal_task(failure_task, failure_cancel).await;

    tokio::time::pause();
    let (success_task, success_cancel) = drive_one_renewal_tick(
        success_path.to_string_lossy().into_owned(),
        format!("{}/success", mock.uri()),
        Arc::new(AtomicBool::new(true)),
    )
    .await;
    tokio::time::resume();
    wait_for_request_path(&mock, "/success").await;
    wait_for_file_content(&success_path, &renewed).await;
    stop_renewal_task(success_task, success_cancel).await;

    let requests = mock.received_requests().await.unwrap();
    assert!(requests
        .iter()
        .any(|request| request.url.path() == "/failure"));
    assert!(requests
        .iter()
        .any(|request| request.url.path() == "/success"));
}

// =============================================================================
// activate_license — happy path (Linux only — needs fp to land)
// Adds round-trip coverage of the verify-and-write branches.
// =============================================================================

#[cfg(target_os = "linux")]
#[tokio::test]
async fn activate_license_persists_jwt_on_linux() {
    let mock = MockServer::start().await;
    let fp = cronometrix_api::license::fingerprint::collect_fingerprint().unwrap();
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
    let path = format!("/tmp/cronometrix-persist-{}.jwt", uuid::Uuid::new_v4());
    let result = activate_license("KEY", &url, &path).await;
    assert!(result.is_ok());
    assert!(std::path::Path::new(&path).exists());

    let cached = std::fs::read_to_string(&path).unwrap();
    assert!(!cached.is_empty());
    let _ = std::fs::remove_file(&path);
}
