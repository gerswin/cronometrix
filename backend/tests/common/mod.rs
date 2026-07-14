// Shared fixtures are intentionally broader than any single integration test;
// each test crate selects only the helpers it needs.
#![allow(dead_code)]

use cronometrix_api::db::run_migrations;

pub mod mock_hikvision;

/// Deterministic 32-byte key (base64) used by every test that spins up a Config
/// with device-credential crypto wired in. DO NOT use in production.
pub const TEST_DEVICE_CREDS_KEY_B64: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

pub fn test_device_creds_key() -> [u8; 32] {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(TEST_DEVICE_CREDS_KEY_B64)
        .expect("test key is valid base64")
        .as_slice()
        .try_into()
        .expect("test key decodes to 32 bytes")
}

/// Create a temporary file-based libSQL database with all migrations applied.
/// Each test gets its own isolated database instance via a unique temp path.
///
/// NOTE: We use a temp file (not :memory:) because each call to db.connect() on
/// an :memory: database opens a NEW isolated SQLite connection with no shared state.
/// A temp file ensures all connections see the same schema.
pub async fn test_db() -> libsql::Database {
    // Generate a unique temp path per call so tests are isolated from each other
    let tmp_path = format!("/tmp/cronometrix_test_{}.db", uuid::Uuid::new_v4());

    let db = libsql::Builder::new_local(&tmp_path)
        .build()
        .await
        .expect("Failed to create test database");

    let conn = db.connect().expect("Failed to connect to test database");

    // Match the production connection PRAGMAs (src/db/mod.rs): WAL is persisted at
    // the file level so every later `db.connect()` (handlers, the db_write worker)
    // inherits it, letting a writer proceed while another connection holds a read
    // lock. Without this, code paths that read on one connection and write through
    // the single-writer queue collide with "database is locked".
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; \
         PRAGMA journal_mode = WAL; \
         PRAGMA synchronous = NORMAL; \
         PRAGMA busy_timeout = 5000;",
    )
    .await
    .expect("Failed to apply test database PRAGMAs");

    // Run migrations via the same production code path
    run_migrations(&conn)
        .await
        .expect("Failed to run migrations in test database");

    db
}

/// Generate a test JWT access token for a given role.
/// Uses a fixed test secret — NEVER use in production.
pub const TEST_JWT_SECRET: &str = "test-secret-key-at-least-32-characters-long!!";

pub fn test_access_token(user_id: &str, role: &str) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    let claims = json!({
        "sub": user_id,
        "role": role,
        "exp": chrono::Utc::now().timestamp() + 3600,  // 1 hour for tests
        "iat": chrono::Utc::now().timestamp(),
        "jti": uuid::Uuid::new_v4().to_string(),
        "token_type": "access"
    });

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .expect("Failed to create test token")
}

/// Create a test admin user directly in the database.
/// Returns the user ID.
pub async fn create_test_admin(db: &libsql::Database) -> String {
    let conn = db.connect().expect("Failed to connect");
    let user_id = uuid::Uuid::new_v4().to_string();

    // Use a pre-hashed password for speed in tests
    // The actual hash is for "testpassword123"
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) VALUES (?1, 'testadmin', 'Test Admin', ?2, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), "$argon2id$v=19$m=19456,t=2,p=1$placeholder_test_hash"],
    )
    .await
    .expect("Failed to create test admin");

    user_id
}

/// Create a test supervisor user. Returns the user ID. Used by the command-dispatch
/// forbidden tests and any future Supervisor-scoped write tests.
pub async fn create_test_supervisor(db: &libsql::Database) -> String {
    let conn = db.connect().expect("Failed to connect");
    let user_id = uuid::Uuid::new_v4().to_string();
    let username = format!("testsupervisor-{}", &user_id[..8]);

    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) VALUES (?1, ?2, 'Test Supervisor', ?3, 'supervisor', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), username, "$argon2id$v=19$m=19456,t=2,p=1$placeholder_test_hash"],
    )
    .await
    .expect("Failed to create test supervisor");

    user_id
}

/// Seed a department row with Phase 3 shift fields. Returns the generated department id.
/// Mirrors the parameter order used by Plan 03-01 calc fixtures: shift_type / overnight
/// flag / ordinary daily minutes / shift start / shift end.
#[allow(dead_code)]
pub async fn create_test_department_with_shift(
    db: &libsql::Database,
    name: &str,
    shift_type: &str, // "day" | "night" | "mixed"
    is_overnight: bool,
    ordinary_daily_minutes: i64,
    shift_start: &str, // "HH:MM"
    shift_end: &str,   // "HH:MM"
) -> String {
    let conn = db.connect().expect("connect");
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, ?3, ?4, 'fixed', 60, ?5, ?6, ?7, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![
            id.clone(),
            name.to_string(),
            shift_start.to_string(),
            shift_end.to_string(),
            shift_type.to_string(),
            is_overnight as i64,
            ordinary_daily_minutes,
        ],
    )
    .await
    .expect("seed department with shift");
    id
}

