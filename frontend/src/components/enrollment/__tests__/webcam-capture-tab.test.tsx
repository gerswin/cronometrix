import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import React from 'react'
import { WebcamCaptureTab } from '../webcam-capture-tab'

describe('WebcamCaptureTab', () => {
  const mockOnCaptured = vi.fn()
  const mockOnCleared = vi.fn()

  // Mock getUserMedia
  const mockStop = vi.fn()
  const mockGetTracks = vi.fn().mockReturnValue([{ stop: mockStop }])
  const mockStream = { getTracks: mockGetTracks } as unknown as MediaStream

  beforeEach(() => {
    vi.clearAllMocks()
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: {
        getUserMedia: vi.fn().mockResolvedValue(mockStream),
      },
      writable: true,
      configurable: true,
    })
  })

  it('getUserMedia called with width:640 height:480', async () => {
    await act(async () => {
      render(
        <WebcamCaptureTab
          onCaptured={mockOnCaptured}
          onCleared={mockOnCleared}
        />
      )
    })
    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith(
      expect.objectContaining({
        video: expect.objectContaining({
          width: expect.objectContaining({ ideal: 640 }),
          height: expect.objectContaining({ ideal: 480 }),
        }),
        audio: false,
      })
    )
  })

  it('stream tracks stopped on unmount (Pitfall 6)', async () => {
    let unmount: () => void
    await act(async () => {
      const result = render(
        <WebcamCaptureTab
          onCaptured={mockOnCaptured}
          onCleared={mockOnCleared}
        />
      )
      unmount = result.unmount
    })
    act(() => { unmount() })
    expect(mockStop).toHaveBeenCalled()
  })

  it('shows permission-denied banner with Spanish copy when DOMException thrown', async () => {
    Object.defineProperty(globalThis.navigator, 'mediaDevices', {
      value: {
        getUserMedia: vi.fn().mockRejectedValue(
          new DOMException('Permission denied', 'NotAllowedError')
        ),
      },
      writable: true,
      configurable: true,
    })
    await act(async () => {
      render(
        <WebcamCaptureTab
          onCaptured={mockOnCaptured}
          onCleared={mockOnCleared}
        />
      )
    })
    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('El navegador bloqueó la cámara')
  })
})
