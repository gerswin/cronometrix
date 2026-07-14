//! Integration coverage for the test-only E2E helper binaries.
//!
//! These tests intentionally drive the compiled processes through SQLite and
//! HTTP. The mock is a deterministic protocol fixture; it does not prove
//! digest authentication or compatibility with physical Hikvision hardware.

use std::io::Read;
use std::net::TcpListener;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use image::GenericImageView;
use libsql::Connection;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

const TEST_DEVICE_CREDS_KEY: &str = "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=";
const TEST_JWT_SECRET: &str = "test-secret-at-least-32-chars-padding!!";

fn isolated_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    command.env("PATH", std::env::var("PATH").unwrap_or_default());
    if let Some(profile_file) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile_file);
    }
    command
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed with {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn drain_pipe<R>(mut pipe: R) -> JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_pipe(reader: JoinHandle<std::io::Result<Vec<u8>>>, name: &str) -> Vec<u8> {
    reader
        .join()
        .unwrap_or_else(|_| panic!("{name} reader thread panicked"))
        .unwrap_or_else(|error| panic!("read child {name}: {error}"))
}

fn run_with_deadline(mut command: Command, timeout: Duration, context: &str) -> Output {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {context}: {error}"));
    let mut guard = ChildGuard::new(child);
    let stdout_reader = drain_pipe(guard.child_mut().stdout.take().expect("child stdout pipe"));
    let stderr_reader = drain_pipe(guard.child_mut().stderr.take().expect("child stderr pipe"));
    let deadline = Instant::now() + timeout;

    let status = loop {
        if guard
            .try_wait()
            .unwrap_or_else(|error| panic!("poll {context}: {error}"))
            .is_some()
        {
            break guard
                .reap()
                .unwrap_or_else(|error| panic!("reap {context}: {error}"));
        }
        if Instant::now() >= deadline {
            let status = guard
                .kill_and_wait()
                .unwrap_or_else(|error| panic!("kill timed-out {context}: {error}"));
            let stdout = join_pipe(stdout_reader, "stdout");
            let stderr = join_pipe(stderr_reader, "stderr");
            panic!(
                "{context} exceeded {timeout:?}; terminated with {status:?}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };

    Output {
        status,
        stdout: join_pipe(stdout_reader, "stdout"),
        stderr: join_pipe(stderr_reader, "stderr"),
    }
}

async fn scalar_count(conn: &Connection, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let mut rows = conn.query(&sql, ()).await.expect("count query");
    rows.next()
        .await
        .expect("count row read")
        .expect("count row")
        .get(0)
        .expect("count value")
}

async fn string_rows(conn: &Connection, sql: &str, columns: usize) -> Vec<Vec<String>> {
    let mut rows = conn.query(sql, ()).await.expect("string query");
    let mut result = Vec::new();
    while let Some(row) = rows.next().await.expect("row read") {
        result.push(
            (0..columns)
                .map(|column| {
                    row.get(i32::try_from(column).expect("column index"))
                        .expect("string value")
                })
                .collect(),
        );
    }
    result
}

fn seeded_command(db_path: &std::path::Path) -> Command {
    let mut command = isolated_command(env!("CARGO_BIN_EXE_seed_e2e"));
    command
        .env("CRONOMETRIX_E2E", "true")
        .env("CRONOMETRIX_DB_PATH", db_path)
        .env("JWT_SECRET", TEST_JWT_SECRET)
        .env("DEVICE_CREDS_KEY", TEST_DEVICE_CREDS_KEY)
        .env("SERVER_HOST", "127.0.0.1")
        .env("SERVER_PORT", "0")
        .env("TURSO_SYNC_INTERVAL", "300")
        .env("TZ", "America/Caracas")
        .env("COOKIE_SECURE", "false")
        .env("RUST_LOG", "warn");
    command
}

#[tokio::test]
async fn seed_e2e_refuses_unsafe_use_and_idempotently_seeds_stable_domain_data() {
    let refused = run_with_deadline(
        isolated_command(env!("CARGO_BIN_EXE_seed_e2e")),
        Duration::from_secs(30),
        "refused seed_e2e",
    );
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr)
        .contains("seed_e2e refuses to run without CRONOMETRIX_E2E=true"));

    let tempdir = tempfile::TempDir::new().expect("seed tempdir");
    let db_path = tempdir.path().join("seed.db");
    let first = run_with_deadline(
        seeded_command(&db_path),
        Duration::from_secs(30),
        "first seed_e2e",
    );
    assert_success(&first, "first seed_e2e run");
    assert!(String::from_utf8_lossy(&first.stdout).contains("seed_e2e: complete"));

    let db = libsql::Builder::new_local(db_path.to_str().expect("UTF-8 database path"))
        .build()
        .await
        .expect("open seeded database");
    let conn = db.connect().expect("connect seeded database");

    let expected_counts = [
        ("users", 7),
        ("departments", 3),
        ("employees", 6),
        ("devices", 2),
        ("face_enrollments", 1),
        ("enrollments", 1),
        ("enrollment_device_pushes", 2),
    ];
    for (table, count) in expected_counts {
        assert_eq!(scalar_count(&conn, table).await, count, "count for {table}");
    }

    assert_eq!(
        string_rows(&conn, "SELECT id, username, role FROM users ORDER BY id", 3).await,
        vec![
            vec!["demo-admin-id", "demo_admin", "admin"],
            vec!["demo-super-id", "demo_super", "supervisor"],
            vec!["demo-viewer-id", "demo_viewer", "viewer"],
            vec!["e2e-admin-id", "e2e_admin", "admin"],
            vec!["e2e-enrollment-admin-id", "e2e_enrollment_admin", "admin"],
            vec!["e2e-supervisor-id", "e2e_supervisor", "supervisor"],
            vec!["e2e-viewer-id", "e2e_viewer", "viewer"],
        ]
    );
    assert_eq!(
        string_rows(&conn, "SELECT id FROM departments ORDER BY id", 1).await,
        vec![vec!["dept-admin"], vec!["dept-prod"], vec!["dept-rrhh"]]
    );
    assert_eq!(
        string_rows(
            &conn,
            "SELECT id, department_id FROM employees ORDER BY id",
            2
        )
        .await,
        vec![
            vec!["emp-ana", "dept-prod"],
            vec!["emp-carmen", "dept-rrhh"],
            vec!["emp-jose", "dept-rrhh"],
            vec!["emp-luis", "dept-prod"],
            vec!["emp-maria", "dept-admin"],
            vec!["emp-pedro", "dept-admin"],
        ]
    );
    assert_eq!(
        string_rows(
            &conn,
            "SELECT id, direction, CAST(port AS TEXT) FROM devices ORDER BY id",
            3
        )
        .await,
        vec![
            vec!["dev-entry", "entry", "4400"],
            vec!["dev-exit", "exit", "4401"],
        ]
    );
    assert_eq!(
        string_rows(
            &conn,
            "SELECT id, employee_id, captured_via, source_device_id, created_by \
             FROM face_enrollments",
            5
        )
        .await,
        vec![vec![
            "e2e-seed-face-enrollment",
            "emp-carmen",
            "device",
            "dev-entry",
            "e2e-enrollment-admin-id"
        ]]
    );
    assert_eq!(
        string_rows(
            &conn,
            "SELECT id, employee_id, face_enrollment_id, status, started_by FROM enrollments",
            5
        )
        .await,
        vec![vec![
            "e2e-seed-enrollment",
            "emp-carmen",
            "e2e-seed-face-enrollment",
            "in_progress",
            "e2e-enrollment-admin-id"
        ]]
    );
    assert_eq!(
        string_rows(
            &conn,
            "SELECT id, enrollment_id, device_id, status \
             FROM enrollment_device_pushes ORDER BY id",
            4
        )
        .await,
        vec![
            vec![
                "e2e-seed-push-entry",
                "e2e-seed-enrollment",
                "dev-entry",
                "pending"
            ],
            vec![
                "e2e-seed-push-exit",
                "e2e-seed-enrollment",
                "dev-exit",
                "pending"
            ],
        ]
    );

    let password_hashes =
        string_rows(&conn, "SELECT password_hash FROM users ORDER BY id", 1).await;
    assert_eq!(password_hashes.len(), 7);
    assert!(password_hashes
        .iter()
        .all(|row| !row[0].is_empty() && row[0].starts_with("$argon2")));
    let ciphertexts = string_rows(
        &conn,
        "SELECT encrypted_password FROM devices ORDER BY id",
        1,
    )
    .await;
    assert_eq!(ciphertexts.len(), 2);
    assert!(ciphertexts.iter().all(|row| !row[0].is_empty()));

    drop(conn);
    drop(db);

    let second = run_with_deadline(
        seeded_command(&db_path),
        Duration::from_secs(30),
        "second seed_e2e",
    );
    assert_success(&second, "second seed_e2e run");
    assert!(String::from_utf8_lossy(&second.stdout).contains("seed_e2e: complete"));

    let db = libsql::Builder::new_local(db_path.to_str().expect("UTF-8 database path"))
        .build()
        .await
        .expect("reopen seeded database");
    let conn = db.connect().expect("reconnect seeded database");
    for (table, count) in expected_counts {
        assert_eq!(
            scalar_count(&conn, table).await,
            count,
            "idempotent count for {table}"
        );
    }
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child still owned")
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child_mut().try_wait()
    }

    fn reap(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child_mut().wait()?;
        self.child.take();
        Ok(status)
    }

    fn kill_and_wait(&mut self) -> std::io::Result<ExitStatus> {
        if let Err(kill_error) = self.child_mut().kill() {
            if self.child_mut().try_wait()?.is_none() {
                return Err(kill_error);
            }
        }
        self.reap()
    }

    fn sigint_and_wait(mut self) -> std::process::ExitStatus {
        let child_id = self.child_mut().id().to_string();
        let signal = Command::new("/bin/kill")
            .args(["-INT", &child_id])
            .status()
            .expect("send SIGINT");
        assert!(signal.success(), "SIGINT command failed: {signal:?}");

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if self.try_wait().expect("poll mock child").is_some() {
                return self.reap().expect("reap mock child after SIGINT");
            }
            if Instant::now() >= deadline {
                self.kill_and_wait()
                    .expect("kill mock after SIGINT timeout");
                panic!("mock did not exit within 10s after SIGINT");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn pick_two_ports() -> (u16, u16) {
    let first = TcpListener::bind("127.0.0.1:0").expect("bind first port");
    let second = TcpListener::bind("127.0.0.1:0").expect("bind second port");
    let ports = (
        first.local_addr().expect("first local addr").port(),
        second.local_addr().expect("second local addr").port(),
    );
    assert_ne!(ports.0, ports.1);
    ports
}

enum Readiness {
    Ready,
    Exited(ExitStatus),
    TimedOut,
}

async fn wait_until_ready(
    client: &Client,
    guard: &mut ChildGuard,
    admin_url: &str,
) -> Result<Readiness, String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = guard.try_wait().map_err(|error| error.to_string())? {
            return Ok(Readiness::Exited(status));
        }
        if let Ok(response) = client.get(format!("{admin_url}/admin/health")).send().await {
            let is_ready = response.status().is_success()
                && response.text().await.is_ok_and(|body| body == "ok");
            if is_ready {
                if let Some(status) = guard.try_wait().map_err(|error| error.to_string())? {
                    return Ok(Readiness::Exited(status));
                }
                return Ok(Readiness::Ready);
            }
        }
        if Instant::now() >= deadline {
            return Ok(Readiness::TimedOut);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn spawn_ready_mock(client: &Client) -> Result<(ChildGuard, String, String), String> {
    const MAX_ATTEMPTS: usize = 3;
    let mut early_exits = Vec::new();

    for attempt in 1..=MAX_ATTEMPTS {
        let (public_port, admin_port) = pick_two_ports();
        let public_url = format!("http://127.0.0.1:{public_port}");
        let admin_url = format!("http://127.0.0.1:{admin_port}");
        let child = isolated_command(env!("CARGO_BIN_EXE_mock_hikvision"))
            .env("MOCK_HIKVISION_PORT", public_port.to_string())
            .env("MOCK_HIKVISION_ADMIN_PORT", admin_port.to_string())
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("spawn mock_hikvision attempt {attempt}: {error}"))?;
        let mut guard = ChildGuard::new(child);

        match wait_until_ready(client, &mut guard, &admin_url).await? {
            Readiness::Ready => return Ok((guard, public_url, admin_url)),
            Readiness::Exited(status) => {
                early_exits.push(format!("attempt {attempt}: {status:?}"));
                drop(guard);
            }
            Readiness::TimedOut => {
                return Err(format!(
                    "mock readiness timed out after 20s on attempt {attempt}"
                ));
            }
        }
    }

    Err(format!(
        "mock exited before readiness on all {MAX_ATTEMPTS} fresh port pairs: {}",
        early_exits.join(", ")
    ))
}

async fn response_json(response: reqwest::Response) -> Value {
    response
        .error_for_status()
        .expect("successful response")
        .json()
        .await
        .expect("JSON response")
}

fn command_triples(commands: &[Value]) -> Vec<(&str, &str, &str)> {
    commands
        .iter()
        .map(|entry| {
            (
                entry["method"].as_str().expect("logged method"),
                entry["path"].as_str().expect("logged path"),
                entry["body"].as_str().expect("logged body"),
            )
        })
        .collect()
}

#[tokio::test]
async fn mock_hikvision_serves_and_records_real_public_and_admin_interfaces() {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP client");

    let (guard, public_url, admin_url) = spawn_ready_mock(&client).await.expect("mock readiness");

    let health = client
        .get(format!("{admin_url}/admin/health"))
        .send()
        .await
        .expect("admin health");
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(health.text().await.expect("health body"), "ok");

    let status = response_json(
        client
            .get(format!("{public_url}/ISAPI/System/status"))
            .send()
            .await
            .expect("device status"),
    )
    .await;
    assert_eq!(status["status"], "OK");
    assert_eq!(status["deviceModel"], "DS-K1T341");

    let event_xml = "<EventNotificationAlert><eventType>AccessControllerEvent</eventType></EventNotificationAlert>";
    let queued = response_json(
        client
            .post(format!("{admin_url}/admin/push-event"))
            .json(&json!({ "xml": event_xml }))
            .send()
            .await
            .expect("push event"),
    )
    .await;
    assert_eq!(queued["queued"], true);
    let stream = client
        .get(format!("{public_url}/ISAPI/Event/notification/alertStream"))
        .send()
        .await
        .expect("alert stream");
    assert_eq!(stream.status(), StatusCode::OK);
    assert_eq!(
        stream.headers()[reqwest::header::CONTENT_TYPE],
        "multipart/mixed; boundary=MIME_boundary"
    );
    let stream_body = stream.text().await.expect("stream body");
    assert!(stream_body.contains("--MIME_boundary"));
    assert!(stream_body.contains(event_xml));
    assert!(stream_body.ends_with("--MIME_boundary--\r\n"));
    let drained_response = client
        .get(format!("{public_url}/ISAPI/Event/notification/alertStream"))
        .send()
        .await
        .expect("drained alert stream");
    assert_eq!(drained_response.status(), StatusCode::OK);
    let drained = drained_response.text().await.expect("drained stream body");
    assert_eq!(drained, "--MIME_boundary--\r\n");

    response_json(
        client
            .post(format!("{admin_url}/admin/push-event"))
            .json(&json!({ "xml": "<must-be-cleared/>" }))
            .send()
            .await
            .expect("push clearable event"),
    )
    .await;
    let cleared = response_json(
        client
            .post(format!("{admin_url}/admin/clear-queue"))
            .send()
            .await
            .expect("clear event queue"),
    )
    .await;
    assert_eq!(cleared["cleared"], true);
    let cleared_stream_response = client
        .get(format!("{public_url}/ISAPI/Event/notification/alertStream"))
        .send()
        .await
        .expect("cleared alert stream");
    assert_eq!(cleared_stream_response.status(), StatusCode::OK);
    let cleared_stream = cleared_stream_response
        .text()
        .await
        .expect("cleared stream body");
    assert!(!cleared_stream.contains("must-be-cleared"));

    let capture = client
        .get(format!(
            "{public_url}/ISAPI/AccessControl/CapturedFacePicture"
        ))
        .send()
        .await
        .expect("capture JPEG");
    assert_eq!(capture.status(), StatusCode::OK);
    assert_eq!(
        capture.headers()[reqwest::header::CONTENT_TYPE],
        "image/jpeg"
    );
    let jpeg = capture.bytes().await.expect("capture bytes");
    assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
    assert_eq!(&jpeg[jpeg.len() - 2..], &[0xff, 0xd9]);
    assert_eq!(
        image::load_from_memory(&jpeg)
            .expect("decode capture")
            .dimensions(),
        (640, 480)
    );

    let xml_commands = [
        (
            "PUT",
            "/ISAPI/AccessControl/RemoteControl/door/1",
            "<door>open</door>",
        ),
        ("PUT", "/ISAPI/System/reboot", "<reboot>true</reboot>"),
        ("POST", "/ISAPI/AccessControl/CaptureFaceData", "<capture/>"),
        (
            "PUT",
            "/ISAPI/AccessControl/UserInfoDetail/Delete",
            "<delete>EMP999</delete>",
        ),
    ];
    for (method, path, body) in xml_commands {
        let response = client
            .request(
                method.parse().expect("HTTP method"),
                format!("{public_url}{path}"),
            )
            .body(body)
            .send()
            .await
            .expect("XML command");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[reqwest::header::CONTENT_TYPE],
            "application/xml"
        );
        let body = response.text().await.expect("XML response");
        assert!(body.contains("<statusCode>1</statusCode>"));
        assert!(body.contains("<statusString>OK</statusString>"));
    }

    let enrollment_commands = [
        (
            "/ISAPI/AccessControl/UserInfo/Record",
            r#"{"employeeNo":"EMP001"}"#,
        ),
        (
            "/ISAPI/Intelligent/FDLib/FaceDataRecord",
            r#"{"employeeNo":"EMP001","faceURL":"data"}"#,
        ),
    ];
    for (path, body) in enrollment_commands {
        let response = client
            .post(format!("{public_url}{path}"))
            .body(body)
            .send()
            .await
            .expect("JSON enrollment command");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[reqwest::header::CONTENT_TYPE],
            "application/json"
        );
        assert_eq!(response_json(response).await["statusCode"], 1);
    }

    let log = response_json(
        client
            .get(format!("{admin_url}/admin/recv-log"))
            .send()
            .await
            .expect("receive log"),
    )
    .await;
    let commands = log["commands"].as_array().expect("commands array");
    assert_eq!(
        command_triples(commands),
        vec![
            (
                "PUT",
                "/ISAPI/AccessControl/RemoteControl/door/1",
                "<door>open</door>"
            ),
            ("PUT", "/ISAPI/System/reboot", "<reboot>true</reboot>"),
            ("POST", "/ISAPI/AccessControl/CaptureFaceData", "<capture/>"),
            (
                "PUT",
                "/ISAPI/AccessControl/UserInfoDetail/Delete",
                "<delete>EMP999</delete>"
            ),
            (
                "POST",
                "/ISAPI/AccessControl/UserInfo/Record",
                r#"{"employeeNo":"EMP001"}"#
            ),
            (
                "POST",
                "/ISAPI/Intelligent/FDLib/FaceDataRecord",
                r#"{"employeeNo":"EMP001","faceURL":"data"}"#
            ),
        ]
    );
    assert!(commands
        .iter()
        .all(|entry| entry["timestamp_ms"].as_u64().is_some()));

    let oversized = vec![b'x'; 3 * 1024 * 1024 + 1];
    let oversized_response = client
        .put(format!("{public_url}/ISAPI/System/reboot"))
        .body(oversized)
        .send()
        .await
        .expect("oversized command");
    assert_eq!(oversized_response.status(), StatusCode::OK);
    let log = response_json(
        client
            .get(format!("{admin_url}/admin/recv-log"))
            .send()
            .await
            .expect("receive oversized log"),
    )
    .await;
    let oversized_entry = log["commands"]
        .as_array()
        .expect("commands array")
        .last()
        .expect("oversized log entry");
    assert_eq!(oversized_entry["method"], "PUT");
    assert_eq!(oversized_entry["path"], "/ISAPI/System/reboot");
    assert_eq!(oversized_entry["body"], "");

    let cleared_log = response_json(
        client
            .post(format!("{admin_url}/admin/clear-recv-log"))
            .send()
            .await
            .expect("clear receive log"),
    )
    .await;
    assert_eq!(cleared_log["cleared"], true);
    let empty_log = response_json(
        client
            .get(format!("{admin_url}/admin/recv-log"))
            .send()
            .await
            .expect("empty receive log"),
    )
    .await;
    assert_eq!(empty_log["commands"], json!([]));

    let reset = response_json(
        client
            .post(format!("{admin_url}/admin/reset-enrollment-script"))
            .send()
            .await
            .expect("reset enrollment script"),
    )
    .await;
    assert_eq!(reset["face_data_failures_remaining"], 1);

    let admin_user = client
        .post(format!("{admin_url}/ISAPI/AccessControl/UserInfo/Record"))
        .body(r#"{"employeeNo":"EMP002"}"#)
        .send()
        .await
        .expect("admin user record");
    assert_eq!(admin_user.status(), StatusCode::OK);
    assert_eq!(response_json(admin_user).await["statusCode"], 1);

    let face_path = format!("{admin_url}/ISAPI/Intelligent/FDLib/FaceDataRecord");
    let first_face = client
        .post(&face_path)
        .body(r#"{"employeeNo":"EMP002"}"#)
        .send()
        .await
        .expect("scripted face failure");
    assert_eq!(first_face.status(), StatusCode::SERVICE_UNAVAILABLE);
    let first_face_json: Value = first_face.json().await.expect("failure JSON");
    assert_eq!(first_face_json["statusCode"], 5);
    let second_face = client
        .post(&face_path)
        .body(r#"{"employeeNo":"EMP002"}"#)
        .send()
        .await
        .expect("scripted face success");
    assert_eq!(second_face.status(), StatusCode::OK);
    assert_eq!(response_json(second_face).await["statusCode"], 1);

    let scripted_log = response_json(
        client
            .get(format!("{admin_url}/admin/recv-log"))
            .send()
            .await
            .expect("scripted receive log"),
    )
    .await;
    let scripted_commands = scripted_log["commands"]
        .as_array()
        .expect("scripted commands");
    assert_eq!(
        command_triples(scripted_commands),
        vec![
            (
                "POST",
                "/ISAPI/AccessControl/UserInfo/Record",
                r#"{"employeeNo":"EMP002"}"#
            ),
            (
                "POST",
                "/ISAPI/Intelligent/FDLib/FaceDataRecord",
                r#"{"employeeNo":"EMP002"}"#
            ),
            (
                "POST",
                "/ISAPI/Intelligent/FDLib/FaceDataRecord",
                r#"{"employeeNo":"EMP002"}"#
            ),
        ]
    );

    let status = guard.sigint_and_wait();
    assert!(status.success(), "mock SIGINT exit failed: {status:?}");
}
