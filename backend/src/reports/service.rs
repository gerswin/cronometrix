//! Reports service — SQL aggregation across daily_records + overrides + leaves
//! + anomalies, plus secondary leaves aggregation (W-5), money math (LOTTT
//!   Art. 117/118/120), and completed-export audit recording (D-21).
//!
//! `compute_report` is read-only. `record_export` is the only write and routes
//! through the serialized writer after the requested artifact is complete
//! (Pitfall 7: failed reports must not leak audit rows that imply success).
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
use crate::{
    common::epoch_to_iso, employees::models::SalaryKind, errors::AppError, state::AppState,
};

/// H-08: a row cannot be monetized without a valid `salary_kind`. Callers use
/// this only at the money-math call sites (the leave branches that never pay —
/// medical/unpaid/manual — don't call it, since no monetization happens there).
/// A missing/unrecognized unit is never assumed to be `Daily` — that
/// assumption is exactly how the H-08 ambiguity came back in the first place.
///
/// I7: this used to return `Result` and get `?`-propagated, so ONE employee
/// with a NULL `salary_kind` produced a 500 for the whole report — every
/// OTHER employee's payroll became uncomputable because of one unrelated
/// row. A missing `hire_date`, by contrast, only ever raised a per-row
/// anomaly (`HIRE_DATE_MISSING`). Returning `Option` instead lets the caller
/// degrade the same way: zero this row's money and attach
/// `SALARY_KIND_MISSING` to its `anomaly_codes`, while the rest of the
/// report — every other employee, every other row — computes normally.
fn require_salary_kind(raw: Option<&str>) -> Option<SalaryKind> {
    raw.and_then(SalaryKind::from_db_str)
}

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
    /// C-05: employment window bounds for días_ausentes. `None` hire_date is
    /// NOT treated as "no lower bound forever" silently — the días_ausentes
    /// loop below flags it with an anomaly, since an employee with an
    /// unknown start date cannot be computed exactly.
    hire_date: Option<NaiveDate>,
    /// `None` = still employed (migration 026: nullable = sigue activo).
    terminated_on: Option<NaiveDate>,
}

/// Convert an optional epoch-seconds (UTC midnight) column to an optional
/// `NaiveDate`. Mirrors `employees::service::epoch_to_iso_date_opt` but
/// returns a `NaiveDate` since the días_ausentes loop compares dates
/// directly, not ISO strings.
fn epoch_to_naive_date(epoch: Option<i64>) -> Option<NaiveDate> {
    epoch.and_then(|t| {
        chrono::DateTime::<chrono::Utc>::from_timestamp(t, 0).map(|dt| dt.naive_utc().date())
    })
}

