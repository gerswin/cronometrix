//! Excel workbook builder for the Phase 5 'Resumen' sheet (D-26..D-28).
//!
//! Layout:
//! - Rows 0-2: Branding header (D-28). Merged title, client name + RIF, period
//!   range + generated timestamp.
//! - Row 3: blank spacer.
//! - Row 4: Column headers (20 columns indexed 0-19, D-14).
//! - Row 5+: Per-employee data rows, grouped by department (D-26 sort: dept
//!   name, then employee name — already imposed by `compute_report` ordering).
//!   Per-dept subtotal row labeled `Total {Departamento}` (D-27, bold + thin
//!   top border) appears after each block; a blank spacer separates depts.
//! - Final row: `Total General` (D-27, bold + blue tint + double top border).
//!
//! Anomaly rows (anomaly_count > 0) have an amber-100 (#FEF3C7) row tint via
//! `set_row_format` (D-16).
//!
//! W-7 — API name pinning: rust_xlsxwriter 0.94.0 exposes background color via
//! `Format::set_background_color(Color)`. The legacy `set_bg_color` name does
//! NOT exist on 0.94 (renamed in 0.50+). All four background-color call sites
//! below use `set_background_color` exclusively. If a future bump to a
//! different API surface lands, recheck docs.rs/rust_xlsxwriter/<version>/
//! before silently switching names — the compiler enforces it.
//!
//! All work in this module is synchronous and CPU-bound. Callers MUST wrap it
//! in `tokio::task::spawn_blocking` (Pitfall 6).

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet};

use super::models::{Aggregates, EmployeeReportRow, ReportPayload};
use crate::errors::AppError;

const N_COLS: u16 = 20;

