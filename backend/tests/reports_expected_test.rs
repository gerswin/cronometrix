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

/// Regresión Critical 1 (code review de Task 3): un `daily_record` con
/// `work_minutes = 0` es el caso real de `MissingEntry`/`MissingExit`
/// (`calc/engine.rs:60-68`) — el motor SÍ escribe una fila ese día, solo que
/// con 0 minutos. Antes del fix, esa fila pasaba por la rama de "día
/// trabajado" (sumando 480 de déficit) Y, como `worked_dates` solo se puebla
/// cuando `effective_work_min > 0`, el mismo día también caía en el filtro de
/// "ausente" del bloque de días 5 (otros 480 más) — doble conteo: 1620 en vez
/// de 1140. `worked_by_date` es ahora la única fuente que consulta el bucle
/// de `expected_min`/`deficit_min`, así que un día con fila-pero-0-minutos
/// aporta su déficit UNA sola vez.
#[tokio::test]
async fn a_zero_minute_record_is_not_double_counted_as_deficit_and_absence() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    // Jueves 08-06: fila real con 0 minutos (MissingEntry/MissingExit), NO
    // una ausencia sin fila. Viernes 08-07 sigue siendo una ausencia real
    // (sin fila alguna), tal como en `seed()`.
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, \
         shift_type, work_minutes, overtime_minutes, late_minutes, \
         early_departure_minutes, is_rest_day_worked, computed_at, created_at, updated_at) \
         VALUES ('rec-4', 'emp-1', 'dept-A', '2026-08-06', 'day', 0, 0, 0, 0, 0, \
         unixepoch(), unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();
    let row = &payload.rows[0];

    assert_eq!(row.aggregates.expected_min, 2400);
    // work_min sin cambios: la fila de 0 minutos no aporta minutos trabajados.
    assert_eq!(row.aggregates.work_min, 1260);
    // Jueves (0 min, con fila) aporta 480 UNA vez, no dos: 1140, exactamente
    // el mismo total que el test base — sin la fila de jueves habría sido
    // una ausencia sin fila y hubiera aportado los mismos 480 por ese camino.
    assert_eq!(row.aggregates.deficit_min, 1140);
}

/// Regresión Critical 2 (code review de Task 3): `expected_min` ya filtraba
/// por `hire_date`/`terminated_on`, pero `deficit_min` se calculaba aparte
/// sin ese filtro — un registro anterior a la contratación (dato espurio o
/// de prueba) inflaba el déficit por un día que ni siquiera cuenta como
/// esperado. Ahora ambos campos recorren la MISMA ventana de empleo en el
/// mismo bucle, así que un día fuera de ventana no puede contribuir a
/// ninguno de los dos.
#[tokio::test]
async fn deficit_respects_the_employment_window_like_expected_does() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, \
         shift_end_time, lunch_mode, lunch_duration_min, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES ('dept-B', 'Producción', 100000, '08:00', '17:00', 'fixed', 60, 480, \
         'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    // Contratada el martes 08-04: el lunes 08-03 queda fuera de la ventana de
    // empleo, aunque exista (espuriamente) una fila de daily_record ese día.
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, \
         base_salary_cents, salary_kind, hire_date, created_at, updated_at) \
         VALUES ('emp-2', 'EMP002', 'Luis Gómez', 'dept-B', 'active', 1, 100000, \
         'monthly', unixepoch('2026-08-04'), unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    for (id, date, work) in [
        ("rec-b1", "2026-08-03", 300), // fuera de ventana — no debe contar
        ("rec-b2", "2026-08-04", 480), // dentro de ventana, completo
        ("rec-b3", "2026-08-05", 480), // dentro de ventana, completo
                                       // 08-06 y 08-07 ausentes, ambos dentro de la ventana de empleo.
    ] {
        conn.execute(
            "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, \
             shift_type, work_minutes, overtime_minutes, late_minutes, \
             early_departure_minutes, is_rest_day_worked, computed_at, created_at, updated_at) \
             VALUES (?1, 'emp-2', 'dept-B', ?2, 'day', ?3, 0, 0, 0, 0, unixepoch(), \
             unixepoch(), unixepoch())",
            (id, date, work),
        )
        .await
        .unwrap();
    }
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();
    let row = &payload.rows[0];

    // 4 días hábiles dentro de la ventana (mar-vie) × 480; el lunes fuera de
    // ventana no cuenta como esperado.
    assert_eq!(row.aggregates.expected_min, 1920);
    // work_min SÍ incluye el lunes espurio — es un total crudo, no filtrado
    // por ventana; solo expected_min/deficit_min lo están.
    assert_eq!(row.aggregates.work_min, 1260);
    // martes y miércoles completos (0 + 0), jueves y viernes ausentes dentro
    // de ventana (480 + 480) = 960. El lunes fuera de ventana NUNCA se visita
    // en este bucle, así que sus 300 min ni reducen ni inflan el déficit.
    assert_eq!(row.aggregates.deficit_min, 960);
}

/// Regresión Important 1 (code review de Task 3): `has_leave` estaba cableado
/// a `false` en la rama de día trabajado del bucle principal, que solo ve
/// permisos adjuntos como overlay a un `daily_record`. Un permiso que existe
/// SOLO en la tabla `leaves` (agregación W-5, sin `daily_record` ese día) se
/// escribe en `entry.leave_dates` en un bucle posterior — el bucle de
/// `expected_min`/`deficit_min` corre después de AMBAS fuentes y filtra por
/// `leave_dates` sin importar cuál de las dos lo pobló.
#[tokio::test]
async fn deficit_excludes_a_leave_day_that_only_exists_in_the_leaves_table() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed(&conn).await;
    // Un usuario para satisfacer el FK created_by de `leaves`.
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, \
         created_at, updated_at) VALUES ('user-1', 'admin1', 'Admin Uno', 'hash', 'admin', \
         'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    // Permiso 'manual' el jueves 08-06 — SIN daily_record ese día (W-5: solo
    // vive en `leaves`). Viernes 08-07 sigue ausente de verdad.
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, justification, \
         evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES ('leave-1', 'emp-1', '2026-08-06', '2026-08-06', 'manual', 'test', NULL, \
         'user-1', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    let (state, _tmp) = make_state(db);

    let payload = service::compute_report(&state, &params()).await.unwrap();
    let row = &payload.rows[0];

    // 4 días esperables (jueves excluido por permiso) × 480.
    assert_eq!(row.aggregates.expected_min, 1920);
    assert_eq!(row.aggregates.work_min, 1260);
    // Lunes 0 + martes 180 + miércoles 0 + viernes ausente 480 = 660. El
    // jueves con permiso no aporta déficit (queda fuera de la iteración).
    assert_eq!(row.aggregates.deficit_min, 660);
}
