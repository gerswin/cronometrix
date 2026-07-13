import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { InProgressList } from '../in-progress-list'
import type { Employee, Enrollment, EnrollmentDevicePush } from '@/types/api'

const EMPLOYEE: Employee = {
  id: 'emp-1',
  employee_code: 'V-1',
  cedula: 'V-1',
  name: 'Ana García',
  department_id: 'd1',
  position: 'Analista',
  hire_date: '2023-01-01',
  status: 'active',
  version: 1,
  created_at: '2023-01-01T00:00:00Z',
  updated_at: '2023-01-01T00:00:00Z',
  base_salary_cents: 100000,
}

function push(over: Partial<EnrollmentDevicePush>): EnrollmentDevicePush {
  return {
    device_id: 'dev-1',
    device_name: 'Entrada',
    status: 'pending',
    error_message: null,
    started_at: null,
    completed_at: null,
    ...over,
  }
}

function enrollment(pushes: EnrollmentDevicePush[]): Enrollment {
  return {
    id: 'enr-1',
    employee_id: EMPLOYEE.id,
    status: 'in_progress',
    started_at: '2026-04-28T12:00:00Z',
    completed_at: null,
    device_pushes: pushes,
  }
}

describe('InProgressList', () => {
  it('returns null when activeEnrollmentId is missing', () => {
    const { container } = render(<InProgressList activeEnrollmentId={null} activeEmployee={EMPLOYEE} enrollment={enrollment([push({})])} />)
    expect(container.firstChild).toBeNull()
  })

  it('returns null when activeEmployee is missing', () => {
    const { container } = render(<InProgressList activeEnrollmentId="enr-1" activeEmployee={null} enrollment={enrollment([push({})])} />)
    expect(container.firstChild).toBeNull()
  })

  it('returns null when enrollment is missing', () => {
    const { container } = render(<InProgressList activeEnrollmentId="enr-1" activeEmployee={EMPLOYEE} enrollment={null} />)
    expect(container.firstChild).toBeNull()
  })

  it('returns null when all device pushes are terminal (success or failed)', () => {
    const e = enrollment([
      push({ device_id: 'a', status: 'success' }),
      push({ device_id: 'b', status: 'failed' }),
    ])
    const { container } = render(
      <InProgressList activeEnrollmentId="enr-1" activeEmployee={EMPLOYEE} enrollment={e} />
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders title + employee name + success/total badge when enrollment is in progress', () => {
    const e = enrollment([
      push({ device_id: 'a', status: 'success' }),
      push({ device_id: 'b', status: 'in_progress' }),
      push({ device_id: 'c', status: 'pending' }),
    ])
    render(<InProgressList activeEnrollmentId="enr-1" activeEmployee={EMPLOYEE} enrollment={e} />)
    expect(screen.getByText('Enrolamientos en curso')).toBeTruthy()
    expect(screen.getByText('Ana García')).toBeTruthy()
    expect(screen.getByText(/1\/3 dispositivos/)).toBeTruthy()
  })

  it('omits the Ver detalles button when onReopen is not supplied', () => {
    const e = enrollment([push({ status: 'in_progress' })])
    render(<InProgressList activeEnrollmentId="enr-1" activeEmployee={EMPLOYEE} enrollment={e} />)
    expect(screen.queryByText('Ver detalles')).toBeNull()
  })

  it('Ver detalles button invokes onReopen', () => {
    const e = enrollment([push({ status: 'in_progress' })])
    const onReopen = vi.fn()
    render(
      <InProgressList
        activeEnrollmentId="enr-1"
        activeEmployee={EMPLOYEE}
        enrollment={e}
        onReopen={onReopen}
      />
    )
    fireEvent.click(screen.getByText('Ver detalles'))
    expect(onReopen).toHaveBeenCalled()
  })
})
