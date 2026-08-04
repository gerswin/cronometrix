//! M-03: punch pairing must not distort the recorded workday.
//!
//! Two defects fixed together, in the same code path (`calc::aggregation`,
//! `calc::lunch`, wired by `calc::engine::compute_daily_record`):
//!
//! - Aggregation used to take the window's first event as entry and last
//!   event as exit ("extremes"), so a long absence in the middle of the day
//!   counted as worked time, and several disjoint blocks of work collapsed
//!   into one span.
//! - `fixed`-mode lunch was deducted unconditionally, regardless of whether
//!   the shift was even long enough to have contained the lunch it assumes.
//!
//! Each test below goes through the real `recompute_for_day` pipeline (seed
//! events in SQLite, run the engine, read the persisted `daily_records` +
//! `daily_record_anomalies` rows) rather than calling `compute_daily_record`
//! directly, matching this codebase's existing DB-integration test style for
//! this area (see `daily_record_tests.rs`).
//!
//! All departments here use `create_test_department_with_shift`, which
//! hardcodes `lunch_mode='fixed'`, `lunch_duration_min=60` — see
//! `tests/common/mod.rs`. `ensure_global_rules` seeds
//! `late_arrival_tolerance_min=10`, `early_departure_tolerance_min=10`,
//! `bonus_minutes=0`.

mod common;

use std::sync::Arc;

use chrono::{NaiveDate, TimeZone};
use cronometrix_api::config::Config;
use cronometrix_api::daily_records::service as dr_service;
use cronometrix_api::state::AppState;
use libsql::params;
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

struct DailyRow {
    work_minutes: i64,
    overtime_minutes: i64,
    late_minutes: i64,
    early_departure_minutes: i64,
}

async fn read_daily_row(db: &libsql::Database, emp: &str, date: &str) -> DailyRow {
    let conn = db.connect().expect("connect");
    let mut rows = conn
        .query(
            "SELECT work_minutes, overtime_minutes, late_minutes, early_departure_minutes \
             FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![emp.to_string(), date.to_string()],
        )
        .await
        .expect("row query");
    let row = rows.next().await.unwrap().expect("row exists");
    DailyRow {
        work_minutes: row.get(0).unwrap(),
        overtime_minutes: row.get(1).unwrap(),
        late_minutes: row.get(2).unwrap(),
        early_departure_minutes: row.get(3).unwrap(),
    }
}

async fn anomaly_codes(db: &libsql::Database, emp: &str, date: &str) -> Vec<String> {
    let conn = db.connect().expect("connect");
    let mut id_rows = conn
        .query(
            "SELECT id FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![emp.to_string(), date.to_string()],
        )
        .await
        .expect("id query");
    let dr_id: String = id_rows.next().await.unwrap().expect("row exists").get(0).unwrap();
    let mut rows = conn
        .query(
            "SELECT code FROM daily_record_anomalies WHERE daily_record_id = ?1",
            params![dr_id],
        )
        .await
        .expect("anomaly query");
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.unwrap() {
        out.push(r.get::<String>(0).unwrap());
    }
    out
}

