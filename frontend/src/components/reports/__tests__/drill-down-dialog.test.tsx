import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { DrillDownDialog } from '../drill-down-dialog'

const API = 'http://localhost:3001/api/v1'

const server = setupServer(
  http.get(`${API}/daily-records`, ({ request }) => {
    const url = new URL(request.url)
    const employee = url.searchParams.get('employee_id')
    if (employee !== 'e1') {
      return HttpResponse.json({ data: [], total: 0, limit: 100, offset: 0 })
    }
    return HttpResponse.json({
      data: [
        {
          id: 'r1',
          employee_id: 'e1',
          department_id: 'd1',
          anchor_date: '2026-04-01',
          shift_type: 'day',
          work_minutes: 480,
          overtime_minutes: 30,
          late_minutes: 5,
          early_departure_minutes: 0,
          is_rest_day_worked: false,
          entry_at: '2026-04-01T08:00:00Z',
          exit_at: '2026-04-01T17:00:00Z',
          leave_id: null,
          computed_at: '2026-04-01T18:00:00Z',
          created_at: '2026-04-01T18:00:00Z',
          updated_at: '2026-04-01T18:00:00Z',
          anomalies: [],
        },
      ],
      total: 1,
      limit: 100,
      offset: 0,
    })
  }),
)

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
})

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('<DrillDownDialog>', () => {
  it('does not query when employeeId is null', () => {
    render(
      wrap(
        <DrillDownDialog
          employeeId={null}
          from="2026-04-01"
          to="2026-04-30"
          onClose={() => {}}
        />,
      ),
    )
    // Dialog content is portalled but with employeeId null the query is
    // disabled — assert via no fetch state by checking title is not in DOM.
    expect(screen.queryByText('Detalle por Día')).toBeNull()
  })

  it('opens dialog and renders fetched daily records when employeeId is set', async () => {
    render(
      wrap(
        <DrillDownDialog
          employeeId="e1"
          from="2026-04-01"
          to="2026-04-30"
          onClose={() => {}}
        />,
      ),
    )
    await waitFor(() =>
      expect(screen.getByText('2026-04-01')).toBeInTheDocument(),
    )
    expect(screen.getByText('Detalle por Día')).toBeInTheDocument()
    expect(screen.getByText('480')).toBeInTheDocument() // work minutes
  })

  it('shows empty placeholder when API returns no records', async () => {
    render(
      wrap(
        <DrillDownDialog
          employeeId="e2"
          from="2026-04-01"
          to="2026-04-30"
          onClose={() => {}}
        />,
      ),
    )
    await waitFor(() =>
      expect(
        screen.getByText('Sin registros para este período.'),
      ).toBeInTheDocument(),
    )
  })
})
