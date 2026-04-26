import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { PeriodPicker, deriveDates } from '../period-picker'
import type { ReportFilters } from '@/types/api'

const REF = new Date('2026-04-15T12:00:00Z') // Wed 15 Apr 2026

describe('deriveDates (I-8 mirror of backend periods.rs)', () => {
  it('weekly: ISO Monday..Sunday', () => {
    const { from, to } = deriveDates('weekly', REF)
    // 15 Apr 2026 is Wednesday → Mon 13 Apr → Sun 19 Apr
    expect(from).toBe('2026-04-13')
    expect(to).toBe('2026-04-19')
  })

  it('biweekly_first: 1..15 of ref month', () => {
    const { from, to } = deriveDates('biweekly_first', REF)
    expect(from).toBe('2026-04-01')
    expect(to).toBe('2026-04-15')
  })

  it('biweekly_second: 16..end-of-month', () => {
    const { from, to } = deriveDates('biweekly_second', REF)
    expect(from).toBe('2026-04-16')
    expect(to).toBe('2026-04-30')
  })

  it('monthly: 1..end-of-month', () => {
    const { from, to } = deriveDates('monthly', REF)
    expect(from).toBe('2026-04-01')
    expect(to).toBe('2026-04-30')
  })
})

const baseFilters: ReportFilters = {
  period_type: 'monthly',
  from_date: '2026-04-01',
  to_date: '2026-04-30',
  include_inactive: false,
}

describe('<PeriodPicker>', () => {
  it('renders the period type select with Spanish labels', () => {
    render(
      <PeriodPicker value={baseFilters} onChange={() => {}} refDate={REF} />,
    )
    const sel = screen.getByLabelText('Tipo de período') as HTMLSelectElement
    const opts = Array.from(sel.options).map((o) => o.text)
    expect(opts).toEqual(['Semanal', 'Quincenal', 'Mensual', 'Personalizado'])
  })

  it('switching to Semanal calls onChange with weekly dates', () => {
    const onChange = vi.fn()
    render(
      <PeriodPicker value={baseFilters} onChange={onChange} refDate={REF} />,
    )
    fireEvent.change(screen.getByLabelText('Tipo de período'), {
      target: { value: 'weekly' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'weekly',
        from_date: '2026-04-13',
        to_date: '2026-04-19',
      }),
    )
  })

  it('switching to Quincenal defaults to 1ra quincena (biweekly_first)', () => {
    const onChange = vi.fn()
    render(
      <PeriodPicker value={baseFilters} onChange={onChange} refDate={REF} />,
    )
    fireEvent.change(screen.getByLabelText('Tipo de período'), {
      target: { value: 'biweekly' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'biweekly_first',
        from_date: '2026-04-01',
        to_date: '2026-04-15',
      }),
    )
  })

  it('switching to 2da quincena emits biweekly_second', () => {
    const onChange = vi.fn()
    render(
      <PeriodPicker
        value={{ ...baseFilters, period_type: 'biweekly_first' }}
        onChange={onChange}
        refDate={REF}
      />,
    )
    fireEvent.change(screen.getByLabelText('Quincena'), {
      target: { value: '2' },
    })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'biweekly_second',
        from_date: '2026-04-16',
        to_date: '2026-04-30',
      }),
    )
  })

  it('Personalizado mode shows date inputs and propagates changes', () => {
    const onChange = vi.fn()
    render(
      <PeriodPicker
        value={{ ...baseFilters, period_type: 'custom' }}
        onChange={onChange}
        refDate={REF}
      />,
    )
    const fromInput = screen.getByLabelText('Fecha desde') as HTMLInputElement
    fireEvent.change(fromInput, { target: { value: '2026-03-01' } })
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        period_type: 'custom',
        from_date: '2026-03-01',
      }),
    )
  })
})
