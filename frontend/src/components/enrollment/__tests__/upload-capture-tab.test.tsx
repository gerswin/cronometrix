import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import React from 'react'
import { UploadCaptureTab } from '../upload-capture-tab'

// Stub URL.createObjectURL / revokeObjectURL for jsdom
globalThis.URL.createObjectURL = vi.fn(() => 'blob:test-url')
globalThis.URL.revokeObjectURL = vi.fn()

function makeFile(name: string, type: string, sizeBytes: number): File {
  const buf = new Uint8Array(sizeBytes)
  return new File([buf], name, { type })
}

describe('UploadCaptureTab', () => {
  const mockOnCaptured = vi.fn()
  const mockOnCleared = vi.fn()

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('rejects non-JPG file with Spanish error banner', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const pngFile = makeFile('photo.png', 'image/png', 100 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [pngFile] } })
    })
    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('El archivo debe ser JPG y pesar menos de 2 MB')
    expect(mockOnCaptured).not.toHaveBeenCalled()
  })

  it('rejects JPEG >2MB with Spanish error banner', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const bigJpeg = makeFile('big.jpg', 'image/jpeg', 3 * 1024 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [bigJpeg] } })
    })
    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('El archivo debe ser JPG y pesar menos de 2 MB')
    expect(mockOnCaptured).not.toHaveBeenCalled()
  })

  it('accepts 100KB JPEG and calls onCaptured', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = makeFile('photo.jpg', 'image/jpeg', 100 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [goodJpeg] } })
    })
    expect(mockOnCaptured).toHaveBeenCalledWith({
      blob: goodJpeg,
      capturedVia: 'upload',
      sourceDeviceId: null,
    })
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('thumbnail preview shown after valid file selection', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = makeFile('photo.jpg', 'image/jpeg', 100 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [goodJpeg] } })
    })
    const img = screen.getByRole('img', { name: /vista previa/i })
    expect(img).toBeTruthy()
    expect((img as HTMLImageElement).src).toContain('blob:')
  })

  it('"Cambiar archivo" link resets state', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = makeFile('photo.jpg', 'image/jpeg', 100 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [goodJpeg] } })
    })
    // Preview is shown — click "Cambiar archivo"
    const changeLink = screen.getByText('Cambiar archivo')
    await act(async () => { fireEvent.click(changeLink) })
    // Drop zone should be visible again
    expect(screen.getByText(/Haz clic para seleccionar/)).toBeTruthy()
    expect(mockOnCleared).toHaveBeenCalledOnce()
  })

  it('Quitar imagen clears the current parent candidate and revokes its preview URL', async () => {
    render(<UploadCaptureTab onCaptured={mockOnCaptured} onCleared={mockOnCleared} />)
    const input = document.querySelector('input[type="file"]') as HTMLInputElement
    const goodJpeg = makeFile('photo.jpg', 'image/jpeg', 100 * 1024)
    await act(async () => {
      fireEvent.change(input, { target: { files: [goodJpeg] } })
    })

    fireEvent.click(screen.getByRole('button', { name: 'Quitar imagen' }))

    expect(mockOnCleared).toHaveBeenCalledOnce()
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:test-url')
  })
})
