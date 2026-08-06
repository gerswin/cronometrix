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
