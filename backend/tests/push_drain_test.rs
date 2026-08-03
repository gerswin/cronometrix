//! C-10, part 2: `workers::push_drain` turns `device_push_inbox` rows into
//! attendance events off the HTTP response path, retrying transient failures
//! and dead-lettering permanent ones. See `backend/src/workers/push_drain.rs`
//! for the transient-vs-permanent classification this file locks down.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::state::AppState;
use cronometrix_api::workers::push_drain;
use libsql::params;
use uuid::Uuid;

use common::test_device_creds_key;

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: "test-secret-key-at-least-32-characters-long!!".to_string(),
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

async fn seed_device(conn: &libsql::Connection, device_id: &str) {
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at, \
          ingest_mode, push_token) \
         VALUES (?1, 'Drain Device', '10.0.0.20', 80, 'http', 'admin', 'ciphertext', 'entry', \
                 0, 'offline', 'active', 1, unixepoch(), unixepoch(), 'push', 'tok')",
        params![device_id],
    )
    .await
    .unwrap();
}

async fn seed_employee(
    conn: &libsql::Connection,
    department_id: &str,
    employee_id: &str,
    employee_code: &str,
) {
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, 'Drain Dept', 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        params![department_id],
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Drain Employee', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![employee_id, employee_code, department_id],
    )
    .await
    .unwrap();
}

/// Insert a `device_push_inbox` row exactly as `receive_push` would have left
/// it: `pending`, `attempts = 0`, no `last_error`/`processed_at`. Mirrors the
/// handler's own INSERT in `devices/push.rs` rather than re-deriving a
/// different shape.
async fn seed_pending_row(
    conn: &libsql::Connection,
    device_id: &str,
    content_type: &str,
    body: &[u8],
) -> String {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO device_push_inbox \
           (id, device_id, content_type, body, body_sha256, received_at, status, attempts) \
         VALUES (?1, ?2, ?3, ?4, 'test-sha', unixepoch(), 'pending', 0)",
        params![id.clone(), device_id, content_type, body.to_vec()],
    )
    .await
    .unwrap();
    id
}

fn recognition_event(employee_code: &str) -> String {
    format!(
        r#"{{"dateTime":"2026-08-02T10:15:30-04:00","eventType":"AccessControllerEvent",
        "AccessControllerEvent":{{"majorEventType":5,"subEventType":75,
        "name":"Drain Employee","employeeNoString":"{employee_code}","attendanceStatus":"checkIn"}}}}"#
    )
}

/// A door/tamper notification: shares the envelope with a real marking but
/// names nobody, so it never reaches identity resolution at all.
fn door_event() -> String {
    r#"{"dateTime":"2026-08-02T10:15:29-04:00","eventType":"AccessControllerEvent",
        "AccessControllerEvent":{"majorEventType":5,"subEventType":22,"doorNo":1,
        "attendanceStatus":"undefined"}}"#
        .to_string()
}

async fn inbox_row(
    state: &AppState,
    id: &str,
) -> (String, String, i64, Option<String>, Option<i64>) {
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT status, device_id, attempts, last_error, processed_at \
             FROM device_push_inbox WHERE id = ?1",
            params![id],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("row must exist");
    (
        row.get::<String>(0).unwrap(),
        row.get::<String>(1).unwrap(),
        row.get::<i64>(2).unwrap(),
        row.get::<Option<String>>(3).unwrap(),
        row.get::<Option<i64>>(4).unwrap(),
    )
}

/// The worker converts a pending row into an attendance event and marks the
/// row as processed.
#[tokio::test]
async fn the_drainer_turns_a_pending_row_into_an_attendance_event() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-ok";
    seed_device(&conn, device_id).await;
    seed_employee(&conn, "dept-drain-ok", "emp-drain-ok", "EMP-DRAIN-OK").await;
    let inbox_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-DRAIN-OK").as_bytes(),
    )
    .await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let stats = push_drain::drain_once(&state).await.unwrap();
    assert_eq!(stats.selected, 1);
    assert_eq!(
        stats.done, 1,
        "a valid recognition event must resolve to done"
    );
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.retried, 0);

    let (status, _device, attempts, last_error, processed_at) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "done");
    assert_eq!(attempts, 0);
    assert!(last_error.is_none());
    assert!(
        processed_at.is_some(),
        "processed_at must be stamped on success"
    );

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT employee_id, is_unknown FROM attendance_events WHERE device_id = ?1",
            params![device_id],
        )
        .await
        .unwrap();
    let row = rows
        .next()
        .await
        .unwrap()
        .expect("attendance event must be persisted");
    assert_eq!(
        row.get::<Option<String>>(0).unwrap().as_deref(),
        Some("emp-drain-ok")
    );
    assert_eq!(row.get::<i64>(1).unwrap(), 0);
}

