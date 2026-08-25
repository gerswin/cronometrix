# Presencia en vivo y déficit de horas — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el dashboard muestre quién está dentro ahora y quién asistió hoy, y que tanto el dashboard como `/reports` muestren, por empleado, cuántos minutos de jornada quedaron sin cumplir.

**Architecture:** Una función pura en `backend/src/calc/expected.rs` define la jornada esperada y el déficit. La consumen dos superficies: un endpoint nuevo `GET /api/v1/presence/today` y dos campos nuevos en `Aggregates` de `/reports`. El frontend solo renderiza lo que el backend calcula.

**Tech Stack:** Rust + Axum 0.8 + libSQL (backend); Next.js 15 + React 19 + TanStack Query v5 + Tailwind 4 (frontend); Vitest + Playwright (pruebas frontend); `cargo nextest` (pruebas backend).

Spec: `docs/superpowers/specs/2026-08-05-presencia-y-deficit-de-horas-design.md`

## Global Constraints

- La jornada esperada es `departments.ordinary_daily_minutes`. **Nunca** derivarla de `shift_start_time`/`shift_end_time` ni restarle el almuerzo: ese campo ya es la referencia del motor de extras (`calc/engine.rs:134`) y de `money.rs`.
- Sábado y domingo esperan 0. Un día con permiso activo espera 0.
- El déficit nunca es negativo: `max(0, esperada − trabajada)`.
- `ordinary_daily_minutes <= 0` → esperada 0 (misma guarda que `money.rs:47`).
- Toda lectura nueva aplica `ActorScope` (H-11). Una consulta de lista sin predicado de departamento para un actor `Department(_)` es un bug, no un default.
- Rutas de lectura para cualquier rol autenticado van en `viewer_routes` (D-09).
- Los `Paths` y estado de test se obtienen con `common::test_state_with_tmpdir(db, config)`; el `TempDir` devuelto debe vivir hasta el final del test (convención de `CLAUDE.md`).
- Timestamps hacia el exterior en ISO 8601 (D-13). Minutos como enteros, nunca floats.
- Un commit por tarea, con los tests de esa tarea en verde.

---

### Task 1: Función pura de jornada esperada y déficit

**Files:**
- Create: `backend/src/calc/expected.rs`
- Modify: `backend/src/calc/mod.rs`

**Interfaces:**
- Consumes: nada.
- Produces: `pub fn expected_minutes(ordinary_daily_minutes: i64, date: chrono::NaiveDate, has_leave: bool) -> i64` y `pub fn deficit_minutes(expected: i64, worked: i64) -> i64`. Las tareas 2 y 4 las llaman.

- [ ] **Step 1: Write the failing test**

Al final de `backend/src/calc/expected.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn a_weekday_expects_the_departments_ordinary_day() {
        // 2026-08-05 es miércoles.
        assert_eq!(expected_minutes(480, d("2026-08-05"), false), 480);
    }

    #[test]
    fn weekends_expect_nothing() {
        // 2026-08-08 sábado, 2026-08-09 domingo.
        assert_eq!(expected_minutes(480, d("2026-08-08"), false), 0);
        assert_eq!(expected_minutes(480, d("2026-08-09"), false), 0);
    }

    #[test]
    fn a_leave_day_expects_nothing() {
        // El déficit mide incumplimiento real, no vacaciones.
        assert_eq!(expected_minutes(480, d("2026-08-05"), true), 0);
    }

    #[test]
    fn a_non_positive_ordinary_day_expects_nothing() {
        // Configuración inválida no debe producir déficit negativo (money.rs:47).
        assert_eq!(expected_minutes(0, d("2026-08-05"), false), 0);
        assert_eq!(expected_minutes(-60, d("2026-08-05"), false), 0);
    }

    #[test]
    fn deficit_is_the_shortfall_and_never_negative() {
        assert_eq!(deficit_minutes(480, 210), 270);
        assert_eq!(deficit_minutes(480, 480), 0);
        // Trabajar de más no compensa: las extras tienen su propia columna.
        assert_eq!(deficit_minutes(480, 600), 0);
        assert_eq!(deficit_minutes(0, 0), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend && cargo test --lib calc::expected`
Expected: FAIL — el módulo no existe (`file not found for module 'expected'` o `cannot find function 'expected_minutes'`).

- [ ] **Step 3: Write minimal implementation**

Cabecera y cuerpo de `backend/src/calc/expected.rs`, encima del bloque `mod tests`:

