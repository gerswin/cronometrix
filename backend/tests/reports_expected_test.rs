//! `/reports`: horas esperadas y déficit por empleado, subtotal y gran total.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::reports::models::ReportParamsRequest;
use cronometrix_api::reports::service;
use cronometrix_api::state::AppState;

use common::{test_device_creds_key, TEST_JWT_SECRET};

/// Build (AppState, TempDir) for report tests, mirroring `reports_test.rs`'s
/// `make_state`: AppState requires `paths` populated to match the production
/// shape even though this test never touches a path root.
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

/// Un departamento de 480 min/día y un empleado que trabaja 3 de los 5 días
/// laborables de la semana del 2026-08-03 (lun) al 2026-08-07 (vie):
/// lunes 480, martes 300, miércoles 480. Jueves y viernes ausente.
async fn seed(conn: &libsql::Connection) {
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, \
         shift_end_time, lunch_mode, lunch_duration_min, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES ('dept-A', 'Producción', 100000, '08:00', '17:00', 'fixed', 60, 480, \
         'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, \
         base_salary_cents, salary_kind, hire_date, created_at, updated_at) \
         VALUES ('emp-1', 'EMP001', 'Ana Pérez', 'dept-A', 'active', 1, 100000, \
         'monthly', NULL, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();

    for (id, date, work) in [
        ("rec-1", "2026-08-03", 480),
        ("rec-2", "2026-08-04", 300),
        ("rec-3", "2026-08-05", 480),
    ] {
        conn.execute(
            "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, \
             shift_type, work_minutes, overtime_minutes, late_minutes, \
             early_departure_minutes, is_rest_day_worked, computed_at, created_at, updated_at) \
             VALUES (?1, 'emp-1', 'dept-A', ?2, 'day', ?3, 0, 0, 0, 0, unixepoch(), \
             unixepoch(), unixepoch())",
            (id, date, work),
        )
        .await
        .unwrap();
    }
}

fn params() -> ReportParamsRequest {
    ReportParamsRequest {
        from_date: "2026-08-03".into(),
        to_date: "2026-08-07".into(),
        period_type: "custom".into(),
        department_ids: None,
        include_inactive: None,
        employee_id: None,
        shift_type: None,
    }
}

#[tokio::test]
async fn expected_covers_every_weekday_including_absent_ones() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();
    let row = &payload.rows[0];

    // 5 días laborables × 480, incluidos los dos días sin registro alguno.
    assert_eq!(row.aggregates.expected_min, 2400);
    assert_eq!(row.aggregates.work_min, 1260);
    // 2400 − 1260: los 180 min del martes corto más los dos días ausentes.
    assert_eq!(row.aggregates.deficit_min, 1140);
}

#[tokio::test]
async fn subtotal_and_grand_total_carry_the_new_fields() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();

    assert_eq!(payload.dept_subtotals[0].aggregates.expected_min, 2400);
    assert_eq!(payload.dept_subtotals[0].aggregates.deficit_min, 1140);
    assert_eq!(payload.grand_total.expected_min, 2400);
    assert_eq!(payload.grand_total.deficit_min, 1140);
}

#[tokio::test]
async fn overtime_does_not_offset_a_short_day() {
    // Un día de 600 min no compensa el martes de 300: el déficit no se netea.
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    conn.execute(
        "UPDATE daily_records SET work_minutes = 600 WHERE id = 'rec-1'",
        (),
    )
    .await
    .unwrap();
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();
    // La esperada no cambia; el déficit tampoco baja por las extras del lunes.
    assert_eq!(payload.rows[0].aggregates.expected_min, 2400);
    assert_eq!(payload.rows[0].aggregates.deficit_min, 1140);
}
