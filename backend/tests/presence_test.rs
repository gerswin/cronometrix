//! GET /api/v1/presence/today — dos métricas separadas (dentro ahora vs
//! asistieron hoy) y el déficit del día, con scope por departamento (H-11).

mod common;

use chrono::NaiveDate;
use cronometrix_api::auth::scope::ActorScope;
use cronometrix_api::presence::service;

/// Siembra dos departamentos con jornada de 480 min y tres empleados:
/// - emp-inside  (dept-A): entró y no ha salido, 210 min trabajados
/// - emp-left    (dept-A): entró y salió, 480 min trabajados
/// - emp-other   (dept-B): entró y no ha salido, 480 min trabajados
async fn seed(conn: &libsql::Connection, date: &str) {
    for (id, name) in [("dept-A", "Producción"), ("dept-B", "Administración")] {
        conn.execute(
            "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, \
             shift_end_time, lunch_mode, lunch_duration_min, ordinary_daily_minutes, \
             status, version, created_at, updated_at) \
             VALUES (?1, ?2, 100000, '08:00', '17:00', 'fixed', 60, 480, 'active', 1, \
             unixepoch(), unixepoch())",
            (id, name),
        )
        .await
        .unwrap();
    }

    for (id, name, dept) in [
        ("emp-inside", "Ana Pérez", "dept-A"),
        ("emp-left", "Luis García", "dept-A"),
        ("emp-other", "María López", "dept-B"),
    ] {
        conn.execute(
            "INSERT INTO employees (id, employee_code, name, department_id, status, \
             version, base_salary_cents, salary_kind, created_at, updated_at) \
             VALUES (?1, ?1, ?2, ?3, 'active', 1, 100000, 'monthly', unixepoch(), unixepoch())",
            (id, name, dept),
        )
        .await
        .unwrap();
    }

    // (record_id, employee_id, dept, work_minutes, entry_at, exit_at)
    let rows: [(&str, &str, &str, i64, i64, Option<i64>); 3] = [
        ("rec-1", "emp-inside", "dept-A", 210, 1_786_000_000, None),
        ("rec-2", "emp-left", "dept-A", 480, 1_786_000_000, Some(1_786_030_000)),
        ("rec-3", "emp-other", "dept-B", 480, 1_786_000_000, None),
    ];
    for (rid, eid, dept, work, entry, exit) in rows {
        conn.execute(
            "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, \
             shift_type, work_minutes, overtime_minutes, late_minutes, \
             early_departure_minutes, is_rest_day_worked, entry_at, exit_at, \
             computed_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'day', ?5, 0, 0, 0, 0, ?6, ?7, unixepoch(), \
             unixepoch(), unixepoch())",
            (rid, eid, dept, date, work, entry, exit),
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn separates_inside_now_from_attended_today_and_computes_deficit() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "2026-08-05").await; // miércoles

    let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
    let result = service::today(&conn, date, &ActorScope::Unscoped)
        .await
        .unwrap();

    // Dos dentro (sin exit_at), tres asistieron.
    assert_eq!(result.inside_now, 2);
    assert_eq!(result.attended_today, 3);
    assert_eq!(result.date, "2026-08-05");

    let ana = result
        .data
        .iter()
        .find(|r| r.employee_id == "emp-inside")
        .expect("emp-inside presente");
    assert_eq!(ana.status, "inside");
    assert_eq!(ana.employee_name, "Ana Pérez");
    assert_eq!(ana.department_name, "Producción");
    assert_eq!(ana.expected_min, 480);
    assert_eq!(ana.worked_min, 210);
    assert_eq!(ana.deficit_min, 270);
    assert!(ana.exit_at.is_none());

    let luis = result
        .data
        .iter()
        .find(|r| r.employee_id == "emp-left")
        .expect("emp-left presente");
    assert_eq!(luis.status, "left");
    assert_eq!(luis.deficit_min, 0);
    assert!(luis.exit_at.is_some());
}

#[tokio::test]
async fn a_scoped_actor_only_sees_its_own_department() {
    // H-11: deny-by-default. Sin este filtro un supervisor vería toda la empresa.
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "2026-08-05").await;

    let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
    let scope = ActorScope::Department("dept-A".into());
    let result = service::today(&conn, date, &scope).await.unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.inside_now, 1);
    assert_eq!(result.attended_today, 2);
    assert!(result.data.iter().all(|r| r.employee_id != "emp-other"));
}

#[tokio::test]
async fn a_leave_day_expects_nothing_so_it_has_no_deficit() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "2026-08-05").await;

    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, \
         version, created_at, updated_at) VALUES ('u1', 'u1', 'U', 'x', 'admin', \
         'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, \
         justification, created_by, status, version, created_at, updated_at) \
         VALUES ('lv-1', 'emp-inside', '2026-08-05', '2026-08-05', 'vacation', \
         'Vacaciones', 'u1', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "UPDATE daily_records SET leave_id = 'lv-1' WHERE id = 'rec-1'",
        (),
    )
    .await
    .unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
    let result = service::today(&conn, date, &ActorScope::Unscoped)
        .await
        .unwrap();

    let ana = result
        .data
        .iter()
        .find(|r| r.employee_id == "emp-inside")
        .unwrap();
    assert_eq!(ana.expected_min, 0);
    assert_eq!(ana.deficit_min, 0);
}

#[tokio::test]
async fn a_day_with_no_records_returns_empty_counters() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn, "2026-08-05").await;

    let date = NaiveDate::from_ymd_opt(2026, 8, 6).unwrap();
    let result = service::today(&conn, date, &ActorScope::Unscoped)
        .await
        .unwrap();

    assert_eq!(result.inside_now, 0);
    assert_eq!(result.attended_today, 0);
    assert!(result.data.is_empty());
}
