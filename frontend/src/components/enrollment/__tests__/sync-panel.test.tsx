import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { SyncPanel } from '../sync-panel'
import type { EnrollmentDevicePush } from '@/types/api'

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
  })

  it('Reintentar fires POST /enrollments/:id/devices/:device_id/retry', async () => {
    const pushes = [makePush('dev-2', 'Salida', 'failed')]
    await act(async () => {
      render(<SyncPanel device_pushes={pushes} enrollmentId={enrollmentId} />, { wrapper })
    })
    const retryBtn = screen.getByRole('button', { name: /Reintentar/i })
    await act(async () => { fireEvent.click(retryBtn) })
    expect(api.post).toHaveBeenCalledWith(`/enrollments/${enrollmentId}/devices/dev-2/retry`)
  })

  it('empty state when device_pushes is empty', async () => {
    await act(async () => {
      render(<SyncPanel device_pushes={[]} enrollmentId={enrollmentId} />, { wrapper })
    })
    expect(screen.getByText(/No hay dispositivos activos/)).toBeTruthy()
  })
})