```rust
//! Jornada esperada y déficit de horas.
//!
//! La esperada es `departments.ordinary_daily_minutes` — el mismo número contra
//! el que `engine::compute_daily_record` decide qué es hora extra
//! (`calc/engine.rs:134`) y contra el que `money.rs` convierte el sueldo a día
//! ordinario. Derivarla de `shift_start_time`/`shift_end_time` menos el almuerzo
//! crearía una segunda definición de jornada capaz de contradecir a la primera.
//!
//! El almuerzo no entra aquí: ya está descontado de `work_minutes` (en `fixed`
//! por `calc/lunch.rs`, en `punch` por el pareo de intervalos de
//! `calc/aggregation.rs`), y restarlo otra vez lo contaría dos veces.

use chrono::{Datelike, NaiveDate, Weekday};

/// Minutos que el empleado debía trabajar ese día. Devuelve 0 en fin de semana,
/// en días con permiso activo, y ante una jornada ordinaria no positiva.
pub fn expected_minutes(ordinary_daily_minutes: i64, date: NaiveDate, has_leave: bool) -> i64 {
    if has_leave || ordinary_daily_minutes <= 0 {
        return 0;
    }
    match date.weekday() {
        Weekday::Sat | Weekday::Sun => 0,
        _ => ordinary_daily_minutes,
    }
}

/// Minutos de jornada incumplidos. Nunca negativo: trabajar de más no compensa
/// un día corto.
pub fn deficit_minutes(expected: i64, worked: i64) -> i64 {
    (expected - worked).max(0)
}
```

Y registrar el módulo en `backend/src/calc/mod.rs`, junto a las declaraciones existentes:

```rust
pub mod expected;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd backend && cargo test --lib calc::expected`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add backend/src/calc/expected.rs backend/src/calc/mod.rs
git commit -m "feat(calc): jornada esperada y déficit de horas como función pura"
```

---

### Task 2: Endpoint `GET /api/v1/presence/today`

**Files:**
- Create: `backend/src/presence/mod.rs`, `backend/src/presence/models.rs`, `backend/src/presence/service.rs`, `backend/src/presence/handlers.rs`
- Modify: `backend/src/lib.rs` (declarar `pub mod presence;`), `backend/src/main.rs:325` (añadir la ruta a `viewer_routes`)
- Test: `backend/tests/presence_test.rs`

**Interfaces:**
- Consumes: `calc::expected::{expected_minutes, deficit_minutes}` de la Task 1; `auth::scope::ActorScope`; `common::epoch_to_iso`.
- Produces: `presence::service::today(conn: &libsql::Connection, today: NaiveDate, scope: &ActorScope) -> Result<PresenceToday, AppError>` y los structs `PresenceToday` / `PresenceRow`. La Task 5 consume su forma JSON.

- [ ] **Step 1: Write the failing test**

`backend/tests/presence_test.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend && cargo test --test presence_test`
Expected: FAIL — `unresolved import cronometrix_api::presence`.

- [ ] **Step 3: Write minimal implementation**

`backend/src/presence/mod.rs`:

```rust
//! Presencia del día: quién está dentro ahora y quién asistió hoy.

pub mod handlers;
pub mod models;
pub mod service;
```

`backend/src/presence/models.rs`:

```rust
use serde::Serialize;

/// Una fila por empleado con registro del día.
#[derive(Debug, Serialize)]
pub struct PresenceRow {
    pub employee_id: String,
    pub employee_name: String,
    pub department_name: String,
    /// "inside" = entró y no ha salido; "left" = ya marcó salida.
    pub status: String,
    pub entry_at: Option<String>, // ISO 8601 (D-13)
    pub exit_at: Option<String>,  // ISO 8601
    pub expected_min: i64,
    pub worked_min: i64,
    pub deficit_min: i64,
}

#[derive(Debug, Serialize)]
pub struct PresenceToday {
    pub date: String, // YYYY-MM-DD
    pub inside_now: i64,
    pub attended_today: i64,
    pub data: Vec<PresenceRow>,
}
```

`backend/src/presence/service.rs`:

```rust
use chrono::NaiveDate;

use crate::auth::scope::ActorScope;
use crate::calc::expected::{deficit_minutes, expected_minutes};
use crate::common::epoch_to_iso;
use crate::errors::AppError;

use super::models::{PresenceRow, PresenceToday};

