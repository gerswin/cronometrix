import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import React, { createRef } from 'react'

// Mock @vladmandic/face-api dynamic import
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

import { ValidationPanel } from '../validation-panel'
import * as faceDetection from '@/lib/face-detection'

describe('AI Validation Panel', () => {
  const mockOnValidationChange = vi.fn()

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows skeleton + "Cargando modelo de IA…" while model loads', async () => {
    // loadFaceApi never resolves during this test
    vi.mocked(faceDetection.loadFaceApi).mockImplementation(() => new Promise(() => {}))
    const ref = createRef<HTMLVideoElement>()
    render(
      <ValidationPanel
        videoRef={ref}
        onValidationChange={mockOnValidationChange}
        active={true}
      />
    )
    expect(screen.getByText('Cargando modelo de IA…')).toBeTruthy()
  })

  it('renders 3 validation rows after model loads', async () => {
    vi.mocked(faceDetection.loadFaceApi).mockResolvedValue({} as never)
    const ref = createRef<HTMLVideoElement>()
    await act(async () => {
      render(
        <ValidationPanel
          videoRef={ref}
          onValidationChange={mockOnValidationChange}
          active={true}
        />
      )
    })
    expect(screen.getByText('Rostro Detectado')).toBeTruthy()
    expect(screen.getByText('Buena Iluminación')).toBeTruthy()
    expect(screen.getByText('Resolución Óptima')).toBeTruthy()
  })

  it('shows "Inicia la captura para evaluar." when active=false', async () => {
    vi.mocked(faceDetection.loadFaceApi).mockResolvedValue({} as never)
    const ref = createRef<HTMLVideoElement>()
    await act(async () => {
      render(
        <ValidationPanel
          videoRef={ref}
          onValidationChange={mockOnValidationChange}
          active={false}
        />
      )
    })
    expect(screen.getByText('Inicia la captura para evaluar.')).toBeTruthy()
  })

  it('lazy-loads @vladmandic/face-api on mount', async () => {
    vi.mocked(faceDetection.loadFaceApi).mockResolvedValue({} as never)
    const ref = createRef<HTMLVideoElement>()
    await act(async () => {
      render(
        <ValidationPanel
          videoRef={ref}
          onValidationChange={mockOnValidationChange}
          active={true}
        />
      )
    })
    expect(faceDetection.loadFaceApi).toHaveBeenCalledOnce()
  })
})
