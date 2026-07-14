/**
 * Branch & error-path coverage extension for EnrollmentModal.
 * The original enrollment-modal.test.tsx covers the happy paths; this file
 * adds: closed-modal early-return, webcam tab activation, partial-success
 * sticky toast, in-flight close (sticky toast with Infinity duration),
 * and the api error onError branch.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EnrollmentModal } from '../enrollment-modal'
import type { Employee } from '@/types/api'

globalThis.URL.createObjectURL = vi.fn(() => 'blob:test-extra-url')
globalThis.URL.revokeObjectURL = vi.fn()

const { toastSuccess, toastWarning, toastError, toastBase, toastDismiss } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastWarning: vi.fn(),
  toastError: vi.fn(),
  toastBase: vi.fn(),
  toastDismiss: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: Object.assign((msg: unknown, opts?: unknown) => toastBase(msg, opts), {
    success: toastSuccess,
    warning: toastWarning,
    error: toastError,
    dismiss: toastDismiss,
  }),
}))

vi.mock('@/lib/api', () => ({
  api: { get: vi.fn(), post: vi.fn() },
}))
vi.mock('@/lib/face-detection', () => ({
  loadFaceApi: vi.fn().mockResolvedValue({}),
  analyzeFrame: vi.fn().mockResolvedValue({
    faceDetected: true, luminanceOk: true, sizeOk: true, luminance: 120, width: 200, height: 200,
  }),
  analyzePhotoBlob: vi.fn().mockResolvedValue({
    faceDetected: true, luminanceOk: true, sizeOk: true, luminance: 120, width: 200, height: 200,
  }),
  isAcceptableFace: vi.fn((analysis) =>
    analysis.faceDetected && analysis.luminanceOk && analysis.sizeOk
  ),
}))

Object.defineProperty(globalThis.navigator, 'mediaDevices', {
  value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] }) },
  writable: true, configurable: true,
})

import { api } from '@/lib/api'

const EMPLOYEE: Employee = {
  id: 'emp-001', employee_code: 'V-12345678', name: 'Ana García',
  department_id: 'dept-1', position: 'Analista', hire_date: '2023-01-01',
  status: 'active', version: 1, base_salary_cents: 100000, created_at: '2023-01-01T00:00:00Z', updated_at: '2023-01-01T00:00:00Z',
}

function makeWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  }
}

describe('EnrollmentModal — extra branches', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(api.get).mockResolvedValue({ data: { data: [] } })
    vi.mocked(api.post).mockResolvedValue({
      data: {
        enrollment_id: 'enr-1',
        device_pushes: [
          { device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
  })

  it('closed modal does not render the title (early DialogTitle absent)', () => {
    render(<EnrollmentModal open={false} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    expect(screen.queryByText(/Enrolamiento Facial/)).toBeNull()
  })

  it('webcam tab activation: switches to webcam and shows the validation panel header', async () => {
    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    const webcamTab = screen.getByText('Webcam')
    await act(async () => { fireEvent.click(webcamTab) })
    // ValidationPanel renders only when tab='webcam' and !photoBlob
    expect(screen.getByText(/Validación de IA/i)).toBeTruthy()
  })

  it('Cerrar without enrollmentId fires onClose without sticky toast', async () => {
    const onClose = vi.fn()
    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={onClose} />, { wrapper: makeWrapper() })
    })
    const closeBtn = screen.getByRole('button', { name: /^Cerrar$/ })
    await act(async () => { fireEvent.click(closeBtn) })
    expect(onClose).toHaveBeenCalled()
    expect(toastBase).not.toHaveBeenCalled()
  })

  it('closing a terminal enrollment resets to a clean session for the same employee', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-terminal-close',
        face_id: 'face-terminal-close',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url === '/enrollments/enr-terminal-close') {
        return Promise.resolve({
          data: {
            id: 'enr-terminal-close', employee_id: EMPLOYEE.id,
            employee_name: EMPLOYEE.name, employee_code: EMPLOYEE.employee_code,
            status: 'success', started_at: '2026-04-28T12:00:00Z',
            completed_at: '2026-04-28T12:01:00Z', version: 1,
            device_pushes: [
              { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })

    const rendered = render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: makeWrapper() },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())

    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))
    rendered.rerender(<EnrollmentModal open={false} employee={EMPLOYEE} onClose={() => {}} />)
    rendered.rerender(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />)

    expect(screen.getByText('Lector Hikvision')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
  })

  it('clears a terminal enrollment observed in the background before reopening the same employee', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-background-terminal',
        face_id: 'face-background-terminal',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>
    }

    const rendered = render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: Wrapper },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/enr-background-terminal'))

    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))
    rendered.rerender(<EnrollmentModal open={false} employee={EMPLOYEE} onClose={() => {}} />)
    await act(async () => {
      client.setQueryData(['enrollment', 'enr-background-terminal'], {
        id: 'enr-background-terminal',
        employee_id: EMPLOYEE.id,
        employee_name: EMPLOYEE.name,
        employee_code: EMPLOYEE.employee_code,
        status: 'success',
        started_at: '2026-04-28T12:00:00Z',
        completed_at: '2026-04-28T12:01:00Z',
        version: 1,
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
        ],
      })
    })

    await waitFor(() => {
      expect(toastSuccess).toHaveBeenCalledWith(
        `Enrolamiento completado para ${EMPLOYEE.name}.`,
        { id: 'enrollment-enr-background-terminal' },
      )
    })
    rendered.rerender(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />)

    expect(screen.getByText('Lector Hikvision')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
    expect(screen.queryByText(/Enrolamiento enviado/)).toBeNull()
  })

  it('cleans an already-notified terminal enrollment when the controlled modal closes', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-terminal-before-close',
        face_id: 'face-terminal-before-close',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>
    }

    const rendered = render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: Wrapper },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/enr-terminal-before-close'))

    await act(async () => {
      client.setQueryData(['enrollment', 'enr-terminal-before-close'], {
        id: 'enr-terminal-before-close',
        employee_id: EMPLOYEE.id,
        employee_name: EMPLOYEE.name,
        employee_code: EMPLOYEE.employee_code,
        status: 'success',
        started_at: '2026-04-28T12:00:00Z',
        completed_at: '2026-04-28T12:01:00Z',
        version: 1,
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
        ],
      })
    })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledTimes(1))
    expect(screen.getByText(/Enrolamiento enviado/)).toBeTruthy()

    rendered.rerender(<EnrollmentModal open={false} employee={EMPLOYEE} onClose={() => {}} />)
    rendered.rerender(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />)

    expect(toastSuccess).toHaveBeenCalledTimes(1)
    expect(screen.getByText('Lector Hikvision')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
    expect(screen.queryByText(/Enrolamiento enviado/)).toBeNull()
  })

  it('invalidates the in-progress list exactly once when polling first observes terminal state', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-terminal-invalidate', face_id: 'face-terminal-invalidate',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url === '/enrollments/enr-terminal-invalidate') {
        return Promise.resolve({
          data: {
            id: 'enr-terminal-invalidate', employee_id: EMPLOYEE.id,
            employee_name: EMPLOYEE.name, employee_code: EMPLOYEE.employee_code,
            status: 'success', started_at: '2026-04-28T12:00:00Z',
            completed_at: '2026-04-28T12:01:00Z', version: 1,
            device_pushes: [
              { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>
    }

    render(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />, { wrapper: Wrapper })
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())

    const listInvalidations = invalidate.mock.calls.filter(
      ([filters]) => JSON.stringify(filters?.queryKey) === JSON.stringify(['enrollments', 'in_progress']),
    )
    expect(listInvalidations).toHaveLength(2)
    await act(async () => { await Promise.resolve() })
    expect(invalidate.mock.calls.filter(
      ([filters]) => JSON.stringify(filters?.queryKey) === JSON.stringify(['enrollments', 'in_progress']),
    )).toHaveLength(2)
  })

  it('closing during submit preserves a backend-created enrollment and starts recovery when it resolves', async () => {
    let resolveSubmit!: (value: { data: unknown }) => void
    vi.mocked(api.post).mockReturnValueOnce(
      new Promise((resolve) => { resolveSubmit = resolve }),
    )
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: makeWrapper() },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(screen.getByText('Enviando…')).toBeTruthy())

    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))
    await act(async () => {
      resolveSubmit({
        data: {
          enrollment_id: 'enr-after-close', face_id: 'face-after-close',
          device_pushes: [
            { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
          ],
        },
      })
    })

    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/enr-after-close'))
    expect(toastBase).toHaveBeenCalledWith(
      'Enrolamiento en curso — 0/1 dispositivos',
      { id: 'enrollment-enr-after-close', duration: Infinity },
    )
  })

  it('locks capture controls while submit is pending and adopts the eventual enrollment', async () => {
    let resolveSubmit!: (value: { data: unknown }) => void
    vi.mocked(api.post).mockReturnValueOnce(
      new Promise((resolve) => { resolveSubmit = resolve }),
    )
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: makeWrapper() },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'pending.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(screen.getByText('Enviando…')).toBeTruthy())

    const removeButton = screen.getByRole('button', { name: 'Quitar imagen' })
    const changeButton = screen.getByRole('button', { name: 'Cambiar archivo' })
    const webcamTab = screen.getByTestId('enroll-tab-webcam')
    const closeButton = screen.getByRole('button', { name: /^Cerrar$/ })
    expect(removeButton).toBeDisabled()
    expect(changeButton).toBeDisabled()
    expect(webcamTab).toBeDisabled()
    expect(screen.getByTestId('enroll-tab-hikvision')).toBeDisabled()
    expect(screen.getByTestId('enroll-tab-upload')).toBeDisabled()
    expect(closeButton).not.toBeDisabled()

    fireEvent.click(removeButton)
    fireEvent.click(webcamTab)
    await act(async () => {
      resolveSubmit({
        data: {
          enrollment_id: 'enr-pending-controls',
          face_id: 'face-pending-controls',
          device_pushes: [
            { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
          ],
        },
      })
    })

    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/enr-pending-controls'))
    expect(screen.getByText(/Enrolamiento enviado/)).toBeTruthy()
  })

  it('closing after enrollment_id but before the first poll shows an in-flight recovery toast', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-before-poll', face_id: 'face-before-poll',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
          { id: 'p2', device_id: 'd2', device_name: 'Salida', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    render(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />, {
      wrapper: makeWrapper(),
    })
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(screen.getByText(/Enrolamiento enviado/)).toBeTruthy())

    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))

    expect(toastBase).toHaveBeenCalledWith(
      'Enrolamiento en curso — 0/2 dispositivos',
      { id: 'enrollment-enr-before-poll', duration: Infinity },
    )
  })

  it('dismisses the old infinite recovery toast when changing enrollment session', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-switch', face_id: 'face-switch',
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url === '/enrollments/enr-switch') {
        return Promise.resolve({
          data: {
            id: 'enr-switch', employee_id: EMPLOYEE.id,
            employee_name: EMPLOYEE.name, employee_code: EMPLOYEE.employee_code,
            status: 'in_progress', started_at: '2026-04-28T12:00:00Z',
            completed_at: null, version: 1,
            device_pushes: [
              { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })
    const rendered = render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
      { wrapper: makeWrapper() },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(screen.getByText(/Enrolamiento enviado/)).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))
    expect(toastBase).toHaveBeenCalled()

    rendered.rerender(
      <EnrollmentModal
        open
        employee={{ ...EMPLOYEE, id: 'emp-2', name: 'Luis Pérez' }}
        onClose={() => {}}
      />,
    )

    expect(toastDismiss).toHaveBeenCalledWith('enrollment-enr-switch')
    expect(screen.getByText(/Luis Pérez/)).toBeTruthy()
  })

  it('does not emit a stale terminal toast with the next employee identity', async () => {
    vi.mocked(api.get).mockReturnValue(new Promise(() => {}))
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>
    }
    const rendered = render(
      <EnrollmentModal
        open
        employee={EMPLOYEE}
        initialEnrollmentId="enr-old-terminal"
        onClose={() => {}}
      />,
      { wrapper: Wrapper },
    )

    await act(async () => {
      client.setQueryData(['enrollment', 'enr-old-terminal'], {
        id: 'enr-old-terminal', employee_id: EMPLOYEE.id,
        employee_name: EMPLOYEE.name, employee_code: EMPLOYEE.employee_code,
        status: 'success', started_at: '2026-04-28T12:00:00Z',
        completed_at: '2026-04-28T12:01:00Z', version: 1,
        device_pushes: [
          { id: 'p1', device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
        ],
      })
      rendered.rerender(
        <EnrollmentModal
          open
          employee={{ ...EMPLOYEE, id: 'emp-2', name: 'Luis Pérez' }}
          initialEnrollmentId={null}
          onClose={() => {}}
        />,
      )
    })

    expect(toastSuccess).not.toHaveBeenCalled()
    expect(screen.getByText(/Luis Pérez/)).toBeTruthy()
  })

  it('submit error path triggers toast.error with the server message', async () => {
    vi.mocked(api.post).mockRejectedValueOnce({
      response: {
        data: {
          error: {
            code: 'ENROLLMENT_VALIDATION_FAILED',
            message: 'Validación falló en el servidor',
            status: 422,
          },
        },
      },
    })
    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    // Force submit-able state by uploading a file
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = new File([new Uint8Array(100)], 'photo.jpg', { type: 'image/jpeg' })
    await act(async () => { fireEvent.change(input, { target: { files: [goodJpeg] } }) })

    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    const enrollBtn = screen.getByRole('button', { name: /Enrolar/i })
    await act(async () => { fireEvent.click(enrollBtn) })
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Validación falló en el servidor'))
  })

  it('submit error path falls back to default Spanish copy when no server message', async () => {
    vi.mocked(api.post).mockRejectedValueOnce({})
    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = new File([new Uint8Array(100)], 'photo.jpg', { type: 'image/jpeg' })
    await act(async () => { fireEvent.change(input, { target: { files: [goodJpeg] } }) })
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith('No se pudo registrar el enrolamiento.')
    )
  })

  it('keeps polling an in-progress enrollment with zero device pushes', async () => {
    vi.mocked(api.get).mockResolvedValue({
      data: {
        id: 'enr-zero',
        employee_id: EMPLOYEE.id,
        employee_name: EMPLOYEE.name,
        employee_code: EMPLOYEE.employee_code,
        status: 'in_progress',
        started_at: '2026-04-28T12:00:00Z',
        completed_at: null,
        version: 1,
        device_pushes: [],
      },
    })
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    })
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <QueryClientProvider client={client}>{children}</QueryClientProvider>
    }
    render(
      <EnrollmentModal
        open
        employee={null}
        initialEnrollmentId="enr-zero"
        onClose={() => {}}
      />,
      { wrapper: Wrapper },
    )
    await waitFor(() => expect(api.get).toHaveBeenCalledWith('/enrollments/enr-zero'))

    const query = client.getQueryCache().find({ queryKey: ['enrollment', 'enr-zero'] })
    const interval = (query?.options as { refetchInterval?: unknown } | undefined)?.refetchInterval
    expect(typeof interval).toBe('function')
    expect((interval as (q: typeof query) => number | false)(query)).toBe(1500)
  })

  it('terminal poll all-success: fires toast.success with completed copy', async () => {
    // First api.post for /enrollments creates the enrollment_id; subsequent
    // api.get('/enrollments/enr-1') returns the polling status. We resolve
    // every poll with a fully-success enrollment to trigger the all-success
    // toast branch (line 99-101).
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-1',
        device_pushes: [
          { device_id: 'd1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.startsWith('/enrollments/enr-1')) {
        return Promise.resolve({
          data: {
            id: 'enr-1', employee_id: EMPLOYEE.id, status: 'success',
            started_at: '2026-04-28T12:00:00Z', completed_at: '2026-04-28T12:01:00Z',
            device_pushes: [
              { device_id: 'd1', device_name: 'Entrada', status: 'success', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })

    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    await act(async () => {
      fireEvent.change(input, { target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] } })
    })
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled(), { timeout: 3000 })
    const successCalls = toastSuccess.mock.calls.map((c) => c[0])
    expect(successCalls.some((m) => typeof m === 'string' && m.includes('Enrolamiento completado'))).toBe(true)
  })

  it('terminal poll partial: fires toast.warning with partial copy', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-2',
        device_pushes: [
          { device_id: 'd1', device_name: 'A', status: 'pending', error_message: null, started_at: null, completed_at: null },
          { device_id: 'd2', device_name: 'B', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.startsWith('/enrollments/enr-2')) {
        return Promise.resolve({
          data: {
            id: 'enr-2', employee_id: EMPLOYEE.id, status: 'partial',
            started_at: '2026-04-28T12:00:00Z', completed_at: '2026-04-28T12:01:00Z',
            device_pushes: [
              { device_id: 'd1', device_name: 'A', status: 'success', error_message: null, started_at: null, completed_at: null },
              { device_id: 'd2', device_name: 'B', status: 'failed', error_message: 'oh no', started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })

    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    await act(async () => {
      fireEvent.change(input, { target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] } })
    })
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    await waitFor(() => expect(toastWarning).toHaveBeenCalled(), { timeout: 3000 })
    const warnArg = toastWarning.mock.calls[0][0] as string
    expect(warnArg).toContain('Enrolamiento parcial')
    expect(warnArg).toContain('1/2')
  })

  it('terminal poll all-failed: fires toast.error with all-failed copy', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-3',
        device_pushes: [
          { device_id: 'd1', device_name: 'A', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.startsWith('/enrollments/enr-3')) {
        return Promise.resolve({
          data: {
            id: 'enr-3', employee_id: EMPLOYEE.id, status: 'failed',
            started_at: '2026-04-28T12:00:00Z', completed_at: '2026-04-28T12:01:00Z',
            device_pushes: [
              { device_id: 'd1', device_name: 'A', status: 'failed', error_message: 'down', started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })

    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />, { wrapper: makeWrapper() })
    })
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    await act(async () => {
      fireEvent.change(input, { target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] } })
    })
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    await waitFor(() => expect(toastError).toHaveBeenCalled(), { timeout: 3000 })
    const errArgs = toastError.mock.calls.map((c) => c[0]) as string[]
    expect(errArgs.some((m) => typeof m === 'string' && m.includes('falló en todos los dispositivos'))).toBe(true)
  })

  it('terminal failed enrollment with zero device pushes fires error and never success', async () => {
    vi.mocked(api.get).mockResolvedValueOnce({
      data: {
        id: 'enr-zero-failed',
        employee_id: EMPLOYEE.id,
        employee_name: EMPLOYEE.name,
        employee_code: EMPLOYEE.employee_code,
        status: 'failed',
        started_at: '2026-04-28T12:00:00Z',
        completed_at: '2026-04-28T12:01:00Z',
        version: 1,
        device_pushes: [],
      },
    })

    render(
      <EnrollmentModal
        open
        employee={null}
        initialEnrollmentId="enr-zero-failed"
        onClose={() => {}}
      />,
      { wrapper: makeWrapper() },
    )

    await waitFor(() => {
      expect(toastError.mock.calls.length + toastSuccess.mock.calls.length).toBeGreaterThan(0)
    })
    expect(toastSuccess).not.toHaveBeenCalled()
    expect(toastError).toHaveBeenCalledWith(
      'Enrolamiento falló en todos los dispositivos. Reintenta desde el panel.',
      { id: 'enrollment-enr-zero-failed' },
    )
  })

  it('Cerrar mid-flight (non-terminal status): fires sticky toast with Infinity duration', async () => {
    vi.mocked(api.post).mockResolvedValueOnce({
      data: {
        enrollment_id: 'enr-4',
        device_pushes: [
          { device_id: 'd1', device_name: 'A', status: 'in_progress', error_message: null, started_at: null, completed_at: null },
          { device_id: 'd2', device_name: 'B', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (url.startsWith('/enrollments/enr-4')) {
        return Promise.resolve({
          data: {
            id: 'enr-4', employee_id: EMPLOYEE.id, status: 'in_progress',
            started_at: '2026-04-28T12:00:00Z', completed_at: null,
            device_pushes: [
              { device_id: 'd1', device_name: 'A', status: 'in_progress', error_message: null, started_at: null, completed_at: null },
              { device_id: 'd2', device_name: 'B', status: 'pending', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })

    const onClose = vi.fn()
    await act(async () => {
      render(<EnrollmentModal open={true} employee={EMPLOYEE} onClose={onClose} />, { wrapper: makeWrapper() })
    })
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    await act(async () => {
      fireEvent.change(input, { target: { files: [new File([new Uint8Array(100)], 'p.jpg', { type: 'image/jpeg' })] } })
    })
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    // Wait for the polling query to kick in
    await waitFor(() =>
      expect(screen.getByText(/Enrolamiento enviado/)).toBeTruthy(),
      { timeout: 3000 }
    )
    // Click Cerrar — sticky toast should fire (duration: Infinity)
    const closeBtn = screen.getByRole('button', { name: /^Cerrar$/ })
    await act(async () => { fireEvent.click(closeBtn) })

    await waitFor(() => expect(toastBase).toHaveBeenCalled())
    const [msg, opts] = toastBase.mock.calls[0]
    expect(msg as string).toContain('Enrolamiento en curso')
    expect((opts as { duration?: number }).duration).toBe(Infinity)
    expect(onClose).toHaveBeenCalled()
  })
})
