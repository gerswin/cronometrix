import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { FiltersBar } from '../filters-bar'
import type { ReportFilters, DeptSummary } from '@/types/api'

const baseFilters: ReportFilters = {
  period_type: 'monthly',
  from_date: '2026-04-01',
  to_date: '2026-04-30',
  include_inactive: false,
}

const departments: DeptSummary[] = [
  { id: 'd1', name: 'Operaciones' },
  { id: 'd2', name: 'Administración' },
]

describe('<FiltersBar>', () => {
  it('toggling a department checkbox calls onChange with department_ids', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={baseFilters}
        onChange={onChange}
        departments={departments}
      />,
    )
    fireEvent.click(screen.getByLabelText('Departamento Operaciones'))
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ department_ids: ['d1'] }),
    )
  })

  it('toggling include_inactive emits include_inactive: true', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={baseFilters}
        onChange={onChange}
        departments={departments}
      />,
    )
    fireEvent.click(screen.getByLabelText('Incluir empleados inactivos'))
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ include_inactive: true }),
    )
  })

  it('selecting a shift_type emits shift_type', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={baseFilters}
        onChange={onChange}
        departments={departments}
      />,
    )
    fireEvent.change(screen.getByLabelText('Tipo de turno'), {
      target: { value: 'night' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ shift_type: 'night' }),
    )
  })

  it('selecting "Todos" shift_type emits undefined', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={{ ...baseFilters, shift_type: 'day' }}
        onChange={onChange}
        departments={departments}
      />,
    )
    fireEvent.change(screen.getByLabelText('Tipo de turno'), {
      target: { value: '' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ shift_type: undefined }),
    )
  })

  it('employee picker emits employee_id when employees prop is supplied', () => {
    const onChange = vi.fn()
    render(
      <FiltersBar
        value={baseFilters}
        onChange={onChange}
        departments={departments}
        employees={[{ id: 'e1', label: 'Iñaki Núñez' }]}
      />,
    )
    fireEvent.change(screen.getByLabelText('Filtrar por empleado'), {
      target: { value: 'e1' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ employee_id: 'e1' }),
    )
  })

  it('employee picker is hidden when employees prop omitted', () => {
    render(
      <FiltersBar
        value={baseFilters}
        onChange={() => {}}
        departments={departments}
      />,
    )
    expect(screen.queryByLabelText('Filtrar por empleado')).toBeNull()
  })
})
