import { describe, it, expect } from 'vitest'
import { aggregateKPIs } from '../lib/kpi-utils'

describe('aggregateKPIs', () => {
  it('counts present employees (work_minutes > 0)', () => {
    const records = [
      { work_minutes: 480, late_minutes: 0, leave_id: null },
      { work_minutes: 0, late_minutes: 0, leave_id: null },
    ]
    expect(aggregateKPIs(records).present).toBe(1)
  })
  it('counts late arrivals (late_minutes > 0)', () => {
    const records = [
      { work_minutes: 480, late_minutes: 15, leave_id: null },
      { work_minutes: 480, late_minutes: 0, leave_id: null },
    ]
    expect(aggregateKPIs(records).late).toBe(1)
  })
  it('counts absentees (work_minutes === 0, no leave)', () => {
    const records = [
      { work_minutes: 0, late_minutes: 0, leave_id: null },
      { work_minutes: 0, late_minutes: 0, leave_id: 'leave-1' },
    ]
    expect(aggregateKPIs(records).absent).toBe(1)
  })
})
