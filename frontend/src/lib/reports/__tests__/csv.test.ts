import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ReportPayload } from '@/types/api'
import { renderReportCsv } from '../csv'

const ZERO_TOTALS = {
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

const payload: ReportPayload = {
  header: {
    client_name: 'Cronometrix',
    client_rif: 'J-1',
    from_date: '2026-04-01',
    to_date: '2026-04-30',
    generated_at_iso: '2026-05-01T00:00:00Z',
  },
  rows: [
    {
      employee_id: 'e1',
      dept_id: 'd1',
      cedula: 'V-1',
      nombre: 'Núñez, "Ana"',
      departamento: 'Operaciones\nNorte',
      cargo: 'Analista',
      shift_type: 'day',
      anomaly_codes: ['LATE', 'NO_EXIT'],
      anomaly_count: 2,
      ...ZERO_TOTALS,
      work_min: 480,
      work_pay_cents: 12345,
      total_a_pagar_cents: 12345,
    },
  ],
  dept_subtotals: [],
  grand_total: { ...ZERO_TOTALS, work_min: 480, work_pay_cents: 12345, total_a_pagar_cents: 12345 },
  departments_in_order: [],
}

describe('renderReportCsv', () => {
  let capturedBlob: Blob | undefined
  const createObjectURL = vi.fn((blob: Blob) => {
    capturedBlob = blob
    return 'blob:report'
  })
  const revokeObjectURL = vi.fn()
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})

  function readBlob(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(String(reader.result))
      reader.onerror = () => reject(reader.error)
      reader.readAsText(blob)
    })
  }

  beforeEach(() => {
    capturedBlob = undefined
    createObjectURL.mockClear()
    revokeObjectURL.mockClear()
    click.mockClear()
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL })
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL })
  })

  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('downloads an Excel-friendly CSV with escaped values, money and totals', async () => {
    renderReportCsv(payload)

    expect(createObjectURL).toHaveBeenCalledOnce()
    expect(capturedBlob?.type).toBe('text/csv;charset=utf-8')
    const csv = await readBlob(capturedBlob!)
    // FileReader consumes the UTF-8 BOM while decoding; the three extra bytes
    // in the Blob prove it was emitted for Excel compatibility.
    expect(csv.startsWith('Cédula,Nombre,Departamento')).toBe(true)
    expect(capturedBlob!.size).toBe(new TextEncoder().encode(csv).length + 3)
    expect(csv).toContain('"Núñez, ""Ana"""')
    expect(csv).toContain('"Operaciones\nNorte"')
    expect(csv).toContain('123.45')
    expect(csv).toContain('LATE|NO_EXIT')
    expect(csv).toContain('TOTALES (1 empleados)')

    const anchor = click.mock.contexts[0] as HTMLAnchorElement
    expect(anchor.href).toBe('blob:report')
    expect(anchor.download).toBe('prenomina_2026-04-01_2026-04-30.csv')
    expect(document.body.contains(anchor)).toBe(false)
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:report')
  })

  it('renders an empty report and keeps blank nullable cells empty', async () => {
    renderReportCsv({ ...payload, rows: [], grand_total: ZERO_TOTALS })

    const csv = await readBlob(capturedBlob!)
    expect(csv).toContain('TOTALES (0 empleados)')
    expect(csv).not.toContain('undefined')
  })
})
