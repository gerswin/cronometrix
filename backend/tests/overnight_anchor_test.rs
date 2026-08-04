//! H-02: an overnight exit must recompute the day the shift STARTED, not the
//! event's own local date.
//!
//! `backend/src/events/service.rs` used to anchor every recompute request on
//! `captured_at`'s own local date. On a 22:00→06:00 shift the 06:00 exit is
//! captured on the FOLLOWING calendar date, so the day that actually opened
//! the shift never got recomputed and was left with `MISSING_EXIT`, while the
//! following day risked a spurious exit-only record.
//!
//! These tests exercise the real ingest path
//! (`events::service::persist_attendance_event_queued`) end to end: they wire
//! a real `recompute_tx` channel, drain whatever `RecomputeRequest::Day`
//! anchors it actually published, and run `daily_records::service::recompute_for_day`
//! against each one exactly the way `RecomputeWorker` would — so a regression
//! in the anchor-selection logic fails here, not just in a unit test of the
//! pure helper.

mod common;

use std::sync::Arc;

use chrono::{NaiveDate, TimeZone};
use cronometrix_api::config::Config;
use cronometrix_api::daily_records::service as dr_service;
use cronometrix_api::events::models::NewAttendanceEvent;
use cronometrix_api::events::service as events_service;
use cronometrix_api::recompute::RecomputeRequest;
use cronometrix_api::state::AppState;
use libsql::params;
use tokio::sync::mpsc;
use uuid::Uuid;

use common::{create_test_department_with_shift, test_device_creds_key};

fn make_state(db: libsql::Database) -> (AppState, tempfile::TempDir) {
    let config = Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
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
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
}

async fn seed_employee(db: &libsql::Database, dept_id: &str, code: &str) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![id.clone(), code.to_string(), dept_id.to_string()],
    )
    .await
    .expect("seed employee");
    id
}

async fn seed_device(db: &libsql::Database, id: &str) {
    let conn = db.connect().expect("connect");
    let hash: u32 = id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, b| acc.wrapping_mul(131).wrapping_add(*b as u32));
    let port = 1024 + (hash % 60000) as i64;
    let ip = format!("10.0.{}.{}", (hash >> 8) & 0xFF, hash & 0xFF);
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, \
         direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'https', 'admin', 'ciphertext', \
         'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        params![id.to_string(), format!("dev-{}", id), ip, port],
    )
    .await
    .expect("seed device");
}

async fn ensure_global_rules(db: &libsql::Database) {
    let conn = db.connect().expect("connect");
    conn.execute(
        "INSERT OR IGNORE INTO global_rules \
         (id, late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, \
          effective_from, version, updated_at) \
         VALUES ('singleton', 10, 10, 0, unixepoch(), 1, unixepoch())",
        (),
    )
    .await
    .expect("seed global_rules");
}

fn caracas_epoch(date: NaiveDate, hh: u32, mm: u32) -> i64 {
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let naive = date.and_time(chrono::NaiveTime::from_hms_opt(hh, mm, 0).unwrap());
    tz.from_local_datetime(&naive).single().unwrap().timestamp()
}

fn new_event(
    id: &str,
    employee_id: &str,
    device_id: &str,
    direction: &str,
    captured_at: i64,
) -> NewAttendanceEvent {
    NewAttendanceEvent {
        id: id.to_string(),
        employee_id: Some(employee_id.to_string()),
        device_id: device_id.to_string(),
        direction: direction.to_string(),
        captured_at,
        is_unknown: false,
        face_id: Some("42".to_string()),
        employee_no_string: Some("EMP001".to_string()),
        raw_payload: "<EventNotificationAlert/>".to_string(),
        photo_bytes: None,
    }
}

async fn count_daily_records(db: &libsql::Database, emp: &str, date: &str) -> i64 {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![emp.to_string(), date.to_string()],
        )
        .await
        .expect("count query");
    rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap()
}

async fn daily_record_id(db: &libsql::Database, emp: &str, date: &str) -> String {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT id FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![emp.to_string(), date.to_string()],
        )
        .await
        .expect("id query");
    rows.next()
        .await
        .unwrap()
        .unwrap()
        .get::<String>(0)
        .unwrap()
}

async fn daily_record_entry_exit(
    db: &libsql::Database,
    dr_id: &str,
) -> (Option<i64>, Option<i64>) {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT entry_at, exit_at FROM daily_records WHERE id = ?1",
            params![dr_id.to_string()],
        )
        .await
        .expect("entry/exit query");
    let row = rows.next().await.unwrap().expect("row");
    (row.get(0).unwrap(), row.get(1).unwrap())
}

async fn anomaly_codes_for(db: &libsql::Database, dr_id: &str) -> Vec<String> {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT code FROM daily_record_anomalies WHERE daily_record_id = ?1 ORDER BY created_at ASC",
            params![dr_id.to_string()],
        )
        .await
        .expect("anom query");
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.unwrap() {
        out.push(r.get::<String>(0).unwrap());
    }
    out
}