/// A body that cannot be parsed is NEVER retried in a loop: it is marked
/// `failed` and stays visible. Retrying invalid XML/JSON does not fix it.
#[tokio::test]
async fn an_unparseable_body_lands_in_the_dead_letter_queue() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-garbage";
    seed_device(&conn, device_id).await;
    let inbox_id =
        seed_pending_row(&conn, device_id, "application/json", b"{not json at all").await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let stats = push_drain::drain_once(&state).await.unwrap();
    assert_eq!(stats.done, 0);
    assert_eq!(
        stats.failed, 1,
        "unparseable body must be dead-lettered, not retried"
    );
    assert_eq!(stats.retried, 0);

    let (status, _device, _attempts, last_error, processed_at) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "failed");
    assert!(last_error.is_some(), "the dead letter must record why");
    assert!(processed_at.is_some());

    // A second pass must not touch it again: it is no longer `pending`, so it
    // is not even selected — proving there is no retry loop hiding here.
    let stats_again = push_drain::drain_once(&state).await.unwrap();
    assert_eq!(stats_again.selected, 0);
    let (status_again, ..) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status_again, "failed");
}

/// A transient failure (here: the database read the marking needs fails) IS
/// retried, and the attempts counter goes up. The row stays `pending`.
#[tokio::test]
async fn a_transient_failure_is_retried_and_counted() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-transient";
    seed_device(&conn, device_id).await;
    // Break the read `attendance::ingest::ingest` needs to resolve an
    // identified marking's employee, WITHOUT touching parsing or the write
    // queue: `device_face_mappings` is only consulted once a marking clears
    // `has_identity()`, so this affects exactly the row below and nothing
    // that lacks an employeeNoString/faceId (proven by the other tests in
    // this file, which never touch this table).
    conn.execute("DROP TABLE device_face_mappings", ())
        .await
        .unwrap();
    let inbox_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-DOES-NOT-MATTER").as_bytes(),
    )
    .await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let stats = push_drain::drain_once(&state).await.unwrap();
    assert_eq!(stats.done, 0);
    assert_eq!(
        stats.failed, 0,
        "a transient failure must not be dead-lettered on the first try"
    );
    assert_eq!(stats.retried, 1);

    let (status, _device, attempts, last_error, processed_at) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "pending", "must stay pending to be retried");
    assert_eq!(attempts, 1);
    assert!(last_error.is_some());
    assert!(processed_at.is_none(), "not resolved yet");

    // Retrying again increments the counter again — it is genuinely retried,
    // not just marked once and left alone.
    push_drain::drain_once(&state).await.unwrap();
    let (status2, _device, attempts2, ..) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status2, "pending");
    assert_eq!(attempts2, 2);
}

/// No accepted push is ever lost: after draining, every row that started
/// `pending` ends in `done` or `failed`. None is left silently `pending`.
#[tokio::test]
async fn every_accepted_push_ends_up_done_or_failed_never_lost() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-mixed";
    seed_device(&conn, device_id).await;
    seed_employee(
        &conn,
        "dept-drain-mixed",
        "emp-drain-mixed",
        "EMP-DRAIN-MIXED",
    )
    .await;

    // A door event never reaches identity resolution, so dropping
    // device_face_mappings below cannot affect it — it must resolve to done.
    let done_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        door_event().as_bytes(),
    )
    .await;
    // Garbage must dead-letter immediately.
    let failed_id =
        seed_pending_row(&conn, device_id, "application/json", b"{not json at all").await;

    conn.execute("DROP TABLE device_face_mappings", ())
        .await
        .unwrap();
    // A recognition event now hits the broken read on every attempt, and
    // must exhaust its retry budget into `failed` rather than sit pending
    // forever.
    let exhausted_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-DRAIN-MIXED").as_bytes(),
    )
    .await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    // Enough passes to exhaust MAX_ATTEMPTS for the transient row.
    for _ in 0..(push_drain::MAX_ATTEMPTS + 1) {
        push_drain::drain_once(&state).await.unwrap();
    }

    let (done_status, ..) = inbox_row(&state, &done_id).await;
    let (failed_status, ..) = inbox_row(&state, &failed_id).await;
    let (exhausted_status, _device, exhausted_attempts, exhausted_error, _processed) =
        inbox_row(&state, &exhausted_id).await;

    assert_eq!(done_status, "done");
    assert_eq!(failed_status, "failed");
    assert_eq!(
        exhausted_status, "failed",
        "must give up once MAX_ATTEMPTS is reached"
    );
    assert!(exhausted_attempts >= push_drain::MAX_ATTEMPTS);
    assert!(exhausted_error.is_some());

    // The invariant this test exists to prove: nothing pending is left over.
    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM device_push_inbox WHERE device_id = ?1 AND status = 'pending'",
            params![device_id],
        )
        .await
        .unwrap();
    let pending_count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(pending_count, 0, "no row may be left pending forever");

    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM device_push_inbox \
             WHERE device_id = ?1 AND status NOT IN ('done', 'failed')",
            params![device_id],
        )
        .await
        .unwrap();
    let stray_count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(stray_count, 0, "every row must end in done or failed");
}

