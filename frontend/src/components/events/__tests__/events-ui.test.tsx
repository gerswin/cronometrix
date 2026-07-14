import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { RawAttendanceEvent } from '@/types/api'
import { EventsFilters, type EventsFilterState } from '../events-filters'
import { EventsTable } from '../events-table'

const baseEvent: RawAttendanceEvent = {
  id: 'ev-1',
  employee_id: 'e1',
  device_id: 'd1',
  direction: 'entry',
  captured_at: '2026-04-23T12:00:00Z',
  is_unknown: false,
  face_id: null,
  employee_no_string: '001',
  photo_path: null,
  created_at: '2026-04-23T12:00:01Z',
}

describe('EventsFilters', () => {
  it('emits employee, device, date and unknown filters while preserving current state', () => {
    const onChange = vi.fn<(next: EventsFilterState) => void>()
    const { rerender } = render(
      <EventsFilters
        value={{ include_unknown: true }}
        onChange={onChange}
        employees={[{ id: 'e1', label: 'Ana' }]}
        devices={[{ id: 'd1', label: 'Entrada' }]}
      />,
    )

    fireEvent.click(screen.getByTestId('events-filter-employee'))
    fireEvent.click(screen.getByRole('option', { name: 'Ana' }))
    expect(onChange).toHaveBeenLastCalledWith({ include_unknown: true, employee_id: 'e1' })

    fireEvent.click(screen.getByTestId('events-filter-device'))
    fireEvent.click(screen.getByRole('option', { name: 'Entrada' }))
    expect(onChange).toHaveBeenLastCalledWith({ include_unknown: true, device_id: 'd1' })

    fireEvent.change(screen.getByTestId('events-filter-from'), { target: { value: '2026-04-23T08:30' } })
    const from = Math.floor(new Date('2026-04-23T08:30').getTime() / 1000)
    expect(onChange).toHaveBeenLastCalledWith({ include_unknown: true, from })

    rerender(
      <EventsFilters
        value={{ include_unknown: true, to: from }}
        onChange={onChange}
        employees={[{ id: 'e1', label: 'Ana' }]}
        devices={[{ id: 'd1', label: 'Entrada' }]}
      />,
    )
    fireEvent.change(screen.getByTestId('events-filter-to'), { target: { value: '' } })
    expect(onChange).toHaveBeenLastCalledWith({ include_unknown: true, to: undefined })

    fireEvent.click(screen.getByTestId('events-filter-unknown'))
    expect(onChange).toHaveBeenLastCalledWith({ include_unknown: undefined, to: from })

    rerender(
      <EventsFilters
        value={{ from: Number.NaN, to: from, employee_id: 'e1' }}
        onChange={onChange}
        employees={[{ id: 'e1', label: 'Ana' }]}
        devices={[]}
      />,
    )
    expect(screen.getByTestId('events-filter-from')).toHaveValue('')
    expect(screen.getByTestId('events-filter-to')).toHaveValue('2026-04-23T08:30')
    fireEvent.click(screen.getByLabelText('Limpiar selección'))
    expect(onChange).toHaveBeenLastCalledWith({ from: Number.NaN, to: from, employee_id: undefined })
    fireEvent.click(screen.getByRole('button', { name: 'Limpiar' }))
    expect(onChange).toHaveBeenLastCalledWith({})
  })
})

describe('EventsTable', () => {
  it('renders mapped and fallback event identities and opens the selected event', () => {
    const onView = vi.fn()
    const events: RawAttendanceEvent[] = [
      baseEvent,
      { ...baseEvent, id: 'ev-2', direction: 'exit', is_unknown: true, employee_id: null, device_id: 'd2' },
      { ...baseEvent, id: 'ev-3', employee_id: 'e2', employee_no_string: 'HIK-2' },
      { ...baseEvent, id: 'ev-4', employee_id: null, employee_no_string: null, face_id: 'face-4' },
      { ...baseEvent, id: 'ev-5', employee_id: null, employee_no_string: null, face_id: null },
    ]
    render(
      <EventsTable
        data={events}
        total={5}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
        onView={onView}
        employeeNameById={new Map([['e1', 'Ana Pérez']])}
        deviceNameById={new Map([['d1', 'Puerta principal']])}
        pageSize={10}
      />,
    )

    expect(screen.getAllByText('Entrada')).not.toHaveLength(0)
    expect(screen.getByText('Salida')).toBeVisible()
    expect(screen.getByText('Ana Pérez')).toBeVisible()
    expect(screen.getByText('Desconocido')).toBeVisible()
    expect(screen.getByText('HIK-2')).toBeVisible()
    expect(screen.getByText('face-4')).toBeVisible()
    expect(screen.getAllByText('—').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Puerta principal')).toHaveLength(4)
    expect(screen.getByText('d2')).toBeVisible()
    fireEvent.click(screen.getByTestId('event-view-ev-2'))
    expect(onView).toHaveBeenCalledWith(events[1])
  })

  it('shows loading and empty pagination states', () => {
    const common = {
      data: [] as RawAttendanceEvent[], total: 0, pagination: { pageIndex: 0, pageSize: 10 },
      onPaginationChange: vi.fn(), onView: vi.fn(), employeeNameById: new Map<string, string>(),
      deviceNameById: new Map<string, string>(), pageSize: 10,
    }
    const { rerender } = render(<EventsTable {...common} isLoading />)
    expect(screen.getByText('Cargando eventos…')).toBeVisible()
    rerender(<EventsTable {...common} />)
    expect(screen.getByTestId('events-empty')).toBeVisible()
    expect(screen.getByText('0 entradas')).toBeVisible()
    expect(screen.getByTestId('events-pagination-prev')).toBeDisabled()
    expect(screen.getByTestId('events-pagination-next')).toBeDisabled()
  })

  it('moves between server-controlled pages', () => {
    const onPaginationChange = vi.fn()
    render(
      <EventsTable
        data={[baseEvent]}
        total={25}
        pagination={{ pageIndex: 1, pageSize: 10 }}
        onPaginationChange={onPaginationChange}
        onView={() => {}}
        employeeNameById={new Map()}
        deviceNameById={new Map()}
        pageSize={10}
      />,
    )
    expect(screen.getByText('Página 2 de 3 (25 total)')).toBeVisible()
    fireEvent.click(screen.getByTestId('events-pagination-prev'))
    expect(onPaginationChange).toHaveBeenCalledWith({ pageIndex: 0, pageSize: 10 })
    fireEvent.click(screen.getByTestId('events-pagination-next'))
    expect(onPaginationChange).toHaveBeenCalledWith({ pageIndex: 2, pageSize: 10 })
  })
})