/// Presencia del día `today`, confinada al departamento del actor cuando
/// aplica (H-11). Una sola consulta con joins; nada de N+1.
pub async fn today(
    conn: &libsql::Connection,
    today: NaiveDate,
    scope: &ActorScope,
) -> Result<PresenceToday, AppError> {
    let date_str = today.format("%Y-%m-%d").to_string();

    let mut sql = String::from(
        "SELECT e.id, e.name, d.name, d.ordinary_daily_minutes, \
                dr.work_minutes, dr.entry_at, dr.exit_at, dr.leave_id \
         FROM daily_records dr \
         JOIN employees e ON e.id = dr.employee_id \
         JOIN departments d ON d.id = dr.department_id \
         WHERE dr.anchor_date = ?1 AND dr.entry_at IS NOT NULL \
           AND e.deleted_at IS NULL",
    );
    let mut values: Vec<libsql::Value> = vec![libsql::Value::Text(date_str.clone())];
    if let Some(dept) = scope.department_id() {
        sql.push_str(" AND dr.department_id = ?2");
        values.push(libsql::Value::Text(dept.to_string()));
    }
    sql.push_str(" ORDER BY e.name");

    let mut rows_iter = conn
        .query(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data: Vec<PresenceRow> = Vec::new();
    let mut inside_now = 0i64;

    while let Some(row) = rows_iter
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let employee_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let employee_name: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let department_name: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
        let ordinary: i64 = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
        let worked_min: i64 = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
        let entry_epoch: Option<i64> = row.get(5).ok();
        let exit_epoch: Option<i64> = row.get(6).ok();
        let leave_id: Option<String> = row.get(7).ok();

        let expected_min = expected_minutes(ordinary, today, leave_id.is_some());
        let status = if exit_epoch.is_none() {
            inside_now += 1;
            "inside"
        } else {
            "left"
        };

        data.push(PresenceRow {
            employee_id,
            employee_name,
            department_name,
            status: status.to_string(),
            entry_at: entry_epoch.map(epoch_to_iso),
            exit_at: exit_epoch.map(epoch_to_iso),
            expected_min,
            worked_min,
            deficit_min: deficit_minutes(expected_min, worked_min),
        });
    }

    Ok(PresenceToday {
        date: date_str,
        inside_now,
        attended_today: data.len() as i64,
        data,
    })
}
```

`backend/src/presence/handlers.rs`:

```rust
use axum::{extract::State, Json};
use chrono::Utc;

use crate::auth::rbac::AuthUser;
use crate::auth::scope::ActorScope;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::PresenceToday;
use super::service;

/// GET /api/v1/presence/today — viewer-or-above (D-09), con scope H-11.
pub async fn presence_today(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PresenceToday>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let scope = ActorScope::from_claims(&claims);
    let today = Utc::now().date_naive();
    Ok(Json(service::today(&conn, today, &scope).await?))
}
```

En `backend/src/lib.rs`, junto a los demás módulos: `pub mod presence;`

En `backend/src/main.rs`, dentro de `viewer_routes` (después de la ruta `/daily-records`):

```rust
        .route("/presence/today", get(presence::handlers::presence_today))
```

y añadir `presence` a la lista de `use cronometrix_api::{...}` del binario si el import es explícito.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd backend && cargo test --test presence_test`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add backend/src/presence backend/src/lib.rs backend/src/main.rs backend/tests/presence_test.rs
git commit -m "feat(presence): endpoint GET /presence/today con scope por departamento"
```

---

### Task 3: `expected_min` y `deficit_min` en los agregados de `/reports`

**Files:**
- Modify: `backend/src/reports/models.rs:73-84` (struct `Aggregates`), `backend/src/reports/service.rs:80-102` (struct `AccRow`), `backend/src/reports/service.rs:819-825` (bucle de días ausentes), `backend/src/reports/service.rs:890` (fn `accumulate`)
- Test: `backend/tests/reports_expected_test.rs`

**Interfaces:**
- Consumes: `calc::expected::{expected_minutes, deficit_minutes}` de la Task 1.
- Produces: campos `expected_min: i64` y `deficit_min: i64` en `Aggregates`, presentes en filas por empleado, subtotales por departamento y gran total. Las Tasks 4 y 7 los consumen.

Contexto para quien implementa: el `SELECT` de reportes ya trae `d.ordinary_daily_minutes` (`service.rs:323`), y ya existe `weekdays_in_period` junto con `entry.worked_dates`, `entry.leave_dates`, `entry.hire_date` y `entry.terminated_on` — el bloque que calcula `days_absent` en la línea 819 usa exactamente esos filtros. La esperada del periodo se calcula ahí mismo, no fila a fila: un día ausente no tiene fila en el `LEFT JOIN` y aun así debe esperar jornada.

- [ ] **Step 1: Write the failing test**

`backend/tests/reports_expected_test.rs`:

```rust
//! `/reports`: horas esperadas y déficit por empleado, subtotal y gran total.

mod common;

use cronometrix_api::reports::models::ReportParamsRequest;
use cronometrix_api::reports::service;

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

    let payload = service::generate(&conn, &params()).await.unwrap();
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

    let payload = service::generate(&conn, &params()).await.unwrap();

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

    let payload = service::generate(&conn, &params()).await.unwrap();
    // La esperada no cambia; el déficit tampoco baja por las extras del lunes.
    assert_eq!(payload.rows[0].aggregates.expected_min, 2400);
    assert_eq!(payload.rows[0].aggregates.deficit_min, 1140);
}
```

Nota para quien implementa: si la firma real de `service::generate` difiere (por ejemplo recibe `&AppState` o parámetros extra), ajusta la llamada mirando `backend/src/reports/handlers.rs`; el resto del test no cambia.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend && cargo test --test reports_expected_test`
Expected: FAIL — `no field 'expected_min' on type 'Aggregates'`.

- [ ] **Step 3: Write minimal implementation**

