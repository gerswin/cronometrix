/**
 * Branch-coverage extension for PeriodPicker. Targets:
 *  - deriveDates monthly branch
 *  - default refDate (no refDate prop) → uses new Date()
 *  - handlePeriodTypeChange custom branch
 *  - halfValue branch when period_type === 'biweekly_second'
 */
import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { PeriodPicker, deriveDates } from '../period-picker'
import type { ReportFilters } from '@/types/api'

const baseValue: ReportFilters = {
  period_type: 'weekly',
  from_date: '2026-04-20',
  to_date: '2026-04-26',
  include_inactive: false,
}

describe('PeriodPicker — extra branches', () => {
  it('deriveDates monthly returns first..last day of the reference month', () => {
    const { from, to } = deriveDates('monthly', new Date(2026, 3, 15))
    expect(from).toBe('2026-04-01')
    expect(to).toBe('2026-04-30')
  })

  it('deriveDates custom defaults to current month range when no override', () => {
    const { from, to } = deriveDates('custom', new Date(2026, 3, 15))
    expect(from).toBe('2026-04-01')
    expect(to).toBe('2026-04-30')
  })

  it('deriveDates biweekly_second yields day 16..end of month', () => {
    const { from, to } = deriveDates('biweekly_second', new Date(2026, 3, 5))
    expect(from).toBe('2026-04-16')
    expect(to).toBe('2026-04-30')
  })

  it('renders without refDate prop (uses current Date as default)', () => {
    const onChange = vi.fn()
    render(<PeriodPicker value={baseValue} onChange={onChange} />)
    // The select should be rendered with the current period_type value
    expect(screen.getByLabelText('Tipo de período')).toBeTruthy()
  })

  it('switching to custom period_type only sets period_type (no date overwrite)', () => {
    const onChange = vi.fn()
    const refDate = new Date(2026, 3, 15)
    render(<PeriodPicker value={baseValue} onChange={onChange} refDate={refDate} />)
    fireEvent.change(screen.getByLabelText('Tipo de período'), { target: { value: 'custom' } })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'custom',
        from_date: baseValue.from_date,
        to_date: baseValue.to_date,
      })
    )
  })

  it('halfValue=2 when value.period_type=biweekly_second', () => {
    const onChange = vi.fn()
    render(
      <PeriodPicker
        value={{ ...baseValue, period_type: 'biweekly_second' }}
        onChange={onChange}
        refDate={new Date(2026, 3, 15)}
      />
    )
    const halfSel = screen.getByLabelText('Quincena') as HTMLSelectElement
    expect(halfSel.value).toBe('2')
  })

  it('selecting biweekly via top-level select fires handleHalfChange("1")', () => {
    const onChange = vi.fn()
    render(<PeriodPicker value={baseValue} onChange={onChange} refDate={new Date(2026, 3, 15)} />)
    fireEvent.change(screen.getByLabelText('Tipo de período'), { target: { value: 'biweekly' } })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'biweekly_first',
        from_date: '2026-04-01',
        to_date: '2026-04-15',
      })
    )
  })
})
