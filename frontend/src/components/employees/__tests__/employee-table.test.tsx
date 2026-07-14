import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { EmployeeTable } from '../employee-table'
import type { Employee } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const { useAuthMock } = vi.hoisted(() => ({ useAuthMock: vi.fn() }))
vi.mock('@/hooks/use-auth', () => ({
  useAuth: () => useAuthMock(),
}))

const alertSpy = vi.fn()
vi.stubGlobal('alert', alertSpy)

function makeEmployee(over: Partial<Employee> = {}): Employee {
  return {
    id: 'emp-1',
    employee_code: 'V-12345678',
    name: 'Ana García',
    department_id: 'd1',
    department_name: 'Operaciones',
    position: 'Analista',
    hire_date: '2023-01-15',
    status: 'active',
    version: 1,
    created_at: '2023-01-15T00:00:00Z',
    updated_at: '2023-01-15T00:00:00Z',
    ...over,
    base_salary_cents: over.base_salary_cents ?? 100000,
  }
}

describe('EmployeeTable', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
  })

  it('renders all column headers in Spanish', () => {
    render(
      <EmployeeTable
        data={[]}
        total={0}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    for (const h of ['Nombre', 'Identificativo', 'Departamento', 'Cargo', 'Fecha Ingreso', 'Estatus', 'Acciones']) {
      expect(screen.getByText(h)).toBeTruthy()
    }
  })

  it('shows the empty-state cell when no employees', () => {
    render(
      <EmployeeTable
        data={[]}
        total={0}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getByText(/Sin empleados para los filtros seleccionados/)).toBeTruthy()
  })

  it('renders status badges for active / pending / inactive', () => {
    render(
      <EmployeeTable
        data={[
          makeEmployee({ id: '1', name: 'A', status: 'active' }),
          makeEmployee({ id: '2', name: 'B', status: 'pending' }),
          makeEmployee({ id: '3', name: 'C', status: 'inactive' }),
        ]}
        total={3}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getByText('Activo')).toBeTruthy()
    expect(screen.getByText('Pendiente')).toBeTruthy()
    expect(screen.getByText('Inactivo')).toBeTruthy()
  })

  it('renders em-dash for missing department_name', () => {
    render(
      <EmployeeTable
        data={[makeEmployee({ id: '1', name: 'NoDept', department_name: undefined })]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getAllByText('—').length).toBeGreaterThan(0)
  })

  it('admin sees Editar and Ver detalles per row; supervisor sees only Ver detalles', () => {
    // Admin
    const { unmount } = render(
      <EmployeeTable
        data={[makeEmployee()]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getByLabelText('Editar empleado')).toBeTruthy()
    expect(screen.getByLabelText('Ver detalles')).toBeTruthy()
    unmount()

    // Supervisor
    useAuthMock.mockReturnValue({ role: 'supervisor', sub: 'u1', claims: null })
    render(
      <EmployeeTable
        data={[makeEmployee()]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.queryByLabelText('Editar empleado')).toBeNull()
    expect(screen.getByLabelText('Ver detalles')).toBeTruthy()
  })

  it('admin sees Enrolar Rostro button only when onEnrollClick prop is supplied', () => {
    const onEnrollClick = vi.fn()
    const emp = makeEmployee()
    render(
      <EmployeeTable
        data={[emp]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
        onEnrollClick={onEnrollClick}
      />
    )
    const btn = screen.getByLabelText('Enrolar Rostro')
    fireEvent.click(btn)
    expect(onEnrollClick).toHaveBeenCalledWith(emp)
  })

  it('clicking Editar invokes the placeholder alert with the row id', () => {
    const emp = makeEmployee({ id: 'emp-42' })
    render(
      <EmployeeTable
        data={[emp]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    fireEvent.click(screen.getByLabelText('Editar empleado'))
    expect(alertSpy).toHaveBeenCalledWith('Editar: emp-42')
  })

  it('pagination is hidden when pageCount <= 1', () => {
    render(
      <EmployeeTable
        data={[makeEmployee()]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.queryByText('Anterior')).toBeNull()
    expect(screen.queryByText('Siguiente')).toBeNull()
  })

  it('pagination shows Anterior / page label / Siguiente when pageCount > 1', () => {
    const onPaginationChange = vi.fn()
    render(
      <EmployeeTable
        data={Array.from({ length: 10 }, (_, i) => makeEmployee({ id: `e-${i}`, name: `Emp ${i}` }))}
        total={25}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={onPaginationChange}
      />
    )
    expect(screen.getByText('Anterior')).toBeTruthy()
    expect(screen.getByText('Siguiente')).toBeTruthy()
    expect(screen.getByText(/Página 1 de 3/)).toBeTruthy()
    fireEvent.click(screen.getByText('Siguiente'))
    expect(onPaginationChange).toHaveBeenCalledWith(
      expect.objectContaining({ pageIndex: 1 })
    )
  })

  it('Anterior is disabled on the first page; Siguiente disabled on the last page', () => {
    // First page
    const { unmount } = render(
      <EmployeeTable
        data={Array.from({ length: 10 }, (_, i) => makeEmployee({ id: `e-${i}`, name: `Emp ${i}` }))}
        total={25}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    const prev = screen.getByText('Anterior') as HTMLButtonElement
    expect(prev.disabled).toBe(true)
    unmount()

    // Last page
    render(
      <EmployeeTable
        data={Array.from({ length: 5 }, (_, i) => makeEmployee({ id: `e-${i}`, name: `Emp ${i}` }))}
        total={25}
        pagination={{ pageIndex: 2, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    const next = screen.getByText('Siguiente') as HTMLButtonElement
    expect(next.disabled).toBe(true)
  })

  it('formats hire_date as dd/MM/yyyy', () => {
    render(
      <EmployeeTable
        data={[makeEmployee({ id: '1', name: 'X', hire_date: '2023-05-12T12:00:00-04:00' })]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    expect(screen.getByText('12/05/2023')).toBeTruthy()
  })

  it('shows em-dash when hire_date is invalid', () => {
    render(
      <EmployeeTable
        data={[makeEmployee({ id: '1', name: 'X', hire_date: 'not-a-date' })]}
        total={1}
        pagination={{ pageIndex: 0, pageSize: 10 }}
        onPaginationChange={() => {}}
      />
    )
    // At least one em-dash on the page (the hire_date catch path)
    expect(screen.getAllByText('—').length).toBeGreaterThan(0)
  })

  it('forwards updater-function pagination changes to onPaginationChange', () => {
    const onPaginationChange = vi.fn()
    const baseline: PaginationState = { pageIndex: 0, pageSize: 10 }
    render(
      <EmployeeTable
        data={Array.from({ length: 10 }, (_, i) => makeEmployee({ id: `e-${i}`, name: `Emp ${i}` }))}
        total={25}
        pagination={baseline}
        onPaginationChange={onPaginationChange}
      />
    )
    // Click Siguiente — internally invokes the state setter w/ object
    fireEvent.click(screen.getByText('Siguiente'))
    expect(onPaginationChange).toHaveBeenCalled()
  })
})