En `Aggregates` (`backend/src/reports/models.rs:73`), junto a `work_min`:

```rust
    /// Minutos que el empleado debía trabajar en el periodo (calc/expected.rs).
    pub expected_min: i64,
    /// Minutos de jornada incumplidos. Nunca negativo; las extras no lo netean.
    pub deficit_min: i64,
```

En `AccRow` (`service.rs:80`), un campo nuevo:

```rust
    /// Jornada ordinaria del departamento, para la esperada del periodo.
    ordinary_daily_minutes: i64,
```

Poblarlo donde se construye `AccRow` — la columna `d.ordinary_daily_minutes` ya viene en el `SELECT`; leer su índice con `row.get(...)` igual que los campos vecinos y pasarlo al constructor (y `0` en el `Default`/fallback si existe).

En el bucle de la línea 819, junto al cálculo de `absent`:

```rust
        let expected_days = weekdays_in_period
            .iter()
            .filter(|d| entry.hire_date.is_none_or(|h| **d >= h))
            .filter(|d| entry.terminated_on.is_none_or(|t| **d <= t))
            .filter(|d| !entry.leave_dates.contains(d))
            .count() as i64;
        // expected_minutes ya devuelve 0 en fin de semana; weekdays_in_period
        // solo trae días hábiles, así que basta multiplicar.
        entry.agg.expected_min = expected_days
            .saturating_mul(entry.ordinary_daily_minutes.max(0));
        entry.agg.deficit_min =
            crate::calc::expected::deficit_minutes(entry.agg.expected_min, entry.agg.work_min);
```

En `accumulate` (`service.rs:890`):

```rust
    into.expected_min = into.expected_min.saturating_add(from.expected_min);
    into.deficit_min = into.deficit_min.saturating_add(from.deficit_min);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd backend && cargo test --test reports_expected_test && cargo test --test reports_scope_test`
Expected: PASS ambos — el segundo confirma que no se rompió el reporte existente.

- [ ] **Step 5: Commit**

```bash
git add backend/src/reports/models.rs backend/src/reports/service.rs backend/tests/reports_expected_test.rs
git commit -m "feat(reports): horas esperadas y déficit por empleado, subtotal y total"
```

---

### Task 4: Columnas nuevas en el Excel

**Files:**
- Modify: `backend/src/reports/excel.rs:38` (`N_COLS`), `:133-152` (cabeceras), `write_employee_row`, `write_aggregate_row`
- Test: `backend/tests/reports_excel_test.rs` (extender el existente — **no** crear archivo nuevo)

**Interfaces:**
- Consumes: `Aggregates.expected_min` y `Aggregates.deficit_min` de la Task 3.
- Produces: nada que consuman otras tareas.

Las dos columnas van **al final** (índices 19 y 20), no intercaladas: mover "Min Trab" de sitio rompería todos los índices ya escritos, y `reports_excel_test.rs:563` lee los códigos de anomalía en la columna 18 por posición.

`backend/tests/reports_excel_test.rs` ya afirma la fila de cabeceras completa (línea 396, array `expected` de 19 entradas) y trae los helpers `parse_xlsx` (línea 121) y `cell_string` (línea 129) con `calamine`. Extender ese archivo: añadir las dos entradas al array existente y un test nuevo que verifique los valores.

- [ ] **Step 1: Write the failing test**

Primero, añadir al array `expected` de la línea 396, después de `"Anomalías"`:

```rust
        "Min Esperados",
        "Min Déficit",
```

Luego, un test nuevo al final de `backend/tests/reports_excel_test.rs`, siguiendo el patrón de los que ya existen (leer uno completo para copiar cómo siembra datos y hace la petición HTTP — usan `axum_test` y devuelven `(status, bytes)`):

```rust
// -----------------------------------------------------------------------------
// Horas esperadas y déficit (columnas 19-20)
// -----------------------------------------------------------------------------

#[tokio::test]
async fn expected_and_deficit_columns_carry_their_values() {
    // Reutilizar el mismo fixture de siembra que usan los tests de arriba: un
    // empleado con jornada ordinaria de 480 min que trabaja menos de lo debido.
    let (status, bytes) = /* misma llamada que los tests vecinos */;
    assert_eq!(status, StatusCode::OK);

    let range = parse_xlsx(bytes);
    let (n_rows, _) = range.get_size();

    let mut found = false;
    for r in 5..(n_rows as u32) {
        if cell_string(&range, r, 1) == /* nombre del empleado sembrado */ {
            let expected_min = cell_string(&range, r, 19);
            let deficit_min = cell_string(&range, r, 20);
            assert_ne!(expected_min, "", "columna Min Esperados vacía");
            // La jornada esperada debe superar a la trabajada en este fixture.
            let expected: f64 = expected_min.parse().expect("Min Esperados numérico");
            let worked: f64 = cell_string(&range, r, 4).parse().expect("Min Trab numérico");
            let deficit: f64 = deficit_min.parse().expect("Min Déficit numérico");
            assert!(expected > worked, "el fixture debe tener déficit");
            assert_eq!(deficit, expected - worked, "déficit = esperadas − trabajadas");
            found = true;
            break;
        }
    }
    assert!(found, "no se encontró la fila del empleado sembrado");
}
```

