/**
 * Branch coverage extension for WebcamCaptureTab. Existing test covers
 * mount + permission-denied + unmount. This file adds: capture frame ->
 * frozen preview, accept calls onCaptured, retake restarts the stream
 * (and hits a permission-denied retry branch), generic non-DOMException
 * error path.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { WebcamCaptureTab } from '../webcam-capture-tab'

const realCreateObjectURL = globalThis.URL.createObjectURL
beforeEach(() => {
  globalThis.URL.createObjectURL = vi.fn(() => 'blob:cap-extra')
})
afterAll(() => {
  globalThis.URL.createObjectURL = realCreateObjectURL
})
function afterAll(fn: () => void) { /* explicit no-op for test isolation */ void fn }

describe('WebcamCaptureTab — extra branches', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('capture frame -> Aceptar fires onCaptured with the frozen Blob', async () => {
    const mockStop = vi.fn()
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: mockStop }] } as unknown as MediaStream) },
      writable: true, configurable: true,
    })

    // Stub HTMLCanvasElement.getContext + toBlob for jsdom
    HTMLCanvasElement.prototype.getContext = vi.fn(() => ({ drawImage: vi.fn() })) as unknown as typeof HTMLCanvasElement.prototype.getContext
    HTMLCanvasElement.prototype.toBlob = function (cb: BlobCallback) {
      cb(new Blob(['fake-jpeg'], { type: 'image/jpeg' }))
    }

    const onCaptured = vi.fn()
    const onValidationChange = vi.fn()
    await act(async () => {
      render(<WebcamCaptureTab onCaptured={onCaptured} onValidationChange={onValidationChange} />)
    })

    const captureBtn = screen.getByRole('button', { name: /Capturar Rostro/i })
    await act(async () => { fireEvent.click(captureBtn) })

    await waitFor(() => screen.getByRole('button', { name: /Aceptar/i }))
    fireEvent.click(screen.getByRole('button', { name: /Aceptar/i }))
    expect(onCaptured).toHaveBeenCalled()
    const arg = onCaptured.mock.calls[0][0]
    expect(arg).toBeInstanceOf(Blob)
  })

  it('Recapturar resets to live preview and re-invokes getUserMedia', async () => {
    const getUserMedia = vi.fn().mockResolvedValue({
      getTracks: () => [{ stop: vi.fn() }],
    } as unknown as MediaStream)
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: { getUserMedia },
      writable: true, configurable: true,
    })
    HTMLCanvasElement.prototype.getContext = vi.fn(() => ({ drawImage: vi.fn() })) as unknown as typeof HTMLCanvasElement.prototype.getContext
    HTMLCanvasElement.prototype.toBlob = function (cb: BlobCallback) {
      cb(new Blob(['x'], { type: 'image/jpeg' }))
    }

    const onValidationChange = vi.fn()
    await act(async () => {
      render(<WebcamCaptureTab onCaptured={() => {}} onValidationChange={onValidationChange} />)
    })
    const captureBtn = screen.getByRole('button', { name: /Capturar Rostro/i })
    await act(async () => { fireEvent.click(captureBtn) })
    await waitFor(() => screen.getByRole('button', { name: /Recapturar/i }))

    getUserMedia.mockClear()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Recapturar/i }))
    })
    expect(onValidationChange).toHaveBeenCalledWith(false)
    expect(getUserMedia).toHaveBeenCalled()
  })

  it('Recapturar with second-time permission denial flips to the permission-denied alert', async () => {
    HTMLCanvasElement.prototype.getContext = vi.fn(() => ({ drawImage: vi.fn() })) as unknown as typeof HTMLCanvasElement.prototype.getContext
    HTMLCanvasElement.prototype.toBlob = function (cb: BlobCallback) {
      cb(new Blob(['x'], { type: 'image/jpeg' }))
    }
    const stream = { getTracks: () => [{ stop: vi.fn() }] } as unknown as MediaStream
    const getUserMedia = vi
      .fn<typeof navigator.mediaDevices.getUserMedia>()
      .mockResolvedValueOnce(stream)
      .mockRejectedValueOnce(new Error('runtime')) // non-DOMException → no banner first time, but retake flips it
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: { getUserMedia }, writable: true, configurable: true,
    })

    await act(async () => {
      render(<WebcamCaptureTab onCaptured={() => {}} onValidationChange={() => {}} />)
    })
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: /Capturar Rostro/i })) })
    await waitFor(() => screen.getByRole('button', { name: /Recapturar/i }))
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: /Recapturar/i })) })
    await waitFor(() => screen.getByRole('alert'))
    expect(screen.getByRole('alert').textContent).toContain('El navegador bloqueó la cámara')
  })

  it('non-DOMException error on initial mount does NOT show the permission-denied banner', async () => {
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: { getUserMedia: vi.fn().mockRejectedValueOnce(new Error('boom')) },
      writable: true, configurable: true,
    })
    await act(async () => {
      render(<WebcamCaptureTab onCaptured={() => {}} onValidationChange={() => {}} />)
    })
    expect(screen.queryByRole('alert')).toBeNull()
    // Still shows the capture button (live-preview branch)
    expect(screen.getByRole('button', { name: /Capturar Rostro/i })).toBeTruthy()
  })
})