/// Build the xlsx workbook bytes for the given report payload.
///
/// Returns serialized bytes ready to ship as the body of an HTTP response.
/// Synchronous — callers MUST run on a blocking-friendly thread (the Axum
/// handler uses `tokio::task::spawn_blocking`).
pub fn build_workbook(payload: &ReportPayload) -> Result<Vec<u8>, AppError> {
    let mut workbook = Workbook::new();

    // -------- Pre-built formats (reuse to keep file size small) --------
    // W-7: All set_background_color call sites verified against
    // rust_xlsxwriter 0.94 docs.rs.
    let header_title = Format::new().set_bold().set_font_size(14);
    let header_meta = Format::new().set_font_size(10);
    let col_header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE5E7EB)) // gray-200 per Tailwind tokens
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);
    let money_fmt = Format::new().set_num_format("$#,##0.00");
    let money_neg = Format::new().set_num_format("$#,##0.00;[Red]-$#,##0.00");
    let int_fmt = Format::new().set_num_format("0");
    let anomaly_tint = Format::new().set_background_color(Color::RGB(0xFEF3C7)); // amber-100 per D-16
    let subtotal_fmt = Format::new()
        .set_bold()
        .set_border_top(FormatBorder::Thin);
    let subtotal_money = Format::new()
        .set_bold()
        .set_num_format("$#,##0.00")
        .set_border_top(FormatBorder::Thin);
    let subtotal_money_neg = Format::new()
        .set_bold()
        .set_num_format("$#,##0.00;[Red]-$#,##0.00")
        .set_border_top(FormatBorder::Thin);
    let subtotal_int = Format::new()
        .set_bold()
        .set_num_format("0")
        .set_border_top(FormatBorder::Thin);
    let grand_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xDBEAFE)) // blue-100 per D-27
        .set_border_top(FormatBorder::Double);
    let grand_money = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xDBEAFE))
        .set_num_format("$#,##0.00")
        .set_border_top(FormatBorder::Double);
    let grand_money_neg = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xDBEAFE))
        .set_num_format("$#,##0.00;[Red]-$#,##0.00")
        .set_border_top(FormatBorder::Double);
    let grand_int = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xDBEAFE))
        .set_num_format("0")
        .set_border_top(FormatBorder::Double);
    let plain = Format::new();

    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Resumen")
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx sheet name: {}", e)))?;

    let dash = |s: &str| {
        if s.is_empty() {
            "—".to_string()
        } else {
            s.to_string()
        }
    };

    // -------- Branding header (rows 0-2) D-28 --------
    sheet
        .merge_range(0, 0, 0, N_COLS - 1, "Reporte Pre-Nómina", &header_title)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx merge title: {}", e)))?;
    sheet
        .merge_range(
            1,
            0,
            1,
            N_COLS - 1,
            &format!(
                "{}    RIF: {}",
                dash(&payload.header.client_name),
                dash(&payload.header.client_rif)
            ),
            &header_meta,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx merge meta: {}", e)))?;
    sheet
        .merge_range(
            2,
            0,
            2,
            N_COLS - 1,
            &format!(
                "Período: {} – {}    Generado: {}",
                payload.header.from_date, payload.header.to_date, payload.header.generated_at_iso
            ),
            &header_meta,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx merge period: {}", e)))?;

    // Row 3 left blank intentionally (visual breathing room).
    // -------- Column headers (row 4) --------
    let cols = [
        "Cédula",
        "Nombre",
        "Departamento",
        "Cargo",
        "Min Trab",
        "Min Extra",
        "Min Retraso",
        "Días Trab",
        "Días Aus",
        "Pago Base",
        "Pago Extra",
        "Prima Nocturna",
        "Recargo Domingo",
        "Descuento Retraso",
        "Total a Pagar",
        "Días IVSS",
        "Días Vacación",
        "Días Permiso",
        "Días No Remunerado",
        "Anomalías",
    ];
    for (i, label) in cols.iter().enumerate() {
        sheet
            .write_with_format(4, i as u16, *label, &col_header)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx col header {}: {}", i, e)))?;
    }

    let mut row: u32 = 5;

    // -------- Data rows grouped by dept --------
    for dept in &payload.departments_in_order {
        // Find rows for this dept (preserve service-layer ordering).
        let dept_rows: Vec<&EmployeeReportRow> = payload
            .rows
            .iter()
            .filter(|r| r.dept_id == dept.id)
            .collect();
        for emp in &dept_rows {
            let is_anomaly = emp.anomaly_count > 0;
            if is_anomaly {
                // D-16: amber row tint for rows with at least one anomaly.
                sheet
                    .set_row_format(row, &anomaly_tint)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx set_row_format: {}", e)))?;
            }
            write_employee_row(sheet, row, emp, &plain, &money_fmt, &money_neg, &int_fmt)?;
            row += 1;
        }

        // Per-dept subtotal row D-27.
        if let Some(sub) = payload
            .dept_subtotals
            .iter()
            .find(|s| s.dept_id == dept.id)
        {
            sheet
                .write_with_format(row, 1, &format!("Total {}", dept.name), &subtotal_fmt)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx subtotal label: {}", e)))?;
            write_aggregate_row(
                sheet,
                row,
                &sub.aggregates,
                &subtotal_int,
                &subtotal_money,
                &subtotal_money_neg,
            )?;
            row += 1;
        }
        row += 1; // blank spacer between departments
    }

    // -------- Grand total D-27 --------
    sheet
        .write_with_format(row, 1, "Total General", &grand_fmt)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx grand label: {}", e)))?;
    write_aggregate_row(
        sheet,
        row,
        &payload.grand_total,
        &grand_int,
        &grand_money,
        &grand_money_neg,
    )?;

    sheet
        .set_freeze_panes(5, 0)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx freeze: {}", e)))?;
    sheet.autofit();

    workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx save_to_buffer: {}", e)))
}

fn write_employee_row(
    sheet: &mut Worksheet,
    row: u32,
    emp: &EmployeeReportRow,
    plain: &Format,
    money_fmt: &Format,
    money_neg: &Format,
    int_fmt: &Format,
) -> Result<(), AppError> {
    fn to_dash(s: &str) -> &str {
        if s.is_empty() {
            "—"
        } else {
            s
        }
    }
    let map_err = |e: rust_xlsxwriter::XlsxError| {
        AppError::Internal(anyhow::anyhow!("xlsx write: {}", e))
    };

    sheet
        .write_with_format(row, 0, to_dash(&emp.cedula), plain)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 1, to_dash(&emp.nombre), plain)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 2, to_dash(&emp.departamento), plain)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 3, to_dash(&emp.cargo), plain)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 4, emp.aggregates.work_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 5, emp.aggregates.ot_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 6, emp.aggregates.late_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 7, emp.aggregates.days_worked as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 8, emp.aggregates.days_absent as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            9,
            emp.aggregates.work_pay_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            10,
            emp.aggregates.ot_pay_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            11,
            emp.aggregates.night_premium_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            12,
            emp.aggregates.rest_day_surcharge_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            13,
            -(emp.aggregates.late_deduction_cents as f64 / 100.0),
            money_neg,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            14,
            emp.aggregates.total_a_pagar_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 15, emp.aggregates.days_ivss as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 16, emp.aggregates.days_vacation as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 17, emp.aggregates.days_permission as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 18, emp.aggregates.days_unpaid as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 19, emp.anomaly_codes.join(", "), plain)
        .map_err(map_err)?;
    Ok(())
}

fn write_aggregate_row(
    sheet: &mut Worksheet,
    row: u32,
    a: &Aggregates,
    int_fmt: &Format,
    money_fmt: &Format,
    money_neg: &Format,
) -> Result<(), AppError> {
    let map_err = |e: rust_xlsxwriter::XlsxError| {
        AppError::Internal(anyhow::anyhow!("xlsx write: {}", e))
    };

    sheet
        .write_with_format(row, 4, a.work_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 5, a.ot_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 6, a.late_min as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 7, a.days_worked as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 8, a.days_absent as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 9, a.work_pay_cents as f64 / 100.0, money_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 10, a.ot_pay_cents as f64 / 100.0, money_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 11, a.night_premium_cents as f64 / 100.0, money_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            12,
            a.rest_day_surcharge_cents as f64 / 100.0,
            money_fmt,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(
            row,
            13,
            -(a.late_deduction_cents as f64 / 100.0),
            money_neg,
        )
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 14, a.total_a_pagar_cents as f64 / 100.0, money_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 15, a.days_ivss as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 16, a.days_vacation as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 17, a.days_permission as f64, int_fmt)
        .map_err(map_err)?;
    sheet
        .write_with_format(row, 18, a.days_unpaid as f64, int_fmt)
        .map_err(map_err)?;
    Ok(())
}
