import type { ReportPayload } from '@/types/api'

function csvEscape(s: unknown): string {
  if (s === null || s === undefined) return ''
  const str = String(s)
  if (/[",\n]/.test(str)) return `"${str.replace(/"/g, '""')}"`
  return str
}

function fmtCents(cents: number): string {
  return (cents / 100).toFixed(2)
}

// Builds a CSV file from the in-memory ReportPayload — Pencil design (M48QM)
// shows a CSV button alongside Excel/PDF. The backend has no /reports/csv
// endpoint; this renders client-side from the same JSON the UI is showing.
export function renderReportCsv(payload: ReportPayload): void {
  const headers = [
    'Cédula',
    'Nombre',
    'Departamento',
    'Cargo',
    'Min Trabajados',
    'Min Extra',
    'Min Retraso',
    'Días Trabajados',
    'Días Ausentes',
    'Pago Base',
    'Pago Extra',
    'Prima Nocturna',
    'Recargo Domingo',
    'Descuento Retraso',
    'Total a Pagar',
    'Días IVSS',
    'Días Vacación',
    'Días Permiso',
    'Días No Remunerado',
    'Anomalías',
  ]

  const rows = payload.rows.map((r) =>
    [
      r.cedula,
      r.nombre,
      r.departamento,
      r.cargo,
      r.work_min,
      r.ot_min,
      r.late_min,
      r.days_worked,
      r.days_absent,
      fmtCents(r.work_pay_cents),
      fmtCents(r.ot_pay_cents),
      fmtCents(r.night_premium_cents),
      fmtCents(r.rest_day_surcharge_cents),
      fmtCents(r.late_deduction_cents),
      fmtCents(r.total_a_pagar_cents),
      r.days_ivss,
      r.days_vacation,
      r.days_permission,
      r.days_unpaid,
      r.anomaly_codes.join('|'),
    ].map(csvEscape).join(','),
  )

  const totalsLine = [
    '',
    `TOTALES (${payload.rows.length} empleados)`,
    '',
    '',
    payload.grand_total.work_min,
    payload.grand_total.ot_min,
    payload.grand_total.late_min,
    payload.grand_total.days_worked,
    payload.grand_total.days_absent,
    fmtCents(payload.grand_total.work_pay_cents),
    fmtCents(payload.grand_total.ot_pay_cents),
    fmtCents(payload.grand_total.night_premium_cents),
    fmtCents(payload.grand_total.rest_day_surcharge_cents),
    fmtCents(payload.grand_total.late_deduction_cents),
    fmtCents(payload.grand_total.total_a_pagar_cents),
    payload.grand_total.days_ivss,
    payload.grand_total.days_vacation,
    payload.grand_total.days_permission,
    payload.grand_total.days_unpaid,
    '',
  ].map(csvEscape).join(',')

  const csv = [headers.join(','), ...rows, totalsLine].join('\n')
  // Excel-friendly UTF-8 BOM so accented characters render correctly.
  const blob = new Blob(['﻿' + csv], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `prenomina_${payload.header.from_date}_${payload.header.to_date}.csv`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}
