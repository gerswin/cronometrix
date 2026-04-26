// D-23 / D-29: client-side PDF rendering of pre-payroll reports.
// Uses jspdf 4.2.x + jspdf-autotable 5.x. Landscape A4, repeated header
// row, page footer "Página N de M", anomaly rows tinted amber-100,
// subtotal + grand-total rows bold, grand-total tinted blue-100.

import { jsPDF } from 'jspdf'
import autoTable from 'jspdf-autotable'
import type { ReportPayload } from '@/types/api'
import { fmtMoney } from '@/lib/format/currency'

const COLUMN_HEADERS = [
  'Cédula',
  'Nombre',
  'Depto',
  'Cargo',
  'Min Trab',
  'Min Extra',
  'Min Retr',
  'Días T',
  'Días A',
  'Pago Base',
  'Pago Extra',
  'Prima Noc',
  'Recargo Dom',
  'Desc Retr',
  'Total',
  'IVSS',
  'Vac',
  'Perm',
  'No Rem',
  'Anom',
] as const

/**
 * Render the given report payload as a downloadable PDF (landscape A4).
 *
 * Side effects: triggers `doc.save(...)` which causes the browser to
 * download the file. Test seams: `jspdf` and `jspdf-autotable` are mocked
 * in unit tests; vitest captures the autotable body argument and the
 * doc.text calls to assert structure.
 */
export function renderReportPdf(payload: ReportPayload): void {
  const doc = new jsPDF({ orientation: 'landscape', format: 'a4' })

  // Deterministic creation date so PDF byte-output is reproducible per
  // payload. Only used by snapshot/contract tests. The jspdf 4.x typings
  // omit `creationDate` from DocumentProperties even though it is a
  // valid runtime property; cast through `Record<string, unknown>` to
  // sidestep the type gap rather than swap to a less-specific signature.
  doc.setProperties({
    creationDate: new Date(payload.header.generated_at_iso),
  } as unknown as Parameters<typeof doc.setProperties>[0])

  // Branding header (D-28). Empty client_name / client_rif render as `—`.
  doc.setFontSize(16)
  doc.setFont('helvetica', 'bold')
  doc.text('Reporte Pre-Nómina', 14, 14)
  doc.setFontSize(10)
  doc.setFont('helvetica', 'normal')
  doc.text(
    `${payload.header.client_name || '—'}    RIF: ${payload.header.client_rif || '—'}`,
    14,
    22,
  )
  doc.text(
    `Período: ${payload.header.from_date} – ${payload.header.to_date}    Generado: ${payload.header.generated_at_iso}`,
    14,
    28,
  )

  const head = [Array.from(COLUMN_HEADERS)]
  const body: (string | number)[][] = []

  for (const dept of payload.departments_in_order) {
    const deptRows = payload.rows.filter((r) => r.dept_id === dept.id)
    for (const r of deptRows) {
      body.push([
        r.cedula || '—',
        r.nombre,
        r.departamento,
        r.cargo || '—',
        r.work_min,
        r.ot_min,
        r.late_min,
        r.days_worked,
        r.days_absent,
        fmtMoney(r.work_pay_cents),
        fmtMoney(r.ot_pay_cents),
        fmtMoney(r.night_premium_cents),
        fmtMoney(r.rest_day_surcharge_cents),
        '-' + fmtMoney(r.late_deduction_cents),
        fmtMoney(r.total_a_pagar_cents),
        r.days_ivss,
        r.days_vacation,
        r.days_permission,
        r.days_unpaid,
        r.anomaly_codes.join(', '),
      ])
    }
    const sub = payload.dept_subtotals.find((s) => s.dept_id === dept.id)
    if (sub) {
      body.push([
        '',
        `Total ${dept.name}`,
        '',
        '',
        sub.aggregates.work_min,
        sub.aggregates.ot_min,
        sub.aggregates.late_min,
        sub.aggregates.days_worked,
        sub.aggregates.days_absent,
        fmtMoney(sub.aggregates.work_pay_cents),
        fmtMoney(sub.aggregates.ot_pay_cents),
        fmtMoney(sub.aggregates.night_premium_cents),
        fmtMoney(sub.aggregates.rest_day_surcharge_cents),
        '-' + fmtMoney(sub.aggregates.late_deduction_cents),
        fmtMoney(sub.aggregates.total_a_pagar_cents),
        sub.aggregates.days_ivss,
        sub.aggregates.days_vacation,
        sub.aggregates.days_permission,
        sub.aggregates.days_unpaid,
        '',
      ])
    }
  }

  const g = payload.grand_total
  body.push([
    '',
    'TOTAL GENERAL',
    '',
    '',
    g.work_min,
    g.ot_min,
    g.late_min,
    g.days_worked,
    g.days_absent,
    fmtMoney(g.work_pay_cents),
    fmtMoney(g.ot_pay_cents),
    fmtMoney(g.night_premium_cents),
    fmtMoney(g.rest_day_surcharge_cents),
    '-' + fmtMoney(g.late_deduction_cents),
    fmtMoney(g.total_a_pagar_cents),
    g.days_ivss,
    g.days_vacation,
    g.days_permission,
    g.days_unpaid,
    '',
  ])

  autoTable(doc, {
    head,
    body,
    startY: 34,
    showHead: 'everyPage',
    styles: {
      font: 'helvetica',
      fontSize: 7,
      cellPadding: 1.5,
      overflow: 'linebreak',
    },
    headStyles: { fillColor: [30, 41, 59], textColor: 255, fontStyle: 'bold' },
    didParseCell: (hook) => {
      const raw = hook.row.raw as (string | number)[]
      if (hook.section !== 'body') return
      const labelCell = String(raw[1] ?? '')
      const anomalyText = String(raw[19] ?? '')
      if (labelCell === 'TOTAL GENERAL') {
        hook.cell.styles.fontStyle = 'bold'
        hook.cell.styles.fillColor = [219, 234, 254] // blue-100
      } else if (labelCell.startsWith('Total ')) {
        hook.cell.styles.fontStyle = 'bold'
        hook.cell.styles.fillColor = [241, 245, 249] // slate-100
      } else if (anomalyText.length > 0) {
        hook.cell.styles.fillColor = [254, 243, 199] // amber-100
      }
    },
    didDrawPage: (data) => {
      const ps = doc.internal.pageSize
      const num = data.pageNumber
      const cnt = doc.getNumberOfPages()
      doc.setFontSize(8)
      doc.text(`Página ${num} de ${cnt}`, ps.width - 14, ps.height - 6, {
        align: 'right',
      })
    },
  })

  doc.save(
    `prenomina_${payload.header.from_date}_${payload.header.to_date}.pdf`,
  )
}
