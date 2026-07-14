import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { Anomaly } from '@/types/api'
import { AnomaliesFilters, type AnomaliesFilterState } from '../anomalies-filters'
import { AnomaliesTable } from '../anomalies-table'

const anomaly: Anomaly = {
  id: 'a1',
  daily_record_id: 'r1',
  employee_id: 'e1',
  anchor_date: '2026-04-23',
  code: 'LATE',
  detail: 'Llegó 12 minutos tarde',
  created_at: '2026-04-23T13:00:00Z',
}

describe('AnomaliesFilters', () => {
  it('emits every filter and can clear employee and all filters', () => {
    const onChange = vi.fn<(next: AnomaliesFilterState) => void>()
    const { rerender } = render(
      <AnomaliesFilters value={{ code: 'LATE' }} onChange={onChange} employees={[{ id: 'e1', label: 'Ana' }]} />,
    )
    fireEvent.change(screen.getByTestId('anomalies-filter-code'), { target: { value: 'NO_EXIT' } })
    expect(onChange).toHaveBeenLastCalledWith({ code: 'NO_EXIT' })
    fireEvent.change(screen.getByTestId('anomalies-filter-code'), { target: { value: '' } })
    expect(onChange).toHaveBeenLastCalledWith({ code: undefined })

    fireEvent.click(screen.getByTestId('anomalies-filter-employee'))
    fireEvent.click(screen.getByRole('option', { name: 'Ana' }))
    expect(onChange).toHaveBeenLastCalledWith({ code: 'LATE', employee_id: 'e1' })
    fireEvent.change(screen.getByTestId('anomalies-filter-from'), { target: { value: '2026-04-01' } })
    expect(onChange).toHaveBeenLastCalledWith({ code: 'LATE', from_date: '2026-04-01' })
    rerender(
      <AnomaliesFilters
        value={{ code: 'LATE', to_date: '2026-04-30' }}
        onChange={onChange}
        employees={[{ id: 'e1', label: 'Ana' }]}
      />,
    )
    fireEvent.change(screen.getByTestId('anomalies-filter-to'), { target: { value: '' } })
    expect(onChange).toHaveBeenLastCalledWith({ code: 'LATE', to_date: undefined })

    rerender(
      <AnomaliesFilters value={{ employee_id: 'e1' }} onChange={onChange} employees={[{ id: 'e1', label: 'Ana' }]} />,
    )
    fireEvent.click(screen.getByLabelText('Limpiar selección'))
    expect(onChange).toHaveBeenLastCalledWith({ employee_id: undefined })
    fireEvent.click(screen.getByRole('button', { name: 'Limpiar' }))
    expect(onChange).toHaveBeenLastCalledWith({})
  })
})

describe('AnomaliesTable', () => {
  it('renders mapped employees, fallbacks and opens a row', () => {
    const onView = vi.fn()
    const data = [anomaly, { ...anomaly, id: 'a2', employee_id: 'e2', detail: null, code: 'NO_EXIT' }]
    render(
      <AnomaliesTable
        data={data}
        total={2}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
        onView={onView}
        employeeNameById={new Map([['e1', 'Ana Pérez']])}
        pageSize={10}
      />,
    )
    expect(screen.getByText('Ana Pérez')).toBeVisible()
    expect(screen.getByText('e2')).toBeVisible()
    expect(screen.getByText('Llegó 12 minutos tarde')).toBeVisible()
    expect(screen.getByText('—')).toBeVisible()
    fireEvent.click(screen.getByTestId('anomaly-view-a2'))
    expect(onView).toHaveBeenCalledWith(data[1])
  })

  it('shows loading, empty and paged states', () => {
    const onPaginationChange = vi.fn()
    const common = {
      data: [] as Anomaly[], total: 0, pagination: { pageIndex: 0, pageSize: 10 },
      onPaginationChange, onView: vi.fn(), employeeNameById: new Map<string, string>(), pageSize: 10,
    }
    const { rerender } = render(<AnomaliesTable {...common} isLoading />)
    expect(screen.getByText('Cargando anomalías…')).toBeVisible()
    rerender(<AnomaliesTable {...common} />)
    expect(screen.getByTestId('anomalies-empty')).toBeVisible()
    expect(screen.getByText('0 entradas')).toBeVisible()
    expect(screen.getByTestId('anomalies-pagination-prev')).toBeDisabled()
    expect(screen.getByTestId('anomalies-pagination-next')).toBeDisabled()

    rerender(
      <AnomaliesTable
        {...common}
        data={[anomaly]}
        total={25}
        pagination={{ pageIndex: 1, pageSize: 10 }}
      />,
    )
    expect(screen.getByText('Página 2 de 3 (25 total)')).toBeVisible()
    fireEvent.click(screen.getByTestId('anomalies-pagination-prev'))
    expect(onPaginationChange).toHaveBeenCalledWith({ pageIndex: 0, pageSize: 10 })
    fireEvent.click(screen.getByTestId('anomalies-pagination-next'))
    expect(onPaginationChange).toHaveBeenCalledWith({ pageIndex: 2, pageSize: 10 })
  })
})
