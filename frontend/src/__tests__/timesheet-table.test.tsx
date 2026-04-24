import { describe, it, expect } from 'vitest'
import { startOfWeek, endOfWeek, format } from 'date-fns'

describe('week navigation', () => {
  it('week starts on Monday', () => {
    const thursday = new Date('2026-04-23') // known Thursday
    const start = startOfWeek(thursday, { weekStartsOn: 1 })
    expect(format(start, 'EEEE')).toBe('Monday')
  })

  it('week ends on Sunday', () => {
    const thursday = new Date('2026-04-23')
    const end = endOfWeek(thursday, { weekStartsOn: 1 })
    expect(format(end, 'EEEE')).toBe('Sunday')
  })

  it('Monday is included in the week that contains it', () => {
    // Use noon local time to avoid UTC midnight / timezone shift issues
    const monday = new Date(2026, 3, 20, 12, 0, 0) // April 20 2026 at 12:00 local
    const start = startOfWeek(monday, { weekStartsOn: 1 })
    expect(format(start, 'EEEE')).toBe('Monday')
    expect(format(start, 'yyyy-MM-dd')).toBe('2026-04-20')
  })

  it('Sunday is the last day of its week', () => {
    // Use noon local time to avoid UTC midnight / timezone shift issues
    const sunday = new Date(2026, 3, 26, 12, 0, 0) // April 26 2026 at 12:00 local
    const end = endOfWeek(sunday, { weekStartsOn: 1 })
    expect(format(end, 'EEEE')).toBe('Sunday')
    expect(format(end, 'yyyy-MM-dd')).toBe('2026-04-26')
  })
})
