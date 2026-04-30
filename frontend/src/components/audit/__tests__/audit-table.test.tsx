import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AuditTable } from '../audit-table'
import { AuditFilters, type FilterState } from '../audit-filters'
import type { AuditEntry } from '@/types/audit'

// Mock useAuth (used by DiffCell indirectly through the component tree — not needed here)
// Mock date-fns format to keep timestamps predictable in tests
vi.mock('date-fns', () => ({
  format: (date: Date, _fmt: string) => `formatted:${date.toISOString().slice(0, 10)}`,
}))

function makeEntry(over: Partial<AuditEntry> = {}): AuditEntry {
  return {
    id: 'entry-1',
    table_name: 'employees',
    record_id: 'rec-1',
    operation: 'UPDATE',
    old_data: { name: 'Old' },
    new_data: { name: 'New' },
    actor_id: 'user-1',
    created_at: 1700000000,
    ...over,
  }
}

const pagination = { pageIndex: 0, pageSize: 20 }

describe('AuditTable', () => {
  it('renders column headers', () => {
    render(
      <AuditTable
        data={[]}
        total={0}
        pagination={pagination}
        onPaginationChange={() => {}}
      />
    )
    for (const h of ['Timestamp', 'Actor', 'Tabla', 'Operación', 'ID Registro', 'Cambios']) {
      expect(screen.getByText(h)).toBeTruthy()
    }
  })

  it('renders three mocked AuditEntry rows each with data-testid="audit-row-${id}"', () => {
    const entries = [
      makeEntry({ id: 'e1', operation: 'INSERT', old_data: null, new_data: { a: 1 } }),
      makeEntry({ id: 'e2', operation: 'UPDATE' }),
      makeEntry({ id: 'e3', operation: 'DELETE', old_data: { x: 1 }, new_data: null }),
    ]
    render(
      <AuditTable
        data={entries}
        total={3}
        pagination={pagination}
        onPaginationChange={() => {}}
      />
    )
    expect(document.querySelector('[data-testid="audit-row-e1"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-row-e2"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-row-e3"]')).toBeTruthy()
  })

  it('shows empty state when data=[] with data-testid="audit-empty" and Spanish copy', () => {
    render(
      <AuditTable
        data={[]}
        total={0}
        pagination={pagination}
        onPaginationChange={() => {}}
      />
    )
    const emptyRow = document.querySelector('[data-testid="audit-empty"]')
    expect(emptyRow).toBeTruthy()
    expect(screen.getByText('Sin entradas para los filtros seleccionados')).toBeTruthy()
  })

  it('pagination prev/next buttons are present with correct data-testids', () => {
    render(
      <AuditTable
        data={[makeEntry()]}
        total={1}
        pagination={pagination}
        onPaginationChange={() => {}}
      />
    )
    expect(document.querySelector('[data-testid="audit-pagination-prev"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-pagination-next"]')).toBeTruthy()
  })

  it('Anterior is disabled on first page', () => {
    render(
      <AuditTable
        data={Array.from({ length: 20 }, (_, i) => makeEntry({ id: `e${i}` }))}
        total={40}
        pagination={{ pageIndex: 0, pageSize: 20 }}
        onPaginationChange={() => {}}
      />
    )
    const prev = document.querySelector('[data-testid="audit-pagination-prev"]') as HTMLButtonElement
    expect(prev.disabled).toBe(true)
  })

  it('Siguiente is disabled on last page', () => {
    render(
      <AuditTable
        data={Array.from({ length: 20 }, (_, i) => makeEntry({ id: `e${i}` }))}
        total={40}
        pagination={{ pageIndex: 1, pageSize: 20 }}
        onPaginationChange={() => {}}
      />
    )
    const next = document.querySelector('[data-testid="audit-pagination-next"]') as HTMLButtonElement
    expect(next.disabled).toBe(true)
  })

  it('clicking Siguiente calls onPaginationChange with incremented pageIndex', () => {
    const onPaginationChange = vi.fn()
    render(
      <AuditTable
        data={Array.from({ length: 20 }, (_, i) => makeEntry({ id: `e${i}` }))}
        total={40}
        pagination={{ pageIndex: 0, pageSize: 20 }}
        onPaginationChange={onPaginationChange}
      />
    )
    const next = document.querySelector('[data-testid="audit-pagination-next"]')!
    fireEvent.click(next)
    expect(onPaginationChange).toHaveBeenCalledWith(
      expect.objectContaining({ pageIndex: 1 })
    )
  })

  it('renders actor_id column with em-dash for null actor', () => {
    render(
      <AuditTable
        data={[makeEntry({ id: 'e1', actor_id: null })]}
        total={1}
        pagination={pagination}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getAllByText('—').length).toBeGreaterThan(0)
  })

  it('shows loading state when isLoading=true', () => {
    render(
      <AuditTable
        data={[]}
        total={0}
        pagination={pagination}
        onPaginationChange={() => {}}
        isLoading={true}
      />
    )
    expect(screen.getByText('Cargando auditoría…')).toBeTruthy()
  })
})

