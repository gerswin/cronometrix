/**
 * Top-level coverage extension that mounts the component (the existing
 * timesheet-table.test.tsx tests only the date-fns helpers).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TimesheetTable } from '../components/timesheet/timesheet-table'
import type { DailyRecord } from '../types/api'

const { useAuthMock } = vi.hoisted(() => ({ useAuthMock: vi.fn() }))
vi.mock('@/hooks/use-auth', () => ({ useAuth: () => useAuthMock() }))

function makeRecord(over: Partial<DailyRecord> = {}): DailyRecord {
  return {
    id: 'r-1',
    employee_id: 'emp-1',
    employee_name: 'Ana García',
    department_id: 'd1',
    anchor_date: '2026-04-23',
    shift_type: 'day',
    work_minutes: 480,
    overtime_minutes: 0,
    late_minutes: 0,
    early_departure_minutes: 0,
    is_rest_day_worked: false,
    entry_at: '2026-04-23T08:00:00-04:00',
    exit_at: '2026-04-23T16:00:00-04:00',
    leave_id: null,
    computed_at: '2026-04-23T16:30:00-04:00',
    created_at: '2026-04-23T08:00:00-04:00',
    updated_at: '2026-04-23T16:00:00-04:00',
    anomalies: [],
    ...over,
  }
}

function renderTable(data: DailyRecord[], onEditClick = () => {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={client}>
      <TimesheetTable
        data={data}
        total={data.length}
        pagination={{ pageIndex: 0, pageSize: 50 }}
        onPaginationChange={() => {}}
        onEditClick={onEditClick}
      />
    </QueryClientProvider>,
  )
}

describe('TimesheetTable (component)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
  })

  it('renders all column headers', () => {
    renderTable([])
    for (const h of ['Fecha', 'Empleado', 'Departamento', 'Entrada', 'Salida', 'Novedades / Estado', 'Acciones']) {
      expect(screen.getByText(h)).toBeTruthy()
    }
  })

  it('uses employee and anchor date as stable row identity and shows enriched fields', () => {
    const first = makeRecord({ id: 'opaque-1', department_name: 'Operaciones' })
    const second = makeRecord({ id: 'opaque-2', anchor_date: '2026-04-24' })
    renderTable([first, second])
    expect(screen.getByTestId('timesheet-row-emp-1:2026-04-23')).toBeTruthy()
    expect(screen.getByTestId('timesheet-row-emp-1:2026-04-24')).toBeTruthy()
    expect(screen.getByText('23/04/2026')).toBeTruthy()
    expect(screen.getByText('Operaciones')).toBeTruthy()
  })

  it('renders the empty-state row when no records', () => {
    renderTable([])
    expect(screen.getByText('Sin registros para esta semana')).toBeTruthy()
  })

  it('Normal status badge for a record with work_minutes>0 and no leave', () => {
    renderTable([makeRecord()])
    expect(screen.getByText('Normal')).toBeTruthy()
    // Employee name rendered through the cell helper
    expect(screen.getByText('Ana García')).toBeTruthy()
  })

  it('Ausente badge when work_minutes==0 and no leave', () => {
    renderTable([makeRecord({ work_minutes: 0, leave_id: null })])
    expect(screen.getByText('Ausente')).toBeTruthy()
  })

  it('Ausente Justificado badge when work_minutes==0 with a leave', () => {
    renderTable([makeRecord({ work_minutes: 0, leave_id: 'leave-1' })])
    expect(screen.getByText('Ausente Justificado')).toBeTruthy()
  })

  it('Justificado badge when work_minutes>0 with a leave (e.g. partial-day permission)', () => {
    renderTable([makeRecord({ work_minutes: 240, leave_id: 'leave-1' })])
    expect(screen.getByText('Justificado')).toBeTruthy()
  })

  it('shows em-dash for null entry_at / exit_at / minute fields', () => {
    renderTable([makeRecord({ entry_at: null, exit_at: null })])
    // At least one em-dash rendered (entry/exit/late columns)
    expect(screen.getAllByText('—').length).toBeGreaterThanOrEqual(1)
  })

  it('admin sees the edit (Pencil) button per row that fires onEditClick', () => {
    const onEditClick = vi.fn()
    const rec = makeRecord()
    renderTable([rec], onEditClick)
    fireEvent.click(screen.getByLabelText('Registrar novedad'))
    expect(onEditClick).toHaveBeenCalledWith(rec)
  })

  it('non-admin (supervisor) does NOT see the edit button (D-14 RBAC)', () => {
    useAuthMock.mockReturnValue({ role: 'supervisor', sub: 'u1', claims: null })
    renderTable([makeRecord()])
    expect(screen.queryByLabelText('Registrar novedad')).toBeNull()
  })
})
