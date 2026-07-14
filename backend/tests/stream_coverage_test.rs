mod common;

use std::net::SocketAddr;
use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::isapi::stream::{connect_and_stream, DeviceConfig};
use libsql::params;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const BOUNDARY: &str = "coverage-boundary";

fn config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.into(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
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

fn device(id: &str, addr: SocketAddr, direction_default: &str) -> DeviceConfig {
    DeviceConfig {
        id: id.into(),
        base_url: format!("http://{addr}"),
        username: "admin".into(),
        password: "do-not-log".into(),
        direction_default: direction_default.into(),
        allow_insecure_tls: false,
    }
}

async fn spawn_response(content_type: Option<&str>, body: Vec<u8>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let content_type = content_type.map(str::to_string);
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request).await;
        let header = content_type
            .map(|value| format!("Content-Type: {value}\r\n"))
            .unwrap_or_default();
        let response = format!(
            "HTTP/1.1 200 OK\r\n{header}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    addr
}

fn multipart(parts: &[(Option<&str>, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (content_type, bytes) in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        if let Some(content_type) = content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        body.extend_from_slice(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn seed_device(conn: &libsql::Connection, id: &str) {
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?2, '127.0.0.1', 45678, 'http', 'admin', 'ciphertext', \
         'exit', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), format!("device-{id}")],
    )
    .await
    .unwrap();
}

#[test]
fn device_config_debug_redacts_password_and_exposes_connection_fields() {
    let cfg = DeviceConfig {
        id: "device-1".into(),
        base_url: "https://device.local".into(),
        username: "admin".into(),
        password: "super-secret".into(),
        direction_default: "entry".into(),
        allow_insecure_tls: true,
    };

    let debug = format!("{cfg:?}");
    assert!(debug.contains("device-1"));
    assert!(debug.contains("https://device.local"));
    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("super-secret"));
}

#[tokio::test]
async fn stream_rejects_missing_or_invalid_multipart_content_type() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());

    for content_type in [
        None,
        Some("application/json"),
        Some("multipart/mixed"),
        Some("multipart/mixed; boundary=\"\""),
    ] {
        let addr = spawn_response(content_type, Vec::new()).await;
        let error = connect_and_stream(&device("unused", addr, "entry"), &state)
            .await
            .expect_err("invalid stream metadata must fail before persistence");
        let message = error.to_string();
        assert!(
            message.contains("Content-Type")
                || message.contains("multipart")
                || message.contains("boundary"),
            "unexpected error: {message}"
        );
    }
}

#[tokio::test]
async fn stream_handles_magic_byte_parts_unknown_parts_and_skippable_xml() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "stream-branches").await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());

    let without_access_event = br#"<EventNotificationAlert><dateTime>bad</dateTime><eventType>AccessControllerEvent</eventType></EventNotificationAlert>"#;
    let malformed = br#"<EventNotificationAlert><broken>"#;
    let attendance = br#"<EventNotificationAlert><dateTime>not-a-date</dateTime><eventType>AccessControllerEvent</eventType><AccessControllerEvent><employeeNoString></employeeNoString><attendanceStatus></attendanceStatus><faceID></faceID></AccessControllerEvent></EventNotificationAlert>"#;
    let jpeg = common::MINI_JPEG;
    let body = multipart(&[
        (None, jpeg),
        (Some("application/octet-stream"), b"ignored"),
        (Some("application/xml"), malformed),
        (None, without_access_event),
        (None, attendance),
        (None, jpeg),
    ]);
    let addr = spawn_response(
        Some(&format!(
            "multipart/mixed; boundary=\"{BOUNDARY}\"; charset=utf-8"
        )),
        body,
    )
    .await;

    connect_and_stream(&device("stream-branches", addr, "exit"), &state)
        .await
        .unwrap();

    let conn = state.db.connect().unwrap();
    let row = conn
        .query(
            "SELECT employee_id, direction, face_id, employee_no_string, is_unknown, captured_at, photo_path \
             FROM attendance_events WHERE device_id = 'stream-branches'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .expect("the valid access event is persisted");
    assert_eq!(row.get::<Option<String>>(0).unwrap(), None);
    assert_eq!(row.get::<String>(1).unwrap(), "exit");
    assert_eq!(row.get::<Option<String>>(2).unwrap(), None);
    assert_eq!(row.get::<Option<String>>(3).unwrap(), None);
    assert_eq!(row.get::<i64>(4).unwrap(), 1);
    assert!(row.get::<i64>(5).unwrap() > 0);
    assert!(row.get::<Option<String>>(6).unwrap().is_some());
}

#[tokio::test]
async fn stream_rejects_invalid_utf8_xml_part() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "invalid-utf8").await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());
    let invalid_utf8 = [0xff, 0xfe, 0xfd];
    let body = multipart(&[(Some("application/xml"), &invalid_utf8)]);
    let addr = spawn_response(Some(&format!("multipart/mixed; boundary={BOUNDARY}")), body).await;

    let error = connect_and_stream(&device("invalid-utf8", addr, "entry"), &state)
        .await
        .expect_err("invalid UTF-8 must not enter the XML parser");
    assert!(error.to_string().contains("not valid UTF-8"));
}

#[tokio::test]
async fn stream_surfaces_truncated_multipart_parser_error() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_device(&conn, "truncated-multipart").await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());
    let body = format!(
        "--{BOUNDARY}\r\nContent-Type: application/xml\r\nContent-Length: 200\r\n\r\n<EventNotificationAlert>"
    )
    .into_bytes();
    let addr = spawn_response(Some(&format!("multipart/mixed; boundary={BOUNDARY}")), body).await;

    let error = connect_and_stream(&device("truncated-multipart", addr, "entry"), &state)
        .await
        .expect_err("a truncated field must not be treated as a clean stream close");
    let message = error.to_string();
    assert!(message.contains("incomplete data"), "{message}");
}
