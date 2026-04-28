import { describe, it } from 'vitest'

describe('KioskCaptureTab (Wave 0 stubs)', () => {
  it.todo('initial state: Select device + Iniciar Captura visible')
  it.todo('Iniciar Captura mutation fires POST /enrollments/capture-from-device')
  it.todo('transitions to waiting state with 30s countdown visible')
  it.todo('polls GET /captures/:id; when status==captured + photo_b64 present: atob() -> Blob -> URL.createObjectURL preview')
  it.todo('Aceptar button calls onCaptured with Blob of type image/jpeg')
  it.todo('timeout response: amber banner "No se detectó captura." + Reintentar button shown')
})
