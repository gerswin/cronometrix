import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { SummaryTable, buildTableRows } from '../summary-table'
import type { ReportPayload, EmployeeReportRow } from '@/types/api'

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

function makeRow(overrides: Partial<EmployeeReportRow>): EmployeeReportRow {
  return {
    employee_id: 'e1',
    dept_id: 'd1',
    cedula: '12345678',
    nombre: 'Empleado 1',
    departamento: 'Operaciones',
    cargo: 'Ingeniero',
    shift_type: 'day',
    anomaly_codes: [],
    anomaly_count: 0,
    ...EMPTY_AGG,
    ...overrides,
  }
}

const PAYLOAD: ReportPayload = {
  header: {
    client_name: 'Acme',
    client_rif: 'J-1-9',
    from_date: '2026-04-01',
    to_date: '2026-04-30',
    generated_at_iso: '2026-04-25T18:00:00Z',
  },
  rows: [
    makeRow({
      employee_id: 'e1',
      cedula: '11111',
      nombre: 'Anita',
      work_min: 480,
      work_pay_cents: 50_000,
      total_a_pagar_cents: 50_000,
    }),
    makeRow({
      employee_id: 'e2',
      cedula: '22222',
      nombre: 'Boris',
      anomaly_codes: ['MISSING_EXIT'],
      anomaly_count: 1,
    }),
    makeRow({
      employee_id: 'e3',
      dept_id: 'd2',
      cedula: '33333',
      nombre: 'Cami',
      departamento: 'Administración',
    }),
  ],
  dept_subtotals: [
    {
      dept_id: 'd1',
      dept_name: 'Operaciones',
      aggregates: { ...EMPTY_AGG, work_min: 480, work_pay_cents: 50_000 },
    },
    { dept_id: 'd2', dept_name: 'Administración', aggregates: { ...EMPTY_AGG } },
  ],
  grand_total: { ...EMPTY_AGG, work_min: 480, work_pay_cents: 50_000 },
  departments_in_order: [
    { id: 'd1', name: 'Operaciones' },
    { id: 'd2', name: 'Administración' },
  ],
}

const EXPECTED_HEADERS = [
  'Cédula',
  'Nombre',
  'Departamento',
  'Cargo',
  'Min Trab',
  'Min Extra',
  'Min Retraso',
  'Días Trab',
  'Días Aus',
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

describe('buildTableRows', () => {
  it('emits data + subtotal + grandtotal kinds in order', () => {
    const rows = buildTableRows(PAYLOAD)
    const kinds = rows.map((r) => r._kind)
    // Operaciones (2 data) → subtotal → Administración (1 data) → subtotal → grandtotal
    expect(kinds).toEqual([
      'data',
      'data',
      'subtotal',
      'data',
      'subtotal',
      'grandtotal',
    ])
  })

  it('subtotal label is "Total {dept_name}"', () => {
    const rows = buildTableRows(PAYLOAD)
    const subtotal = rows.find((r) => r._kind === 'subtotal')
    expect(subtotal?.nombre).toBe('Total Operaciones')
  })

  it('grandtotal label is "Total General"', () => {
    const rows = buildTableRows(PAYLOAD)
    const grand = rows.find((r) => r._kind === 'grandtotal')
    expect(grand?.nombre).toBe('Total General')
  })
})

describe('<SummaryTable>', () => {
  it('renders all 20 column headers', () => {
    render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={() => {}}
      />,
    )
    for (const header of EXPECTED_HEADERS) {
      expect(screen.getByText(header)).toBeInTheDocument()
    }
  })

  it('shows empty placeholder when no payload', () => {
    render(
      <SummaryTable payload={undefined} isLoading={false} onDrillDown={() => {}} />,
    )
    expect(screen.getByText(/Sin datos/)).toBeInTheDocument()
  })

  it('shows loading placeholder when isLoading', () => {
    render(
      <SummaryTable payload={undefined} isLoading={true} onDrillDown={() => {}} />,
    )
    expect(screen.getByText(/Generando reporte/)).toBeInTheDocument()
  })

  it('anomaly row receives bg-amber-50 class', () => {
    const { container } = render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={() => {}}
      />,
    )
    const anomalyRow = container.querySelector('tr.bg-amber-50')
    expect(anomalyRow).not.toBeNull()
  })

  it('subtotal row receives bg-slate-50 + font-semibold', () => {
    const { container } = render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={() => {}}
      />,
    )
    const subtotalRow = container.querySelector(
      'tr[data-row-kind="subtotal"]',
    )
    expect(subtotalRow).not.toBeNull()
    expect(subtotalRow!.className).toContain('font-semibold')
    expect(subtotalRow!.className).toContain('bg-slate-50')
  })

  it('grandtotal row receives font-bold + bg-blue-50', () => {
    const { container } = render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={() => {}}
      />,
    )
    const grandRow = container.querySelector('tr[data-row-kind="grandtotal"]')
    expect(grandRow).not.toBeNull()
    expect(grandRow!.className).toContain('font-bold')
    expect(grandRow!.className).toContain('bg-blue-50')
  })

  it('clicking a data row calls onDrillDown(employee_id)', () => {
    const onDrillDown = vi.fn()
    render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={onDrillDown}
      />,
    )
    fireEvent.click(screen.getByText('Anita'))
    expect(onDrillDown).toHaveBeenCalledWith('e1')
  })

  it('clicking a subtotal row does NOT call onDrillDown', () => {
    const onDrillDown = vi.fn()
    render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={onDrillDown}
      />,
    )
    fireEvent.click(screen.getByText('Total Operaciones'))
    expect(onDrillDown).not.toHaveBeenCalled()
  })

  it('money cells render via fmtMoney ($X,XXX.XX en-US)', () => {
    render(
      <SummaryTable
        payload={PAYLOAD}
        isLoading={false}
        onDrillDown={() => {}}
      />,
    )
    // 50_000 cents = $500.00 — Anita's row
    const cells = screen.getAllByText('$500.00')
    expect(cells.length).toBeGreaterThan(0)
  })
})
