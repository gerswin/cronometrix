import { describe, expect, it } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { PresenceRow } from '@/types/api'
import { PresenceTable } from '../presence-table'

const rows: PresenceRow[] = [
  {
    employee_id: 'e1',
    employee_name: 'Ana Pérez',
    department_name: 'Producción',
    status: 'inside',
    entry_at: '2026-08-05T12:02:00+00:00',
    exit_at: null,
    expected_min: 480,
    worked_min: 210,
    deficit_min: 270,
  },
  {
    employee_id: 'e2',
    employee_name: 'Luis García',
    department_name: 'Producción',
    status: 'left',
    entry_at: '2026-08-05T12:00:00+00:00',
    exit_at: '2026-08-05T21:00:00+00:00',
    expected_min: 480,
    worked_min: 480,
    deficit_min: 0,
  },
]

describe('PresenceTable', () => {
  it('shows only people still inside by default', () => {
    render(<PresenceTable rows={rows} />)
    expect(screen.getByText('Ana Pérez')).toBeInTheDocument()
    expect(screen.queryByText('Luis García')).not.toBeInTheDocument()
  })

  it('switches to everyone who attended today', () => {
    render(<PresenceTable rows={rows} />)
    fireEvent.click(screen.getByTestId('presence-tab-attended'))
    expect(screen.getByText('Ana Pérez')).toBeInTheDocument()
    expect(screen.getByText('Luis García')).toBeInTheDocument()
  })

  it('renders an empty state when nobody is inside', () => {
    render(<PresenceTable rows={[rows[1]]} />)
    expect(screen.getByTestId('presence-empty')).toBeInTheDocument()
  })
})
