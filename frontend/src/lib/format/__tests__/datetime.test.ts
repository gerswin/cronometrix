/**
 * Task 5 (frontend per-file coverage floor): this file had zero dedicated
 * tests -- its only coverage came incidentally from components that render
 * formatted timestamps -- so it sat under the 70/60/70/70 per-file floor
 * enforced by scripts/enforce-frontend-file-floor.mjs. These tests exercise
 * all three exported formatters directly, including the null/undefined
 * short-circuit shared by each.
 */
import { describe, it, expect } from 'vitest'
import { fmtTime, fmtDateTime, fmtDate } from '../datetime'

// Backend stores epoch UTC; formatters render in America/Caracas (UTC-4, no
// DST), so a UTC instant of 08:15 lands on 04:15 local, same calendar day.
const ISO_UTC = '2026-04-23T08:15:00Z'

describe('fmtTime', () => {
  it('formats an ISO instant as HH:mm in America/Caracas, 24h clock', () => {
    expect(fmtTime(ISO_UTC)).toBe('04:15')
  })

  it('returns an em-dash for null', () => {
    expect(fmtTime(null)).toBe('—')
  })

  it('returns an em-dash for undefined', () => {
    expect(fmtTime(undefined)).toBe('—')
  })

  it('returns an em-dash for an empty string', () => {
    expect(fmtTime('')).toBe('—')
  })
})

describe('fmtDateTime', () => {
  it('formats an ISO instant as dd/mm/yyyy, HH:mm in America/Caracas', () => {
    expect(fmtDateTime(ISO_UTC)).toBe('23/04/2026, 04:15')
  })

  it('returns an em-dash for null', () => {
    expect(fmtDateTime(null)).toBe('—')
  })

  it('returns an em-dash for undefined', () => {
    expect(fmtDateTime(undefined)).toBe('—')
  })
})

describe('fmtDate', () => {
  it('formats an ISO instant as dd/mm/yyyy in America/Caracas', () => {
    expect(fmtDate(ISO_UTC)).toBe('23/04/2026')
  })

  it('returns an em-dash for null', () => {
    expect(fmtDate(null)).toBe('—')
  })

  it('returns an em-dash for undefined', () => {
    expect(fmtDate(undefined)).toBe('—')
  })

  it('handles a date-only ISO string, which JS parses as UTC midnight (rolls back a day in UTC-4)', () => {
    expect(fmtDate('2026-04-23')).toBe('22/04/2026')
  })
})