/// Drain every `RecomputeRequest::Day` currently queued (published by the
/// `persist_attendance_event_queued` calls the test already awaited) and run
/// `recompute_for_day` for each — exactly what `RecomputeWorker::process_day`
/// does, minus its debounce/collapsing, which is irrelevant to H-02: this
/// test is about which anchors get PUBLISHED, not how the worker paces them.
async fn drain_and_recompute(
    state: &AppState,
    rx: &mut mpsc::UnboundedReceiver<RecomputeRequest>,
) -> Vec<NaiveDate> {
    let mut anchors = Vec::new();
    while let Ok(request) = rx.try_recv() {
        if let RecomputeRequest::Day {
            employee_id,
            anchor_date,
        } = request
        {
            anchors.push(anchor_date);
            dr_service::recompute_for_day(state, &employee_id, anchor_date)
                .await
                .expect("recompute_for_day ok");
        }
    }
    anchors
}

/// H-02: in a 22:00-06:00 shift, the 06:05 exit belongs to the day that
/// opened the shift. Requesting recompute for the event's own local date
/// leaves the day the shift started with `MISSING_EXIT`.
#[tokio::test]
async fn an_overnight_exit_recomputes_the_day_the_shift_started() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "Night", "night", true, 480, "22:00", "06:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E100").await;
    seed_device(&db, "dev-1").await;

    let (mut state, _tmp) = make_state(db);
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.recompute_tx = Some(tx);

    let day = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    let next_day = day.succ_opt().unwrap(); // Tuesday

    let entry = new_event(
        "evt-entry",
        &emp_id,
        "dev-1",
        "entry",
        caracas_epoch(day, 22, 10),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, entry)
        .await
        .expect("persist entry");

    let exit = new_event(
        "evt-exit",
        &emp_id,
        "dev-1",
        "exit",
        caracas_epoch(next_day, 6, 5),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, exit)
        .await
        .expect("persist exit");

    let anchors = drain_and_recompute(&state, &mut rx).await;
    assert!(
        !anchors.contains(&next_day),
        "H-02: the exit must never publish recompute for its own local date {}; got {:?}",
        next_day,
        anchors
    );
    assert!(
        anchors.contains(&day),
        "the day the shift started must be recomputed; got {:?}",
        anchors
    );

    let day_str = day.format("%Y-%m-%d").to_string();
    assert_eq!(
        count_daily_records(&state.db, &emp_id, &day_str).await,
        1,
        "exactly one daily_record for the day the shift started, even though \
         both the entry and the exit publish anchor=D (idempotency)"
    );
    let dr_id = daily_record_id(&state.db, &emp_id, &day_str).await;
    let (entry_at, exit_at) = daily_record_entry_exit(&state.db, &dr_id).await;
    assert!(entry_at.is_some(), "day D must have an entry");
    assert!(
        exit_at.is_some(),
        "day D must have the overnight exit — this is the H-02 bug"
    );
    let anoms = anomaly_codes_for(&state.db, &dr_id).await;
    assert!(
        !anoms.contains(&"MISSING_EXIT".to_string()),
        "MISSING_EXIT must not fire once the exit is correctly anchored; got {:?}",
        anoms
    );

    let next_day_str = next_day.format("%Y-%m-%d").to_string();
    assert_eq!(
        count_daily_records(&state.db, &emp_id, &next_day_str).await,
        0,
        "D+1 must not get a spurious exit-only record"
    );
}

/// A day shift is never ambiguous: entry and exit share a calendar date, so
/// the fix must not change anchoring for the common case.
#[tokio::test]
async fn a_day_shift_still_anchors_on_its_own_date() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "Day", "day", false, 480, "08:00", "17:00").await;
    let emp_id = seed_employee(&db, &dept_id, "E200").await;
    seed_device(&db, "dev-1").await;

    let (mut state, _tmp) = make_state(db);
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.recompute_tx = Some(tx);

    let day = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();

    let entry = new_event(
        "evt-entry",
        &emp_id,
        "dev-1",
        "entry",
        caracas_epoch(day, 8, 0),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, entry)
        .await
        .expect("persist entry");

    let exit = new_event(
        "evt-exit",
        &emp_id,
        "dev-1",
        "exit",
        caracas_epoch(day, 17, 0),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, exit)
        .await
        .expect("persist exit");

    let anchors = drain_and_recompute(&state, &mut rx).await;
    assert!(
        anchors.iter().all(|a| *a == day),
        "day-shift events must only ever anchor their own date; got {:?}",
        anchors
    );

    let day_str = day.format("%Y-%m-%d").to_string();
    assert_eq!(count_daily_records(&state.db, &emp_id, &day_str).await, 1);
    let dr_id = daily_record_id(&state.db, &emp_id, &day_str).await;
    let (entry_at, exit_at) = daily_record_entry_exit(&state.db, &dr_id).await;
    assert!(entry_at.is_some());
    assert!(exit_at.is_some());
    let anoms = anomaly_codes_for(&state.db, &dr_id).await;
    assert!(
        !anoms.contains(&"MISSING_EXIT".to_string()) && !anoms.contains(&"MISSING_ENTRY".to_string()),
        "got {:?}",
        anoms
    );
}

