import { describe, expect, it } from 'vitest'
import { resolveInternalApiBase, resolvePublicApiBase } from '@/lib/api-base'

describe('API base resolution', () => {
  it('uses localhost only when NEXT_PUBLIC_API_URL is absent', () => {
    expect(resolvePublicApiBase(undefined)).toBe('http://localhost:3001')
  })

  it('preserves an explicitly empty public base for same-origin requests', () => {
    expect(resolvePublicApiBase('')).toBe('')
  })

  it('prefers the private Docker DNS address in server-side proxy code', () => {
    expect(resolveInternalApiBase('http://api:3001', '')).toBe('http://api:3001')
  })

  it('falls back to an explicit public URL outside Docker', () => {
    expect(resolveInternalApiBase(undefined, 'http://127.0.0.1:4001')).toBe(
      'http://127.0.0.1:4001',
    )
  })

  it('uses localhost when neither usable value is configured', () => {
    expect(resolveInternalApiBase(undefined, '')).toBe('http://localhost:3001')
  })
})
