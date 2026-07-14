import { describe, expect, it } from 'vitest'
import { dailyRecordKey } from '../daily-record-key'

describe('dailyRecordKey', () => {
  it('combines employee and anchor date without collapsing separate days', () => {
    expect(dailyRecordKey({ employee_id: 'emp-ana', anchor_date: '2026-04-15' })).toBe(
      'emp-ana:2026-04-15',
    )
    expect(dailyRecordKey({ employee_id: 'emp-ana', anchor_date: '2026-04-16' })).not.toBe(
      'emp-ana:2026-04-15',
    )
  })
})
