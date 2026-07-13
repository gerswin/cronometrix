import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { EnrollmentModal } from '../enrollment-modal'
import type { Employee } from '@/types/api'

// Stub URL methods
globalThis.URL.createObjectURL = vi.fn(() => 'blob:test-modal-url')
globalThis.URL.revokeObjectURL = vi.fn()

// Mock api
vi.mock('@/lib/api', () => ({
  api: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

// Mock face-detection to avoid loading model
vi.mock('@/lib/face-detection', () => ({
  loadFaceApi: vi.fn().mockResolvedValue({}),
  analyzeFrame: vi.fn().mockResolvedValue({
    faceDetected: true,
    luminanceOk: true,
    sizeOk: true,
    luminance: 120,
    width: 200,
    height: 200,
  }),
  analyzePhotoBlob: vi.fn().mockResolvedValue({
    faceDetected: true,
    luminanceOk: true,
    sizeOk: true,
    luminance: 120,
    width: 200,
    height: 200,
  }),
  isAcceptableFace: vi.fn((analysis) =>
    analysis.faceDetected && analysis.luminanceOk && analysis.sizeOk
  ),
}))

// Mock navigator.mediaDevices to avoid webcam errors
Object.defineProperty(globalThis.navigator, 'mediaDevices', {
  value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] }) },
  writable: true,
  configurable: true,
})

import { api } from '@/lib/api'
import * as faceDetection from '@/lib/face-detection'

const EMPLOYEE: Employee = {
  id: 'emp-001',
  employee_code: 'V-12345678',
  cedula: 'V-12345678',
  name: 'Ana García',
  department_id: 'dept-1',
  position: 'Analista',
  hire_date: '2023-01-01',
  status: 'active',
  version: 1,
  created_at: '2023-01-01T00:00:00Z',
  updated_at: '2023-01-01T00:00:00Z',
  base_salary_cents: 100000,
}

const OTHER_EMPLOYEE: Employee = {
  ...EMPLOYEE,
  id: 'emp-002',
  employee_code: 'V-87654321',
  cedula: 'V-87654321',
  name: 'Luis Pérez',
}

function makeWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  }
}

