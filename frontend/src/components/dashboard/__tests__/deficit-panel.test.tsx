import { describe, expect, it } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import type { PresenceRow } from '@/types/api'
import { DeficitPanel } from '../deficit-panel'
import { fmtMinutes } from '@/lib/format/minutes'

const row = (id: string, name: string, deficit: number): PresenceRow => ({
  employee_id: id,
  employee_name: name,
  department_name: 'Producción',
  status: 'left',
  entry_at: '2026-08-05T12:00:00+00:00',
  exit_at: '2026-08-05T18:00:00+00:00',
  expected_min: 480,
  worked_min: 480 - deficit,
  deficit_min: deficit,
})

describe('fmtMinutes', () => {
  it('formats minutes as hours and minutes', () => {
    expect(fmtMinutes(270)).toBe('4h 30m')
    expect(fmtMinutes(60)).toBe('1h')
    expect(fmtMinutes(45)).toBe('45m')
    expect(fmtMinutes(0)).toBe('0m')
  })
})

describe('DeficitPanel', () => {
  it('lists only people with a deficit, worst first', () => {
    render(<DeficitPanel rows={[row('e1', 'Ana', 30), row('e2', 'Luis', 270), row('e3', 'María', 0)]} />)
    const items = screen.getAllByTestId(/deficit-row-/)
    expect(items).toHaveLength(2)
    expect(within(items[0]).getByText('Luis')).toBeInTheDocument()
    expect(within(items[0]).getByText('4h 30m')).toBeInTheDocument()
    expect(within(items[1]).getByText('Ana')).toBeInTheDocument()
  })

  it('renders an empty state when everyone met their hours', () => {
    render(<DeficitPanel rows={[row('e3', 'María', 0)]} />)
    expect(screen.getByTestId('deficit-empty')).toBeInTheDocument()
  })
})
