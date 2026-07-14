import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { InProgressList } from '../in-progress-list'
import type { Enrollment, EnrollmentDevicePush, PaginatedResponse } from '@/types/api'

const { listMock } = vi.hoisted(() => ({ listMock: vi.fn() }))
vi.mock('@/lib/enrollment-api', () => ({
  listInProgressEnrollments: (...args: unknown[]) => listMock(...args),
}))

function push(over: Partial<EnrollmentDevicePush> = {}): EnrollmentDevicePush {
  return {
    id: 'push-1',
    device_id: 'dev-1',
    device_name: 'Entrada',
    status: 'pending',
    error_message: null,
    started_at: null,
    completed_at: null,
    ...over,
  }
}

function enrollment(over: Partial<Enrollment> = {}): Enrollment {
  return {
    id: 'enr-1',
    employee_id: 'emp-1',
    employee_name: 'Ana García',
    employee_code: 'V-1',
    status: 'in_progress',
    started_at: '2026-04-28T12:00:00Z',
    completed_at: null,
    version: 1,
    device_pushes: [push()],
    ...over,
  }
}

function page(data: Enrollment[]): PaginatedResponse<Enrollment> {
  return { data, total: data.length, limit: 100, offset: 0 }
}

function makeWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
}

describe('InProgressList', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listMock.mockResolvedValue(page([]))
  })

  it('queries the server-backed in-progress page at limit 100', async () => {
    render(<InProgressList onReopen={() => {}} />, { wrapper: makeWrapper() })
    await waitFor(() => expect(listMock).toHaveBeenCalledWith({ limit: 100 }))
  })

  it('renders every server result with stable row and reopen test IDs', async () => {
    listMock.mockResolvedValueOnce(page([
      enrollment(),
      enrollment({ id: 'enr-2', employee_name: 'Luis Pérez', employee_code: 'V-2' }),
    ]))
    render(<InProgressList onReopen={() => {}} />, { wrapper: makeWrapper() })

    expect(await screen.findByTestId('enrollment-row-enr-1')).toBeTruthy()
    expect(screen.getByTestId('enrollment-row-enr-2')).toBeTruthy()
    expect(screen.getByTestId('enrollment-reopen-enr-1')).toBeTruthy()
  })

  it('treats an in-progress enrollment with zero pushes as non-terminal', async () => {
    listMock.mockResolvedValueOnce(page([enrollment({ device_pushes: [] })]))
    render(<InProgressList onReopen={() => {}} />, { wrapper: makeWrapper() })

    expect(await screen.findByText('Ana García')).toBeTruthy()
    expect(screen.getByText('0/0 dispositivos')).toBeTruthy()
  })

  it('reopens by enrollment id and remains recoverable after a fresh remount', async () => {
    listMock.mockResolvedValue(page([enrollment()]))
    const onReopen = vi.fn()
    const first = render(<InProgressList onReopen={onReopen} />, { wrapper: makeWrapper() })
    fireEvent.click(await screen.findByTestId('enrollment-reopen-enr-1'))
    expect(onReopen).toHaveBeenCalledWith('enr-1')
    first.unmount()

    render(<InProgressList onReopen={onReopen} />, { wrapper: makeWrapper() })
    expect(await screen.findByTestId('enrollment-row-enr-1')).toBeTruthy()
    expect(listMock).toHaveBeenCalledTimes(2)
  })

  it('renders nothing when the server has no in-progress enrollments', async () => {
    const { container } = render(<InProgressList onReopen={() => {}} />, { wrapper: makeWrapper() })
    await waitFor(() => expect(listMock).toHaveBeenCalled())
    expect(container.firstChild).toBeNull()
  })
})
