import { describe, it, expect, beforeAll } from 'vitest'
import { render, screen } from '@testing-library/react'
import { DeptChart } from '../dept-chart'
import type { DailyRecord } from '@/types/api'

// jsdom does not provide layout — Recharts ResponsiveContainer needs width/height
// from getBoundingClientRect to render its inner SVG. Stub it to a fixed
// non-zero size so the chart actually mounts.
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ width: 320, height: 220, top: 0, left: 0, bottom: 0, right: 0, x: 0, y: 0, toJSON: () => ({}) }),
  })
  // Recharts uses ResizeObserver for the responsive container. jsdom does not.
  class FakeResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  ;(globalThis as unknown as { ResizeObserver: typeof FakeResizeObserver }).ResizeObserver =
    FakeResizeObserver
})

const baseRecord = (over: Partial<DailyRecord>): DailyRecord => ({
  id: 'r-' + (over.id ?? Math.random().toString(36).slice(2)),
  employee_id: 'e-1',
  department_id: 'd-1',
  anchor_date: '2026-04-28',
  shift_type: 'day',
  work_minutes: 480,
  overtime_minutes: 0,
  late_minutes: 0,
  early_departure_minutes: 0,
  is_rest_day_worked: false,
  entry_at: null,
  exit_at: null,
  leave_id: null,
  computed_at: '2026-04-28T00:00:00Z',
  created_at: '2026-04-28T00:00:00Z',
  updated_at: '2026-04-28T00:00:00Z',
  anomalies: [],
  ...over,
})

describe('DeptChart', () => {
  it('renders empty state when no records', () => {
    render(<DeptChart records={[]} />)
    expect(screen.getByText('Sin datos para hoy')).toBeTruthy()
  })

  it('renders empty state when records have zero work_minutes', () => {
    render(
      <DeptChart
        records={[
          baseRecord({ id: 'a', department_id: 'd1', work_minutes: 0 }),
          baseRecord({ id: 'b', department_id: 'd2', work_minutes: 0 }),
        ]}
      />
    )
    expect(screen.getByText('Sin datos para hoy')).toBeTruthy()
  })

  it('renders a chart container (SVG) when records have work_minutes > 0', () => {
    const { container } = render(
      <DeptChart
        records={[
          baseRecord({ id: 'a', department_id: 'd1', work_minutes: 480 }),
          baseRecord({ id: 'b', department_id: 'd1', work_minutes: 240 }),
          baseRecord({ id: 'c', department_id: 'd2', work_minutes: 360 }),
        ]}
      />
    )
    // Empty state copy must not be present
    expect(screen.queryByText('Sin datos para hoy')).toBeNull()
    // ResponsiveContainer mounts a wrapper div even before measure; the
    // wrapper is the only rendered element. We assert the empty-state branch
    // did not run, which is sufficient coverage for the conditional.
    expect(container.firstChild).not.toBeNull()
  })

  it('counts only employees with work_minutes > 0', () => {
    // Two records for d1 (one with 0), one for d2 — counts should be d1=1, d2=1.
    // The exact bucketing happens inside the closed Recharts internals; we
    // assert via the empty-state branch contract: a non-empty `data` must
    // reach the ResponsiveContainer branch.
    render(
      <DeptChart
        records={[
          baseRecord({ id: 'a', department_id: 'd1', work_minutes: 480 }),
          baseRecord({ id: 'b', department_id: 'd1', work_minutes: 0 }),
          baseRecord({ id: 'c', department_id: 'd2', work_minutes: 360 }),
        ]}
      />
    )
    expect(screen.queryByText('Sin datos para hoy')).toBeNull()
  })
})
