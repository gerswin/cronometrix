//! Integration tests for `daily_records::service::recompute_for_day`.
//!
//! These tests verify:
//! - ON CONFLICT DO UPDATE preserves the row id and replaces (not accumulates)
//!   anomalies across recomputes.
//! - RECOMPUTE_AFTER_EDIT fires on the second call (prior row existed).

mod common;

use std::sync::Arc;

use chrono::{NaiveDate, TimeZone};
use cronometrix_api::config::Config;
use cronometrix_api::daily_records::service as dr_service;
use cronometrix_api::state::AppState;
use libsql::params;
use uuid::Uuid;

use common::{create_test_department_with_shift, test_device_creds_key};

/// Build (AppState, TempDir) for daily_record tests. Per Plan 08-02 D-20:
/// AppState is rooted in a per-test tempdir; caller binds the TempDir to a
/// local that outlives every assertion (Pitfall 1 in 08-RESEARCH.md).
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

async fn seed_event(
    db: &libsql::Database,
    employee_id: &str,
    device_id: &str,
    direction: &str,
    captured_at: i64,
) {
    let conn = db.connect().expect("connect");
    let bucket = captured_at / 30;
    conn.execute(
        "INSERT INTO attendance_events (id, employee_id, device_id, direction, captured_at, \
         bucket_30s, is_unknown, face_id, employee_no_string, raw_xml, photo_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, '<EventNotificationAlert/>', NULL, unixepoch())",
        params![
            Uuid::new_v4().to_string(),
            employee_id.to_string(),
            device_id.to_string(),
            direction.to_string(),
            captured_at,
            bucket
        ],
    )
    .await
    .expect("seed event");
}