/// The boundary case named in the task brief: an exit shortly after midnight
/// must still resolve to the day the overnight shift started, not the
/// calendar date it was physically captured on.
#[tokio::test]
async fn an_exit_at_00_30_belongs_to_the_previous_day() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "Night2", "night", true, 480, "22:00", "06:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E300").await;
    seed_device(&db, "dev-1").await;

    let (mut state, _tmp) = make_state(db);
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.recompute_tx = Some(tx);

    let day = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    let next_day = day.succ_opt().unwrap();

    let entry = new_event(
        "evt-entry",
        &emp_id,
        "dev-1",
        "entry",
        caracas_epoch(day, 22, 10),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, entry)
        .await
        .expect("persist entry");

    let exit = new_event(
        "evt-exit",
        &emp_id,
        "dev-1",
        "exit",
        caracas_epoch(next_day, 0, 30),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, exit)
        .await
        .expect("persist exit");

    let anchors = drain_and_recompute(&state, &mut rx).await;
    assert!(
        !anchors.contains(&next_day),
        "00:30 exit must not publish recompute for its own local date; got {:?}",
        anchors
    );
    assert!(anchors.contains(&day), "got {:?}", anchors);

    let day_str = day.format("%Y-%m-%d").to_string();
    let dr_id = daily_record_id(&state.db, &emp_id, &day_str).await;
    let (entry_at, exit_at) = daily_record_entry_exit(&state.db, &dr_id).await;
    assert!(entry_at.is_some());
    assert!(exit_at.is_some());

    let next_day_str = next_day.format("%Y-%m-%d").to_string();
    assert_eq!(
        count_daily_records(&state.db, &emp_id, &next_day_str).await,
        0
    );
}

/// H-02 ambiguity rule, other side: with a pathological tolerance
/// configuration wide enough to make two consecutive overnight windows
/// overlap, the SAME instant legitimately belongs to both anchors. The fix
/// publishes both rather than guessing — and recomputing both (and
/// re-recomputing either one) must never accumulate rows or anomalies.
#[tokio::test]
async fn overnight_ambiguous_instant_recomputes_both_anchors_idempotently() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    {
        let conn = db.connect().expect("connect");
        conn.execute(
            "UPDATE global_rules SET late_arrival_tolerance_min = 0, \
             early_departure_tolerance_min = 0, bonus_minutes = 1380 \
             WHERE id = 'singleton'",
            (),
        )
        .await
        .expect("widen tolerance to force window overlap");
    }
    let dept_id = create_test_department_with_shift(
        &db,
        "NightWide",
        "night",
        true,
        480,
        "22:00",
        "06:00",
    )
    .await;
    let emp_id = seed_employee(&db, &dept_id, "E400").await;
    seed_device(&db, "dev-1").await;

    let (mut state, _tmp) = make_state(db);
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.recompute_tx = Some(tx);

    let day = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    let next_day = day.succ_opt().unwrap();

    let exit = new_event(
        "evt-exit",
        &emp_id,
        "dev-1",
        "exit",
        caracas_epoch(next_day, 6, 5),
    );
    events_service::persist_attendance_event_queued(&state, &state.paths.events_root, exit)
        .await
        .expect("persist exit");

    let anchors = drain_and_recompute(&state, &mut rx).await;
    assert_eq!(
        anchors.len(),
        2,
        "ambiguous instant must recompute both candidate anchors; got {:?}",
        anchors
    );
    assert!(anchors.contains(&day) && anchors.contains(&next_day));

    // Re-recompute both anchors — simulating a duplicate publish or a
    // backfill re-run. Must not create a second row or duplicate anomalies.
    dr_service::recompute_for_day(&state, &emp_id, day)
        .await
        .expect("re-recompute D ok");
    dr_service::recompute_for_day(&state, &emp_id, next_day)
        .await
        .expect("re-recompute D+1 ok");

    let day_str = day.format("%Y-%m-%d").to_string();
    let next_day_str = next_day.format("%Y-%m-%d").to_string();
    assert_eq!(
        count_daily_records(&state.db, &emp_id, &day_str).await,
        1,
        "idempotent: still exactly one row for D after re-recompute"
    );
    assert_eq!(
        count_daily_records(&state.db, &emp_id, &next_day_str).await,
        1,
        "idempotent: still exactly one row for D+1 after re-recompute"
    );
}
