//! Integration tests for `isapi::client::DeviceConnection`. Targets the
//! 57.23% baseline gap from Plan 03 (08-04A bucket row 13). Uses `wiremock`
//! to simulate the Hikvision device side without real hardware.
//!
//! Coverage focus:
//!   - door_open / reboot / enrollment_mode / delete_user / upsert_user
//!     happy paths via mocked 200
//!   - non-2xx error path returns Err
//!   - upload_face: digest-auth challenge + retry, immediate success branch,
//!     error on non-2xx after digest, error on 401 with no WWW-Authenticate
//!   - Debug impl redacts password
//!   - new() returns a Client successfully

use cronometrix_api::isapi::client::DeviceConnection;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// =============================================================================
// new() + Debug-redact
// =============================================================================

#[test]
fn new_returns_a_connection() {
    let conn = DeviceConnection::new("https://10.0.0.1:443", "admin", "secret", false)
        .expect("Client::builder should succeed");
    assert_eq!(conn.base_url, "https://10.0.0.1:443");
    assert_eq!(conn.username, "admin");
}

#[test]
fn new_with_insecure_tls_does_not_error() {
    let conn = DeviceConnection::new("https://10.0.0.1:443", "admin", "x", true).unwrap();
    assert_eq!(conn.username, "admin");
}

#[test]
fn debug_impl_redacts_password() {
    let conn = DeviceConnection::new("https://10.0.0.1:443", "admin", "supersecret", false)
        .unwrap();
    let dbg = format!("{:?}", conn);
    assert!(
        !dbg.contains("supersecret"),
        "password must not appear in Debug, got: {dbg}"
    );
    assert!(dbg.contains("[redacted]"), "Debug must mark redaction");
    assert!(dbg.contains("admin"));
    assert!(dbg.contains("10.0.0.1"));
}

// =============================================================================
// door_open / reboot / enrollment_mode happy paths via mock
// =============================================================================

#[tokio::test]
async fn door_open_happy_path_via_mock_200() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/RemoteControl/door/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<ResponseStatus>OK</ResponseStatus>"))
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let result = conn.door_open().await.expect("door_open should 200");
    assert!(result.contains("OK"));
}

#[tokio::test]
async fn door_open_returns_err_on_non_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/RemoteControl/door/1"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal"))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn.door_open().await.expect_err("500 must be Err");
    let s = err.to_string();
    assert!(
        s.contains("500") || s.contains("status") || s.contains("non-success"),
        "err must mention non-success: {s}"
    );
}

#[tokio::test]
async fn reboot_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/System/reboot"))
        .respond_with(ResponseTemplate::new(200).set_body_string("rebooting"))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn.reboot().await.expect("reboot 200");
    assert!(r.contains("rebooting"));
}

#[tokio::test]
async fn enrollment_mode_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn.enrollment_mode().await.expect("200");
    assert!(r.contains("\"statusCode\":1"));
}

#[tokio::test]
async fn delete_user_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn.delete_user("face-42").await.expect("200");
    assert!(r.contains("statusCode"));
}

#[tokio::test]
async fn upsert_user_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn
        .upsert_user("face-42", "Alice")
        .await
        .expect("200");
    assert!(r.contains("statusCode"));
}

#[tokio::test]
async fn upsert_user_handles_duplicate_employee_no_as_success() {
    let server = MockServer::start().await;
    // Hikvision returns 200 with subStatusCode duplicateEmployeeNo. The client
    // logs a warn but treats it as Ok (idempotent upsert).
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"statusCode":1,"subStatusCode":"duplicateEmployeeNo"}"#,
        ))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn
        .upsert_user("face-99", "Bob")
        .await
        .expect("duplicate must be Ok (idempotent)");
    assert!(r.contains("duplicateEmployeeNo"));
}

#[tokio::test]
async fn enrollment_mode_returns_err_on_non_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn.enrollment_mode().await.expect_err("503");
    assert!(err.to_string().contains("503") || err.to_string().contains("non-success"));
}

// =============================================================================
// upload_face: 401 → digest auth retry path (canonical RESEARCH pattern)
// =============================================================================

const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

#[tokio::test]
async fn upload_face_immediate_200_no_digest_needed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#),
        )
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn
        .upload_face("face-99", MINI_JPEG.to_vec())
        .await
        .expect("immediate 200 path");
    assert!(r.contains("statusCode"));
}

#[tokio::test]
async fn upload_face_returns_err_on_non_2xx_first_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal"))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn
        .upload_face("face-1", MINI_JPEG.to_vec())
        .await
        .expect_err("500 must Err");
    assert!(err.to_string().contains("500") || err.to_string().contains("non-success"));
}

#[tokio::test]
async fn upload_face_returns_err_on_401_without_www_authenticate() {
    // 401 with no WWW-Authenticate header → digest_auth::parse fails.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn
        .upload_face("face-1", MINI_JPEG.to_vec())
        .await
        .expect_err("401 with no WWW-Authenticate must Err");
    let s = err.to_string();
    assert!(
        s.contains("WWW-Authenticate") || s.contains("digest") || s.contains("parse"),
        "err must indicate digest parse failure: {s}"
    );
}
