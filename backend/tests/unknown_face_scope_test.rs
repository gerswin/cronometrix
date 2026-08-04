//! M-02: unknown-face anomalies must be scoped to the device that saw them.
//!
//! Before this fix, `recompute_for_day`'s window query pulled every
//! `is_unknown = 1` event in the window into *every* employee's recompute,
//! with no tie to the device the employee actually used. A single unmatched
//! face at one device could raise UNKNOWN_FACE_IN_WINDOW on every employee in
//! the installation whose window overlapped it.
//!
//! The fix scopes the unknown event to the `device_id` of the employee's own
//! events in the same window. Three scenarios are covered:
//!   1. Employee's own events are at a *different* device than the unknown
//!      face → no anomaly (the defect this ticket fixes).
//!   2. Employee's own events share the *same* device as the unknown face →
//!      anomaly still fires (the fix must not silence the real signal).
//!   3. Employee has *no* events at all in the window (e.g. absent that day)
//!      → anomaly is suppressed. There is no device to compare against, and
//!      emitting anyway would recreate the original defect for every absent
//!      employee in the installation. See the comment in
//!      `daily_records/service.rs` for the full reasoning.

mod common;

use std::sync::Arc;

use chrono::{NaiveDate, TimeZone};
use cronometrix_api::config::Config;
use cronometrix_api::daily_records::service as dr_service;
use cronometrix_api::state::AppState;
use libsql::params;
use uuid::Uuid;

use common::{create_test_department_with_shift, test_device_creds_key, TEST_JWT_SECRET};

fn make_state(db: libsql::Database) -> (AppState, tempfile::TempDir) {
    let config = Arc::new(Config {
        database_path: "test.db".into(),
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
    });
    common::test_state_with_tmpdir(Arc::new(db), config)
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

async fn seed_employee(db: &libsql::Database, dept_id: &str, code: &str) -> String {
    let conn = db.connect().expect("connect");
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
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
         VALUES (?1, ?2, ?3, ?4, 'https', 'admin', 'ct', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
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
         bucket_30s, is_unknown, face_id, employee_no_string, raw_payload, photo_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, '<x/>', NULL, unixepoch())",
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

/// Unknown-face event: `employee_id` NULL, `is_unknown = 1`.
async fn seed_unknown_event(db: &libsql::Database, device_id: &str, captured_at: i64) {
    let conn = db.connect().expect("connect");
    let bucket = captured_at / 30;
    conn.execute(
        "INSERT INTO attendance_events (id, employee_id, device_id, direction, captured_at, \
         bucket_30s, is_unknown, face_id, employee_no_string, raw_payload, photo_path, created_at) \
         VALUES (?1, NULL, ?2, 'entry', ?3, ?4, 1, 'stranger', NULL, '<x/>', NULL, unixepoch())",
        params![
            Uuid::new_v4().to_string(),
            device_id.to_string(),
            captured_at,
            bucket
        ],
    )
    .await
    .expect("seed unknown event");
}

fn caracas_epoch(date: NaiveDate, hh: u32, mm: u32) -> i64 {
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let naive = date.and_time(chrono::NaiveTime::from_hms_opt(hh, mm, 0).unwrap());
    tz.from_local_datetime(&naive).single().unwrap().timestamp()
}

async fn anomaly_codes_for(db: &libsql::Database, emp: &str, date: &str) -> Vec<String> {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT a.code FROM daily_record_anomalies a \
             JOIN daily_records dr ON dr.id = a.daily_record_id \
             WHERE dr.employee_id = ?1 AND dr.anchor_date = ?2 \
             ORDER BY a.created_at ASC",
            params![emp.to_string(), date.to_string()],
        )
        .await
        .expect("anom query");
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.unwrap() {
        out.push(r.get::<String>(0).unwrap());
    }
    out
}

/// M-02: an unknown face at a device the employee never used that day must
/// not flag them — the anomaly used to fire on every employee in the window
/// regardless of device.
#[tokio::test]
async fn an_unknown_face_does_not_flag_employees_from_other_devices() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-Scope-1", "day", false, 480, "09:00", "17:00")
            .await;
    let emp = seed_employee(&db, &dept, "SCOPE-1").await;
    seed_device(&db, "dev-own").await;
    seed_device(&db, "dev-stranger").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    // Employee clocks in/out at their own device.
    seed_event(&db, &emp, "dev-own", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp, "dev-own", "exit", caracas_epoch(anchor, 17, 0)).await;
    // Unrelated unknown face shows up at a different device in the same window.
    seed_unknown_event(&db, "dev-stranger", caracas_epoch(anchor, 12, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp, anchor)
        .await
        .expect("recompute");

    let codes = anomaly_codes_for(&state.db, &emp, "2026-04-20").await;
    assert!(
        !codes.iter().any(|c| c == "UNKNOWN_FACE_IN_WINDOW"),
        "unknown face at a different device must not flag this employee; got {:?}",
        codes
    );
}

/// The fix must not silence the real signal: an unknown face at the same
/// device and window the employee used must still flag them.
#[tokio::test]
async fn an_unknown_face_still_flags_the_same_device_window() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-Scope-2", "day", false, 480, "09:00", "17:00")
            .await;
    let emp = seed_employee(&db, &dept, "SCOPE-2").await;
    seed_device(&db, "dev-shared").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    seed_event(
        &db,
        &emp,
        "dev-shared",
        "entry",
        caracas_epoch(anchor, 9, 0),
    )
    .await;
    seed_event(
        &db,
        &emp,
        "dev-shared",
        "exit",
        caracas_epoch(anchor, 17, 0),
    )
    .await;
    // Unknown face at the *same* device, same window.
    seed_unknown_event(&db, "dev-shared", caracas_epoch(anchor, 12, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp, anchor)
        .await
        .expect("recompute");

    let codes = anomaly_codes_for(&state.db, &emp, "2026-04-20").await;
    assert!(
        codes.iter().any(|c| c == "UNKNOWN_FACE_IN_WINDOW"),
        "unknown face sharing device and window must still flag; got {:?}",
        codes
    );
}

/// The decided case: an employee with NO events at all in the window (e.g.
/// absent that day) has no device to scope against. The anomaly is
/// suppressed rather than emitted — see the rationale comment in
/// `daily_records/service.rs`. Emitting here would recreate the original
/// defect: one unmatched face would flag every absent employee in the
/// installation, regardless of device.
#[tokio::test]
async fn an_unknown_face_does_not_flag_an_employee_with_no_events_in_window() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept =
        create_test_department_with_shift(&db, "D-Scope-3", "day", false, 480, "09:00", "17:00")
            .await;
    let emp = seed_employee(&db, &dept, "SCOPE-3").await;
    seed_device(&db, "dev-absent").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    // No events at all for `emp` in the window — they were absent.
    seed_unknown_event(&db, "dev-absent", caracas_epoch(anchor, 12, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp, anchor)
        .await
        .expect("recompute");

    let codes = anomaly_codes_for(&state.db, &emp, "2026-04-20").await;
    assert!(
        !codes.iter().any(|c| c == "UNKNOWN_FACE_IN_WINDOW"),
        "absent employee (no events in window) must not be flagged by an unrelated unknown face; got {:?}",
        codes
    );
}
