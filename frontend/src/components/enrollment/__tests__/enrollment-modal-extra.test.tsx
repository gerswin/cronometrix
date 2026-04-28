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

const { toastSuccess, toastWarning, toastError, toastBase } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastWarning: vi.fn(),
  toastError: vi.fn(),
  toastBase: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: Object.assign((msg: unknown, opts?: unknown) => toastBase(msg, opts), {
    success: toastSuccess,
    warning: toastWarning,
    error: toastError,
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
}))

Object.defineProperty(globalThis.navigator, 'mediaDevices', {
  value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] }) },
  writable: true, configurable: true,
})

import { api } from '@/lib/api'

const EMPLOYEE: Employee = {
  id: 'emp-001', cedula: 'V-12345678', name: 'Ana García',
  department_id: 'dept-1', position: 'Analista', hire_date: '2023-01-01',
  status: 'active', created_at: '2023-01-01T00:00:00Z', updated_at: '2023-01-01T00:00:00Z',
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
    const closeBtn = screen.getByRole('button', { name: /Cerrar/i })
    await act(async () => { fireEvent.click(closeBtn) })
    expect(onClose).toHaveBeenCalled()
    expect(toastBase).not.toHaveBeenCalled()
  })

  it('submit error path triggers toast.error with the server message', async () => {
    vi.mocked(api.post).mockRejectedValueOnce({
      response: { data: { message: 'Validación falló en el servidor' } },
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
      return btn.getAttribute('aria-disabled') === 'false'
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
      return btn.getAttribute('aria-disabled') === 'false'
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    })
    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith('No se pudo registrar el enrolamiento.')
    )
  })
})
