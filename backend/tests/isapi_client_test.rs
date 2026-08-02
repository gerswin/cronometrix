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

use cronometrix_api::isapi::client::{posix_time_zone, DeviceConnection};
use wiremock::matchers::{body_string_contains, header, method, path};
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
    let conn =
        DeviceConnection::new("https://10.0.0.1:443", "admin", "supersecret", false).unwrap();
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
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<ResponseStatus>OK</ResponseStatus>"),
        )
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

/// The request shape is asserted, not just the response. DS-K1T341CMFW
/// firmware V3.3.8 rejects a JSON body with `statusCode 6 / badParameters`;
/// only the `<CaptureFaceDataCond>` XML root carrying the required
/// `dataType=binary` is accepted. Matching on body + content-type keeps a
/// regression to JSON from passing silently.
#[tokio::test]
async fn enrollment_mode_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .and(header("content-type", "application/xml"))
        .and(body_string_contains("<CaptureFaceDataCond"))
        .and(body_string_contains("<dataType>binary</dataType>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<CaptureFaceData version="2.0"><captureProgress>0</captureProgress></CaptureFaceData>"#,
        ))
        .mount(&server)
        .await;
    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let r = conn.enrollment_mode().await.expect("200");
    assert!(r.contains("<captureProgress>0</captureProgress>"));
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
    let r = conn.upsert_user("face-42", "Alice").await.expect("200");
    assert!(r.contains("statusCode"));
}

#[tokio::test]
async fn upsert_user_handles_duplicate_employee_no_as_success() {
    let server = MockServer::start().await;
    // Hikvision returns 200 with subStatusCode duplicateEmployeeNo. The client
    // logs a warn but treats it as Ok (idempotent upsert).
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"statusCode":1,"subStatusCode":"duplicateEmployeeNo"}"#),
        )
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
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
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

// =============================================================================
// capture_face_image — kiosk capture (hardware-verified wire format)
// =============================================================================

/// Byte-for-byte copy of a real DS-K1T341CMFW V3.3.8 `CaptureFaceData` success
/// response (headers, boundary, part order, CRLF placement), with the biometric
/// image swapped for a dummy JPEG so no real face lives in the repository.
const CAPTURE_MULTIPART: &[u8] = include_bytes!("fixtures/capture_face_data_multipart.bin");

/// The 131-byte answer the device gives when nobody stood in front of it.
const CAPTURE_NO_FACE: &str = r#"<CaptureFaceData version="2.0" xmlns="http://www.isapi.org/ver20/XMLSchema"><captureProgress>0</captureProgress></CaptureFaceData>"#;

/// `deviceBusy` is returned as HTTP 400 while a previous capture window closes.
/// It must be retried, never surfaced as a device rejection.
const CAPTURE_DEVICE_BUSY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ResponseStatus version="1.0" xmlns="http://www.hikvision.com/ver10/XMLSchema">
<statusCode>2</statusCode><statusString>Device Busy</statusString>
<subStatusCode>deviceBusy</subStatusCode><errorMsg>dataType</errorMsg>
</ResponseStatus>"#;

#[tokio::test]
async fn capture_face_image_extracts_jpeg_from_multipart() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(CAPTURE_MULTIPART))
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let jpeg = conn.capture_face_image().await.expect("capture");

    assert!(
        jpeg.starts_with(&[0xFF, 0xD8, 0xFF]),
        "extracted part must start with the JPEG magic bytes"
    );
    assert!(
        jpeg.ends_with(&[0xFF, 0xD9]),
        "trailing CRLF before the boundary must be stripped"
    );
    // Only the image part comes back — no boundary, no XML part.
    assert!(
        !jpeg.windows(9).any(|w| w == b"MIME_boun"),
        "multipart framing leaked into the returned image"
    );
    assert!(!jpeg.windows(7).any(|w| w == b"Content"));
}

/// The device never exposed `CapturedFacePicture` (404 / notSupport); the image
/// arrives inline. A capture that needs several windows must keep retrying
/// rather than failing on the first `captureProgress: 0`.
#[tokio::test]
async fn capture_face_image_retries_past_empty_windows_and_device_busy() {
    let server = MockServer::start().await;
    // wiremock serves scenarios in mount order, each with an expectation cap.
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(200).set_body_string(CAPTURE_NO_FACE))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(400).set_body_string(CAPTURE_DEVICE_BUSY))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(CAPTURE_MULTIPART))
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let jpeg = conn.capture_face_image().await.expect("capture after retries");
    assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
}

/// A genuine device rejection must abort immediately — only `deviceBusy` retries.
#[tokio::test]
async fn capture_face_image_fails_fast_on_non_busy_rejection() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(r#"<ResponseStatus><subStatusCode>badParameters</subStatusCode></ResponseStatus>"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn.capture_face_image().await.expect_err("must not retry");
    assert!(err.to_string().contains("400"), "got: {err}");
}

