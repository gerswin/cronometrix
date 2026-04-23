//! Daily-records persistence layer (CALC-01..06 base, D-01/D-02/D-09/D-18).
//!
//! Responsibilities:
//! - Pre-fetch all engine inputs (employee + dept, global_rules, window events,
//!   weekly/annual OT lookback aggregates, prior row existence).
//! - Call the pure [`crate::calc::compute_daily_record`] engine.
//! - Upsert the `daily_records` row via ON CONFLICT DO UPDATE (Pitfall 1).
//! - Replace the anomaly set in the same transaction (Pitfall 3).
//!
//! Read path: pagination helpers mirror `events/service.rs::list` / `get_by_id`.

use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use libsql::{params, Connection};

use crate::calc::models::{AttendanceEventRow, DepartmentConfig, GlobalRulesRow};
use crate::calc::{self, EngineInput};
use crate::common::{epoch_to_iso, epoch_to_iso_opt, PaginatedResponse};
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{DailyRecordListQuery, DailyRecordResponse};

/// Columns used when rendering a DailyRecordResponse.
const DR_SELECT_COLS: &str = "id, employee_id, department_id, anchor_date, shift_type, \
    work_minutes, overtime_minutes, late_minutes, early_departure_minutes, \
    is_rest_day_worked, entry_at, exit_at, leave_id, computed_at, created_at, updated_at";

