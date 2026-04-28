/**
 * Branch-coverage extension for src/lib/reports/pdf.ts. The existing
 * pdf.test.ts asserts the happy-path scaffolding (title, branding,
 * filename, em-dash fallbacks, accents, TOTAL GENERAL row, dept subtotal,
 * landscape orientation, footer hook). This file fills the branch gap:
 *  - body row generation for an actual employee row (cedula em-dash branch)
 *  - didParseCell branch coverage: TOTAL GENERAL, dept subtotal, anomaly row,
 *    plain row (no shading)
 *  - footer hook with multi-page (getNumberOfPages > 1)
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { ReportPayload } from '@/types/api'

interface ParsedCellHook {
  section: string
  row: { raw: (string | number)[] }
  cell: { styles: { fontStyle?: string; fillColor?: number[] } }
}

interface DrawPageHook {
  pageNumber: number
}

interface CapturedAutoTable {
  doc: unknown
  options: {
    body: (string | number)[][]
    didParseCell: (h: ParsedCellHook) => void
    didDrawPage: (h: DrawPageHook) => void
    head: unknown
  }
}

const textCalls: { args: unknown[] }[] = []
const saveCalls: { filename: string }[] = []
const autoTableCalls: CapturedAutoTable[] = []
let mockNumberOfPages = 1

vi.mock('jspdf', () => {
  class MockJsPDF {
    internal = { pageSize: { width: 297, height: 210 } }
    setProperties() {}
    setFontSize() {}
    setFont() {}
    text(...args: unknown[]) { textCalls.push({ args }) }
    save(filename: string) { saveCalls.push({ filename }) }
    getNumberOfPages() { return mockNumberOfPages }
  }
  return { jsPDF: MockJsPDF }
})

vi.mock('jspdf-autotable', () => {
  const fn = (doc: unknown, options: CapturedAutoTable['options']) => {
    autoTableCalls.push({ doc, options })
  }
  return { default: fn }
})

import { renderReportPdf } from '../pdf'

const ZEROS = {
  work_min: 0, ot_min: 0, late_min: 0, days_worked: 0, days_absent: 0,
  work_pay_cents: 0, ot_pay_cents: 0, night_premium_cents: 0,
  rest_day_surcharge_cents: 0, late_deduction_cents: 0, total_a_pagar_cents: 0,
  days_ivss: 0, days_vacation: 0, days_permission: 0, days_unpaid: 0,
}

function payload(over: Partial<ReportPayload> = {}): ReportPayload {
  return {
    header: { client_name: 'Acme', client_rif: 'J-1', from_date: '2026-04-01', to_date: '2026-04-30', generated_at_iso: '2026-04-25T18:00:00Z' },
    rows: [], dept_subtotals: [], grand_total: { ...ZEROS },
    departments_in_order: [], ...over,
  }
}

beforeEach(() => {
  textCalls.length = 0
  saveCalls.length = 0
  autoTableCalls.length = 0
  mockNumberOfPages = 1
})

describe('renderReportPdf — extra branch coverage', () => {
  it('renders a normal employee row with cedula and cargo present (no em-dash branch)', () => {
    renderReportPdf(payload({
      rows: [{
        employee_id: 'e1', dept_id: 'd1', cedula: 'V-12345678', nombre: 'Ana García',
        departamento: 'Operaciones', cargo: 'Analista', shift_type: 'day',
        anomaly_codes: [], anomaly_count: 0, ...ZEROS,
      }],
      departments_in_order: [{ id: 'd1', name: 'Operaciones' }],
    }))
    const body = autoTableCalls[0].options.body
    const empRow = body.find((r) => r[1] === 'Ana García')!
    expect(empRow[0]).toBe('V-12345678')
    expect(empRow[3]).toBe('Analista')
  })

  it('renders em-dash fallback for missing cedula and missing cargo on an employee row', () => {
    renderReportPdf(payload({
      rows: [{
        employee_id: 'e1', dept_id: 'd1', cedula: '', nombre: 'No-Cedula',
        departamento: 'X', cargo: '', shift_type: 'day',
        anomaly_codes: [], anomaly_count: 0, ...ZEROS,
      }],
      departments_in_order: [{ id: 'd1', name: 'X' }],
    }))
    const body = autoTableCalls[0].options.body
    const r = body.find((row) => row[1] === 'No-Cedula')!
    expect(r[0]).toBe('—')
    expect(r[3]).toBe('—')
  })

  it('didParseCell: TOTAL GENERAL row gets bold + blue-100 fill', () => {
    renderReportPdf(payload())
    const { didParseCell, body } = autoTableCalls[0].options
    const totalRow = body.find((r) => r[1] === 'TOTAL GENERAL')!
    const cell = { styles: {} as { fontStyle?: string; fillColor?: number[] } }
    didParseCell({ section: 'body', row: { raw: totalRow }, cell })
    expect(cell.styles.fontStyle).toBe('bold')
    expect(cell.styles.fillColor).toEqual([219, 234, 254])
  })

  it('didParseCell: dept subtotal "Total {Dept}" gets bold + slate-100 fill', () => {
    renderReportPdf(payload({
      departments_in_order: [{ id: 'd1', name: 'Operaciones' }],
      dept_subtotals: [{ dept_id: 'd1', dept_name: 'Operaciones', aggregates: { ...ZEROS } }],
    }))
    const { didParseCell, body } = autoTableCalls[0].options
    const subRow = body.find((r) => r[1] === 'Total Operaciones')!
    const cell = { styles: {} as { fontStyle?: string; fillColor?: number[] } }
    didParseCell({ section: 'body', row: { raw: subRow }, cell })
    expect(cell.styles.fontStyle).toBe('bold')
    expect(cell.styles.fillColor).toEqual([241, 245, 249])
  })

  it('didParseCell: anomaly row (anomaly_codes non-empty) gets amber-100 fill', () => {
    renderReportPdf(payload({
      rows: [{
        employee_id: 'e1', dept_id: 'd1', cedula: 'C', nombre: 'Anom Person',
        departamento: 'X', cargo: 'C', shift_type: 'day',
        anomaly_codes: ['LATE', 'NO_EXIT'], anomaly_count: 2, ...ZEROS,
      }],
      departments_in_order: [{ id: 'd1', name: 'X' }],
    }))
    const { didParseCell, body } = autoTableCalls[0].options
    const r = body.find((row) => row[1] === 'Anom Person')!
    const cell = { styles: {} as { fontStyle?: string; fillColor?: number[] } }
    didParseCell({ section: 'body', row: { raw: r }, cell })
    expect(cell.styles.fillColor).toEqual([254, 243, 199])
  })

  it('didParseCell: plain employee row with no anomalies gets NO fillColor or bold', () => {
    renderReportPdf(payload({
      rows: [{
        employee_id: 'e1', dept_id: 'd1', cedula: 'C', nombre: 'Plain',
        departamento: 'X', cargo: 'C', shift_type: 'day',
        anomaly_codes: [], anomaly_count: 0, ...ZEROS,
      }],
      departments_in_order: [{ id: 'd1', name: 'X' }],
    }))
    const { didParseCell, body } = autoTableCalls[0].options
    const r = body.find((row) => row[1] === 'Plain')!
    const cell = { styles: {} as { fontStyle?: string; fillColor?: number[] } }
    didParseCell({ section: 'body', row: { raw: r }, cell })
    expect(cell.styles.fillColor).toBeUndefined()
    expect(cell.styles.fontStyle).toBeUndefined()
  })

  it('didParseCell early-returns for non-body sections (head, foot)', () => {
    renderReportPdf(payload())
    const { didParseCell } = autoTableCalls[0].options
    const cell = { styles: {} as { fontStyle?: string; fillColor?: number[] } }
    didParseCell({
      section: 'head',
      row: { raw: ['', 'TOTAL GENERAL'] },
      cell,
    })
    expect(cell.styles.fillColor).toBeUndefined()
  })

  it('didDrawPage: writes the "Página N de M" footer with the correct page count', () => {
    mockNumberOfPages = 3
    renderReportPdf(payload())
    const { didDrawPage } = autoTableCalls[0].options
    didDrawPage({ pageNumber: 2 })
    const footer = textCalls.find((c) =>
      typeof c.args[0] === 'string' && (c.args[0] as string).startsWith('Página ')
    )!
    expect(footer.args[0]).toBe('Página 2 de 3')
  })
})
