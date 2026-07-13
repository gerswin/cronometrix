//! Reports service — SQL aggregation across daily_records + overrides + leaves
//! + anomalies, plus secondary leaves aggregation (W-5), money math (LOTTT
//!   Art. 117/118/120), and app-code audit insert (D-21).
//!
//! Read-only path on daily_records. Audit insert is the only write — it lands
//! AFTER aggregation succeeds (Pitfall 7: failed reports must not leak audit
//! rows that imply a successful export).
//!
//! W-5 fix (leave-day counting): the primary daily_records JOIN only sees days
//! where the engine attached a leave overlay (`dr.leave_id NOT NULL`). A
//! full-week vacation with zero biometric captures would therefore be invisible
//! to the JOIN — under-counting días_vacación / IVSS / permiso / no-remunerado.
//! We run a SECOND aggregation directly against `leaves` to count overlap days
//! per (employee, leave_type) within `[from..to]` and merge into the per-
//! employee accumulator. Leave-day COUNTS come EXCLUSIVELY from the leaves
//! aggregation; the daily_records branch only handles money math (vacation
//! paid-full pay, medical zero pay, etc.) so we never double-count.
//!
//! W-6 fix (shift_type source): night-premium gating reads
//! `daily_records.shift_type` (the per-day actual shift recorded by the engine
//! in Phase 3, migration 007), NOT `departments.shift_type` (the policy/
//! default). The engine's per-day output is authoritative for what actually
//! happened on each day; reading dept policy would miss shift overrides.

use chrono::{Datelike, Duration, NaiveDate};
use libsql::Connection;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;

use super::{
    models::{
        Aggregates, BrandingHeader, DeptSubtotal, DeptSummary, EmployeeReportRow,
        ReportParamsRequest, ReportPayload,
    },
    money, periods,
};
use crate::{common::epoch_to_iso, errors::AppError, state::AppState};

/// Internal accumulator: one entry per employee while we sweep daily_records
/// rows + leaves rows. `worked_dates` and `leave_dates` drive the
/// `días_ausentes` calculation (D-34) — Mon-Fri in [from..to] minus worked
/// minus on-leave.
struct AccRow {
    employee_id: String,
    dept_id: String,
    cedula: String,
    nombre: String,
    departamento: String,
    cargo: String,
    /// Display-only: most-recent per-day shift_type seen (or dept fallback for
    /// employees with no daily_records but only leaves). The night-premium
    /// gating is decided per-day inside the daily_records JOIN — this field
    /// is not the gating source.
    shift_type: String,
    agg: Aggregates,
    anomaly_codes_set: BTreeSet<String>,
    worked_dates: HashSet<NaiveDate>,
    leave_dates: HashSet<NaiveDate>,
}

