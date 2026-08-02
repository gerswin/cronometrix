//! `attendance::ingest` is the vendor-neutral port behind `isapi::ingest`:
//! resolve and persist a marking without ever seeing a vendor payload. These
//! tests exercise it directly, bypassing any Hikvision decoding, to prove the
//! port stands on its own.

mod common;

use std::sync::Arc;

use cronometrix_api::attendance::ingest::{ingest, IngestOutcome};
use cronometrix_api::attendance::marking::RawMarking;
use cronometrix_api::config::Config;
use libsql::params;

use common::{test_device_creds_key, TEST_JWT_SECRET};

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
        device_push_base_url: String::new(),
    })
}

/// Seed a department, an employee (`EMP-1`) and a device (`dev-1`) — the
/// minimum an ingest test needs to resolve an employee and satisfy the
/// `attendance_events.device_id` foreign key.
async fn seed(conn: &libsql::Connection) {
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES ('dev-1', 'Test Device', '10.0.0.9', 80, 'http', 'admin', 'ciphertext', 'entry', \
                 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES ('d-1', 'Dept', 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES ('emp-1', 'EMP-1', 'Test Employee', 'd-1', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
}

/// The domain ingest must resolve and persist a marking without ever seeing
/// a vendor payload — that is the whole point of the port.
#[tokio::test]
async fn persists_a_marking_and_resolves_the_employee() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let outcome = ingest(
        &state,
        "dev-1",
        "entry",
        RawMarking {
            external_person_id: Some("EMP-1".into()),
            face_id: None,
            occurred_at: 1_785_000_000,
            direction: Some("exit".into()),
            photo: None,
            raw_payload: "{}".into(),
        },
    )
    .await
    .expect("ingest");

    assert_eq!(outcome, IngestOutcome::Persisted);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT employee_id, direction, is_unknown FROM attendance_events WHERE device_id = 'dev-1'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("marking persisted");
    assert_eq!(
        row.get::<Option<String>>(0).unwrap().as_deref(),
        Some("emp-1")
    );
    // The marking's own reported direction ("exit") wins over the device's
    // "entry" default.
    assert_eq!(row.get::<String>(1).unwrap(), "exit");
    assert_eq!(row.get::<i64>(2).unwrap(), 0);
}

/// A marking that names nobody is a door or tamper notification, not
/// attendance. Persisting it would invent an unknown-face row every time the
/// door moved.
#[tokio::test]
async fn skips_a_marking_with_no_identity() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let outcome = ingest(
        &state,
        "dev-1",
        "entry",
        RawMarking {
            external_person_id: None,
            face_id: None,
            occurred_at: 1_785_000_000,
            direction: None,
            photo: None,
            raw_payload: "{}".into(),
        },
    )
    .await
    .expect("ingest");

    assert_eq!(outcome, IngestOutcome::Skipped);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE device_id = ?1",
            params!["dev-1"],
        )
        .await
        .unwrap();
    let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count, 0);
}

/// A reader that does not report a direction (`RawMarking::direction: None`)
/// falls back to the device's own configured default — never to some other
/// implicit choice. `direction_default` decides whether the marking is filed
/// as an arrival or a departure, so pinning this at the port's own test file
/// (rather than only transitively, via a Hikvision-flavoured caller) matters.
#[tokio::test]
async fn falls_back_to_the_devices_direction_default_when_the_reader_did_not_report_one() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let outcome = ingest(
        &state,
        "dev-1",
        "exit",
        RawMarking {
            external_person_id: Some("EMP-1".into()),
            face_id: None,
            occurred_at: 1_785_000_100,
            direction: None,
            photo: None,
            raw_payload: "{}".into(),
        },
    )
    .await
    .expect("ingest");

    assert_eq!(outcome, IngestOutcome::Persisted);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT direction FROM attendance_events WHERE device_id = 'dev-1'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("marking persisted");
    assert_eq!(row.get::<String>(0).unwrap(), "exit");
}

/// The device replaying an event, or both transports (stream + push)
/// delivering the same marking, must not double-write the attendance row —
/// `IngestOutcome::Deduplicated` is how the port reports that the write was
/// skipped rather than silently accepted as a second, indistinguishable one.
#[tokio::test]
async fn a_repeated_marking_is_deduplicated_not_persisted_twice() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let marking = || RawMarking {
        external_person_id: Some("EMP-1".into()),
        face_id: None,
        occurred_at: 1_785_000_200,
        direction: Some("entry".into()),
        photo: None,
        raw_payload: "{}".into(),
    };

    let first = ingest(&state, "dev-1", "entry", marking())
        .await
        .expect("ingest");
    assert_eq!(first, IngestOutcome::Persisted);

    let second = ingest(&state, "dev-1", "entry", marking())
        .await
        .expect("ingest");
    assert_eq!(second, IngestOutcome::Deduplicated);

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE device_id = 'dev-1'",
            (),
        )
        .await
        .unwrap();
    let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count, 1, "the replayed marking must not add a second row");
}
