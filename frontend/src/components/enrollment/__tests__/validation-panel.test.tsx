import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ValidationPanel } from '../validation-panel'

const { loadFaceApiMock, analyzeFrameMock } = vi.hoisted(() => ({
  loadFaceApiMock: vi.fn(),
  analyzeFrameMock: vi.fn(),
}))

vi.mock('@/lib/face-detection', async (importOriginal) => ({
  ...await importOriginal<typeof import('@/lib/face-detection')>(),
  loadFaceApi: loadFaceApiMock,
  analyzeFrame: analyzeFrameMock,
}))

describe('ValidationPanel', () => {
  it('is presentation-only and never loads or analyzes a camera frame', () => {
    render(<ValidationPanel analysis={null} analyzing={false} />)
    expect(screen.getByText('Captura una foto para evaluar.')).toBeTruthy()
    expect(loadFaceApiMock).not.toHaveBeenCalled()
    expect(analyzeFrameMock).not.toHaveBeenCalled()
  })

  it('renders each failed quality dimension from the supplied analysis', () => {
    render(
      <ValidationPanel
        analyzing={false}
        analysis={{
          faceDetected: false,
          luminanceOk: false,
          sizeOk: false,
          luminance: 20,
          width: 0,
          height: 0,
        }}
      />
    )
    expect(screen.getAllByText('Falla')).toHaveLength(3)
  })

  it('prefers the analyzing state over a prior analysis', () => {
    render(
      <ValidationPanel
        analyzing
        analysis={{
          faceDetected: true,
          luminanceOk: true,
          sizeOk: true,
          luminance: 120,
          width: 200,
          height: 200,
        }}
      />
    )
    expect(screen.getByText('Analizando foto…')).toBeTruthy()
    expect(screen.queryByText('OK')).toBeNull()
  })
})
