import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { SyncPanel } from '../sync-panel'
import type { EnrollmentDevicePush } from '@/types/api'

const { toastError } = vi.hoisted(() => ({ toastError: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: toastError } }))

vi.mock('@/lib/api', () => ({
  api: {
    get: vi.fn(),
    post: vi.fn().mockResolvedValue({ data: { ok: true } }),
  },
}))

import { api } from '@/lib/api'

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>
}

const makePush = (
  device_id: string,
  device_name: string,
  status: EnrollmentDevicePush['status']
): EnrollmentDevicePush => ({
  id: `push-${device_id}`,
  device_id,
  device_name,
  status,
  error_message: status === 'failed' ? 'Connection refused' : null,
  started_at: null,
  completed_at: null,
})

describe('SyncPanel', () => {
  const enrollmentId = 'enr-001'

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders one SyncRow per device_pushes entry', async () => {
    const pushes = [
      makePush('dev-1', 'Entrada Principal', 'success'),
      makePush('dev-2', 'Salida', 'failed'),
      makePush('dev-3', 'Recepción', 'in_progress'),
    ]
    await act(async () => {
      render(<SyncPanel device_pushes={pushes} enrollmentId={enrollmentId} />, { wrapper })
    })
    expect(screen.getByText('Entrada Principal')).toBeTruthy()
    expect(screen.getByText('Salida')).toBeTruthy()
    expect(screen.getByText('Recepción')).toBeTruthy()
    expect(screen.getByTestId('enrollment-push-row-dev-1')).toBeTruthy()
    expect(screen.getByTestId('enrollment-push-status-dev-3')).toBeTruthy()
  })

  it('Reintentar button rendered only on failed rows', async () => {
    const pushes = [
      makePush('dev-1', 'Entrada Principal', 'success'),
      makePush('dev-2', 'Salida', 'failed'),
      makePush('dev-3', 'Recepción', 'in_progress'),
    ]
    await act(async () => {
      render(<SyncPanel device_pushes={pushes} enrollmentId={enrollmentId} />, { wrapper })
    })
    const retryBtns = screen.getAllByRole('button', { name: /Reintentar/i })
    expect(retryBtns).toHaveLength(1)
    expect(screen.getByTestId('enrollment-retry-dev-2')).toBe(retryBtns[0])
  })

  it('Reintentar uses the canonical push path and invalidates detail plus in-progress list', async () => {
    const invalidate = vi.spyOn(QueryClient.prototype, 'invalidateQueries')
    const pushes = [makePush('dev-2', 'Salida', 'failed')]
    await act(async () => {
      render(<SyncPanel device_pushes={pushes} enrollmentId={enrollmentId} />, { wrapper })
    })
    const retryBtn = screen.getByRole('button', { name: /Reintentar/i })
    await act(async () => { fireEvent.click(retryBtn) })
    expect(api.post).toHaveBeenCalledWith(`/enrollments/${enrollmentId}/pushes/dev-2/retry`)
    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['enrollment', enrollmentId] })
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['enrollments', 'in_progress'] })
    })
  })

  it('Reintentar shows the canonical API error message', async () => {
    vi.mocked(api.post).mockRejectedValueOnce({
      response: {
        data: {
          error: {
            code: 'DEVICE_PUSH_FAILED',
            message: 'El dispositivo sigue fuera de línea.',
            status: 503,
          },
        },
      },
    })
    render(
      <SyncPanel
        device_pushes={[makePush('dev-2', 'Salida', 'failed')]}
        enrollmentId={enrollmentId}
      />,
      { wrapper },
    )
    fireEvent.click(screen.getByRole('button', { name: /Reintentar/i }))

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith('El dispositivo sigue fuera de línea.')
    })
  })

  it('empty state when device_pushes is empty', async () => {
    await act(async () => {
      render(<SyncPanel device_pushes={[]} enrollmentId={enrollmentId} />, { wrapper })
    })
    expect(screen.getByText(/No hay dispositivos activos/)).toBeTruthy()
  })
})