/// Seed a leave row directly into the DB, bypassing the HTTP layer.
/// Returns the generated leave id. `from_date` / `to_date` are 'YYYY-MM-DD'.
/// `created_by` must be a valid users.id (FK).
#[allow(dead_code)]
pub async fn create_test_leave(
    db: &libsql::Database,
    employee_id: &str,
    leave_type: &str,
    from_date: &str,
    to_date: &str,
    created_by: &str,
) -> String {
    let conn = db.connect().expect("connect");
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
         justification, evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'test justification', NULL, ?6, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![
            id.clone(),
            employee_id.to_string(),
            from_date.to_string(),
            to_date.to_string(),
            leave_type.to_string(),
            created_by.to_string(),
        ],
    )
    .await
    .expect("seed leave");
    id
}

/// Create a test viewer user. Returns the user ID.
pub async fn create_test_viewer(db: &libsql::Database) -> String {
    let conn = db.connect().expect("Failed to connect");
    let user_id = uuid::Uuid::new_v4().to_string();
    let username = format!("testviewer-{}", &user_id[..8]);

    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) VALUES (?1, ?2, 'Test Viewer', ?3, 'viewer', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), username, "$argon2id$v=19$m=19456,t=2,p=1$placeholder_test_hash"],
    )
    .await
    .expect("Failed to create test viewer");

    user_id
}

// =============================================================================
// Wave 0 multipart/alertStream fixture helpers (Plan 02-02)
// =============================================================================

/// Build a fully-formed multipart/mixed body with the given XML and JPEG.
/// Boundary is always "MIME_boundary" for test reproducibility. This matches
/// 02-RESEARCH § alertStream Multipart Format.
pub fn build_multipart_fixture(xml: &str, jpeg: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"--MIME_boundary\r\n");
    out.extend_from_slice(b"Content-Type: application/xml\r\n");
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", xml.len()).as_bytes());
    out.extend_from_slice(xml.as_bytes());
    out.extend_from_slice(b"\r\n");
    if let Some(img) = jpeg {
        out.extend_from_slice(b"--MIME_boundary\r\n");
        out.extend_from_slice(b"Content-Type: image/jpeg\r\n");
        out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", img.len()).as_bytes());
        out.extend_from_slice(img);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--MIME_boundary--\r\n");
    out
}

/// Minimal synthetic JPEG magic-byte sequence. Starts with SOI (FFD8) followed by
/// a JFIF APP0 header stub and EOI. NOT a renderable 1×1 image — just enough to
/// pin the parser contract (tests only assert magic bytes).
pub const MINI_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];

/// Canonical DS-K1T341 event XML (one face check-in) used by the k1t341 fixture.
pub fn k1t341_event_xml() -> String {
    r#"<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
  <ipAddress>192.168.1.10</ipAddress>
  <portNo>80</portNo>
  <protocol>HTTP</protocol>
  <macAddress>aa:bb:cc:dd:ee:ff</macAddress>
  <channelID>1</channelID>
  <dateTime>2026-04-19T12:34:56+00:00</dateTime>
  <activePostCount>1</activePostCount>
  <eventType>AccessControllerEvent</eventType>
  <eventState>active</eventState>
  <eventDescription>Access Controller Event</eventDescription>
  <AccessControllerEvent>
    <deviceName>DS-K1T341</deviceName>
    <majorEventType>5</majorEventType>
    <subEventType>75</subEventType>
    <employeeNoString>EMP001</employeeNoString>
    <name>John Doe</name>
    <cardNo>0</cardNo>
    <cardType>1</cardType>
    <currentVerifyMode>face</currentVerifyMode>
    <attendanceStatus>checkIn</attendanceStatus>
    <faceID>42</faceID>
    <pictureURL>/ISAPI/Intelligent/FDLib/pictureUpload?id=42</pictureURL>
  </AccessControllerEvent>
</EventNotificationAlert>"#
        .to_string()
}

/// Canonical heartbeat XML (A3 — videoloss/inactive) for the heartbeat fixture.
pub fn heartbeat_event_xml() -> String {
    r#"<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
  <ipAddress>192.168.1.10</ipAddress>
  <portNo>80</portNo>
  <protocol>HTTP</protocol>
  <macAddress>aa:bb:cc:dd:ee:ff</macAddress>
  <channelID>1</channelID>
  <dateTime>2026-04-19T12:34:56+00:00</dateTime>
  <activePostCount>1</activePostCount>
  <eventType>videoloss</eventType>
  <eventState>inactive</eventState>
  <eventDescription>videoloss</eventDescription>
</EventNotificationAlert>"#
        .to_string()
}