describe('EnrollmentModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(faceDetection.analyzePhotoBlob).mockResolvedValue({
      faceDetected: true,
      luminanceOk: true,
      sizeOk: true,
      luminance: 120,
      width: 200,
      height: 200,
    })
    // Route GETs based on URL: /enrollments/:id polling needs the Enrollment
    // shape (with device_pushes), other GETs (employee list, etc.) get the
    // paginated shape. Without this routing the polling query reads the
    // employee-list response and crashes on device_pushes.map.
    // [Rule 1 fix from 08-04C — was a pre-existing flaky test]
    vi.mocked(api.get).mockImplementation((url: string) => {
      if (typeof url === 'string' && url.startsWith('/enrollments/')) {
        return Promise.resolve({
          data: {
            id: 'enr-001',
            employee_id: 'emp-001',
            employee_name: 'Ana García',
            employee_code: 'V-12345678',
            status: 'in_progress',
            started_at: '2026-04-28T12:00:00Z',
            completed_at: null,
            version: 1,
            device_pushes: [
              { id: 'push-1', device_id: 'dev-1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })
    vi.mocked(api.post).mockResolvedValue({
      data: {
        enrollment_id: 'enr-001',
        face_id: 'face-001',
        device_pushes: [
          { id: 'push-1', device_id: 'dev-1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
        ],
      },
    })
  })

  it('opens with employee name in DialogTitle', async () => {
    await act(async () => {
      render(
        <EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />,
        { wrapper: makeWrapper() }
      )
    })
    expect(screen.getByRole('dialog', { name: 'Enrolamiento Facial' })).toBeTruthy()
    expect(screen.getByText(/Ana García/)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Cerrar enrolamiento' })).toBeTruthy()
    expect(screen.getByRole('button', { name: /^Cerrar$/ })).toBeTruthy()
  })

  it('switches between Lector Hikvision / Webcam / Subir JPG tabs', async () => {
    await act(async () => {
      render(
        <EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />,
        { wrapper: makeWrapper() }
      )
    })
    // Click Webcam tab
    const webcamTab = screen.getByText('Webcam')
    await act(async () => { fireEvent.click(webcamTab) })
    // Click Subir JPG tab
    const uploadTab = screen.getByText('Subir JPG')
    await act(async () => { fireEvent.click(uploadTab) })
    // Drop zone should be visible
    expect(screen.getByText(/Haz clic para seleccionar/)).toBeTruthy()
  })

  it('primary CTA aria-disabled when no photo captured', async () => {
    await act(async () => {
      render(
        <EnrollmentModal open={true} employee={EMPLOYEE} onClose={() => {}} />,
        { wrapper: makeWrapper() }
      )
    })
    const enrollBtn = screen.getByRole('button', { name: /Enrolar/i })
    expect(enrollBtn.getAttribute('aria-disabled')).toBe('true')
  })

  it('closes modal and onClose called when Cerrar clicked', async () => {
    const onClose = vi.fn()
    await act(async () => {
      render(
        <EnrollmentModal open={true} employee={EMPLOYEE} onClose={onClose} />,
        { wrapper: makeWrapper() }
      )
    })
    const closeBtn = screen.getByRole('button', { name: /^Cerrar$/ })
    await act(async () => { fireEvent.click(closeBtn) })
    expect(onClose).toHaveBeenCalled()
  })

  it('clears an unsubmitted capture when closing and reopening the same employee', async () => {
    const onClose = vi.fn()
    const rendered = render(
      <EnrollmentModal open employee={EMPLOYEE} onClose={onClose} />,
      { wrapper: makeWrapper() },
    )
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'photo.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())

    fireEvent.click(screen.getByRole('button', { name: /^Cerrar$/ }))
    rendered.rerender(<EnrollmentModal open={false} employee={EMPLOYEE} onClose={onClose} />)
    rendered.rerender(<EnrollmentModal open employee={EMPLOYEE} onClose={onClose} />)

    expect(screen.getByText('Lector Hikvision')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
    expect(screen.queryByText('photo.jpg')).toBeNull()
  })

  it('submit mutation fires with employee_id and multipart FormData', async () => {
    // Simulate having a photo by switching to Upload tab and providing a file
    const onClose = vi.fn()
    await act(async () => {
      render(
        <EnrollmentModal open={true} employee={EMPLOYEE} onClose={onClose} />,
        { wrapper: makeWrapper() }
      )
    })

    // Switch to upload tab
    await act(async () => { fireEvent.click(screen.getByText('Subir JPG')) })

    // Simulate file upload
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = new File([new Uint8Array(100)], 'photo.jpg', { type: 'image/jpeg' })
    await act(async () => { fireEvent.change(input, { target: { files: [goodJpeg] } }) })

    // Now Enrolar is enabled only after the shared still-photo analysis resolves.
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      expect(btn).not.toBeDisabled()
    })

    const enrollBtn = screen.getByRole('button', { name: /Enrolar/i })
    await act(async () => { fireEvent.click(enrollBtn) })

    await waitFor(() => expect(api.post).toHaveBeenCalledWith('/enrollments', expect.any(FormData)))
    const body = vi.mocked(api.post).mock.calls.find(([url]) => url === '/enrollments')?.[1] as FormData
    expect(JSON.parse(String(body.get('face_quality_score')))).toMatchObject({
      faceDetected: true,
      luminanceOk: true,
      sizeOk: true,
    })
  })

  it('ignores a stale photo analysis after switching employees', async () => {
    let resolveAnalysis!: (analysis: Awaited<ReturnType<typeof faceDetection.analyzePhotoBlob>>) => void
    vi.mocked(faceDetection.analyzePhotoBlob).mockReturnValueOnce(
      new Promise((resolve) => { resolveAnalysis = resolve }),
    )
    let rerender!: (ui: React.ReactNode) => void
    await act(async () => {
      const rendered = render(
        <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
        { wrapper: makeWrapper() },
      )
      rerender = rendered.rerender
    })
    fireEvent.click(screen.getByText('Subir JPG'))
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File([new Uint8Array(100)], 'old.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(faceDetection.analyzePhotoBlob).toHaveBeenCalled())

    rerender(<EnrollmentModal open employee={OTHER_EMPLOYEE} onClose={() => {}} />)
    await act(async () => {
      resolveAnalysis({
        faceDetected: true,
        luminanceOk: true,
        sizeOk: true,
        luminance: 120,
        width: 200,
        height: 200,
      })
    })

    expect(screen.getByText(/Luis Pérez/)).toBeTruthy()
    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
    expect(api.post).not.toHaveBeenCalled()
  })

  it('ignores a stale enrollment submission after switching employees', async () => {
    const invalidate = vi.spyOn(QueryClient.prototype, 'invalidateQueries')
    let resolveSubmit!: (value: { data: unknown }) => void
    vi.mocked(api.post).mockReturnValueOnce(
      new Promise((resolve) => { resolveSubmit = resolve }),
    )
    let rerender!: (ui: React.ReactNode) => void
    await act(async () => {
      const rendered = render(
        <EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />,
        { wrapper: makeWrapper() },
      )
      rerender = rendered.rerender
    })
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'old.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: /Enrolar/i })).not.toBeDisabled())
    fireEvent.click(screen.getByRole('button', { name: /Enrolar/i }))
    await waitFor(() => expect(api.post).toHaveBeenCalled())

    rerender(<EnrollmentModal open employee={OTHER_EMPLOYEE} onClose={() => {}} />)
    await act(async () => {
      resolveSubmit({
        data: { enrollment_id: 'enr-old', face_id: 'face-old', device_pushes: [] },
      })
    })

    expect(screen.getByText(/Luis Pérez/)).toBeTruthy()
    expect(screen.queryByText(/Enrolamiento enviado/)).toBeNull()
    expect(screen.getByText('Lector Hikvision')).toBeTruthy()
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['enrollments', 'in_progress'] })
    invalidate.mockRestore()
  })

  it('ignores an upload analysis that resolves after changing capture tabs', async () => {
    let resolveAnalysis!: (analysis: Awaited<ReturnType<typeof faceDetection.analyzePhotoBlob>>) => void
    vi.mocked(faceDetection.analyzePhotoBlob).mockReturnValueOnce(
      new Promise((resolve) => { resolveAnalysis = resolve }),
    )
    await act(async () => {
      render(<EnrollmentModal open employee={EMPLOYEE} onClose={() => {}} />, {
        wrapper: makeWrapper(),
      })
    })
    fireEvent.click(screen.getByText('Subir JPG'))
    fireEvent.change(document.querySelector('input[type="file"]') as HTMLInputElement, {
      target: { files: [new File([new Uint8Array(100)], 'old.jpg', { type: 'image/jpeg' })] },
    })
    await waitFor(() => expect(faceDetection.analyzePhotoBlob).toHaveBeenCalled())
    fireEvent.click(screen.getByText('Webcam'))

    await act(async () => {
      resolveAnalysis({
        faceDetected: true,
        luminanceOk: true,
        sizeOk: true,
        luminance: 120,
        width: 200,
        height: 200,
      })
    })

    expect(screen.getByRole('button', { name: /Enrolar/i })).toBeDisabled()
  })

  it('resumes a server-backed enrollment without an employee or capture tabs', async () => {
    vi.mocked(api.get).mockResolvedValueOnce({
      data: {
        id: 'enr/resume',
        employee_id: 'emp-server',
        employee_name: 'Nombre del servidor',
        employee_code: 'EMP-77',
        status: 'in_progress',
        started_at: '2026-04-28T12:00:00Z',
        completed_at: null,
        version: 2,
        device_pushes: [],
      },
    })

    await act(async () => {
      render(
        <EnrollmentModal
          open
          employee={null}
          initialEnrollmentId="enr/resume"
          onClose={() => {}}
        />,
        { wrapper: makeWrapper() },
      )
    })

    expect(await screen.findByText(/Nombre del servidor/)).toBeTruthy()
    expect(api.get).toHaveBeenCalledWith('/enrollments/enr%2Fresume')
    expect(screen.queryByText('Lector Hikvision')).toBeNull()
  })
})
