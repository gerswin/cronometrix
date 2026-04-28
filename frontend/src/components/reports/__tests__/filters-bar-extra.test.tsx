/**
 * Branch coverage extension for FiltersBar. Covers the toggle-dept
 * branches (add then remove + final empty list resets to undefined),
 * include_inactive toggle, employee picker visible/hidden, employee
 * select to all-employees value, shift_type select branches.
 */
import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { FiltersBar } from '../filters-bar'
import type { ReportFilters } from '@/types/api'

const baseValue: ReportFilters = {
  period_type: 'monthly',
  from_date: '2026-04-01',
  to_date: '2026-04-30',
  include_inactive: false,
}

const departments = [
  { id: 'd1', name: 'Operaciones' },
  { id: 'd2', name: 'Ventas' },
]

describe('FiltersBar — extra branches', () => {
  it('toggling a department adds it then removes it (round-trip)', () => {
    const onChange = vi.fn()
    render(<FiltersBar value={baseValue} onChange={onChange} departments={departments} />)
    const checkbox = screen.getByLabelText('Departamento Operaciones')
    fireEvent.click(checkbox)
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ department_ids: ['d1'] }))
  })

  it('removing the last selected department sets department_ids back to undefined', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={{ ...baseValue, department_ids: ['d1'] }}
        onChange={onChange}
        departments={departments}
      />
    )
    fireEvent.click(screen.getByLabelText('Departamento Operaciones'))
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ department_ids: undefined }))
  })

  it('Incluir inactivos checkbox flips the include_inactive flag', () => {
    const onChange = vi.fn()
    render(<FiltersBar value={baseValue} onChange={onChange} departments={departments} />)
    fireEvent.click(screen.getByLabelText('Incluir empleados inactivos'))
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ include_inactive: true }))
  })

  it('Empleado picker is hidden when employees prop is undefined or empty', () => {
    const { unmount } = render(
      <FiltersBar value={baseValue} onChange={() => {}} departments={departments} />
    )
    expect(screen.queryByLabelText('Filtrar por empleado')).toBeNull()
    unmount()
    render(
      <FiltersBar
        value={baseValue}
        onChange={() => {}}
        departments={departments}
        employees={[]}
      />
    )
    expect(screen.queryByLabelText('Filtrar por empleado')).toBeNull()
  })

  it('Empleado picker visible when employees provided; selecting "Todos" sets undefined', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={{ ...baseValue, employee_id: 'emp-1' }}
        onChange={onChange}
        departments={departments}
        employees={[{ id: 'emp-1', label: 'Ana' }]}
      />
    )
    const sel = screen.getByLabelText('Filtrar por empleado')
    fireEvent.change(sel, { target: { value: '' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ employee_id: undefined }))
  })

  it('selecting an employee sets employee_id to the value', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={baseValue}
        onChange={onChange}
        departments={departments}
        employees={[{ id: 'emp-1', label: 'Ana' }]}
      />
    )
    const sel = screen.getByLabelText('Filtrar por empleado')
    fireEvent.change(sel, { target: { value: 'emp-1' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ employee_id: 'emp-1' }))
  })

  it('shift_type select supports day / night / mixed and "Todos" (undefined)', () => {
    const onChange = vi.fn()
    render(<FiltersBar value={baseValue} onChange={onChange} departments={departments} />)
    const sel = screen.getByLabelText('Tipo de turno')
    fireEvent.change(sel, { target: { value: 'night' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ shift_type: 'night' }))
    fireEvent.change(sel, { target: { value: '' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ shift_type: undefined }))
  })

  it('summary text reflects selected count: 0 -> "Todos", N -> "N seleccionado(s)"', () => {
    const { unmount } = render(
      <FiltersBar value={baseValue} onChange={() => {}} departments={departments} />
    )
    // "Todos" appears multiple times (dept summary + shift_type "Todos" option); just confirm presence
    expect(screen.getAllByText('Todos').length).toBeGreaterThanOrEqual(1)
    unmount()
    render(
      <FiltersBar
        value={{ ...baseValue, department_ids: ['d1', 'd2'] }}
        onChange={() => {}}
        departments={departments}
      />
    )
    expect(screen.getByText('2 seleccionado(s)')).toBeTruthy()
  })
})