/// Unknown-face XML (faceID not in device_face_mappings) for the unknown_face fixture.
pub fn unknown_face_event_xml() -> String {
    r#"<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
  <ipAddress>192.168.1.10</ipAddress>
  <portNo>80</portNo>
  <protocol>HTTP</protocol>
  <macAddress>aa:bb:cc:dd:ee:ff</macAddress>
  <channelID>1</channelID>
  <dateTime>2026-04-19T12:34:56+00:00</dateTime>
  <activePostCount>1</activePostCount>
  <eventType>AccessControllerEvent</eventType>
  <eventState>active</eventState>
  <eventDescription>Access Controller Event</eventDescription>
  <AccessControllerEvent>
    <deviceName>DS-K1T341</deviceName>
    <majorEventType>5</majorEventType>
    <subEventType>75</subEventType>
    <employeeNoString></employeeNoString>
    <name></name>
    <cardNo>0</cardNo>
    <cardType>1</cardType>
    <currentVerifyMode>face</currentVerifyMode>
    <attendanceStatus>checkIn</attendanceStatus>
    <faceID>9999</faceID>
    <pictureURL>/ISAPI/Intelligent/FDLib/pictureUpload?id=9999</pictureURL>
  </AccessControllerEvent>
</EventNotificationAlert>"#
        .to_string()
}

// =============================================================================
// Phase 7: Facial Enrollment fixture helpers
// =============================================================================

/// Generate a synthetic 100×100 JPEG (~50KB) for enrollment tests.
/// Uses `image` crate to produce a real JPEG that the image pipeline can decode.
pub fn sample_face_jpeg_50kb() -> Vec<u8> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageBuffer, Rgb};

    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(100, 100, |x, y| Rgb([x as u8, y as u8, 128u8]));
    let dynamic = image::DynamicImage::ImageRgb8(img);

    let mut buf = std::io::Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut buf, 90)
        .encode_image(&dynamic)
        .expect("encode 100x100 JPEG");
    buf.into_inner()
}

/// Generate a synthetic 2000×2000 JPEG (>2 MB) for downscale tests.
/// Produces a real JPEG that the image pipeline must compress to ≤200KB.
pub fn sample_face_jpeg_4mb() -> Vec<u8> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageBuffer, Rgb};

    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(2000, 2000, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, 128u8])
    });
    let dynamic = image::DynamicImage::ImageRgb8(img);

    let mut buf = std::io::Cursor::new(Vec::new());
    // Quality 95 on 2000×2000 produces 3–5 MB
    JpegEncoder::new_with_quality(&mut buf, 95)
        .encode_image(&dynamic)
        .expect("encode 2000x2000 JPEG");
    let bytes = buf.into_inner();
    // Sanity: must be >2MB so downscale tests can rely on it
    assert!(
        bytes.len() > 2 * 1024 * 1024,
        "sample_face_jpeg_4mb produced {} bytes, expected >2MB",
        bytes.len()
    );
    bytes
}

