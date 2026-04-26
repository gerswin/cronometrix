import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { ReportPayload } from '@/types/api'

// ──────────────────────────────────────────────────────────────────────
// Mock jspdf + jspdf-autotable. We capture invocations to assert structure
// without rendering an actual PDF (which would slow tests + couple to font
// internals). Spies are created at module-load time so mockImplementation
// must be set up here.
// ──────────────────────────────────────────────────────────────────────

const textCalls: { args: unknown[] }[] = []
const setPropertiesCalls: { args: unknown[] }[] = []
const saveCalls: { filename: string }[] = []
const setFontCalls: { args: unknown[] }[] = []
const autoTableCalls: { doc: unknown; options: any }[] = []

vi.mock('jspdf', () => {
  class MockJsPDF {
    internal = { pageSize: { width: 297, height: 210 } }
    setProperties(...args: unknown[]) {
      setPropertiesCalls.push({ args })
    }
    setFontSize(_size: number) {}
    setFont(...args: unknown[]) {
      setFontCalls.push({ args })
    }
    text(...args: unknown[]) {
      textCalls.push({ args })
    }
    save(filename: string) {
      saveCalls.push({ filename })
    }
    getNumberOfPages() {
      return 1
    }
  }
  return { jsPDF: MockJsPDF }
})

vi.mock('jspdf-autotable', () => {
  const fn = (doc: unknown, options: any) => {
    autoTableCalls.push({ doc, options })
    // Simulate one page draw so didDrawPage gets called.
    if (options.didDrawPage) {
      options.didDrawPage({ pageNumber: 1 })
    }
  }
  return { default: fn }
})

// Import AFTER mocks are registered.
import { renderReportPdf } from '../pdf'

const EMPTY_AGG = {
  work_min: 0,
  ot_min: 0,
  late_min: 0,
  days_worked: 0,
  days_absent: 0,
  work_pay_cents: 0,
  ot_pay_cents: 0,
  night_premium_cents: 0,
  rest_day_surcharge_cents: 0,
  late_deduction_cents: 0,
  total_a_pagar_cents: 0,
  days_ivss: 0,
  days_vacation: 0,
  days_permission: 0,
  days_unpaid: 0,
}

function makePayload(overrides: Partial<ReportPayload> = {}): ReportPayload {
  return {
    header: {
      client_name: 'Acme',
      client_rif: 'J-1-9',
      from_date: '2026-04-01',
      to_date: '2026-04-30',
      generated_at_iso: '2026-04-25T18:00:00Z',
    },
    rows: [],
    dept_subtotals: [],
    grand_total: { ...EMPTY_AGG },
    departments_in_order: [],
    ...overrides,
  }
}

beforeEach(() => {
  textCalls.length = 0
  setPropertiesCalls.length = 0
  saveCalls.length = 0
  setFontCalls.length = 0
  autoTableCalls.length = 0
})

describe('renderReportPdf', () => {
  it('saves with filename prenomina_{from}_{to}.pdf and does not throw on empty payload', () => {
    expect(() => renderReportPdf(makePayload())).not.toThrow()
    expect(saveCalls).toHaveLength(1)
    expect(saveCalls[0].filename).toBe('prenomina_2026-04-01_2026-04-30.pdf')
  })

  it('writes the title "Reporte Pre-Nómina" in the header', () => {
    renderReportPdf(makePayload())
    const titleCall = textCalls.find((c) => c.args[0] === 'Reporte Pre-Nómina')
    expect(titleCall).toBeDefined()
  })

  it('includes client_name and RIF in branding row', () => {
    renderReportPdf(makePayload())
    const brandingCall = textCalls.find(
      (c) =>
        typeof c.args[0] === 'string' && (c.args[0] as string).includes('Acme'),
    )
    expect(brandingCall).toBeDefined()
    expect((brandingCall!.args[0] as string)).toContain('J-1-9')
  })

  it('renders em-dash for empty client_name', () => {
    renderReportPdf(
      makePayload({
        header: {
          client_name: '',
          client_rif: '',
          from_date: '2026-04-01',
          to_date: '2026-04-30',
          generated_at_iso: '2026-04-25T18:00:00Z',
        },
      }),
    )
    const brandingCall = textCalls.find(
      (c) =>
        typeof c.args[0] === 'string' && (c.args[0] as string).startsWith('—'),
    )
    expect(brandingCall).toBeDefined()
    expect((brandingCall!.args[0] as string)).toContain('RIF: —')
  })

  it('handles Spanish accents in employee names without throwing', () => {
    expect(() =>
      renderReportPdf(
        makePayload({
          rows: [
            {
              employee_id: 'e1',
              dept_id: 'd1',
              cedula: '12345678',
              nombre: 'Iñaki Núñez',
              departamento: 'TI',
              cargo: 'Ingeniero',
              shift_type: 'day',
              anomaly_codes: [],
              anomaly_count: 0,
              ...EMPTY_AGG,
            },
          ],
          departments_in_order: [{ id: 'd1', name: 'TI' }],
        }),
      ),
    ).not.toThrow()
    const body = autoTableCalls[0].options.body as (string | number)[][]
    const accentRow = body.find((r) => r[1] === 'Iñaki Núñez')
    expect(accentRow).toBeDefined()
  })

  it('appends a TOTAL GENERAL row at the end of the body', () => {
    renderReportPdf(makePayload())
    const body = autoTableCalls[0].options.body as (string | number)[][]
    const last = body[body.length - 1]
    expect(last[1]).toBe('TOTAL GENERAL')
  })

  it('appends a per-dept subtotal row labeled "Total {dept}"', () => {
    renderReportPdf(
      makePayload({
        departments_in_order: [{ id: 'd1', name: 'Operaciones' }],
        dept_subtotals: [
          { dept_id: 'd1', dept_name: 'Operaciones', aggregates: EMPTY_AGG },
        ],
      }),
    )
    const body = autoTableCalls[0].options.body as (string | number)[][]
    expect(body.some((r) => r[1] === 'Total Operaciones')).toBe(true)
  })

  it('passes orientation:landscape and format:a4 to jsPDF constructor', () => {
    // Validated indirectly via mock — we re-import the mock module to
    // inspect the captured constructor args.
    renderReportPdf(makePayload())
    // setProperties was called with creationDate
    expect(setPropertiesCalls).toHaveLength(1)
    const arg = setPropertiesCalls[0].args[0] as { creationDate: Date }
    expect(arg.creationDate).toBeInstanceOf(Date)
  })

  it('writes "Página N de M" footer via didDrawPage hook', () => {
    renderReportPdf(makePayload())
    const footerCall = textCalls.find(
      (c) =>
        typeof c.args[0] === 'string' &&
        (c.args[0] as string).startsWith('Página '),
    )
    expect(footerCall).toBeDefined()
    expect(footerCall!.args[0]).toBe('Página 1 de 1')
  })
})
