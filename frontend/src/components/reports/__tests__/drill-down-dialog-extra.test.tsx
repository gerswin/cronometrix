/**
 * Branch-coverage extension for DrillDownDialog. Existing test covers
 * null employee, populated rows, empty state. This file fills:
 *  - error branch (HTTP 500 → "Error al cargar el detalle.")
 *  - leave_id branch (Novedad chip)
 *  - anomalies branch (anomaly text rendered, comma-joined)
 *  - fmtTime null → em-dash
 */
import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import React from 'react'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { DrillDownDialog } from '../drill-down-dialog'

const API = 'http://localhost:3001/api/v1'

const server = setupServer()
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
})

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('<DrillDownDialog> — extra branches', () => {
  it('renders the error message when the API returns 500', async () => {
    server.use(
      http.get(`${API}/daily-records`, () =>
        HttpResponse.json({ error: 'fail' }, { status: 500 })
      )
    )
    render(wrap(<DrillDownDialog employeeId="e-err" from="2026-04-01" to="2026-04-30" onClose={() => {}} />))
    await waitFor(() =>
      expect(screen.getByText('Error al cargar el detalle.')).toBeInTheDocument()
    )
  })

  it('renders the Novedad chip when a row has leave_id', async () => {
    server.use(
      http.get(`${API}/daily-records`, () =>
        HttpResponse.json({
          data: [{
            id: 'r1', employee_id: 'e1', department_id: 'd1',
            anchor_date: '2026-04-02', shift_type: 'day',
            work_minutes: 0, overtime_minutes: 0, late_minutes: 0, early_departure_minutes: 0,
            is_rest_day_worked: false, entry_at: null, exit_at: null,
            leave_id: 'leave-1', computed_at: '', created_at: '', updated_at: '',
            anomalies: [],
          }],
          total: 1, limit: 100, offset: 0,
        })
      )
    )
    render(wrap(<DrillDownDialog employeeId="e1" from="2026-04-01" to="2026-04-30" onClose={() => {}} />))
    await waitFor(() => expect(screen.getByText('Novedad')).toBeInTheDocument())
    // entry_at + exit_at are null → em-dashes rendered
    expect(screen.getAllByText('—').length).toBeGreaterThanOrEqual(2)
  })

  it('renders comma-joined anomaly codes when leave_id is null and anomalies non-empty', async () => {
    server.use(
      http.get(`${API}/daily-records`, () =>
        HttpResponse.json({
          data: [{
            id: 'r2', employee_id: 'e2', department_id: 'd1',
            anchor_date: '2026-04-03', shift_type: 'day',
            work_minutes: 240, overtime_minutes: 0, late_minutes: 15, early_departure_minutes: 0,
            is_rest_day_worked: false, entry_at: '2026-04-03T08:15:00Z', exit_at: null,
            leave_id: null, computed_at: '', created_at: '', updated_at: '',
            anomalies: ['LATE', 'NO_EXIT'],
          }],
          total: 1, limit: 100, offset: 0,
        })
      )
    )
    render(wrap(<DrillDownDialog employeeId="e2" from="2026-04-01" to="2026-04-30" onClose={() => {}} />))
    await waitFor(() => expect(screen.getByText('LATE, NO_EXIT')).toBeInTheDocument())
  })
})
