import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import type { Department } from '@/types/api'
import { ToleranceSimulator } from '../tolerance-simulator'

const department: Department = {
  id: 'd1',
  name: 'Operaciones',
  base_salary_cents: 0,
  shift_start_time: '08:00',
  shift_end_time: '17:00',
  lunch_mode: 'fixed',
  lunch_duration_min: 60,
  status: 'active',
  deleted_at: null,
  version: 1,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

describe('ToleranceSimulator', () => {
  it('explains that a department is required', () => {
    render(<ToleranceSimulator lateMin={20} earlyMin={10} bonusMin={5} department={null} />)
    expect(screen.getByText(/Configure al menos un departamento/)).toBeVisible()
  })

  it('compares the ideal schedule with applied late and early tolerances', () => {
    render(<ToleranceSimulator lateMin={20} earlyMin={15} bonusMin={5} department={department} />)

    expect(screen.getAllByText('08:00')).toHaveLength(1)
    expect(screen.getByText('08:15')).toBeVisible()
    expect(screen.getByText('16:50')).toBeVisible()
    expect(screen.getByText('+15m')).toBeVisible()
    expect(screen.getByText('+10m')).toBeVisible()
    expect(screen.getByText('Total: 8h efectivos')).toBeVisible()
    expect(screen.getByText('Total: 7h 35m efectivos')).toBeVisible()
  })

  it('clamps negative totals and handles shifts without lunch or applied tolerances', () => {
    const shortShift = {
      ...department,
      shift_start_time: '00:00',
      shift_end_time: '00:10',
      lunch_duration_min: null,
    }
    render(<ToleranceSimulator lateMin={60} earlyMin={60} bonusMin={0} department={shortShift} />)

    expect(screen.getByText('Total: 10m efectivos')).toBeVisible()
    expect(screen.getByText('Total: 0m efectivos')).toBeVisible()
    expect(screen.getAllByText('+60m')).toHaveLength(2)
  })

  it('does not show tolerance markers when the bonus absorbs both allowances', () => {
    render(<ToleranceSimulator lateMin={5} earlyMin={5} bonusMin={10} department={department} />)
    expect(screen.queryByText(/^\+/)).toBeNull()
    expect(screen.getAllByText('Total: 8h efectivos')).toHaveLength(2)
  })
})
