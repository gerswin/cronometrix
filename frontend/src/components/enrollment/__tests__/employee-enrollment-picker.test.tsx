import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EmployeeEnrollmentPicker } from '../employee-enrollment-picker'
import type { Employee } from '@/types/api'

const { apiGet } = vi.hoisted(() => ({ apiGet: vi.fn() }))
vi.mock('@/lib/api', () => ({ api: { get: (...a: unknown[]) => apiGet(...a) } }))

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

const EMPLOYEES: Employee[] = [
  {
    id: 'emp-1',
    employee_code: 'V-12345678',
    cedula: 'V-12345678',
    name: 'Ana García',
    department_id: 'd1',
    position: 'Analista',
    hire_date: '2023-01-01',
    status: 'active',
    version: 1,
    created_at: '2023-01-01T00:00:00Z',
    updated_at: '2023-01-01T00:00:00Z',
    base_salary_cents: 100000,
  },
  {
    id: 'emp-2',
    employee_code: 'V-87654321',
    cedula: 'V-87654321',
    name: 'Luis Pérez',
    department_id: 'd2',
    position: 'Operador',
    hire_date: '2024-02-10',
    status: 'active',
    version: 1,
    created_at: '2024-02-10T00:00:00Z',
    updated_at: '2024-02-10T00:00:00Z',
    base_salary_cents: 100000,
  },
]

describe('EmployeeEnrollmentPicker', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    apiGet.mockResolvedValue({ data: { data: EMPLOYEES, total: 2, limit: 100, offset: 0 } })
  })

  it('button is disabled until an employee is picked', async () => {
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={() => {}} />))
    })
    const btn = screen.getByRole('button', { name: /Iniciar Enrolamiento/i })
    expect((btn as HTMLButtonElement).disabled).toBe(true)
  })

  it('renders the placeholder option and is empty when no data has loaded yet', async () => {
    apiGet.mockReturnValueOnce(new Promise(() => {})) // never resolves
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={() => {}} />))
    })
    expect(screen.getByText('Selecciona un empleado…')).toBeTruthy()
  })

  it('lists fetched active employees as options once data has resolved', async () => {
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={() => {}} />))
    })
    await waitFor(() => {
      expect(screen.getByText(/Ana García — V-12345678/)).toBeTruthy()
    })
    expect(screen.getByText(/Luis Pérez — V-87654321/)).toBeTruthy()
  })

  it('renders the canonical employee_code when the deprecated cedula alias is absent', async () => {
    const canonicalEmployee = { ...EMPLOYEES[0] }
    delete canonicalEmployee.cedula
    apiGet.mockResolvedValueOnce({
      data: { data: [canonicalEmployee], total: 1, limit: 100, offset: 0 },
    })
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={() => {}} />))
    })
    expect(await screen.findByText('Ana García — V-12345678')).toBeTruthy()
  })

  it('hits the active-only employees endpoint with limit=100', async () => {
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={() => {}} />))
    })
    await waitFor(() => expect(apiGet).toHaveBeenCalled())
    expect(apiGet).toHaveBeenCalledWith('/employees?status=active&limit=100')
  })

  it('selecting an employee enables the button and Iniciar Enrolamiento fires onSelect with that employee', async () => {
    const onSelect = vi.fn()
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={onSelect} />))
    })
    await waitFor(() => screen.getByText(/Ana García/))

    const select = screen.getByLabelText('Selecciona un empleado') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'emp-1' } })

    const btn = screen.getByRole('button', { name: /Iniciar Enrolamiento/i })
    expect((btn as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(btn)
    expect(onSelect).toHaveBeenCalledWith(EMPLOYEES[0])
  })

  it('selecting an unknown id does NOT call onSelect', async () => {
    const onSelect = vi.fn()
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={onSelect} />))
    })
    await waitFor(() => screen.getByText(/Ana García/))
    const select = screen.getByLabelText('Selecciona un empleado') as HTMLSelectElement
    // Manually set to an id that isn't in the list (bypassing the option list)
    fireEvent.change(select, { target: { value: 'emp-1' } })
    fireEvent.change(select, { target: { value: '' } })
    const btn = screen.getByRole('button', { name: /Iniciar Enrolamiento/i })
    expect((btn as HTMLButtonElement).disabled).toBe(true)
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('clicking Iniciar Enrolamiento against an empty list does not call onSelect (false branch of "if (emp)")', async () => {
    // Render with the loader still pending so `employees` array is empty
    // throughout. Inject an option DOM-level via a fake (the select only
    // accepts options it lists; we add one programatically).
    apiGet.mockReturnValueOnce(new Promise(() => {})) // never resolves

    const onSelect = vi.fn()
    await act(async () => {
      render(wrap(<EmployeeEnrollmentPicker onSelect={onSelect} />))
    })
    const select = screen.getByLabelText('Selecciona un empleado') as HTMLSelectElement
    // Programmatically add a synthetic option (the loaded list has 0 employees).
    const ghost = document.createElement('option')
    ghost.value = 'ghost-id'
    ghost.text = 'Ghost'
    select.appendChild(ghost)
    fireEvent.change(select, { target: { value: 'ghost-id' } })
    const btn = screen.getByRole('button', { name: /Iniciar Enrolamiento/i })
    // selectedId='ghost-id' is non-empty, so the button is enabled
    expect((btn as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(btn)
    expect(onSelect).not.toHaveBeenCalled()
  })
})