/// A row whose device row is gone has no `direction_default` to fall back on
/// and no retry brings the device row back — permanent, not transient.
#[tokio::test]
async fn a_row_whose_device_no_longer_exists_is_a_permanent_failure() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-vanished";
    seed_device(&conn, device_id).await;
    let inbox_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-IRRELEVANT").as_bytes(),
    )
    .await;
    // The device row is removed after the push already landed in the inbox
    // (e.g. a hard delete path). FK enforcement is per-connection in SQLite;
    // toggle it off on this connection for the delete, exactly like a
    // migration or an admin tool would need to.
    conn.execute("PRAGMA foreign_keys = OFF", ()).await.unwrap();
    conn.execute("DELETE FROM devices WHERE id = ?1", params![device_id])
        .await
        .unwrap();
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let stats = push_drain::drain_once(&state).await.unwrap();
    assert_eq!(stats.done, 0);
    assert_eq!(stats.retried, 0);
    assert_eq!(stats.failed, 1);

    let (status, _device, attempts, last_error, processed_at) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "failed");
    assert_eq!(attempts, 0, "never retried — this was never transient");
    assert!(last_error.unwrap().contains("no longer exists"));
    assert!(processed_at.is_some());
}

/// A failure to record the outcome (e.g. the write queue is closed) is
/// logged, not propagated: the row is left exactly as it was found, so the
/// next drain pass picks it back up. This is what makes `resolve`'s
/// mutation step safe to fail without losing track of a row.
#[tokio::test]
async fn a_status_update_failure_leaves_the_row_untouched_for_the_next_pass() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-writequeue-closed";
    seed_device(&conn, device_id).await;
    seed_employee(&conn, "dept-wq", "emp-wq", "EMP-WQ").await;
    let inbox_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-WQ").as_bytes(),
    )
    .await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    // Ingest itself reads through `state.db` (unaffected), but every status
    // update push_drain performs goes through `state.db_write`. Closing it
    // makes every mark_* call fail without touching the ingest path at all.
    state.db_write.close_and_flush().await.unwrap();

    let stats = push_drain::drain_once(&state).await.unwrap();
    // The row was selected and a marking may well have been persisted (the
    // read side still works) — but push_drain could not RECORD that outcome,
    // so from the inbox's point of view nothing happened yet.
    assert_eq!(stats.selected, 1);

    let (status, _device, attempts, last_error, processed_at) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "pending", "left exactly as found for the next pass");
    assert_eq!(attempts, 0);
    assert!(last_error.is_none());
    assert!(processed_at.is_none());
}

/// The periodic owner: processes on its cadence tick, and returns cleanly —
/// after one final drain — when the shared shutdown token fires.
#[tokio::test]
async fn run_drains_on_its_cadence_and_exits_cleanly_on_shutdown() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let device_id = "dev-drain-run-loop";
    seed_device(&conn, device_id).await;
    seed_employee(&conn, "dept-run", "emp-run", "EMP-RUN").await;
    let inbox_id = seed_pending_row(
        &conn,
        device_id,
        "application/json",
        recognition_event("EMP-RUN").as_bytes(),
    )
    .await;
    drop(conn);
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let shutdown = tokio_util::sync::CancellationToken::new();
    let handle = tokio::spawn(push_drain::run(state.clone(), shutdown.clone()));

    // DRAIN_CADENCE is 3s; give the periodic tick a real chance to fire and
    // drain the seeded row before asking the worker to stop.
    tokio::time::sleep(push_drain::DRAIN_CADENCE + std::time::Duration::from_millis(500)).await;
    let (status, ..) = inbox_row(&state, &inbox_id).await;
    assert_eq!(status, "done", "the periodic tick must have drained it");

    shutdown.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("run() must return promptly after cancellation")
        .expect("run() must not panic");
    assert!(result.is_ok(), "run() must exit Ok on a clean shutdown");
}