Rellenar los dos comentarios con el fixture real del archivo. Si ningún fixture existente produce déficit, ajustar sus `work_minutes` **en un fixture nuevo propio de este test**, nunca modificando el que usan los demás.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend && cargo test --test reports_excel_test`
Expected: FAIL — el test de cabeceras falla en la columna 19 (`column 19 mismatch: got ""`), porque `N_COLS` sigue en 19 y esas columnas no se escriben.

- [ ] **Step 3: Write minimal implementation**

En `excel.rs:38`:

```rust
const N_COLS: u16 = 21;
```

En el array `cols` (`:133`), añadir al final, después de `"Anomalías"`:

```rust
        "Min Esperados",
        "Min Déficit",
```

En `write_employee_row`, después de la última escritura existente:

```rust
    sheet
        .write_with_format(row, 19, emp.aggregates.expected_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 20, emp.aggregates.deficit_min as f64, int_fmt)
        .map_err(map_err)?;
```

Y lo mismo en `write_aggregate_row` con su formato de entero (`subtotal_int` / `grand_int` llegan como parámetro):

```rust
    sheet
        .write_with_format(row, 19, agg.expected_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 20, agg.deficit_min as f64, int_fmt)
        .map_err(map_err)?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd backend && cargo test --test reports_excel_test`
Expected: PASS — el test de cabeceras (ahora con 21 entradas) y el de valores. Los demás tests del archivo deben seguir en verde: el de códigos de anomalía lee la columna 18 y no debe haberse movido.

- [ ] **Step 5: Commit**

```bash
git add backend/src/reports/excel.rs backend/tests/reports_excel_test.rs
git commit -m "feat(reports): columnas Min Esperados y Min Déficit en el Excel"
```

---

### Task 5: Tabla de presencia en el dashboard

**Files:**
- Create: `frontend/src/components/dashboard/presence-table.tsx`, `frontend/src/components/dashboard/__tests__/presence-table.test.tsx`
- Modify: `frontend/src/types/api.ts` (tipos `PresenceRow` y `PresenceToday`), `frontend/src/app/(dashboard)/dashboard/page.tsx` (query nueva, KPI 1 y render)

**Interfaces:**
- Consumes: `GET /presence/today` de la Task 2.
- Produces: componente `<PresenceTable data={...} />`. La Task 6 reutiliza el tipo `PresenceRow`.

- [ ] **Step 1: Write the failing test**

`frontend/src/components/dashboard/__tests__/presence-table.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { PresenceRow } from '@/types/api'
import { PresenceTable } from '../presence-table'

const rows: PresenceRow[] = [
  {
    employee_id: 'e1',
    employee_name: 'Ana Pérez',
    department_name: 'Producción',
    status: 'inside',
    entry_at: '2026-08-05T12:02:00+00:00',
    exit_at: null,
    expected_min: 480,
    worked_min: 210,
    deficit_min: 270,
  },
  {
    employee_id: 'e2',
    employee_name: 'Luis García',
    department_name: 'Producción',
    status: 'left',
    entry_at: '2026-08-05T12:00:00+00:00',
    exit_at: '2026-08-05T21:00:00+00:00',
    expected_min: 480,
    worked_min: 480,
    deficit_min: 0,
  },
]

