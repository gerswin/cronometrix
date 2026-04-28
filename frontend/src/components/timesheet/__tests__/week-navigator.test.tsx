import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { WeekNavigator } from '../week-navigator'

describe('WeekNavigator', () => {
  it('renders the current week range with Monday start (LOTTT)', () => {
    // April 23 2026 (Thursday) → week is 2026-04-20 (Mon) – 2026-04-26 (Sun)
    const thursday = new Date(2026, 3, 23, 12, 0, 0)
    render(<WeekNavigator currentDate={thursday} onChange={() => {}} />)
    // The label format is `dd MMM – dd MMM yyyy`
    expect(screen.getByText(/20 Apr – 26 Apr 2026/)).toBeTruthy()
  })

  it('previous-week button (← / aria-label "Semana anterior") subtracts 7 days', () => {
    const onChange = vi.fn()
    const baseline = new Date(2026, 3, 23, 12, 0, 0) // April 23 2026
    render(<WeekNavigator currentDate={baseline} onChange={onChange} />)
    const prev = screen.getByRole('button', { name: /Semana anterior/i })
    fireEvent.click(prev)
    expect(onChange).toHaveBeenCalledTimes(1)
    const arg = onChange.mock.calls[0][0] as Date
    // 7 days before April 23 2026 is April 16 2026
    expect(arg.getFullYear()).toBe(2026)
    expect(arg.getMonth()).toBe(3) // April (0-indexed)
    expect(arg.getDate()).toBe(16)
  })

  it('next-week button (→ / aria-label "Semana siguiente") adds 7 days', () => {
    const onChange = vi.fn()
    const baseline = new Date(2026, 3, 23, 12, 0, 0) // April 23 2026
    render(<WeekNavigator currentDate={baseline} onChange={onChange} />)
    const next = screen.getByRole('button', { name: /Semana siguiente/i })
    fireEvent.click(next)
    expect(onChange).toHaveBeenCalledTimes(1)
    const arg = onChange.mock.calls[0][0] as Date
    // 7 days after April 23 2026 is April 30 2026
    expect(arg.getFullYear()).toBe(2026)
    expect(arg.getMonth()).toBe(3) // April
    expect(arg.getDate()).toBe(30)
  })
})
