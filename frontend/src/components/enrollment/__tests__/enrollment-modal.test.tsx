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
}))

// Mock navigator.mediaDevices to avoid webcam errors
Object.defineProperty(globalThis.navigator, 'mediaDevices', {
  value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] }) },
  writable: true,
  configurable: true,
})

import { api } from '@/lib/api'

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
            status: 'in_progress',
            started_at: '2026-04-28T12:00:00Z',
            completed_at: null,
            device_pushes: [
              { device_id: 'dev-1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
            ],
          },
        })
      }
      return Promise.resolve({ data: { data: [] } })
    })
    vi.mocked(api.post).mockResolvedValue({
      data: {
        enrollment_id: 'enr-001',
        device_pushes: [
          { device_id: 'dev-1', device_name: 'Entrada', status: 'pending', error_message: null, started_at: null, completed_at: null },
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
    expect(screen.getByText(/Ana García/)).toBeTruthy()
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
    const closeBtn = screen.getByRole('button', { name: /Cerrar/i })
    await act(async () => { fireEvent.click(closeBtn) })
    expect(onClose).toHaveBeenCalled()
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

    // Now Enrolar button should be enabled (upload tab auto-approves validation)
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /Enrolar/i })
      return btn.getAttribute('aria-disabled') === 'false'
    })

    const enrollBtn = screen.getByRole('button', { name: /Enrolar/i })
    await act(async () => { fireEvent.click(enrollBtn) })

    await waitFor(() => expect(api.post).toHaveBeenCalledWith(
      '/enrollments',
      expect.any(FormData),
      expect.objectContaining({ headers: expect.objectContaining({ 'Content-Type': 'multipart/form-data' }) })
    ))
  })
})