pub async fn compute_report(
    state: &AppState,
    params: &ReportParamsRequest,
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

    // M-05: read the whole report from a single consistent snapshot. The three
    // queries below run in autocommit otherwise, so a write committed between
    // them can split the report across two states (e.g. leave counts that the
    // main aggregation never saw). Under WAL this BEGIN gives one stable view
    // for every read until the COMMIT before we return, and does not block the
    // single writer. On any early `?` return the snapshot is rolled back when
    // `conn` drops.
    crate::db::begin_read_snapshot(&conn)
        .await
        .map_err(AppError::Internal)?;

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
    //
    // C-05: the report's universe is now `employees`, not `daily_records`
    // (see the FROM/JOIN below) — an employee is no longer required to have
    // a matching daily_record to appear. The period bound therefore CANNOT
    // live in this WHERE-built `predicates` list: a WHERE-side
    // `dr.anchor_date BETWEEN ?1 AND ?2` filters out every row where dr.* is
    // NULL (no record that day), which silently degrades the LEFT JOIN back
    // into an INNER JOIN and brings the defect back with every test green.
    // ?1/?2 are reserved for `from`/`to` and consumed inside the JOIN's ON
    // clause in the SQL template below.
    let mut predicates: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = vec![
        libsql::Value::Text(from.to_string()),
        libsql::Value::Text(to.to_string()),
    ];

    // C-05: `status` ('active'/'inactive') no longer gates the report.
    // "Who gets paid for this period?" is an employment-validity question
    // (hire_date/terminated_on vs. the period — see the días_ausentes loop
    // below), not a current-state question: an inactive employee who worked
    // days in the period must still be paid for them, and `status='active'`
    // used to make that employee (and their pending pay) disappear entirely.
    // `include_inactive` is kept on the request/audit payload for API
    // compatibility but no longer participates in this predicate — `status`
    // still governs employee-picker UI elsewhere, just not payroll validity.

    // I5: C-05 removed `status` as a universe gate but did not replace the
    // bound it was implicitly providing — without this, someone deactivated
    // in 2024 produces an all-zeros row in every 2026 report forever, since
    // nothing else ever excludes them. Bound the universe to employment that
    // OVERLAPS [from..to] instead: these predicates read e.hire_date /
    // e.terminated_on, columns of `employees` (not `daily_records`), so they
    // are safe in the WHERE-built `predicates` list — unlike `shift_type`
    // below, which had to move into the JOIN's ON clause because it reads
    // dr.*. NULL means "unbounded on that side" (hire date unknown / still
    // employed), never "exclude": an employee with a NULL column on either
    // side must not be dropped by these predicates.
    let from_epoch = from
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always a valid time")
        .and_utc()
        .timestamp();
    let to_epoch = to
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always a valid time")
        .and_utc()
        .timestamp();
    predicates.push(format!(
        "(e.terminated_on IS NULL OR e.terminated_on >= ?{})",
        values.len() + 1
    ));
    values.push(libsql::Value::Integer(from_epoch));
    predicates.push(format!(
        "(e.hire_date IS NULL OR e.hire_date <= ?{})",
        values.len() + 1
    ));
    values.push(libsql::Value::Integer(to_epoch));

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
    //
    // Important 1 fix: this predicate MUST NOT go into the WHERE-built
    // `predicates` list above. `dr.shift_type` is a column of the LEFT-JOINed
    // `daily_records` table; on a row where an employee has zero records that
    // day (or in the whole period) `dr.*` is NULL, and `dr.shift_type = ?`
    // evaluates to NULL (neither true nor false) there — WHERE then drops the
    // row. That silently re-degrades the LEFT JOIN back into an INNER JOIN
    // for exactly the rows C-05 exists to keep (an employee with no records
    // in the period vanishes again, and once they vanish EVERY day of the
    // *other* shift type gets counted as an absence, because `worked_dates`
    // only ever collects days that survived the filter). The fix is to
    // scope this predicate inside the JOIN's own ON clause instead, right
    // beside the period bound — see the `sql` template below — so it only
    // ever excludes real daily_records rows, never the employee's own
    // NULL-filled placeholder row.
    let mut shift_type_on_clause = String::new();
    if let Some(st) = &params.shift_type {
        shift_type_on_clause = format!(" AND dr.shift_type = ?{}", values.len() + 1);
        values.push(libsql::Value::Text(st.clone()));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    // C-05: FROM inverted to `employees` — the report's universe is now
    // "who is employed", not "who has a daily_record". `daily_records`
    // (and everything hanging off it: overrides, leaves) moves to a LEFT
    // JOIN scoped to the period INSIDE the ON clause, so an employee with
    // zero records in [from..to] still produces exactly one row (all dr.*
    // NULL) instead of vanishing. `e.hire_date` / `e.terminated_on` are
    // selected so the días_ausentes loop below can bound absences to the
    // employment window instead of the whole period.
    //
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
            (SELECT GROUP_CONCAT(code) FROM daily_record_anomalies WHERE daily_record_id = dr.id) AS anomaly_codes, \
            e.salary_kind, \
            e.hire_date, \
            e.terminated_on \
         FROM employees e \
         JOIN departments d ON d.id = e.department_id \
         LEFT JOIN daily_records dr \
                ON dr.employee_id = e.id \
               AND dr.anchor_date BETWEEN ?1 AND ?2 \
               {shift_type_on_clause} \
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
        // C-05: everything below is a column of `daily_records` (directly or
        // via a table joined off it), which is now a LEFT JOIN — every one
        // of these reads Option instead of the bare type it used to be, and
        // MUST be treated as "no record that day", not defaulted blindly
        // to a type-appropriate zero at the read site (the anchor_date NULL
        // case below is intentionally NOT defaulted — see the guard).
        let day_shift_type_opt: Option<String> = row.get(8).ok(); // W-6
        let anchor_date_str_opt: Option<String> = row.get(9).ok();
        let work_minutes_opt: Option<i64> = row.get(10).ok();
        let overtime_minutes_opt: Option<i64> = row.get(11).ok();
        let late_minutes_opt: Option<i64> = row.get(12).ok();
        let is_rest_day_worked_opt: Option<i64> = row.get(13).ok();
        let _leave_id_opt: Option<String> = row.get(14).ok();
        let leave_type_opt: Option<String> = row.get(15).ok();
        let override_work_min_opt: Option<i64> = row.get(16).ok();
        let anomaly_codes_str_opt: Option<String> = row.get(17).ok();
        let salary_kind_str_opt: Option<String> = row.get(18).ok();
        let hire_date_opt = epoch_to_naive_date(row.get(19).ok());
        let terminated_on_opt = epoch_to_naive_date(row.get(20).ok());

        dept_seen
            .entry(dept_id.clone())
            .or_insert(dept_name.clone());

        // C-05: the entry is created unconditionally, BEFORE the anchor_date
        // guard below — an employee with zero daily_records in the period
        // must still appear (that is the whole point of the FROM inversion).
        let entry = acc.entry(employee_id.clone()).or_insert_with(|| AccRow {
            employee_id: employee_id.clone(),
            dept_id: dept_id.clone(),
            cedula: cedula.clone(),
            nombre: nombre.clone(),
            departamento: dept_name.clone(),
            cargo: cargo.clone(),
            shift_type: day_shift_type_opt.clone().unwrap_or_default(),
            agg: Aggregates::default(),
            anomaly_codes_set: BTreeSet::new(),
            worked_dates: HashSet::new(),
            leave_dates: HashSet::new(),
            hire_date: hire_date_opt,
            terminated_on: terminated_on_opt,
        });

        // C-05: `dr.anchor_date` NULL means "no daily_record this day" — the
        // LEFT JOIN produced this row purely to carry the employee into the
        // report. There is no work/leave/anomaly data to accumulate; paying
        // for or attaching a leave overlay to a day nobody clocked would pay
        // for a day nobody worked. días_ausentes for this employee is
        // computed later from `worked_dates`/`leave_dates` staying empty.
        let Some(anchor_date_str) = anchor_date_str_opt else {
            continue;
        };
        let anchor_date = NaiveDate::parse_from_str(&anchor_date_str, "%Y-%m-%d")
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let day_shift_type = day_shift_type_opt.unwrap_or_default();
        let work_minutes = work_minutes_opt.unwrap_or(0);
        let overtime_minutes = overtime_minutes_opt.unwrap_or(0);
        let late_minutes = late_minutes_opt.unwrap_or(0);
        let is_rest_day_worked = is_rest_day_worked_opt.unwrap_or(0);

        // Override merge — operator edits invisible if skipped (Pitfall 3).
        let effective_work_min = override_work_min_opt.unwrap_or(work_minutes);

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
                match require_salary_kind(salary_kind_str_opt.as_deref()) {
                    Some(salary_kind) => {
                        let work_pay = money::work_pay_cents(
                            ordinary_daily_minutes,
                            base_salary_cents,
                            ordinary_daily_minutes,
                            salary_kind,
                        );
                        entry.agg.work_pay_cents =
                            entry.agg.work_pay_cents.saturating_add(work_pay);
                        entry.agg.total_a_pagar_cents =
                            entry.agg.total_a_pagar_cents.saturating_add(work_pay);
                    }
                    None => {
                        // I7: this row does not monetize — amounts stay at
                        // zero, exactly like a day with no daily_record.
                        // Never assume `Daily` to keep going; that is the
                        // H-08 defect verbatim (a monthly salary paid every
                        // day).
                        entry
                            .anomaly_codes_set
                            .insert("SALARY_KIND_MISSING".to_string());
                    }
                }
            }
            Some("unpaid") => {
                entry.leave_dates.insert(anchor_date);
            }
            Some("manual") => {
                entry.leave_dates.insert(anchor_date);
            }
            _ => {
                // Standard work day. I7: a missing salary_kind must not fail
                // the whole report over one row — this used to `?`-propagate
                // AppError::CalcError, so ONE employee with a NULL
                // salary_kind produced a 500 for every OTHER employee too. It
                // now degrades to a per-row anomaly, the same treatment
                // HIRE_DATE_MISSING already gets: money amounts stay at zero
                // (`salary_kind_opt` is threaded through as `None` below,
                // never assumed to be `Daily` — that assumption is the H-08
                // defect verbatim) while everything computed directly from
                // daily_records (minutes worked, late minutes, days_worked)
                // is not "invented" data and still accumulates normally —
                // skipping it would falsely mark a day someone demonstrably
                // worked as an absence in the días_ausentes loop below, over
                // an unrelated payroll-config gap.
                let salary_kind_opt = require_salary_kind(salary_kind_str_opt.as_deref());
                if salary_kind_opt.is_none() {
                    entry
                        .anomaly_codes_set
                        .insert("SALARY_KIND_MISSING".to_string());
                }

                // C-01: `overtime_minutes` is a SUBSET of the worked minutes
                // (calc/engine.rs:82). Paying the full total at the ordinary
                // rate and ALSO adding the overtime slice at 150% charges every
                // overtime minute at 250%. The ordinary base excludes the
                // overtime slice; the premium is contributed by ot_pay_cents.
                // `effective_work_min` may come from an override while
                // `overtime_minutes` is the original engine value — hence the
                // `.max(0)`. The override not recomputing the overtime slice
                // is H-07, out of scope here; this comment is for the next
                // reader who wonders why the two can disagree.
                let ordinary_min = (effective_work_min - overtime_minutes).max(0);
                // Important 2: an overtime minute on a premium day earns the
                // Art. 118 overtime surcharge (+50%) on top of the
                // ALREADY-LOADED hour (night +30% / rest day +50%), not a
                // flat 1.5x plus a separately-added day premium. `day_premium_pct`
                // is the day's own rate (100/130/150/180 — see
                // `night`/`rest` gating just below, which decide it) and is
                // multiplied into `ot_pay_cents`'s single division, never
                // summed on top afterward.
                let day_premium_pct = 100
                    + if day_shift_type == "night" { 30 } else { 0 }
                    + if is_rest_day_worked == 1 { 50 } else { 0 };

                // I7: money math only runs when salary_kind is present; a
                // `None` row keeps every cents figure at 0 rather than
                // guessing a unit.
                let (work_pay, ot_pay, night, rest, late, total) = match salary_kind_opt {
                    Some(salary_kind) => {
                        let work_pay = money::work_pay_cents(
                            ordinary_min,
                            base_salary_cents,
                            ordinary_daily_minutes,
                            salary_kind,
                        );
                        let ot_pay = money::ot_pay_cents(
                            overtime_minutes,
                            base_salary_cents,
                            ordinary_daily_minutes,
                            salary_kind,
                            day_premium_pct,
                        );
                        // W-6: night premium gates on dr.shift_type (per-day
                        // actual shift), NOT departments.shift_type. The
                        // engine's per-day output is authoritative for what
                        // actually happened.
                        //
                        // Important 2: uses `ordinary_min`, NOT
                        // `effective_work_min` — the overtime slice's night
                        // premium is already folded into `ot_pay` above
                        // (multiplicatively via `day_premium_pct`). Passing
                        // the full effective minutes here would additively
                        // double-apply the night premium to the overtime
                        // slice on top of that. When there is no overtime,
                        // `ordinary_min == effective_work_min`, so the
                        // ordinary-hours case (no OT) is byte-for-byte
                        // unchanged.
                        let night = if day_shift_type == "night" {
                            money::night_premium_cents(
                                ordinary_min,
                                base_salary_cents,
                                ordinary_daily_minutes,
                                salary_kind,
                            )
                        } else {
                            0
                        };
                        let rest = if is_rest_day_worked == 1 {
                            money::rest_day_surcharge_cents(
                                ordinary_min,
                                base_salary_cents,
                                ordinary_daily_minutes,
                                salary_kind,
                            )
                        } else {
                            0
                        };
                        // C-02: `work_minutes` is measured between the real
                        // entry and exit (calc/engine.rs:70-76), so arriving
                        // late already shrinks the paid minutes. `late` is
                        // kept as an informational metric — reports/excel.rs
                        // no longer renders it as its own money column
                        // (Critical 2 removed that column entirely; the "Min
                        // Retraso" minutes column carries this metric
                        // instead) — but it must NOT be subtracted from the
                        // total again; that would charge the same lateness
                        // twice.
                        let late = money::late_deduction_cents(
                            late_minutes,
                            base_salary_cents,
                            ordinary_daily_minutes,
                            salary_kind,
                        );
                        let total = money::total_a_pagar_cents(work_pay, ot_pay, night, rest, 0);
                        (work_pay, ot_pay, night, rest, late, total)
                    }
                    None => (0, 0, 0, 0, 0, 0),
                };

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
    // C-05: `e.status` removed as a gate here too, symmetric with the main
    // query above — employment validity for leave-day counts is decided by
    // hire_date/terminated_on, not the current status flag.
    //
    // I5: same universe bound as the main query, symmetric for the same
    // reason — without it, an employee whose employment window does not
    // overlap [from..to] could still be pulled into `acc` (and thus the
    // report) purely by way of a leave row whose dates happen to fall in
    // the period. `e.hire_date` / `e.terminated_on` here too, so this is a
    // safe WHERE predicate (this query INNER JOINs employees, no LEFT JOIN
    // to degrade).
    leave_predicates.push(format!(
        "(e.terminated_on IS NULL OR e.terminated_on >= ?{})",
        leave_values.len() + 1
    ));
    leave_values.push(libsql::Value::Integer(from_epoch));
    leave_predicates.push(format!(
        "(e.hire_date IS NULL OR e.hire_date <= ?{})",
        leave_values.len() + 1
    ));
    leave_values.push(libsql::Value::Integer(to_epoch));
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
    // M-04: the main query's `shift_type` filter (see the `shift_type_on_clause`
    // comment above) was never mirrored here, so a report filtered to a single
    // shift silently pulled in leave days from every other shift. Unlike the
    // main query, this predicate is safe to add straight to the WHERE-built
    // `leave_predicates` list: this query INNER JOINs `departments d` (no
    // LEFT JOIN to degrade — every leave row necessarily has an employee and
    // a department), so `d.shift_type = ?` never turns a real row NULL the
    // way `dr.shift_type = ?` would on the LEFT-JOINed `daily_records`. This
    // query also has no per-day `daily_records` row to read a per-day actual
    // shift from — `d.shift_type` (department policy) is the same value
    // already used as the display fallback for leave-only employees a few
    // lines below (`l_dept_shift`), so filtering on it keeps both uses of
    // "shift" for a leave-only day consistent with each other.
    if let Some(st) = &params.shift_type {
        leave_predicates.push(format!("d.shift_type = ?{}", leave_values.len() + 1));
        leave_values.push(libsql::Value::Text(st.clone()));
    }
    let leave_where = format!("WHERE {}", leave_predicates.join(" AND "));

    let leave_sql = format!(
        "SELECT l.employee_id, l.leave_type, l.from_date, l.to_date, \
                e.employee_code, e.name, e.position, \
                d.id AS dept_id, d.name AS dept_name, \
                e.base_salary_cents, d.ordinary_daily_minutes, \
                d.shift_type AS dept_shift_type, \
                e.hire_date, e.terminated_on \
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
        let l_hire_date = epoch_to_naive_date(lr.get(12).ok());
        let l_terminated_on = epoch_to_naive_date(lr.get(13).ok());

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
            hire_date: l_hire_date,
            terminated_on: l_terminated_on,
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
        // C-05: an employee with no hire_date cannot be bounded — silently
        // assuming the period start would understate or overstate absences
        // without any signal that the number is unreliable. Surface it as a
        // visible anomaly instead (never silently absorbed into the period
        // bound).
        if entry.hire_date.is_none() {
            entry.anomaly_codes_set.insert("HIRE_DATE_MISSING".to_string());
        }
        // C-05: absences only accrue while the employment relationship
        // existed. Without this bound, someone hired yesterday shows a full
        // period of absences, and someone terminated mid-period keeps
        // accruing absences for days after they left.
        let absent = weekdays_in_period
            .iter()
            .filter(|d| entry.hire_date.is_none_or(|h| **d >= h))
            .filter(|d| entry.terminated_on.is_none_or(|t| **d <= t))
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

    // M-05: close the consistent-read snapshot now that every query has run.
    crate::db::commit_read_snapshot(&conn)
        .await
        .map_err(AppError::Internal)?;

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
pub async fn record_export(
    state: &AppState,
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

    state
        .db_write
        .statement(
            "reports.record-export",
            "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
             VALUES (?1, 'reports', ?2, 'REPORT_EXPORT', NULL, ?3, ?4, unixepoch())",
            vec![
                libsql::Value::Text(id),
                libsql::Value::Text(synthetic_record_id),
                libsql::Value::Text(payload),
                libsql::Value::Text(actor_id.to_string()),
            ],
        )
        .await?;
    Ok(())
}
