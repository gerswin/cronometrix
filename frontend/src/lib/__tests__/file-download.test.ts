import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { downloadBlob, openBlobInNewTab } from '../file-download'

describe('file download helpers', () => {
  const createObjectURL = vi.fn(() => 'blob:file')
  const revokeObjectURL = vi.fn()
  let click: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    vi.useFakeTimers()
    createObjectURL.mockClear()
    revokeObjectURL.mockClear()
    click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL })
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL })
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('opens a blob in a protected new tab and revokes its URL after one minute', () => {
    const open = vi.spyOn(window, 'open').mockReturnValue({} as Window)
    const blob = new Blob(['photo'], { type: 'image/jpeg' })

    openBlobInNewTab(blob)

    expect(createObjectURL).toHaveBeenCalledWith(blob)
    expect(open).toHaveBeenCalledWith('blob:file', '_blank', 'noopener,noreferrer')
    expect(click).not.toHaveBeenCalled()
    vi.advanceTimersByTime(59_999)
    expect(revokeObjectURL).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:file')
  })

  it('falls back to a secure anchor when the popup is blocked', () => {
    vi.spyOn(window, 'open').mockReturnValue(null)

    openBlobInNewTab(new Blob(['evidence']))

    const anchor = click.mock.contexts[0] as HTMLAnchorElement
    expect(anchor.href).toBe('blob:file')
    expect(anchor.target).toBe('_blank')
    expect(anchor.rel).toBe('noopener noreferrer')
  })

  it('forces a download with the supplied filename and later revokes the URL', () => {
    const blob = new Blob(['report'])

    downloadBlob(blob, 'prenomina.xlsx')

    const anchor = click.mock.contexts[0] as HTMLAnchorElement
    expect(anchor.href).toBe('blob:file')
    expect(anchor.download).toBe('prenomina.xlsx')
    vi.runAllTimers()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:file')
  })
})