// =============================================================================
// set_time / posix_time_zone
// =============================================================================

/// POSIX inverts the sign everyone reads off a clock: it states how far you
/// must move to reach UTC. Caracas (UTC-4) is therefore `CST+4:00:00`. Getting
/// this backwards puts the device eight hours out — worse than not syncing.
#[test]
fn posix_time_zone_inverts_the_utc_offset_sign() {
    assert_eq!(posix_time_zone(-4 * 3600), "CST+4:00:00");
    assert_eq!(posix_time_zone(2 * 3600), "CST-2:00:00");
    assert_eq!(posix_time_zone(0), "CST+0:00:00");
    // India, UTC+5:30 — the half-hour must survive.
    assert_eq!(posix_time_zone(5 * 3600 + 1800), "CST-5:30:00");
    // Chatham, UTC+12:45.
    assert_eq!(posix_time_zone(12 * 3600 + 2700), "CST-12:45:00");
}

#[tokio::test]
async fn set_time_puts_manual_mode_with_local_time_and_zone() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/System/time"))
        .and(header("content-type", "application/xml"))
        .and(body_string_contains("<timeMode>manual</timeMode>"))
        .and(body_string_contains("<localTime>2026-08-02T10:15:30</localTime>"))
        .and(body_string_contains("<timeZone>CST+4:00:00</timeZone>"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<ResponseStatus><statusCode>1</statusCode></ResponseStatus>"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_time("2026-08-02T10:15:30", "CST+4:00:00")
        .await
        .expect("device accepts the clock write");
}

#[tokio::test]
async fn set_time_surfaces_device_rejection() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/System/time"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    let err = conn
        .set_time("2026-08-02T10:15:30", "CST+4:00:00")
        .await
        .expect_err("403 must not be silently swallowed");
    assert!(err.to_string().contains("403"), "got: {err}");
}

// =============================================================================
// Attendance provisioning
// =============================================================================

/// Factory default is `disable`, which makes the reader report
/// `attendanceStatus: "undefined"` on every marking — leaving no way to tell an
/// arrival from a departure, which `calc::aggregation` needs both of.
#[tokio::test]
async fn set_attendance_mode_requests_status_on_every_event() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/Configuration/attendanceMode"))
        .and(body_string_contains(r#""mode":"manualAndAuto""#))
        .and(body_string_contains(r#""reqAttendanceStatus":true"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_attendance_mode("manualAndAuto")
        .await
        .expect("device accepts the mode");
}

#[tokio::test]
async fn set_attendance_key_binds_status_and_label() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/keyCfg/6/attendance"))
        .and(body_string_contains(r#""attendanceStatus":"overtimeOut""#))
        .and(body_string_contains(r#""enable":true"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_attendance_key(6, "overtimeOut", "Overtime Out")
        .await
        .expect("device accepts the key binding");
}

/// The reader's capabilities cap `WeekPlanCfg` at one segment per day, so all
/// seven days must be present in a single write — a partial plan silently leaves
/// the missing days unlabelled.
#[tokio::test]
async fn set_attendance_week_plan_covers_every_day_of_the_week() {
    let server = MockServer::start().await;
    let mut mock = Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/Attendance/weekPlan/1"))
        .and(body_string_contains(r#""endTime":"13:00:00""#));
    for day in [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ] {
        mock = mock.and(body_string_contains(format!(r#""week":"{day}""#)));
    }
    mock.respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_attendance_week_plan(1, "13:00:00")
        .await
        .expect("device accepts the week plan");
}

#[tokio::test]
async fn set_attendance_plan_template_binds_the_check_property() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/Attendance/planTemplate/1"))
        .and(body_string_contains(r#""property":"check""#))
        .and(body_string_contains(r#""weekPlanNo":1"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_attendance_plan_template(1, 1)
        .await
        .expect("device accepts the template");
}

/// Readers ship with picture upload disabled, so `attendance_events.photo_path`
/// stays null and an operator has no visual evidence for a disputed marking.
///
/// The display flags are asserted OFF on purpose: they control what the reader
/// shows to whoever is standing in front of it, and putting one person's name
/// or number on screen for the next in the queue is a privacy leak.
#[tokio::test]
async fn set_capture_upload_enables_uploads_without_exposing_identity_on_screen() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/AcsCfg"))
        .and(body_string_contains(r#""uploadCapPic":true"#))
        .and(body_string_contains(r#""uploadVerificationPic":true"#))
        .and(body_string_contains(r#""showPicture":false"#))
        .and(body_string_contains(r#""showEmployeeNo":false"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1}"#))
        .expect(1)
        .mount(&server)
        .await;

    let conn = DeviceConnection::new(&server.uri(), "admin", "pw", false).unwrap();
    conn.set_capture_upload(true)
        .await
        .expect("device accepts the capture config");
}
