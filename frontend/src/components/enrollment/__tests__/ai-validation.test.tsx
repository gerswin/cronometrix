import { describe, it } from 'vitest'

describe('AI Validation Panel (Wave 0 stubs)', () => {
  it.todo('lazy-loads @vladmandic/face-api on mount')
  it.todo('renders 3 validation rows: Rostro Detectado, Buena Iluminación, Resolución Óptima')
  it.todo('pass mapping: face bbox >=160x160 + luminance 80-200 + faceDetected -> onValidationChange(true)')
  it.todo('fail mapping: no face detected -> onValidationChange(false)')
  it.todo('shows skeleton + "Cargando modelo de IA…" while model loads')
  it.todo('shows "Inicia la captura para evaluar." before any frame analyzed')
})
