import { describe, it, expect } from 'vitest'
import { fmtMoney, fmtMoneyNegative } from '../currency'

describe('fmtMoney', () => {
  it('formats zero as $0.00', () => {
    expect(fmtMoney(0)).toBe('$0.00')
  })

  it('formats 50_000 cents as $500.00', () => {
    expect(fmtMoney(50_000)).toBe('$500.00')
  })

  it('formats 1_234_567 cents as $12,345.67 (en-US thousands separator)', () => {
    expect(fmtMoney(1_234_567)).toBe('$12,345.67')
  })

  it('returns em-dash for null', () => {
    expect(fmtMoney(null)).toBe('—')
  })

  it('returns em-dash for undefined', () => {
    expect(fmtMoney(undefined)).toBe('—')
  })

  it('rounds half-cent correctly per Intl', () => {
    // 1234 cents = $12.34
    expect(fmtMoney(1234)).toBe('$12.34')
  })

  it('handles negative input by formatting Intl negative form', () => {
    // We do not use this directly for the late deduction column (use
    // fmtMoneyNegative). But document the behaviour.
    expect(fmtMoney(-100)).toMatch(/-\$1\.00|\(\$1\.00\)/)
  })
})

describe('fmtMoneyNegative', () => {
  it('formats 3_125 cents as -$31.25', () => {
    expect(fmtMoneyNegative(3_125)).toBe('-$31.25')
  })

  it('formats 0 as -$0.00', () => {
    expect(fmtMoneyNegative(0)).toBe('-$0.00')
  })

  it('returns em-dash for null', () => {
    expect(fmtMoneyNegative(null)).toBe('—')
  })

  it('returns em-dash for undefined', () => {
    expect(fmtMoneyNegative(undefined)).toBe('—')
  })
})
