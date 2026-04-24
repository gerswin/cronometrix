import { describe, it, expect } from 'vitest'
import { evidenceFileSchema } from '../lib/validations'

describe('evidenceFileSchema', () => {
  it('rejects file > 5MB', () => {
    const bigFile = new File([new ArrayBuffer(6 * 1024 * 1024)], 'big.pdf', { type: 'application/pdf' })
    const result = evidenceFileSchema.safeParse(bigFile)
    expect(result.success).toBe(false)
  })
  it('accepts valid PDF under 5MB', () => {
    const smallFile = new File([new ArrayBuffer(1024)], 'doc.pdf', { type: 'application/pdf' })
    const result = evidenceFileSchema.safeParse(smallFile)
    expect(result.success).toBe(true)
  })
})
