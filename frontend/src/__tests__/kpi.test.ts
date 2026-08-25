import { describe, it, expect } from 'vitest'
import { aggregateKPIs } from '../lib/kpi-utils'

describe('aggregateKPIs', () => {
  it('counts late arrivals (late_minutes > 0)', () => {
    const records = [
      { late_minutes: 15 },
      { late_minutes: 0 },
    ]
    expect(aggregateKPIs(records).late).toBe(1)
  })

  it('returns zero late arrivals for an empty list', () => {
    expect(aggregateKPIs([]).late).toBe(0)
  })
})