fn caracas_epoch(date: NaiveDate, hh: u32, mm: u32) -> i64 {
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let naive = date.and_time(chrono::NaiveTime::from_hms_opt(hh, mm, 0).unwrap());
    tz.from_local_datetime(&naive).single().unwrap().timestamp()
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

#[tokio::test]
async fn recompute_upsert_preserves_id_and_replaces_anomalies() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptA", "day", false, 480, "09:00", "17:00").await;
    let emp_id = seed_employee(&db, &dept_id, "E001").await;
    seed_device(&db, "dev-1").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
                                                                // Seed only an entry — will raise MISSING_EXIT.
    seed_event(&db, &emp_id, "dev-1", "entry", caracas_epoch(anchor, 9, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("first recompute ok");

    assert_eq!(
        count_daily_records(&state.db, &emp_id, "2026-04-20").await,
        1,
        "one daily_record after first recompute"
    );
    let first_id = daily_record_id(&state.db, &emp_id, "2026-04-20").await;
    let first_anoms = anomaly_codes_for(&state.db, &first_id).await;
    assert!(
        first_anoms.contains(&"MISSING_EXIT".to_string()),
        "first call raises MISSING_EXIT; got {:?}",
        first_anoms
    );

    // Insert an exit event — should now resolve MISSING_EXIT.
    seed_event(
        &db_ref(&state),
        &emp_id,
        "dev-1",
        "exit",
        caracas_epoch(anchor, 17, 0),
    )
    .await;

    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("second recompute ok");

    let second_id = daily_record_id(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(first_id, second_id, "ON CONFLICT preserves id");

    let second_anoms = anomaly_codes_for(&state.db, &second_id).await;
    // Should NOT contain MISSING_EXIT anymore (replaced, not accumulated).
    assert!(
        !second_anoms.contains(&"MISSING_EXIT".to_string()),
        "MISSING_EXIT must be replaced on second recompute; got {:?}",
        second_anoms
    );
    // RECOMPUTE_AFTER_EDIT must be present because prior row existed.
    assert!(
        second_anoms.contains(&"RECOMPUTE_AFTER_EDIT".to_string()),
        "RECOMPUTE_AFTER_EDIT expected; got {:?}",
        second_anoms
    );
}

#[tokio::test]
async fn recompute_flags_recompute_after_edit_on_second_call() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptB", "day", false, 480, "09:00", "17:00").await;
    let emp_id = seed_employee(&db, &dept_id, "E002").await;
    seed_device(&db, "dev-1").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    seed_event(&db, &emp_id, "dev-1", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp_id, "dev-1", "exit", caracas_epoch(anchor, 17, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("first ok");
    let first_id = daily_record_id(&state.db, &emp_id, "2026-04-20").await;
    let first_anoms = anomaly_codes_for(&state.db, &first_id).await;
    assert!(
        !first_anoms.contains(&"RECOMPUTE_AFTER_EDIT".to_string()),
        "first call must NOT raise RECOMPUTE_AFTER_EDIT; got {:?}",
        first_anoms
    );

    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("second ok");
    let second_id = daily_record_id(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(first_id, second_id, "id preserved across recomputes");
    let second_anoms = anomaly_codes_for(&state.db, &second_id).await;
    assert!(
        second_anoms.contains(&"RECOMPUTE_AFTER_EDIT".to_string()),
        "second call must raise RECOMPUTE_AFTER_EDIT; got {:?}",
        second_anoms
    );
}

/// libsql::Database is not Clone; use this helper to borrow from Arc for seed ops.
fn db_ref(state: &AppState) -> &libsql::Database {
    &state.db
}

// -----------------------------------------------------------------------------
// Plan 03-02: Overnight service-layer integration
// -----------------------------------------------------------------------------
// Verifies that the event-range query in `recompute_for_day` correctly spans
// midnight for overnight shifts: events at 22:00 Mon and 06:00 Tue (shift
// anchored on Monday) are BOTH captured by the `BETWEEN window_start AND
// window_end` query — proving that delegation to
// `shift_window_overnight_aware` already gives the correct UTC epoch range
// without requiring any SQL change in the service layer.

#[tokio::test]
async fn recompute_overnight_captures_post_midnight_events() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;

    // Overnight dept: 22:00 → 06:00, ordinary=420 (LOTTT Art. 117 night).
    let dept_id =
        create_test_department_with_shift(&db, "DeptNight", "night", true, 420, "22:00", "06:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E_NIGHT").await;
    seed_device(&db, "dev-night-1").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday

    // Entry: 22:00 Mon Caracas. Exit: 06:00 Tue Caracas.
    let entry_epoch = caracas_epoch(anchor, 22, 0);
    let exit_epoch = {
        let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
        let tue = anchor.succ_opt().unwrap();
        let naive = tue.and_time(chrono::NaiveTime::from_hms_opt(6, 0, 0).unwrap());
        tz.from_local_datetime(&naive).single().unwrap().timestamp()
    };

    seed_event(&db, &emp_id, "dev-night-1", "entry", entry_epoch).await;
    seed_event(&db, &emp_id, "dev-night-1", "exit", exit_epoch).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("overnight recompute");

    // Verify exactly one daily_records row exists, anchored to Monday.
    assert_eq!(
        count_daily_records(&state.db, &emp_id, "2026-04-20").await,
        1,
        "overnight shift must anchor to shift-start date (D-05), not to the exit date"
    );

    // Read minutes + anomaly codes on the persisted row.
    let conn = state.db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT work_minutes, overtime_minutes, late_minutes, early_departure_minutes, \
             anchor_date FROM daily_records WHERE employee_id = ?1",
            params![emp_id.clone()],
        )
        .await
        .expect("read row");
    let row = rows.next().await.unwrap().expect("row exists");
    let work_minutes: i64 = row.get(0).unwrap();
    let overtime_minutes: i64 = row.get(1).unwrap();
    let late_minutes: i64 = row.get(2).unwrap();
    let early_departure_minutes: i64 = row.get(3).unwrap();
    let anchor_str: String = row.get(4).unwrap();

    assert_eq!(
        anchor_str, "2026-04-20",
        "anchor_date must equal shift-start Monday (D-05)"
    );
    // 8h shift (480min raw) − 60min fixed lunch = 420 work_minutes. Ordinary
    // threshold = 420 per LOTTT Art. 117, so OT = 0.
    assert_eq!(
        work_minutes, 420,
        "overnight 22:00-06:00 minus 60m lunch = 420min work"
    );
    assert_eq!(
        overtime_minutes, 0,
        "no overtime at exactly ordinary threshold"
    );
    assert_eq!(
        late_minutes, 0,
        "entry at exactly nominal shift start → 0 late"
    );
    assert_eq!(
        early_departure_minutes, 0,
        "exit at exactly nominal shift end → 0 early departure"
    );

    // The record must NOT carry MISSING_ENTRY / MISSING_EXIT (both punches
    // were captured across midnight), nor OvernightInferenceAmbiguous (Caracas
    // has no DST).
    let dr_id = daily_record_id(&state.db, &emp_id, "2026-04-20").await;
    let codes = anomaly_codes_for(&state.db, &dr_id).await;
    assert!(
        !codes.contains(&"MISSING_ENTRY".to_string()),
        "MISSING_ENTRY must not fire; got {:?}",
        codes
    );
    assert!(
        !codes.contains(&"MISSING_EXIT".to_string()),
        "MISSING_EXIT must not fire; got {:?}",
        codes
    );
    assert!(
        !codes.contains(&"OVERNIGHT_INFERENCE_AMBIGUOUS".to_string()),
        "Caracas must never raise OVERNIGHT_INFERENCE_AMBIGUOUS; got {:?}",
        codes
    );
}
