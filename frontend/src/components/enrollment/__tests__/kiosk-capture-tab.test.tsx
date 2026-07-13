import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { KioskCaptureTab } from '../kiosk-capture-tab'
import deviceFixture from '../../devices/__tests__/fixtures/device.json'

// Stub URL methods
globalThis.URL.createObjectURL = vi.fn(() => 'blob:test-kiosk-url')
globalThis.URL.revokeObjectURL = vi.fn()

// Mock api
vi.mock('@/lib/api', () => ({
  api: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

import { api } from '@/lib/api'

function makeWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 0 }, mutations: { retry: false } },
  })
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  }
}

const DEVICE = deviceFixture

describe('KioskCaptureTab', () => {
  const mockOnCaptured = vi.fn()
  const employeeId = '00000000-0000-0000-0000-000000000001'

  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('devices')) {
        return Promise.resolve({ data: { data: [DEVICE] } })
      }
      return Promise.resolve({ data: {} })
    })
  })

  it('initial state: Select device + Iniciar Captura visible', async () => {
    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} />, { wrapper: makeWrapper() })
    })
    expect(screen.getByLabelText(/Seleccionar dispositivo/i)).toBeTruthy()
    expect(screen.getByRole('button', { name: /Iniciar Captura/i })).toBeTruthy()
  })

  it('Iniciar Captura mutation fires POST /enrollments/capture-from-device', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-123', status: 'capturing' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({ data: { capture_id: 'cap-123', status: 'capturing', photo_b64: null, photo_path: null, error_message: null } })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} />, { wrapper: makeWrapper() })
    })

    // Wait for device options to appear
    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))

    // Select device then click
    const select = screen.getByLabelText(/Seleccionar dispositivo/i) as HTMLSelectElement
    await act(async () => { fireEvent.change(select, { target: { value: 'dev-entry' } }) })

    // Button should now be enabled — find it and click
    const btn = screen.getByRole('button', { name: /Iniciar Captura/i })
    await act(async () => { fireEvent.click(btn) })

    await waitFor(() => {
      expect(api.post).toHaveBeenCalledWith(
        '/enrollments/capture-from-device',
        { device_id: 'dev-entry', employee_id: employeeId }
      )
    })
  })

  it('polls /captures/:id; when status==captured + photo_b64 → Blob → preview + Aceptar', async () => {
    const b64 = btoa('fake-jpeg-bytes')
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-456', status: 'capturing' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({
          data: { capture_id: 'cap-456', status: 'captured', photo_b64: b64, photo_path: null, error_message: null },
        })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} />, { wrapper: makeWrapper() })
    })

    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))

    const select = screen.getByLabelText(/Seleccionar dispositivo/i) as HTMLSelectElement
    await act(async () => { fireEvent.change(select, { target: { value: 'dev-entry' } }) })
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: /Iniciar Captura/i })) })

    // Wait for mutation + poll to resolve → captured state → Aceptar visible
    await waitFor(() => screen.queryByRole('button', { name: /Aceptar/i }) !== null, { timeout: 4000 })

    const acceptBtn = screen.getByRole('button', { name: /Aceptar/i })
    await act(async () => { fireEvent.click(acceptBtn) })
    expect(mockOnCaptured).toHaveBeenCalledWith(expect.any(Blob))
    const blob = mockOnCaptured.mock.calls[0][0] as Blob
    expect(blob.type).toBe('image/jpeg')
  })

  it('timeout response: amber alert with "No se detectó captura."', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-789', status: 'capturing' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({
          data: { capture_id: 'cap-789', status: 'timeout', photo_b64: null, photo_path: null, error_message: null },
        })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} />, { wrapper: makeWrapper() })
    })

    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))

    const select = screen.getByLabelText(/Seleccionar dispositivo/i) as HTMLSelectElement
    await act(async () => { fireEvent.change(select, { target: { value: 'dev-entry' } }) })

    // Wait for button to become enabled (device selected, not disabled)
    const btn = await waitFor(() => {
      const b = screen.getByRole('button', { name: /Iniciar Captura/i })
      if (b.hasAttribute('disabled')) throw new Error('button still disabled')
      return b
    })
    await act(async () => { fireEvent.click(btn) })

    // Wait for mutation → poll → timeout state transition
    await waitFor(() => {
      if (screen.queryByRole('alert') === null) throw new Error('alert not yet visible')
    }, { timeout: 10000 })
    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('No se detectó captura')
    expect(screen.getByRole('button', { name: /Reintentar/i })).toBeTruthy()
  })
})
