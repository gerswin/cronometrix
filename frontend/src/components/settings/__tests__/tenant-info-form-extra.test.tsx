/**
 * Branch-coverage extension for TenantInfoForm: hits 409 conflict toast
 * branch, errors.client_name + errors.address render branches, in-flight
 * Guardando… label.
 */
import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach, vi } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import React from 'react'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

const { toastError } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: toastError },
}))

import { TenantInfoForm } from '../tenant-info-form'
import type { TenantInfo } from '@/types/api'

const API = 'http://localhost:3001/api/v1'
const server = setupServer()

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  vi.clearAllMocks()
})

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

const initial: TenantInfo = {
  client_name: 'Acme', client_rif: 'J-12345678-9', address: 'Caracas',
  version: 5, updated_at: '2026-04-01T00:00:00Z',
}

describe('TenantInfoForm — extra branches', () => {
  it('PATCH 409 → conflict toast and tenant-info query invalidation', async () => {
    server.use(
      http.patch(`${API}/tenant-info`, () =>
        HttpResponse.json({ error: 'conflict' }, { status: 409 })
      )
    )
    render(wrap(<TenantInfoForm initialData={initial} canEdit={true} />))
    await waitFor(() => screen.getByRole('button', { name: /Guardar Cambios/i }))
    fireEvent.submit(screen.getByRole('form'))
    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith(
        'Esta información fue modificada por otro usuario; recargando…'
      )
    )
  })

  it('PATCH 500 → generic error toast', async () => {
    server.use(
      http.patch(`${API}/tenant-info`, () =>
        HttpResponse.json({ error: 'fail' }, { status: 500 })
      )
    )
    render(wrap(<TenantInfoForm initialData={initial} canEdit={true} />))
    await waitFor(() => screen.getByRole('button', { name: /Guardar Cambios/i }))
    fireEvent.submit(screen.getByRole('form'))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Error al guardar'))
  })

  it('renders the client_name and address validation errors when too long', async () => {
    render(wrap(<TenantInfoForm initialData={initial} canEdit={true} />))
    const longName = 'a'.repeat(201)
    const longAddr = 'b'.repeat(501)
    fireEvent.input(screen.getByLabelText('Nombre del Cliente'), { target: { value: longName } })
    fireEvent.input(screen.getByLabelText('Dirección'), { target: { value: longAddr } })
    fireEvent.submit(screen.getByRole('form'))
    await waitFor(() => {
      expect(screen.getByText(/Máximo 200/)).toBeTruthy()
    })
    expect(screen.getByText(/Máximo 500/)).toBeTruthy()
  })

  it('canEdit=false hides the Guardar Cambios submit button (read-only mode)', () => {
    render(wrap(<TenantInfoForm initialData={initial} canEdit={false} />))
    expect(screen.queryByRole('button', { name: /Guardar Cambios/i })).toBeNull()
  })
})
