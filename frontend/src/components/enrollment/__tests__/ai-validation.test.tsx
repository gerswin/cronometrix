import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ValidationPanel } from '../validation-panel'

describe('AI Validation Panel', () => {
  it('shows the loading copy while the current captured photo is analyzed', () => {
    render(<ValidationPanel analysis={null} analyzing />)
    expect(screen.getByText('Analizando foto…')).toBeTruthy()
  })

  it('shows all three checks from the supplied immutable analysis', () => {
    render(
      <ValidationPanel
        analyzing={false}
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
    expect(screen.getByText('Rostro Detectado')).toBeTruthy()
    expect(screen.getByText('Buena Iluminación')).toBeTruthy()
    expect(screen.getByText('Resolución Óptima')).toBeTruthy()
    expect(screen.getAllByText('OK')).toHaveLength(3)
  })
})