/// Full recompute for a single (employee_id, anchor_date) pair.
pub async fn recompute_for_day(
    state: &AppState,
    employee_id: &str,
    anchor_date: NaiveDate,
) -> Result<(), AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    // 1. Load employee + department config. If employee is inactive/missing,
    //    skip silently — event publishers shouldn't be able to wedge the worker
    //    by deleting an employee between publish and recompute.
    let mut rows = conn
        .query(
            "SELECT e.id, d.id, d.shift_start_time, d.shift_end_time, d.shift_type, \
             d.is_overnight_shift, d.ordinary_daily_minutes, d.lunch_mode, d.lunch_duration_min \
             FROM employees e JOIN departments d ON d.id = e.department_id \
             WHERE e.id = ?1 AND e.status = 'active'",
            params![employee_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    else {
        tracing::warn!(
            employee_id,
            "recompute_for_day: employee inactive or missing; skipping"
        );
        return Ok(());
    };
    let dept_id: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
    let is_overnight: i64 = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
    let dept = DepartmentConfig {
        id: dept_id.clone(),
        shift_start_time: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        shift_end_time: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        shift_type: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        is_overnight_shift: is_overnight != 0,
        ordinary_daily_minutes: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        lunch_mode: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        lunch_duration_min: row.get(8).map_err(|e| AppError::Internal(e.into()))?,
    };
    drop(rows);

    // 2. Load global_rules singleton.
    let mut rrows = conn
        .query(
            "SELECT late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes \
             FROM global_rules WHERE id = 'singleton'",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let rules_row = rrows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("global_rules singleton missing")))?;
    let rules = GlobalRulesRow {
        late_arrival_tolerance_min: rules_row
            .get(0)
            .map_err(|e| AppError::Internal(e.into()))?,
        early_departure_tolerance_min: rules_row
            .get(1)
            .map_err(|e| AppError::Internal(e.into()))?,
        bonus_minutes: rules_row.get(2).map_err(|e| AppError::Internal(e.into()))?,
    };
    drop(rrows);

    // 3. Window-bounded event fetch. We use the same aggregation helper the
    //    engine uses, so we never pull events outside the calc window.
    //
    //    Plan 03-02: `shift_window` delegates to `shift_window_overnight_aware`,
    //    so when `dept.is_overnight_shift = true` the returned `window_end`
    //    crosses midnight (anchor_date + 1 day). The `captured_at BETWEEN ?2
    //    AND ?3` query below therefore picks up post-midnight exit events
    //    automatically — no SQL change required. Covered by integration test
    //    `recompute_overnight_captures_post_midnight_events` (T-3-12).
    let tz: Tz = state.config.timezone;
    let (window_start, window_end, _ns, _ne) =
        calc::aggregation::shift_window(anchor_date, &dept, &rules, tz);

    let mut ev_rows = conn
        .query(
            "SELECT id, employee_id, device_id, direction, captured_at, is_unknown \
             FROM attendance_events \
             WHERE (employee_id = ?1 OR (employee_id IS NULL AND is_unknown = 1)) \
               AND captured_at BETWEEN ?2 AND ?3",
            params![employee_id.to_string(), window_start, window_end],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut events: Vec<AttendanceEventRow> = Vec::new();
    while let Some(r) = ev_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let is_unknown_int: i64 = r.get(5).map_err(|e| AppError::Internal(e.into()))?;
        events.push(AttendanceEventRow {
            id: r.get(0).map_err(|e| AppError::Internal(e.into()))?,
            employee_id: r.get(1).map_err(|e| AppError::Internal(e.into()))?,
            device_id: r.get(2).map_err(|e| AppError::Internal(e.into()))?,
            direction: r.get(3).map_err(|e| AppError::Internal(e.into()))?,
            captured_at: r.get(4).map_err(|e| AppError::Internal(e.into()))?,
            is_unknown: is_unknown_int != 0,
        });
    }
    drop(ev_rows);

    // 4. Weekly OT lookback: ISO week Monday → anchor_date (exclusive).
    let iso_week_monday = {
        let wd = anchor_date.weekday().num_days_from_monday();
        anchor_date - chrono::Duration::days(wd as i64)
    };
    let mut wk_rows = conn
        .query(
            "SELECT COALESCE(SUM(overtime_minutes), 0) FROM daily_records \
             WHERE employee_id = ?1 AND anchor_date >= ?2 AND anchor_date < ?3",
            params![
                employee_id.to_string(),
                iso_week_monday.format("%Y-%m-%d").to_string(),
                anchor_date.format("%Y-%m-%d").to_string(),
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let weekly_ot_minutes_so_far: i64 = wk_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map(|r| r.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);
    drop(wk_rows);

    // 5. Annual OT lookback: Jan 1 of the anchor year → anchor_date (exclusive).
    let year_start = NaiveDate::from_ymd_opt(anchor_date.year(), 1, 1).unwrap();
    let mut an_rows = conn
        .query(
            "SELECT COALESCE(SUM(overtime_minutes), 0) FROM daily_records \
             WHERE employee_id = ?1 AND anchor_date >= ?2 AND anchor_date < ?3",
            params![
                employee_id.to_string(),
                year_start.format("%Y-%m-%d").to_string(),
                anchor_date.format("%Y-%m-%d").to_string(),
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let annual_ot_minutes_so_far: i64 = an_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map(|r| r.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);
    drop(an_rows);

    // 6. Prior row existence check — drives RECOMPUTE_AFTER_EDIT.
    let mut prev_rows = conn
        .query(
            "SELECT id FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![
                employee_id.to_string(),
                anchor_date.format("%Y-%m-%d").to_string()
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let prior_row_id: Option<String> = match prev_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        Some(r) => r.get::<String>(0).ok(),
        None => None,
    };
    drop(prev_rows);

    // 7. Pure engine. Plan 03-01: leave is always None (Plan 03-03 populates).
    let input = EngineInput {
        events,
        dept: dept.clone(),
        rules,
        leave: None,
        anchor_date,
        tz,
        weekly_ot_minutes_so_far,
        annual_ot_minutes_so_far,
        prior_record_existed: prior_row_id.is_some(),
    };
    let out = calc::compute_daily_record(&input);

    // 8. Upsert + anomaly replacement on the SAME connection. libSQL uses
    //    SQLite's shared-cache + file-locked backend; opening a second
    //    connection for the write side while the read `conn` was still open
    //    produced "database is locked" under test loads. Reusing `conn` for
    //    the transaction is safe because all read rows have been drained + the
    //    statement cursors dropped before BEGIN.
    let txn_conn = &conn;
    txn_conn
        .execute("BEGIN", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let now = chrono::Utc::now().timestamp();
    let new_id = prior_row_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // ON CONFLICT DO UPDATE preserves the id of the existing row so
    // daily_record_anomalies FK remains valid (Pitfall 1).
    txn_conn
        .execute(
            "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, \
             work_minutes, overtime_minutes, late_minutes, early_departure_minutes, \
             is_rest_day_worked, entry_at, exit_at, leave_id, computed_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?14) \
             ON CONFLICT(employee_id, anchor_date) DO UPDATE SET \
                work_minutes = excluded.work_minutes, \
                overtime_minutes = excluded.overtime_minutes, \
                late_minutes = excluded.late_minutes, \
                early_departure_minutes = excluded.early_departure_minutes, \
                is_rest_day_worked = excluded.is_rest_day_worked, \
                entry_at = excluded.entry_at, \
                exit_at = excluded.exit_at, \
                leave_id = excluded.leave_id, \
                shift_type = excluded.shift_type, \
                computed_at = excluded.computed_at, \
                updated_at = excluded.updated_at",
            params![
                new_id.clone(),
                employee_id.to_string(),
                dept.id.clone(),
                anchor_date.format("%Y-%m-%d").to_string(),
                dept.shift_type.clone(),
                out.work_minutes,
                out.overtime_minutes,
                out.late_minutes,
                out.early_departure_minutes,
                out.is_rest_day_worked as i64,
                out.entry_at,
                out.exit_at,
                out.leave_id.clone(),
                now,
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Fetch resolved id (may differ from `new_id` if the ON CONFLICT path
    // preserved an older row's id).
    let mut id_rows = txn_conn
        .query(
            "SELECT id FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![
                employee_id.to_string(),
                anchor_date.format("%Y-%m-%d").to_string()
            ],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let resolved_row = id_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("upserted row vanished")))?;
    let resolved_id: String = resolved_row
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;
    drop(id_rows);

    // Replace anomalies (Pitfall 3 — never accumulate).
    txn_conn
        .execute(
            "DELETE FROM daily_record_anomalies WHERE daily_record_id = ?1",
            params![resolved_id.clone()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    for code in &out.anomalies {
        txn_conn
            .execute(
                "INSERT INTO daily_record_anomalies (id, daily_record_id, code, detail, created_at) \
                 VALUES (?1, ?2, ?3, NULL, ?4)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    resolved_id.clone(),
                    code.as_str().to_string(),
                    now
                ],
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }

    txn_conn
        .execute("COMMIT", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

/// Nightly reconcile: recompute yesterday's daily_record for every active
/// employee. Errors on individual employees are logged and swallowed so one
/// bad record cannot wedge the whole pass.
pub async fn reconcile_prior_day(state: &AppState, tz: Tz) -> Result<i64, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let yesterday = {
        let now_local = chrono::Utc::now().with_timezone(&tz);
        now_local.date_naive() - chrono::Duration::days(1)
    };
    let mut rows = conn
        .query("SELECT id FROM employees WHERE status = 'active'", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut count = 0i64;
    while let Some(r) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let emp_id: String = r.get(0).map_err(|e| AppError::Internal(e.into()))?;
        if let Err(e) = recompute_for_day(state, &emp_id, yesterday).await {
            tracing::warn!(
                employee_id = %emp_id,
                err = %e,
                "nightly reconcile: per-employee failure, continuing"
            );
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Fetch the anomaly codes for a given daily_record id, in insertion order.
async fn fetch_anomaly_codes(conn: &Connection, daily_record_id: &str) -> Result<Vec<String>, AppError> {
    let mut rows = conn
        .query(
            "SELECT code FROM daily_record_anomalies \
             WHERE daily_record_id = ?1 ORDER BY created_at ASC, id ASC",
            params![daily_record_id.to_string()],
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut out = Vec::new();
    while let Some(r) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let code: String = r.get(0).map_err(|e| AppError::Internal(e.into()))?;
        out.push(code);
    }
    Ok(out)
}

fn row_to_dr(row: libsql::Row) -> Result<DailyRecordResponse, AppError> {
    let is_rest_day_worked_int: i64 =
        row.get(9).map_err(|e| AppError::Internal(e.into()))?;
    let entry_at: Option<i64> = row.get(10).map_err(|e| AppError::Internal(e.into()))?;
    let exit_at: Option<i64> = row.get(11).map_err(|e| AppError::Internal(e.into()))?;
    Ok(DailyRecordResponse {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        employee_id: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
        department_id: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        anchor_date: row.get(3).map_err(|e| AppError::Internal(e.into()))?,
        shift_type: row.get(4).map_err(|e| AppError::Internal(e.into()))?,
        work_minutes: row.get(5).map_err(|e| AppError::Internal(e.into()))?,
        overtime_minutes: row.get(6).map_err(|e| AppError::Internal(e.into()))?,
        late_minutes: row.get(7).map_err(|e| AppError::Internal(e.into()))?,
        early_departure_minutes: row.get(8).map_err(|e| AppError::Internal(e.into()))?,
        is_rest_day_worked: is_rest_day_worked_int != 0,
        entry_at: epoch_to_iso_opt(entry_at),
        exit_at: epoch_to_iso_opt(exit_at),
        leave_id: row.get(12).map_err(|e| AppError::Internal(e.into()))?,
        computed_at: epoch_to_iso(row.get(13).map_err(|e| AppError::Internal(e.into()))?),
        created_at: epoch_to_iso(row.get(14).map_err(|e| AppError::Internal(e.into()))?),
        updated_at: epoch_to_iso(row.get(15).map_err(|e| AppError::Internal(e.into()))?),
        anomalies: Vec::new(),
    })
}

pub async fn list(
    conn: &Connection,
    q: DailyRecordListQuery,
) -> Result<PaginatedResponse<DailyRecordResponse>, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut predicates: Vec<String> = Vec::new();
    let mut count_values: Vec<libsql::Value> = Vec::new();
    let mut fetch_values: Vec<libsql::Value> = Vec::new();

    if let Some(emp) = &q.employee_id {
        predicates.push(format!("employee_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(emp.clone()));
        fetch_values.push(libsql::Value::Text(emp.clone()));
    }
    if let Some(dept) = &q.department_id {
        predicates.push(format!("department_id = ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(dept.clone()));
        fetch_values.push(libsql::Value::Text(dept.clone()));
    }
    if let Some(from) = &q.from_date {
        predicates.push(format!("anchor_date >= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(from.clone()));
        fetch_values.push(libsql::Value::Text(from.clone()));
    }
    if let Some(to) = &q.to_date {
        predicates.push(format!("anchor_date <= ?{}", predicates.len() + 1));
        count_values.push(libsql::Value::Text(to.clone()));
        fetch_values.push(libsql::Value::Text(to.clone()));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM daily_records {}", where_clause);
    let total: i64 = conn
        .query(&count_sql, libsql::params_from_iter(count_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("COUNT query returned no rows")))?
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;

    let fetch_sql = format!(
        "SELECT {cols} FROM daily_records {where_clause} \
         ORDER BY anchor_date DESC, id ASC LIMIT ?{lim} OFFSET ?{off}",
        cols = DR_SELECT_COLS,
        where_clause = where_clause,
        lim = fetch_values.len() + 1,
        off = fetch_values.len() + 2,
    );
    fetch_values.push(libsql::Value::Integer(limit));
    fetch_values.push(libsql::Value::Integer(offset));

    let mut rows = conn
        .query(&fetch_sql, libsql::params_from_iter(fetch_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut data: Vec<DailyRecordResponse> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let mut resp = row_to_dr(row)?;
        resp.anomalies = fetch_anomaly_codes(conn, &resp.id).await?;
        data.push(resp);
    }

    Ok(PaginatedResponse {
        data,
        total,
        limit,
        offset,
    })
}

pub async fn get_by_id(conn: &Connection, id: &str) -> Result<DailyRecordResponse, AppError> {
    let sql = format!(
        "SELECT {} FROM daily_records WHERE id = ?1",
        DR_SELECT_COLS
    );
    let row = conn
        .query(&sql, params![id.to_string()])
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::NotFound {
            code: "DAILY_RECORD_NOT_FOUND",
            message: format!("DailyRecord '{}' not found", id),
        })?;
    let mut resp = row_to_dr(row)?;
    resp.anomalies = fetch_anomaly_codes(conn, &resp.id).await?;
    Ok(resp)
}
