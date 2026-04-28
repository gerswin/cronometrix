import { describe, it } from 'vitest'

describe('UploadCaptureTab (Wave 0 stubs)', () => {
  it.todo('rejects non-JPG file with Spanish error banner "El archivo debe ser JPG y pesar menos de 2 MB."')
  it.todo('rejects JPEG >2MB with Spanish error banner')
  it.todo('accepts 100KB JPEG — calls onCaptured with the File')
  it.todo('thumbnail preview shown after valid file selection')
  it.todo('"Cambiar archivo" link resets state and allows re-selection')
})