describe('AuditFilters', () => {
  const actors = [
    { id: 'user-1', username: 'admin' },
    { id: 'user-2', username: 'supervisor' },
  ]
  const tables = ['employees', 'departments', 'leaves']

  it('renders all 6 filter inputs with correct data-testids', () => {
    render(
      <AuditFilters
        value={{}}
        onChange={() => {}}
        actors={actors}
        tables={tables}
      />
    )
    expect(document.querySelector('[data-testid="audit-filter-actor"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-filter-table"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-filter-from"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-filter-to"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-filter-operation"]')).toBeTruthy()
    expect(document.querySelector('[data-testid="audit-filter-record-id"]')).toBeTruthy()
  })

  it('calls onChange with actor_id when actor dropdown changes', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const select = document.querySelector('[data-testid="audit-filter-actor"]') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'user-1' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ actor_id: 'user-1' }))
  })

  it('calls onChange with table_name when table dropdown changes', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const select = document.querySelector('[data-testid="audit-filter-table"]') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'employees' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ table_name: 'employees' }))
  })

  it('calls onChange with operation when operation dropdown changes', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const select = document.querySelector('[data-testid="audit-filter-operation"]') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'INSERT' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ operation: 'INSERT' }))
  })

  it('calls onChange with record_id when record-id input changes', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const input = document.querySelector('[data-testid="audit-filter-record-id"]') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'rec-abc' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ record_id: 'rec-abc' }))
  })

  it('renders actor options from the actors prop', () => {
    render(
      <AuditFilters
        value={{}}
        onChange={() => {}}
        actors={actors}
        tables={tables}
      />
    )
    expect(screen.getByText('admin')).toBeTruthy()
    expect(screen.getByText('supervisor')).toBeTruthy()
  })

  it('reflects current filter values in controlled inputs', () => {
    const currentValue: FilterState = {
      actor_id: 'user-1',
      table_name: 'employees',
      operation: 'UPDATE',
      record_id: 'rec-999',
    }
    render(
      <AuditFilters
        value={currentValue}
        onChange={() => {}}
        actors={actors}
        tables={tables}
      />
    )
    const actorSel = document.querySelector('[data-testid="audit-filter-actor"]') as HTMLSelectElement
    expect(actorSel.value).toBe('user-1')
    const tableSel = document.querySelector('[data-testid="audit-filter-table"]') as HTMLSelectElement
    expect(tableSel.value).toBe('employees')
    const opSel = document.querySelector('[data-testid="audit-filter-operation"]') as HTMLSelectElement
    expect(opSel.value).toBe('UPDATE')
    const recordInput = document.querySelector('[data-testid="audit-filter-record-id"]') as HTMLInputElement
    expect(recordInput.value).toBe('rec-999')
  })

  it('from date input calls onChange with from_ts epoch (start of day)', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const fromInput = document.querySelector('[data-testid="audit-filter-from"]') as HTMLInputElement
    fireEvent.change(fromInput, { target: { value: '2024-01-15' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({
      from_ts: expect.any(Number),
    }))
    const call = onChange.mock.calls[0][0]
    // Should be a valid epoch (seconds since 1970)
    expect(call.from_ts).toBeGreaterThan(0)
  })

  it('to date input calls onChange with to_ts epoch (end of day)', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{}}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const toInput = document.querySelector('[data-testid="audit-filter-to"]') as HTMLInputElement
    fireEvent.change(toInput, { target: { value: '2024-01-15' } })
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({
      to_ts: expect.any(Number),
    }))
  })

  it('clearing from date input calls onChange with from_ts undefined', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{ from_ts: 1700000000 }}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const fromInput = document.querySelector('[data-testid="audit-filter-from"]') as HTMLInputElement
    fireEvent.change(fromInput, { target: { value: '' } })
    const call = onChange.mock.calls[0][0]
    expect(call.from_ts).toBeUndefined()
  })

  it('from date input shows current from_ts value as YYYY-MM-DD', () => {
    // epoch for 2024-01-15T00:00:00Z = 1705276800
    render(
      <AuditFilters
        value={{ from_ts: 1705276800 }}
        onChange={() => {}}
        actors={actors}
        tables={tables}
      />
    )
    const fromInput = document.querySelector('[data-testid="audit-filter-from"]') as HTMLInputElement
    // epochToDate converts back to YYYY-MM-DD
    expect(fromInput.value).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })

  it('clearing actor dropdown sets actor_id to undefined', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{ actor_id: 'user-1' }}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const select = document.querySelector('[data-testid="audit-filter-actor"]') as HTMLSelectElement
    fireEvent.change(select, { target: { value: '' } })
    const call = onChange.mock.calls[0][0]
    expect(call.actor_id).toBeUndefined()
  })

  it('clearing operation dropdown sets operation to undefined', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{ operation: 'INSERT' }}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const select = document.querySelector('[data-testid="audit-filter-operation"]') as HTMLSelectElement
    fireEvent.change(select, { target: { value: '' } })
    const call = onChange.mock.calls[0][0]
    expect(call.operation).toBeUndefined()
  })

  it('clearing record_id input sets record_id to undefined', () => {
    const onChange = vi.fn()
    render(
      <AuditFilters
        value={{ record_id: 'old-id' }}
        onChange={onChange}
        actors={actors}
        tables={tables}
      />
    )
    const input = document.querySelector('[data-testid="audit-filter-record-id"]') as HTMLInputElement
    fireEvent.change(input, { target: { value: '' } })
    const call = onChange.mock.calls[0][0]
    expect(call.record_id).toBeUndefined()
  })

  it('renders actors with username (role) display format', () => {
    const enrichedActors = [
      { id: 'user-1', username: 'admin (admin)' },
      { id: 'user-2', username: 'jsmith (supervisor)' },
    ]
    render(
      <AuditFilters
        value={{}}
        onChange={() => {}}
        actors={enrichedActors}
        tables={[]}
      />
    )
    expect(screen.getByText('admin (admin)')).toBeTruthy()
    expect(screen.getByText('jsmith (supervisor)')).toBeTruthy()
    const select = document.querySelector(
      '[data-testid="audit-filter-actor"]'
    ) as HTMLSelectElement
    const opt = Array.from(select.options).find(o => o.value === 'user-1')
    expect(opt?.text).toBe('admin (admin)')
  })
})