/// Spawn a wiremock `MockServer` pre-configured with all Hikvision face
/// endpoints used by Phase 7:
///   - POST /ISAPI/AccessControl/UserInfo/Record     → 200 {"statusCode":1}
///   - POST /ISAPI/Intelligent/FDLib/FaceDataRecord  → 200 {"statusCode":1}
///   - PUT  /ISAPI/AccessControl/UserInfoDetail/Delete → 200 {"statusCode":1}
///   - POST /ISAPI/AccessControl/CaptureFaceData     → 200 {"statusCode":1}
///   - GET  /ISAPI/AccessControl/CapturedFacePicture → 200 <50KB JPEG bytes>
///
/// All responses are 200 OK so integration tests exercise the happy path.
/// Tests that need failure responses should spawn their own MockServer.
pub async fn mock_hikvision_server() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // UserInfo/Record — create person
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/UserInfo/Record"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;

    // FaceDataRecord — upload face image (multipart)
    Mock::given(method("POST"))
        .and(path("/ISAPI/Intelligent/FDLib/FaceDataRecord"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;

    // UserInfoDetail/Delete — delete person by employeeNo
    Mock::given(method("PUT"))
        .and(path("/ISAPI/AccessControl/UserInfoDetail/Delete"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;

    // CaptureFaceData — enter enrollment mode
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"statusCode":1,"statusString":"OK"}"#),
        )
        .mount(&server)
        .await;

    // CapturedFacePicture — return a sample JPEG
    Mock::given(method("GET"))
        .and(path("/ISAPI/AccessControl/CapturedFacePicture"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "image/jpeg")
                .set_body_bytes(sample_face_jpeg_50kb()),
        )
        .mount(&server)
        .await;

    server
}

/// Deterministically (re)generate the three canned multipart byte samples if
/// any are missing from `tests/fixtures/`. Safe to call from any test — it is
/// idempotent and only writes when a fixture is absent. The files produced are
/// the same bytes regardless of machine, so they are safe to commit and CI-
/// reproducible.
pub fn ensure_fixtures_present() -> anyhow::Result<()> {
    use std::fs;
    use std::path::Path;

    let root = Path::new("tests/fixtures");
    if !root.exists() {
        fs::create_dir_all(root)?;
    }

    let k1t341 = root.join("alertstream_k1t341.bin");
    if !k1t341.exists() {
        let body = build_multipart_fixture(&k1t341_event_xml(), Some(MINI_JPEG));
        fs::write(&k1t341, body)?;
    }

    let heartbeat = root.join("alertstream_heartbeat.bin");
    if !heartbeat.exists() {
        let body = build_multipart_fixture(&heartbeat_event_xml(), None);
        fs::write(&heartbeat, body)?;
    }

    let unknown = root.join("alertstream_unknown_face.bin");
    if !unknown.exists() {
        let body = build_multipart_fixture(&unknown_face_event_xml(), Some(MINI_JPEG));
        fs::write(&unknown, body)?;
    }

    Ok(())
}

// =============================================================================
// Shared AppState fixture (260428-3qg) — single source of truth for AppState
// construction in integration tests. When AppState gains new fields, update
// ONLY this function and every test crate compiles again.
// =============================================================================

/// Build an AppState with sensible test defaults:
///  - all optional channels (`lifecycle_tx`, `recompute_tx`, `event_broadcast`,
///    `purge_tx`, `backfill_tx`) are `None` (no workers running)
///  - `license_valid` is `true` (so license-gated routes are reachable)
///  - `captures` is a fresh empty map
///
/// Tests that need a non-default channel (e.g. supervisor lifecycle tests
/// that need `lifecycle_tx: Some(tx)`) should override the field after
/// construction:
///
/// ```ignore
/// let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
/// state.lifecycle_tx = Some(lifecycle_tx);
/// ```
///
/// Phase 8 / Plan 08-02 (D-20): the third `paths` argument is required —
/// every AppState in tests carries an `Arc<Paths>` rooted at a per-test
/// `TempDir` (see `test_state_with_tmpdir` for the convenience helper that
/// owns the TempDir lifetime for you).
#[allow(dead_code)]
pub fn test_state(
    db: std::sync::Arc<libsql::Database>,
    config: std::sync::Arc<cronometrix_api::config::Config>,
    paths: std::sync::Arc<cronometrix_api::state::Paths>,
) -> cronometrix_api::state::AppState {
    // Mirror production wiring: spawn the single-writer queue worker against the
    // same db so handlers exercising `state.db_write` actually persist. Requires
    // a Tokio runtime — every caller is a `#[tokio::test]`. The CancellationToken
    // is never cancelled; the worker is reaped when the test runtime shuts down.
    let (db_write, db_write_rx) =
        cronometrix_api::db::write_queue::DbWriteQueue::channel(Default::default());
    tokio::spawn(cronometrix_api::db::write_queue::run_write_worker(
        db.clone(),
        db_write_rx,
    ));

    cronometrix_api::state::AppState {
        db,
        config,
        paths,
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
        license_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        purge_tx: None,
        backfill_tx: None,
        captures: cronometrix_api::enrollments::handlers::new_captures_map(),
        enrollment_tasks: cronometrix_api::enrollments::pusher::EnrollmentTaskTracker::new(),
        db_write,
        e2e_enabled: false,
        test_reset_enabled: false,
    }
}

/// Build a test AppState backed by a fresh per-test `TempDir`.
///
/// CALLER MUST bind the returned `TempDir` to a local variable that outlives
/// every assertion in the test — see Pitfall 1 in 08-RESEARCH.md. Dropping the
/// `TempDir` removes the directory and any path-touching assertion will fail
/// nondeterministically.
///
/// Idiomatic call site:
///
/// ```ignore
/// let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
/// // ... use `state` for handler tests; `_tmp` keeps the dir alive ...
/// ```
#[allow(dead_code)]
pub fn test_state_with_tmpdir(
    db: std::sync::Arc<libsql::Database>,
    config: std::sync::Arc<cronometrix_api::config::Config>,
) -> (cronometrix_api::state::AppState, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("create tempdir for test_state");
    let paths = std::sync::Arc::new(cronometrix_api::state::Paths::for_test(tmp.path()));
    let state = test_state(db, config, paths);
    (state, tmp)
}
