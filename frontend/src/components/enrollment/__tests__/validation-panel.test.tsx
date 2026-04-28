import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, act, waitFor } from '@testing-library/react'
import React from 'react'
import { ValidationPanel } from '../validation-panel'

const { loadFaceApiMock, analyzeFrameMock } = vi.hoisted(() => ({
  loadFaceApiMock: vi.fn(),
  analyzeFrameMock: vi.fn(),
}))
vi.mock('@/lib/face-detection', () => ({
  loadFaceApi: (...a: unknown[]) => loadFaceApiMock(...a),
  analyzeFrame: (...a: unknown[]) => analyzeFrameMock(...a),
}))

function makeVideoRef() {
  const video = document.createElement('video')
  // simulate readyState >= 2 (HAVE_CURRENT_DATA)
  Object.defineProperty(video, 'readyState', { value: 4, configurable: true })
  return { current: video } as React.RefObject<HTMLVideoElement | null>
}

describe('ValidationPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    loadFaceApiMock.mockResolvedValue({})
  })

  it('renders the section header in Spanish', () => {
    loadFaceApiMock.mockReturnValueOnce(new Promise(() => {})) // never resolves -> stays in loading
    render(
      <ValidationPanel videoRef={makeVideoRef()} onValidationChange={() => {}} active={true} />
    )
    expect(screen.getByText(/Validación de IA/)).toBeTruthy()
  })

  it('loading state: renders the Cargando modelo de IA copy', () => {
    loadFaceApiMock.mockReturnValueOnce(new Promise(() => {}))
    render(
      <ValidationPanel videoRef={makeVideoRef()} onValidationChange={() => {}} active={true} />
    )
    expect(screen.getByText(/Cargando modelo de IA/)).toBeTruthy()
  })

  it('inactive (post-load) state shows the prompt copy', async () => {
    let resolved = false
    loadFaceApiMock.mockReturnValueOnce(
      new Promise((res) => { resolved = true; res({}) })
    )
    await act(async () => {
      render(
        <ValidationPanel videoRef={makeVideoRef()} onValidationChange={() => {}} active={false} />
      )
    })
    expect(resolved).toBe(true)
    await waitFor(() => {
      expect(screen.getByText(/Inicia la captura para evaluar/)).toBeTruthy()
    })
  })

  it('all-pass branch: each row gets the OK pill and onValidationChange is called with true', async () => {
    analyzeFrameMock.mockResolvedValue({
      faceDetected: true,
      luminanceOk: true,
      sizeOk: true,
      luminance: 120,
      width: 200,
      height: 200,
    })
    const onValidationChange = vi.fn()
    await act(async () => {
      render(
        <ValidationPanel videoRef={makeVideoRef()} onValidationChange={onValidationChange} active={true} />
      )
    })
    // Wait for setInterval to fire (every 500ms) and update state
    await waitFor(() => {
      expect(analyzeFrameMock).toHaveBeenCalled()
    }, { timeout: 2000 })
    await waitFor(() => {
      expect(screen.getAllByText('OK').length).toBe(3)
    }, { timeout: 2000 })
    expect(onValidationChange).toHaveBeenCalledWith(true)
  })

  it('mixed-fail branch: any single check failing renders Falla and reports false', async () => {
    analyzeFrameMock.mockResolvedValue({
      faceDetected: true,
      luminanceOk: false, // dark scene
      sizeOk: true,
      luminance: 30,
      width: 200,
      height: 200,
    })
    const onValidationChange = vi.fn()
    await act(async () => {
      render(
        <ValidationPanel videoRef={makeVideoRef()} onValidationChange={onValidationChange} active={true} />
      )
    })
    await waitFor(() => expect(analyzeFrameMock).toHaveBeenCalled(), { timeout: 2000 })
    await waitFor(() => {
      expect(screen.getAllByText('Falla').length).toBeGreaterThan(0)
    }, { timeout: 2000 })
    expect(onValidationChange).toHaveBeenLastCalledWith(false)
  })

  it('analyzeFrame error path is swallowed without throwing', async () => {
    analyzeFrameMock.mockRejectedValue(new Error('frame error'))
    await act(async () => {
      render(
        <ValidationPanel videoRef={makeVideoRef()} onValidationChange={() => {}} active={true} />
      )
    })
    await waitFor(() => expect(analyzeFrameMock).toHaveBeenCalled(), { timeout: 2000 })
    // Component remains mounted; section header still rendered
    expect(screen.getByText(/Validación de IA/)).toBeTruthy()
  })

  it('loadFaceApi rejection path leaves the panel in inactive prompt (no crash)', async () => {
    loadFaceApiMock.mockReset()
    loadFaceApiMock.mockRejectedValueOnce(new Error('model load failed'))
    await act(async () => {
      render(
        <ValidationPanel videoRef={makeVideoRef()} onValidationChange={() => {}} active={false} />
      )
    })
    await waitFor(() => {
      expect(screen.getByText(/Inicia la captura para evaluar/)).toBeTruthy()
    })
  })
})