describe('PresenceTable', () => {
  it('shows only people still inside by default', () => {
    render(<PresenceTable rows={rows} />)
    expect(screen.getByText('Ana Pérez')).toBeInTheDocument()
    expect(screen.queryByText('Luis García')).not.toBeInTheDocument()
  })

  it('switches to everyone who attended today', () => {
    render(<PresenceTable rows={rows} />)
    fireEvent.click(screen.getByTestId('presence-tab-attended'))
    expect(screen.getByText('Ana Pérez')).toBeInTheDocument()
    expect(screen.getByText('Luis García')).toBeInTheDocument()
  })

  it('renders an empty state when nobody is inside', () => {
    render(<PresenceTable rows={[rows[1]]} />)
    expect(screen.getByTestId('presence-empty')).toBeInTheDocument()
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend && npx vitest run src/components/dashboard/__tests__/presence-table.test.tsx`
Expected: FAIL — `Failed to resolve import "../presence-table"`.

- [ ] **Step 3: Write minimal implementation**

En `frontend/src/types/api.ts`:

```ts
export interface PresenceRow {
  employee_id: string
  employee_name: string
  department_name: string
  status: 'inside' | 'left'
  entry_at: string | null
  exit_at: string | null
  expected_min: number
  worked_min: number
  deficit_min: number
}

export interface PresenceToday {
  date: string
  inside_now: number
  attended_today: number
  data: PresenceRow[]
}
```

`frontend/src/components/dashboard/presence-table.tsx`:

```tsx
'use client'
import { useState } from 'react'
import { fmtTime } from '@/lib/format/datetime'
import type { PresenceRow } from '@/types/api'

interface Props {
  rows: PresenceRow[]
}

export function PresenceTable({ rows }: Props) {
  const [tab, setTab] = useState<'inside' | 'attended'>('inside')
  const visible = tab === 'inside' ? rows.filter(r => r.status === 'inside') : rows

  const tabClass = (active: boolean) =>
    `px-3 py-1.5 text-[13px] rounded-md ${
      active ? 'bg-[#1E3FB8] text-white' : 'text-[#666666] hover:bg-[#F5F6F8]'
    }`

  return (
    <div className="bg-white rounded-lg border border-[#EEF0F2]">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-[#EEF0F2]">
        <button
          data-testid="presence-tab-inside"
          className={tabClass(tab === 'inside')}
          onClick={() => setTab('inside')}
        >
          Dentro ahora
        </button>
        <button
          data-testid="presence-tab-attended"
          className={tabClass(tab === 'attended')}
          onClick={() => setTab('attended')}
        >
          Asistieron hoy
        </button>
      </div>

      {visible.length === 0 ? (
        <p data-testid="presence-empty" className="px-4 py-8 text-center text-[13px] text-[#666666]">
          Sin registros
        </p>
      ) : (
        <table className="w-full text-[13px]">
          <thead>
            <tr className="text-left text-[11px] uppercase text-[#666666]">
              <th className="px-4 py-2 font-medium">Empleado</th>
              <th className="px-4 py-2 font-medium">Entrada</th>
              <th className="px-4 py-2 font-medium">Departamento</th>
            </tr>
          </thead>
          <tbody>
            {visible.map(r => (
              <tr key={r.employee_id} className="border-t border-[#EEF0F2]">
                <td className="px-4 py-2 text-[#1A1A1A]">{r.employee_name}</td>
                <td className="px-4 py-2 text-[#666666]">
                  {r.entry_at ? fmtTime(r.entry_at) : '—'}
                </td>
                <td className="px-4 py-2 text-[#666666]">{r.department_name}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
```

En `frontend/src/app/(dashboard)/dashboard/page.tsx`, junto a las demás queries:

```tsx
  const { data: presenceData } = useQuery<PresenceToday>({
    queryKey: ['presence-today', today],
    queryFn: () => api.get('/presence/today').then(r => r.data),
    refetchInterval: 60_000,
  })
```

Cambiar el KPI 1 para que use el contador del backend y añadir el segundo KPI y la tabla. `value={kpis.present}` pasa a `value={presenceData?.inside_now ?? 0}`, con `title="Dentro Ahora"`; y bajo la fila de KPIs:

```tsx
        <PresenceTable rows={presenceData?.data ?? []} />
```

Mantener el `data-testid="kpi-empleados-presentes"` intacto: `dashboard.spec.ts` (T-01) lo asume.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd frontend && npx vitest run src/components/dashboard`
Expected: PASS — 3 tests nuevos y los existentes del dashboard sin romper.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/dashboard/presence-table.tsx frontend/src/components/dashboard/__tests__/presence-table.test.tsx frontend/src/types/api.ts "frontend/src/app/(dashboard)/dashboard/page.tsx"
git commit -m "feat(dashboard): tabla de presencia con dentro ahora y asistieron hoy"
```

---

### Task 6: Panel de déficit del día

**Files:**
- Create: `frontend/src/components/dashboard/deficit-panel.tsx`, `frontend/src/components/dashboard/__tests__/deficit-panel.test.tsx`, `frontend/src/lib/format/minutes.ts`
- Modify: `frontend/src/app/(dashboard)/dashboard/page.tsx`

**Interfaces:**
- Consumes: `PresenceRow[]` de la Task 5.
- Produces: `fmtMinutes(min: number): string` en `lib/format/minutes.ts`. Solo lo usa este panel; la tabla de `/reports` deja los minutos como enteros crudos por coherencia con sus columnas vecinas.

- [ ] **Step 1: Write the failing test**

`frontend/src/components/dashboard/__tests__/deficit-panel.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import type { PresenceRow } from '@/types/api'
import { DeficitPanel } from '../deficit-panel'
import { fmtMinutes } from '@/lib/format/minutes'

const row = (id: string, name: string, deficit: number): PresenceRow => ({
  employee_id: id,
  employee_name: name,
  department_name: 'Producción',
  status: 'left',
  entry_at: '2026-08-05T12:00:00+00:00',
  exit_at: '2026-08-05T18:00:00+00:00',
  expected_min: 480,
  worked_min: 480 - deficit,
  deficit_min: deficit,
})

describe('fmtMinutes', () => {
  it('formats minutes as hours and minutes', () => {
    expect(fmtMinutes(270)).toBe('4h 30m')
    expect(fmtMinutes(60)).toBe('1h')
    expect(fmtMinutes(45)).toBe('45m')
    expect(fmtMinutes(0)).toBe('0m')
  })
})

describe('DeficitPanel', () => {
  it('lists only people with a deficit, worst first', () => {
    render(<DeficitPanel rows={[row('e1', 'Ana', 30), row('e2', 'Luis', 270), row('e3', 'María', 0)]} />)
    const items = screen.getAllByTestId(/deficit-row-/)
    expect(items).toHaveLength(2)
    expect(within(items[0]).getByText('Luis')).toBeInTheDocument()
    expect(within(items[0]).getByText('4h 30m')).toBeInTheDocument()
    expect(within(items[1]).getByText('Ana')).toBeInTheDocument()
  })

  it('renders an empty state when everyone met their hours', () => {
    render(<DeficitPanel rows={[row('e3', 'María', 0)]} />)
    expect(screen.getByTestId('deficit-empty')).toBeInTheDocument()
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend && npx vitest run src/components/dashboard/__tests__/deficit-panel.test.tsx`
Expected: FAIL — `Failed to resolve import "../deficit-panel"`.

- [ ] **Step 3: Write minimal implementation**

`frontend/src/lib/format/minutes.ts`:

```ts
/** Formatea minutos como "4h 30m", "1h", "45m". Nunca negativo. */
export function fmtMinutes(min: number): string {
  const total = Math.max(0, Math.round(min))
  const h = Math.floor(total / 60)
  const m = total % 60
  if (h === 0) return `${m}m`
  if (m === 0) return `${h}h`
  return `${h}h ${m}m`
}
```

`frontend/src/components/dashboard/deficit-panel.tsx`:

```tsx
'use client'
import { fmtMinutes } from '@/lib/format/minutes'
import type { PresenceRow } from '@/types/api'

interface Props {
  rows: PresenceRow[]
}

export function DeficitPanel({ rows }: Props) {
  const short = rows
    .filter(r => r.deficit_min > 0)
    .sort((a, b) => b.deficit_min - a.deficit_min)

  return (
    <div className="bg-white rounded-lg border border-[#EEF0F2]">
      <div className="px-4 py-[14px] border-b border-[#EEF0F2]">
        <span className="text-[15px] font-semibold text-[#1A1A1A]">Jornada incumplida hoy</span>
      </div>

      {short.length === 0 ? (
        <p data-testid="deficit-empty" className="px-4 py-8 text-center text-[13px] text-[#666666]">
          Todos cumplieron su jornada
        </p>
      ) : (
        <ul>
          {short.map(r => (
            <li
              key={r.employee_id}
              data-testid={`deficit-row-${r.employee_id}`}
              className="flex items-center justify-between px-4 py-[10px] border-t border-[#EEF0F2]"
            >
              <span className="text-[14px] text-[#1A1A1A]">{r.employee_name}</span>
              <span className="text-[13px] font-medium text-[#EF4444]">
                {fmtMinutes(r.deficit_min)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
```

En el dashboard, junto a la tabla de presencia:

```tsx
        <DeficitPanel rows={presenceData?.data ?? []} />
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd frontend && npx vitest run src/components/dashboard src/lib/format`
Expected: PASS — 4 tests nuevos.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/dashboard/deficit-panel.tsx frontend/src/components/dashboard/__tests__/deficit-panel.test.tsx frontend/src/lib/format/minutes.ts "frontend/src/app/(dashboard)/dashboard/page.tsx"
git commit -m "feat(dashboard): panel de jornada incumplida del día"
```

---

### Task 7: Columnas de esperadas y déficit en la UI de `/reports`

**Files:**
- Modify: `frontend/src/components/reports/summary-table.tsx:93-125` (definición de columnas), `frontend/src/types/api.ts:188` (interface `Aggregates`)
- Test: `frontend/src/components/reports/__tests__/summary-table.test.tsx`

**Interfaces:**
- Consumes: `expected_min` y `deficit_min` del payload de `/reports/json` (Task 3).
- Produces: nada.

Las columnas van justo después de `Min Trab` para que se lean juntas: trabajadas, esperadas, déficit. Se renderizan como enteros con `String(getValue() ?? 0)`, igual que sus vecinas — `fmtMinutes` es para el panel del dashboard, no para esta tabla, donde el resto de celdas de minutos son números crudos y mezclar formatos descuadraría la lectura.

- [ ] **Step 1: Write the failing test**

Añadir a `frontend/src/components/reports/__tests__/summary-table.test.tsx`, reutilizando el fixture de fila que ya exista en ese archivo (leer sus primeras 40 líneas para el nombre real; si construye filas inline, copiar esa forma y añadir los dos campos):

```tsx
  it('shows expected and deficit minutes next to worked minutes', () => {
    const row = { ...baseRow, work_min: 1260, expected_min: 2400, deficit_min: 1140 }
    render(<SummaryTable rows={[row]} deptSubtotals={[]} grandTotal={row} />)

    expect(screen.getByText('Min Esperados')).toBeInTheDocument()
    expect(screen.getByText('Min Déficit')).toBeInTheDocument()
    expect(screen.getByText('2400')).toBeInTheDocument()
    expect(screen.getByText('1140')).toBeInTheDocument()
  })
```

Ajustar las props de `SummaryTable` a las que el archivo de test ya usa en sus otros casos — la aserción es lo que importa.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend && npx vitest run src/components/reports/__tests__/summary-table.test.tsx`
Expected: FAIL — `Unable to find an element with the text: Min Esperados`.

- [ ] **Step 3: Write minimal implementation**

En `frontend/src/types/api.ts`, dentro de `interface Aggregates` (línea 188), después de `work_min`:

```ts
  expected_min: number
  deficit_min: number
```

En `frontend/src/components/reports/summary-table.tsx`, en el array `columns`, inmediatamente después del bloque de `work_min`:

```tsx
      {
        accessorKey: 'expected_min',
        header: 'Min Esperados',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'deficit_min',
        header: 'Min Déficit',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
```

Añadir dos campos obligatorios a `Aggregates` rompe a todo fixture que construya el tipo completo. Correr `npx tsc --noEmit` y arreglar los que salgan — al menos `frontend/src/test-utils/msw-handlers.ts:13` construye agregados de reporte. Poner `expected_min: 0, deficit_min: 0` salvo que el test afirme otra cosa.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd frontend && npx vitest run src/components/reports && npx vitest run`
Expected: PASS — toda la suite del frontend en verde.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/types/api.ts frontend/src/components/reports "frontend/src/app/(dashboard)/reports"
git commit -m "feat(reports): columnas de horas esperadas y déficit en la tabla"
```

---

### Task 8: E2E de presencia y scope

**Files:**
- Create: `frontend/e2e/presence.spec.ts`
- Modify: ninguno

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: nada.

- [ ] **Step 1: Write the failing test**

`frontend/e2e/presence.spec.ts`, siguiendo el patrón de login de `frontend/e2e/dashboard.spec.ts` (leer ese archivo primero para reutilizar sus helpers y selectores):

```ts
import { expect, test } from '@playwright/test'
import { login } from './helpers'  // ajustar al helper real del repo

test('el dashboard muestra presencia y jornada incumplida', async ({ page }) => {
  await login(page, 'e2e_admin', 'e2e-admin-pass')
  await page.goto('/dashboard')

  await expect(page.getByTestId('presence-tab-inside')).toBeVisible()
  await page.getByTestId('presence-tab-attended').click()
  await expect(page.getByRole('table')).toBeVisible()
})

test('un supervisor con departamento no ve empleados de otro', async ({ page }) => {
  await login(page, 'e2e_supervisor', 'e2e-supervisor-pass')
  await page.goto('/dashboard')

  const table = page.getByRole('table')
  await expect(table).toBeVisible()
  // El seed pone al supervisor en un departamento; ninguna fila debe salir de él.
  const departments = await table.locator('tbody tr td:nth-child(3)').allTextContents()
  const unique = new Set(departments.map(d => d.trim()).filter(Boolean))
  expect(unique.size).toBeLessThanOrEqual(1)
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd .. && make e2e-build && cd frontend && npx playwright test presence.spec.ts`
Expected: FAIL si algún commit anterior falta; PASS solo con las Tasks 1-7 aplicadas.

- [ ] **Step 3: Ajustar selectores**

No hay implementación nueva: si el test falla por selectores, corregirlos contra el DOM real; si falla por scope, es un bug de la Task 2 y se arregla ahí.

- [ ] **Step 4: Run the full gate**

Run: `cd backend && cargo nextest run` y `cd frontend && npx vitest run`
Expected: PASS ambos. Opcional antes del PR: `make coverage`.

- [ ] **Step 5: Commit**

```bash
git add frontend/e2e/presence.spec.ts
git commit -m "test(e2e): presencia en el dashboard y aislamiento por departamento"
```

---

## Notas de integración

- El seed de demo (`scripts/seed-reports-data.py`) escribe `entry_at` y `exit_at` en casi todas las filas, así que "dentro ahora" saldrá casi vacío salvo por los 6 registros a los que `scripts/seed-anomalies.py` anuló la salida. Para una demo con gente dentro, correr el seed de anomalías o anular `exit_at` de unas filas del día en curso.
- El seed siembra el mes completo, incluidos días futuros. `/presence/today` filtra por la fecha de hoy, así que eso no le afecta.
- `scripts/seed-employees.py` y el fix del filtro de `/anomalies` viven en la rama `fix/demo-seed-and-anomaly-codes`, aún sin PR.