pub async fn compute_report(
    state: &AppState,
    actor_id: &str,
    params: &ReportParamsRequest,
    format: &str,
) -> Result<ReportPayload, AppError> {
    // 1. Parse period_type → PeriodPreset → (from, to).
    //    For non-custom presets, from_date is treated as the anchor/ref date.
    let preset = periods::parse_period(&params.period_type, &params.from_date, &params.to_date)?;
    let ref_date = NaiveDate::parse_from_str(&params.from_date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be YYYY-MM-DD".to_string(),
        }
    })?;
    let (from, to) = periods::resolve_period(preset, ref_date);

    // DoS guard (T-05-10 / Security V13). Any range >366 days is rejected up
    // front before we touch the database. AppError::Validation maps to
    // HTTP 422 UNPROCESSABLE_ENTITY per errors.rs:88-89.
    if (to - from).num_days() > 366 {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "Period range cannot exceed 366 days".to_string(),
        });
    }
    if from > to {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "from_date must be ≤ to_date".to_string(),
        });
    }

    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    // 2. Fetch tenant_info for the branding header (D-28).
    let mut tenant_rows = conn
        .query(
            "SELECT client_name, client_rif FROM tenant_info WHERE id = 1",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let tenant_row = tenant_rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("tenant_info row missing")))?;
    let client_name: String = tenant_row
        .get(0)
        .map_err(|e| AppError::Internal(e.into()))?;
    let client_rif: String = tenant_row
        .get(1)
        .map_err(|e| AppError::Internal(e.into()))?;
    drop(tenant_rows);

    let header = BrandingHeader {
        client_name,
        client_rif,
        from_date: from.to_string(),
        to_date: to.to_string(),
        generated_at_iso: epoch_to_iso(chrono::Utc::now().timestamp()),
    };

    // 3. Build dynamic SQL with parameterized predicates for the daily_records
    //    JOIN. T-05-08 mitigation: every user-supplied value goes through
    //    libsql::Value + params_from_iter; zero string-concat of user input.
    let mut predicates: Vec<String> = vec!["dr.anchor_date BETWEEN ?1 AND ?2".to_string()];
    let mut values: Vec<libsql::Value> = vec![
        libsql::Value::Text(from.to_string()),
        libsql::Value::Text(to.to_string()),
    ];

    // include_inactive: default false → only status='active'
    if !params.include_inactive.unwrap_or(false) {
        predicates.push("e.status = 'active'".to_string());
    }

    if let Some(dept_ids) = &params.department_ids {
        if !dept_ids.is_empty() {
            let placeholders: Vec<String> = dept_ids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", values.len() + 1 + i))
                .collect();
            predicates.push(format!("d.id IN ({})", placeholders.join(",")));
            for id in dept_ids {
                values.push(libsql::Value::Text(id.clone()));
            }
        }
    }

    if let Some(eid) = &params.employee_id {
        predicates.push(format!("e.id = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(eid.clone()));
    }

    // shift_type filter scopes the daily_records JOIN by daily_records.shift_type
    // (the per-day actual shift). Operators can request "show me only the night-
    // shift days in this period" by passing this filter.
    if let Some(st) = &params.shift_type {
        predicates.push(format!("dr.shift_type = ?{}", values.len() + 1));
        values.push(libsql::Value::Text(st.clone()));
    }

    let where_clause = format!("WHERE {}", predicates.join(" AND "));

    // W-6: SELECT dr.shift_type for night-premium gating, NOT d.shift_type.
    // d.shift_type is the policy/default; dr.shift_type is what the engine
    // recorded for the specific day (migration 007 line 12). Reading dr is
    // authoritative — it captures any per-day shift overrides the engine made.
    let sql = format!(
        "SELECT \
            e.id            AS employee_id, \
            e.employee_code AS cedula, \
            e.name          AS nombre, \
            e.position      AS cargo, \
            d.id            AS dept_id, \
            d.name          AS dept_name, \
            e.base_salary_cents, \
            d.ordinary_daily_minutes, \
            dr.shift_type   AS day_shift_type, \
            dr.anchor_date, \
            dr.work_minutes, \
            dr.overtime_minutes, \
            dr.late_minutes, \
            dr.is_rest_day_worked, \
            dr.leave_id, \
            l.leave_type, \
            dro.override_work_minutes, \
            (SELECT GROUP_CONCAT(code) FROM daily_record_anomalies WHERE daily_record_id = dr.id) AS anomaly_codes \
         FROM daily_records dr \
         JOIN employees e   ON e.id = dr.employee_id \
         JOIN departments d ON d.id = dr.department_id \
         LEFT JOIN daily_record_overrides dro ON dro.daily_record_id = dr.id AND dro.status = 'active' \
         LEFT JOIN leaves l ON l.id = dr.leave_id AND l.status = 'active' \
         {where_clause} \
         ORDER BY d.name, e.name, dr.anchor_date"
    );

    let mut rows_iter = conn
        .query(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut acc: BTreeMap<String, AccRow> = BTreeMap::new();
    // dept_id -> dept_name, populated as we encounter departments. Used to
    // build the departments_in_order vec sorted by name (D-26).
    let mut dept_seen: BTreeMap<String, String> = BTreeMap::new();

    while let Some(row) = rows_iter
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let employee_id: String = row.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let cedula: String = row.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let nombre: String = row.get(2).map_err(|e| AppError::Internal(e.into()))?;
        let cargo: String = row.get(3).map_err(|e| AppError::Internal(e.into()))?;
        let dept_id: String = row.get(4).map_err(|e| AppError::Internal(e.into()))?;
        let dept_name: String = row.get(5).map_err(|e| AppError::Internal(e.into()))?;
        let base_salary_cents: i64 = row.get(6).map_err(|e| AppError::Internal(e.into()))?;
        let ordinary_daily_minutes: i64 = row.get(7).map_err(|e| AppError::Internal(e.into()))?;
        let day_shift_type: String = row.get(8).map_err(|e| AppError::Internal(e.into()))?; // W-6
        let anchor_date_str: String = row.get(9).map_err(|e| AppError::Internal(e.into()))?;
        let work_minutes: i64 = row.get(10).map_err(|e| AppError::Internal(e.into()))?;
        let overtime_minutes: i64 = row.get(11).map_err(|e| AppError::Internal(e.into()))?;
        let late_minutes: i64 = row.get(12).map_err(|e| AppError::Internal(e.into()))?;
        let is_rest_day_worked: i64 = row.get(13).map_err(|e| AppError::Internal(e.into()))?;
        let _leave_id_opt: Option<String> = row.get(14).ok();
        let leave_type_opt: Option<String> = row.get(15).ok();
        let override_work_min_opt: Option<i64> = row.get(16).ok();
        let anomaly_codes_str_opt: Option<String> = row.get(17).ok();

        let anchor_date = NaiveDate::parse_from_str(&anchor_date_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Override merge — operator edits invisible if skipped (Pitfall 3).
        let effective_work_min = override_work_min_opt.unwrap_or(work_minutes);

        dept_seen
            .entry(dept_id.clone())
            .or_insert(dept_name.clone());

        let entry = acc.entry(employee_id.clone()).or_insert_with(|| AccRow {
            employee_id: employee_id.clone(),
            dept_id: dept_id.clone(),
            cedula: cedula.clone(),
            nombre: nombre.clone(),
            departamento: dept_name.clone(),
            cargo: cargo.clone(),
            shift_type: day_shift_type.clone(),
            agg: Aggregates::default(),
            anomaly_codes_set: BTreeSet::new(),
            worked_dates: HashSet::new(),
            leave_dates: HashSet::new(),
        });

        // Money treatment per leave_type when an active overlay is attached
        // to this daily_record (D-07). Leave-day COUNTS are NOT incremented
        // here — they come from the W-5 secondary aggregation below.
        let leave_kind = leave_type_opt.as_deref();

        match leave_kind {
            Some("medical") => {
                // No work pay; medical paid externally via IVSS.
                entry.leave_dates.insert(anchor_date);
            }
            Some("vacation") => {
                // Vacation paid full at ordinary daily salary (only when
                // overlay is attached to a daily_record). Leave-only days
                // without an overlay are documented as a v1 limitation —
                // they produce only counter increments (see W-5 block below).
                entry.leave_dates.insert(anchor_date);
                let work_pay = money::work_pay_cents(
                    ordinary_daily_minutes,
                    base_salary_cents,
                    ordinary_daily_minutes,
                );
                entry.agg.work_pay_cents = entry.agg.work_pay_cents.saturating_add(work_pay);
                entry.agg.total_a_pagar_cents =
                    entry.agg.total_a_pagar_cents.saturating_add(work_pay);
            }
            Some("unpaid") => {
                entry.leave_dates.insert(anchor_date);
            }
            Some("manual") => {
                entry.leave_dates.insert(anchor_date);
            }
            _ => {
                // Standard work day money math.
                let work_pay = money::work_pay_cents(
                    effective_work_min,
                    base_salary_cents,
                    ordinary_daily_minutes,
                );
                let ot_pay = money::ot_pay_cents(
                    overtime_minutes,
                    base_salary_cents,
                    ordinary_daily_minutes,
                );
                // W-6: night premium gates on dr.shift_type (per-day actual
                // shift), NOT departments.shift_type. The engine's per-day
                // output is authoritative for what actually happened.
                let night = if day_shift_type == "night" {
                    money::night_premium_cents(
                        effective_work_min,
                        base_salary_cents,
                        ordinary_daily_minutes,
                    )
                } else {
                    0
                };
                let rest = if is_rest_day_worked == 1 {
                    money::rest_day_surcharge_cents(
                        effective_work_min,
                        base_salary_cents,
                        ordinary_daily_minutes,
                    )
                } else {
                    0
                };
                let late = money::late_deduction_cents(
                    late_minutes,
                    base_salary_cents,
                    ordinary_daily_minutes,
                );
                let total = money::total_a_pagar_cents(work_pay, ot_pay, night, rest, late);

                entry.agg.work_min = entry.agg.work_min.saturating_add(effective_work_min);
                entry.agg.ot_min = entry.agg.ot_min.saturating_add(overtime_minutes);
                entry.agg.late_min = entry.agg.late_min.saturating_add(late_minutes);
                entry.agg.work_pay_cents = entry.agg.work_pay_cents.saturating_add(work_pay);
                entry.agg.ot_pay_cents = entry.agg.ot_pay_cents.saturating_add(ot_pay);
                entry.agg.night_premium_cents = entry.agg.night_premium_cents.saturating_add(night);
                entry.agg.rest_day_surcharge_cents =
                    entry.agg.rest_day_surcharge_cents.saturating_add(rest);
                entry.agg.late_deduction_cents =
                    entry.agg.late_deduction_cents.saturating_add(late);
                entry.agg.total_a_pagar_cents = entry.agg.total_a_pagar_cents.saturating_add(total);
                if effective_work_min > 0 {
                    entry.agg.days_worked += 1;
                    entry.worked_dates.insert(anchor_date);
                }
                // Update display shift_type to the most recent day seen.
                entry.shift_type = day_shift_type.clone();
            }
        }

        if let Some(s) = anomaly_codes_str_opt.filter(|s| !s.is_empty()) {
            for code in s.split(',') {
                entry.anomaly_codes_set.insert(code.to_string());
            }
        }
    }
    drop(rows_iter);

    // 4. W-5 FIX — secondary aggregation against `leaves` directly.
    //
    // The daily_records JOIN above only sees days where the engine attached a
    // leave overlay (dr.leave_id NOT NULL). A full-week vacation with zero
    // biometric captures would be invisible — under-counting días_vacación /
    // IVSS / permiso / no-remunerado. Run a separate query against `leaves`
    // scoped to the same employee filter the JOIN used; for each leave row
    // compute overlap days with [from..to] and increment the counters on the
    // matching employee's accumulator. Leave dates are also inserted into
    // entry.leave_dates so días_ausentes excludes them.
    //
    // Money treatment for leaves WITHOUT a daily_record overlay is a known v1
    // limitation: only overlays attached to a daily_record produce vacation
    // pay (the daily_records branch above). Leave-only days produce counter
    // increments and entry into leave_dates (so absent-day calc skips them)
    // but no pay is synthesized. Future work could synthesize vacation pay
    // for leave-only days; out of scope for v1.
    let mut leave_predicates: Vec<String> = vec![
        "l.status = 'active'".to_string(),
        "l.deleted_at IS NULL".to_string(),
        "l.from_date <= ?1".to_string(),
        "l.to_date   >= ?2".to_string(),
    ];
    let mut leave_values: Vec<libsql::Value> = vec![
        libsql::Value::Text(to.to_string()),
        libsql::Value::Text(from.to_string()),
    ];
    if !params.include_inactive.unwrap_or(false) {
        leave_predicates.push("e.status = 'active'".to_string());
    }
    if let Some(dept_ids) = &params.department_ids {
        if !dept_ids.is_empty() {
            let placeholders: Vec<String> = dept_ids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", leave_values.len() + 1 + i))
                .collect();
            leave_predicates.push(format!("e.department_id IN ({})", placeholders.join(",")));
            for id in dept_ids {
                leave_values.push(libsql::Value::Text(id.clone()));
            }
        }
    }
    if let Some(eid) = &params.employee_id {
        leave_predicates.push(format!("e.id = ?{}", leave_values.len() + 1));
        leave_values.push(libsql::Value::Text(eid.clone()));
    }
    let leave_where = format!("WHERE {}", leave_predicates.join(" AND "));

    let leave_sql = format!(
        "SELECT l.employee_id, l.leave_type, l.from_date, l.to_date, \
                e.employee_code, e.name, e.position, \
                d.id AS dept_id, d.name AS dept_name, \
                e.base_salary_cents, d.ordinary_daily_minutes, \
                d.shift_type AS dept_shift_type \
           FROM leaves l \
           JOIN employees e   ON e.id = l.employee_id \
           JOIN departments d ON d.id = e.department_id \
           {leave_where}"
    );

    let mut leave_iter = conn
        .query(&leave_sql, libsql::params_from_iter(leave_values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    while let Some(lr) = leave_iter
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    {
        let l_employee_id: String = lr.get(0).map_err(|e| AppError::Internal(e.into()))?;
        let l_type: String = lr.get(1).map_err(|e| AppError::Internal(e.into()))?;
        let l_from_str: String = lr.get(2).map_err(|e| AppError::Internal(e.into()))?;
        let l_to_str: String = lr.get(3).map_err(|e| AppError::Internal(e.into()))?;
        let l_cedula: String = lr.get(4).map_err(|e| AppError::Internal(e.into()))?;
        let l_nombre: String = lr.get(5).map_err(|e| AppError::Internal(e.into()))?;
        let l_cargo: String = lr.get(6).map_err(|e| AppError::Internal(e.into()))?;
        let l_dept_id: String = lr.get(7).map_err(|e| AppError::Internal(e.into()))?;
        let l_dept_name: String = lr.get(8).map_err(|e| AppError::Internal(e.into()))?;
        let _l_base: i64 = lr.get(9).map_err(|e| AppError::Internal(e.into()))?;
        let _l_ord: i64 = lr.get(10).map_err(|e| AppError::Internal(e.into()))?;
        let l_dept_shift: String = lr.get(11).map_err(|e| AppError::Internal(e.into()))?;

        let l_from = NaiveDate::parse_from_str(&l_from_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let l_to = NaiveDate::parse_from_str(&l_to_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let overlap_from = std::cmp::max(from, l_from);
        let overlap_to = std::cmp::min(to, l_to);
        if overlap_from > overlap_to {
            continue;
        }

        dept_seen
            .entry(l_dept_id.clone())
            .or_insert(l_dept_name.clone());

        let entry = acc.entry(l_employee_id.clone()).or_insert_with(|| AccRow {
            employee_id: l_employee_id.clone(),
            dept_id: l_dept_id.clone(),
            cedula: l_cedula.clone(),
            nombre: l_nombre.clone(),
            departamento: l_dept_name.clone(),
            cargo: l_cargo.clone(),
            // Fallback: dept policy when no per-day shift was seen.
            shift_type: l_dept_shift.clone(),
            agg: Aggregates::default(),
            anomaly_codes_set: BTreeSet::new(),
            worked_dates: HashSet::new(),
            leave_dates: HashSet::new(),
        });

        let mut d = overlap_from;
        while d <= overlap_to {
            entry.leave_dates.insert(d);
            match l_type.as_str() {
                "medical" => entry.agg.days_ivss += 1,
                "vacation" => entry.agg.days_vacation += 1,
                "manual" => entry.agg.days_permission += 1,
                "unpaid" => entry.agg.days_unpaid += 1,
                _ => {}
            }
            d += Duration::days(1);
        }
    }
    drop(leave_iter);

    // 5. días_ausentes — Mon-Fri in [from..=to] minus worked_dates minus
    //    leave_dates (D-34). Saturday/Sunday excluded regardless of
    //    is_rest_day_worked.
    let weekdays_in_period: Vec<NaiveDate> = (0..=(to - from).num_days())
        .map(|d| from + Duration::days(d))
        .filter(|d| d.weekday().num_days_from_monday() < 5)
        .collect();

    for entry in acc.values_mut() {
        let absent = weekdays_in_period
            .iter()
            .filter(|d| !entry.worked_dates.contains(d) && !entry.leave_dates.contains(d))
            .count() as i64;
        entry.agg.days_absent = absent;
    }

    // 6. Build EmployeeReportRow vec, dept_subtotals, grand_total.
    let mut rows: Vec<EmployeeReportRow> = Vec::new();
    let mut dept_to_subtotal: BTreeMap<String, (String, Aggregates)> = BTreeMap::new();
    let mut grand = Aggregates::default();

    for (_, e) in acc {
        let codes_vec: Vec<String> = e.anomaly_codes_set.into_iter().collect();
        let count = codes_vec.len() as i64;

        let entry = dept_to_subtotal
            .entry(e.dept_id.clone())
            .or_insert_with(|| (e.departamento.clone(), Aggregates::default()));
        accumulate(&mut entry.1, &e.agg);
        accumulate(&mut grand, &e.agg);

        rows.push(EmployeeReportRow {
            employee_id: e.employee_id,
            dept_id: e.dept_id,
            cedula: e.cedula,
            nombre: e.nombre,
            departamento: e.departamento,
            cargo: e.cargo,
            shift_type: e.shift_type,
            aggregates: e.agg,
            anomaly_codes: codes_vec,
            anomaly_count: count,
        });
    }

    // departments_in_order — sort by name (D-26). Use dept_seen so departments
    // that only appeared via the leaves aggregation also show up.
    let mut dept_order: Vec<DeptSummary> = dept_seen
        .into_iter()
        .map(|(id, name)| DeptSummary { id, name })
        .collect();
    dept_order.sort_by(|a, b| a.name.cmp(&b.name));

    let dept_subtotals: Vec<DeptSubtotal> = dept_order
        .iter()
        .filter_map(|d| {
            dept_to_subtotal.get(&d.id).map(|(_, agg)| DeptSubtotal {
                dept_id: d.id.clone(),
                dept_name: d.name.clone(),
                aggregates: agg.clone(),
            })
        })
        .collect();

    // 7. Audit insert AFTER aggregation succeeds (Pitfall 7 — failed reports
    //    must not write audit rows that imply success).
    write_export_audit(&conn, actor_id, params, format).await?;

    Ok(ReportPayload {
        header,
        rows,
        dept_subtotals,
        grand_total: grand,
        departments_in_order: dept_order,
    })
}

fn accumulate(into: &mut Aggregates, from: &Aggregates) {
    into.work_min = into.work_min.saturating_add(from.work_min);
    into.ot_min = into.ot_min.saturating_add(from.ot_min);
    into.late_min = into.late_min.saturating_add(from.late_min);
    into.days_worked = into.days_worked.saturating_add(from.days_worked);
    into.days_absent = into.days_absent.saturating_add(from.days_absent);
    into.work_pay_cents = into.work_pay_cents.saturating_add(from.work_pay_cents);
    into.ot_pay_cents = into.ot_pay_cents.saturating_add(from.ot_pay_cents);
    into.night_premium_cents = into
        .night_premium_cents
        .saturating_add(from.night_premium_cents);
    into.rest_day_surcharge_cents = into
        .rest_day_surcharge_cents
        .saturating_add(from.rest_day_surcharge_cents);
    into.late_deduction_cents = into
        .late_deduction_cents
        .saturating_add(from.late_deduction_cents);
    into.total_a_pagar_cents = into
        .total_a_pagar_cents
        .saturating_add(from.total_a_pagar_cents);
    into.days_ivss = into.days_ivss.saturating_add(from.days_ivss);
    into.days_vacation = into.days_vacation.saturating_add(from.days_vacation);
    into.days_permission = into.days_permission.saturating_add(from.days_permission);
    into.days_unpaid = into.days_unpaid.saturating_add(from.days_unpaid);
}

/// App-code audit insert per D-21. Captures who exported (actor_id from JWT —
/// T-05-14: never trust request body for identity), what filters were used,
/// and the export format (json/excel — Plan 05-03 reuses this helper).
async fn write_export_audit(
    conn: &Connection,
    actor_id: &str,
    params: &ReportParamsRequest,
    format: &str,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let synthetic_record_id = Uuid::new_v4().to_string();
    let payload = json!({
        "period_type": params.period_type,
        "from_date": params.from_date,
        "to_date": params.to_date,
        "filters": {
            "department_ids": params.department_ids,
            "include_inactive": params.include_inactive,
            "employee_id": params.employee_id,
            "shift_type": params.shift_type,
        },
        "format": format,
    })
    .to_string();

    conn.execute(
        "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
         VALUES (?1, 'reports', ?2, 'REPORT_EXPORT', NULL, ?3, ?4, unixepoch())",
        libsql::params![id, synthetic_record_id, payload, actor_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}