/// M-03: entrada, salida a mediodia, vuelta, salida final. Las dos horas de
/// ausencia intermedia NO son trabajo.
///
/// Hand derivation: entry 09:00, exit 12:00 (3h = 180min), entry 14:00,
/// exit 17:00 (3h = 180min). Paired intervals: [09:00-12:00, 14:00-17:00].
/// raw_minutes = 180 + 180 = 360 (NOT last_exit(17:00) - first_entry(09:00)
/// = 480, which is the pre-M-03 bug — it would silently count the 12:00-14:00
/// absence as worked time).
///
/// Department is `fixed` mode, lunch=60min. 360 > 60, so the full configured
/// lunch is deducted: work_minutes = 360 - 60 = 300.
/// entry at exactly nominal shift start (09:00) → late_minutes = 0.
/// exit at exactly nominal shift end (17:00) → early_departure_minutes = 0.
/// overtime_minutes = max(0, 300 - 480) = 0.
#[tokio::test]
async fn a_long_midday_absence_is_not_counted_as_worked_time() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptAbsence", "day", false, 480, "09:00", "17:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E-ABSENCE").await;
    seed_device(&db, "dev-absence").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    seed_event(&db, &emp_id, "dev-absence", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp_id, "dev-absence", "exit", caracas_epoch(anchor, 12, 0)).await;
    seed_event(&db, &emp_id, "dev-absence", "entry", caracas_epoch(anchor, 14, 0)).await;
    seed_event(&db, &emp_id, "dev-absence", "exit", caracas_epoch(anchor, 17, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("recompute ok");

    let row = read_daily_row(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(
        row.work_minutes, 300,
        "two disjoint 3h blocks (360 raw) minus 60min fixed lunch = 300; \
         pre-M-03 extremes would have given 480 - 60 = 420, silently paying \
         for the 2h absence"
    );
    assert_eq!(row.overtime_minutes, 0);
    assert_eq!(row.late_minutes, 0, "entry exactly at nominal shift start");
    assert_eq!(
        row.early_departure_minutes, 0,
        "exit exactly at nominal shift end"
    );

    let codes = anomaly_codes(&state.db, &emp_id, "2026-04-20").await;
    assert!(
        !codes.contains(&"MISSING_EXIT".to_string()),
        "every entry has a matching exit; got {:?}",
        codes
    );
}

/// M-03: una jornada de 3 horas no pierde un almuerzo que nadie tomo.
///
/// Hand derivation: entry 09:00, exit 09:45 — a single 45-minute presence.
/// Paired interval: [(09:00,09:45)], raw_minutes = 45.
///
/// Department is `fixed` mode, lunch=60min. 45 is NOT > 60 (the shift is
/// shorter than the lunch itself), so the M-03 decision applies: deduct
/// nothing. work_minutes = 45.
///
/// Pre-M-03 behaviour: raw_minutes(45) - fixed_lunch(60) = -15, clamped by
/// `.max(0)` to 0 — the entire 45-minute presence would have been erased,
/// which is the bug this test locks against.
#[tokio::test]
async fn a_short_shift_does_not_lose_a_lunch_that_never_happened() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptShort", "day", false, 480, "09:00", "17:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E-SHORT").await;
    seed_device(&db, "dev-short").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    seed_event(&db, &emp_id, "dev-short", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp_id, "dev-short", "exit", caracas_epoch(anchor, 9, 45)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("recompute ok");

    let row = read_daily_row(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(
        row.work_minutes, 45,
        "a 45-minute shift is shorter than the 60-minute configured lunch; \
         deducting the lunch anyway would erase real, observed work"
    );

    let codes = anomaly_codes(&state.db, &emp_id, "2026-04-20").await;
    assert!(
        !codes.contains(&"LUNCH_PUNCH_MISSING".to_string()),
        "fixed mode never raises LUNCH_PUNCH_MISSING (that's a punch-mode-only anomaly); got {:?}",
        codes
    );
}

/// Marcaciones impares: entrada, salida, entrada, sin salida final.
///
/// Policy decided in `aggregation::pair_work_intervals` / `engine.rs`: the
/// dangling entry is discarded from worked-minutes (there is nothing to
/// pair it with) AND flagged — reusing `MISSING_EXIT`, the same code used
/// when the whole window lacks an exit, because from this entry's point of
/// view it does.
///
/// Hand derivation: entry 09:00, exit 12:00, entry 14:00 (no further exit).
/// Paired interval: [(09:00,12:00)] = 180 raw minutes; the 14:00 entry is
/// an unmatched orphan → MISSING_EXIT.
///
/// Department is `fixed` mode, lunch=60min. 180 > 60, so 60 is deducted:
/// work_minutes = 180 - 60 = 120.
#[tokio::test]
async fn an_odd_number_of_punches_has_a_defined_outcome() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptOdd", "day", false, 480, "09:00", "17:00")
            .await;
    let emp_id = seed_employee(&db, &dept_id, "E-ODD").await;
    seed_device(&db, "dev-odd").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    seed_event(&db, &emp_id, "dev-odd", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp_id, "dev-odd", "exit", caracas_epoch(anchor, 12, 0)).await;
    seed_event(&db, &emp_id, "dev-odd", "entry", caracas_epoch(anchor, 14, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("recompute ok");

    let row = read_daily_row(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(
        row.work_minutes, 120,
        "only the closed (09:00,12:00) pair counts as worked time (180min raw \
         - 60min fixed lunch); the trailing unmatched 14:00 entry contributes \
         nothing"
    );

    let codes = anomaly_codes(&state.db, &emp_id, "2026-04-20").await;
    assert!(
        codes.contains(&"MISSING_EXIT".to_string()),
        "the dangling entry must raise MISSING_EXIT so a supervisor reviews it; got {:?}",
        codes
    );
}

/// Several separate blocks of work in one window must be summed, not
/// collapsed into the single span from first entry to last exit. Three
/// clean shifts of exactly 1h each, punch mode so there's no fixed-lunch
/// interference muddying the arithmetic.
///
/// Hand derivation: (09:00,10:00), (11:00,12:00), (13:00,14:00) → 60 + 60 +
/// 60 = 180 raw minutes. Pre-M-03 extremes would have given
/// last_exit(14:00) - first_entry(09:00) = 300 minutes — 2 hours of pure
/// gap time counted as work. `punch` mode with 3 intervals still has
/// `had_mid_shift_break = true` (more than one interval), so no fixed
/// fallback is added on top: work_minutes = 180.
#[tokio::test]
async fn several_disjoint_work_blocks_are_summed_not_collapsed() {
    let db = common::test_db().await;
    ensure_global_rules(&db).await;
    let dept_id =
        create_test_department_with_shift(&db, "DeptBlocks", "day", false, 480, "09:00", "17:00")
            .await;
    // create_test_department_with_shift hardcodes lunch_mode='fixed'; switch
    // this department to 'punch' with a 0 fixed fallback so the assertion
    // below is a pure test of interval summation, independent of either
    // lunch policy's deduction arithmetic.
    {
        let conn = db.connect().expect("connect");
        conn.execute(
            "UPDATE departments SET lunch_mode = 'punch', lunch_duration_min = 0 WHERE id = ?1",
            params![dept_id.clone()],
        )
        .await
        .expect("switch to punch mode");
    }
    let emp_id = seed_employee(&db, &dept_id, "E-BLOCKS").await;
    seed_device(&db, "dev-blocks").await;

    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(); // Monday
    seed_event(&db, &emp_id, "dev-blocks", "entry", caracas_epoch(anchor, 9, 0)).await;
    seed_event(&db, &emp_id, "dev-blocks", "exit", caracas_epoch(anchor, 10, 0)).await;
    seed_event(&db, &emp_id, "dev-blocks", "entry", caracas_epoch(anchor, 11, 0)).await;
    seed_event(&db, &emp_id, "dev-blocks", "exit", caracas_epoch(anchor, 12, 0)).await;
    seed_event(&db, &emp_id, "dev-blocks", "entry", caracas_epoch(anchor, 13, 0)).await;
    seed_event(&db, &emp_id, "dev-blocks", "exit", caracas_epoch(anchor, 14, 0)).await;

    let (state, _tmp) = make_state(db);
    dr_service::recompute_for_day(&state, &emp_id, anchor)
        .await
        .expect("recompute ok");

    let row = read_daily_row(&state.db, &emp_id, "2026-04-20").await;
    assert_eq!(
        row.work_minutes, 180,
        "three 1h blocks sum to 180min; extremes (last exit - first entry) \
         would have given 300min, counting two 1h gaps as work"
    );
}
