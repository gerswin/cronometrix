import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { KioskCaptureTab } from '../kiosk-capture-tab'
import deviceFixture from '../../devices/__tests__/fixtures/device.json'

const { toastError } = vi.hoisted(() => ({ toastError: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: toastError } }))

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
  const mockOnCleared = vi.fn()
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
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />, { wrapper: makeWrapper() })
    })
    expect(screen.getByLabelText(/Seleccionar dispositivo/i)).toBeTruthy()
    expect(screen.getByRole('button', { name: /Iniciar Captura/i })).toBeTruthy()
  })

  it('Iniciar Captura mutation fires the canonical POST /enrollments/captures', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-123', status: 'capturing', source_device_id: 'dev-entry' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({ data: { capture_id: 'cap-123', status: 'capturing', source_device_id: 'dev-entry', photo_path: null, error_message: null } })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />, { wrapper: makeWrapper() })
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
        '/enrollments/captures',
        { device_id: 'dev-entry', employee_id: employeeId }
      )
    })
  })

  it('polls /captures/:id; when status==captured + photo_b64 → Blob → preview + Aceptar', async () => {
    const b64 = btoa('fake-jpeg-bytes')
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-456', status: 'capturing', source_device_id: 'dev-entry' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({
          data: { capture_id: 'cap-456', status: 'captured', source_device_id: 'dev-origin', photo_b64: b64, photo_path: null, error_message: null },
        })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />, { wrapper: makeWrapper() })
    })

    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))

    const select = screen.getByLabelText(/Seleccionar dispositivo/i) as HTMLSelectElement
    await act(async () => { fireEvent.change(select, { target: { value: 'dev-entry' } }) })
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: /Iniciar Captura/i })) })

    // Wait for mutation + poll to resolve → captured state → Aceptar visible
    await waitFor(() => screen.queryByRole('button', { name: /Aceptar/i }) !== null, { timeout: 4000 })

    const acceptBtn = screen.getByRole('button', { name: /Aceptar/i })
    await act(async () => { fireEvent.click(acceptBtn) })
    expect(mockOnCaptured).toHaveBeenCalledWith({
      blob: expect.any(Blob),
      capturedVia: 'device',
      sourceDeviceId: 'dev-origin',
    })
    const blob = mockOnCaptured.mock.calls[0][0].blob as Blob
    expect(blob.type).toBe('image/jpeg')
  })

  it('timeout response: amber alert with "No se detectó captura."', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-789', status: 'capturing', source_device_id: 'dev-entry' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return Promise.resolve({
          data: { capture_id: 'cap-789', status: 'timeout', source_device_id: 'dev-entry', photo_path: null, error_message: null },
        })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    await act(async () => {
      render(<KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />, { wrapper: makeWrapper() })
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

  it('ignores a stale capture poll when the employee changes', async () => {
    let resolveCapture!: (value: { data: unknown }) => void
    vi.mocked(api.post).mockResolvedValueOnce({
      data: { capture_id: 'cap-old', status: 'capturing', source_device_id: 'dev-entry' },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.includes('captures')) {
        return new Promise((resolve) => { resolveCapture = resolve })
      }
      return Promise.resolve({ data: { data: [DEVICE] } })
    })

    let rerender!: (ui: React.ReactNode) => void
    await act(async () => {
      const rendered = render(
        <KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />,
        { wrapper: makeWrapper() },
      )
      rerender = rendered.rerender
    })
    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))
    fireEvent.change(screen.getByLabelText(/Seleccionar dispositivo/i), {
      target: { value: 'dev-entry' },
    })
    fireEvent.click(screen.getByRole('button', { name: /Iniciar Captura/i }))
    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/captures/cap-old'))

    rerender(
      <KioskCaptureTab employeeId="emp-new" onCaptured={mockOnCaptured} onCleared={mockOnCleared} />,
    )
    expect(screen.getByRole('button', { name: /Iniciar Captura/i })).toBeTruthy()
    await act(async () => {
      resolveCapture({
        data: {
          capture_id: 'cap-old',
          status: 'captured',
          source_device_id: 'dev-entry',
          photo_b64: btoa('stale'),
          photo_path: null,
          error_message: null,
        },
      })
    })

    expect(screen.queryByRole('button', { name: /Aceptar/i })).toBeNull()
    expect(mockOnCaptured).not.toHaveBeenCalled()
  })

  it('ignores a stale capture-start error after the employee changes', async () => {
    let rejectStart!: (reason: unknown) => void
    vi.mocked(api.post).mockReturnValueOnce(
      new Promise((_resolve, reject) => { rejectStart = reject }),
    )
    const rendered = render(
      <KioskCaptureTab employeeId={employeeId} onCaptured={mockOnCaptured} onCleared={mockOnCleared} />,
      { wrapper: makeWrapper() },
    )
    await waitFor(() => screen.getByText('Entrada Principal (127.0.0.1)'))
    fireEvent.change(screen.getByLabelText(/Seleccionar dispositivo/i), {
      target: { value: 'dev-entry' },
    })
    fireEvent.click(screen.getByRole('button', { name: /Iniciar Captura/i }))
    await waitFor(() => expect(api.post).toHaveBeenCalled())

    rendered.rerender(
      <KioskCaptureTab employeeId="emp-new" onCaptured={mockOnCaptured} onCleared={mockOnCleared} />,
    )
    await act(async () => {
      rejectStart({ response: { data: { message: 'captura vieja' } } })
    })

    expect(toastError).not.toHaveBeenCalled()
  })
})
