import {
  describe,
  it,
  expect,
  beforeAll,
  afterAll,
  beforeEach,
  afterEach,
  vi,
} from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

// Mock toast and pdf renderer BEFORE importing the component.
// Use vi.hoisted so the mocks (which are referenced inside the factory
// callbacks) are evaluated before vi.mock hoisting kicks in.
const { toastSuccess, toastError, renderReportPdfMock } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  renderReportPdfMock: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: { success: toastSuccess, error: toastError },
}))
vi.mock('@/lib/reports/pdf', () => ({
  renderReportPdf: (...args: unknown[]) => renderReportPdfMock(...args),
}))

// Stub URL.createObjectURL / revokeObjectURL — jsdom provides createObjectURL
// only as a no-op that returns 'blob:nodedata:...' but we want to assert.
const createObjectURLMock = vi.fn(() => 'blob:mock')
const revokeObjectURLMock = vi.fn()
Object.defineProperty(URL, 'createObjectURL', { value: createObjectURLMock, writable: true })
Object.defineProperty(URL, 'revokeObjectURL', { value: revokeObjectURLMock, writable: true })

// Track api.post calls — easiest by spying on the api module before import.
import { ExportButtons } from '../export-buttons'
import type { ReportFilters } from '@/types/api'

const API = 'http://localhost:3001/api/v1'

let postedToExcelWith: { body: unknown; contentType: string | null } | null =
  null

const server = setupServer(
  http.post(`${API}/reports/excel`, async ({ request }) => {
    postedToExcelWith = {
      body: await request.text(),
      contentType: request.headers.get('content-type'),
    }
    return new HttpResponse(new Blob(['xlsx-bytes']), {
      status: 200,
      headers: {
        'content-type':
          'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        'content-disposition':
          'attachment; filename="prenomina_2026-04-01_2026-04-30.xlsx"',
      },
    })
  }),
  http.post(`${API}/reports/json`, async () => {
    return HttpResponse.json({
      header: {
        client_name: 'Test',
        client_rif: 'J-1-9',
        from_date: '2026-04-01',
        to_date: '2026-04-30',
        generated_at_iso: '2026-04-25T18:00:00Z',
      },
      rows: [],
      dept_subtotals: [],
      grand_total: {
        work_min: 0,
        ot_min: 0,
        late_min: 0,
        days_worked: 0,
        days_absent: 0,
        work_pay_cents: 0,
        ot_pay_cents: 0,
        night_premium_cents: 0,
        rest_day_surcharge_cents: 0,
        late_deduction_cents: 0,
        total_a_pagar_cents: 0,
        days_ivss: 0,
        days_vacation: 0,
        days_permission: 0,
        days_unpaid: 0,
      },
      departments_in_order: [],
    })
  }),
)

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => {
  server.resetHandlers()
  vi.clearAllMocks()
  postedToExcelWith = null
})

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })
})

const filters: ReportFilters = {
  period_type: 'monthly',
  from_date: '2026-04-01',
  to_date: '2026-04-30',
  include_inactive: false,
}

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('<ExportButtons>', () => {
  it('renders both Excel and PDF buttons', () => {
    render(wrap(<ExportButtons filters={filters} />))
    expect(screen.getByLabelText('Exportar Excel')).toBeInTheDocument()
    expect(screen.getByLabelText('Exportar PDF')).toBeInTheDocument()
  })

  it('clicking Exportar Excel posts to /reports/excel and triggers download', async () => {
    render(wrap(<ExportButtons filters={filters} />))
    fireEvent.click(screen.getByLabelText('Exportar Excel'))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())
    // Server received the POST with content-type application/json.
    expect(postedToExcelWith).not.toBeNull()
    expect(postedToExcelWith!.contentType).toContain('application/json')
    // URL.createObjectURL was called with the Blob.
    expect(createObjectURLMock).toHaveBeenCalled()
    // toast.success message
    expect(toastSuccess).toHaveBeenCalledWith('Reporte Excel descargado')
  })

  it('clicking Exportar PDF posts to /reports/json and calls renderReportPdf', async () => {
    render(wrap(<ExportButtons filters={filters} />))
    fireEvent.click(screen.getByLabelText('Exportar PDF'))
    await waitFor(() =>
      expect(renderReportPdfMock).toHaveBeenCalledTimes(1),
    )
    expect(toastSuccess).toHaveBeenCalledWith('Reporte PDF generado')
    // Verify the payload shape was passed through.
    const arg = renderReportPdfMock.mock.calls[0][0] as {
      header: { from_date: string }
    }
    expect(arg.header.from_date).toBe('2026-04-01')
  })

  it('Excel button toasts error when server returns 500', async () => {
    server.use(
      http.post(`${API}/reports/excel`, () =>
        HttpResponse.json(
          { error: { code: 'INTERNAL', message: 'fail' } },
          { status: 500 },
        ),
      ),
    )
    render(wrap(<ExportButtons filters={filters} />))
    fireEvent.click(screen.getByLabelText('Exportar Excel'))
    await waitFor(() => expect(toastError).toHaveBeenCalled())
    expect(toastError).toHaveBeenCalledWith('Error al generar el Excel')
  })

  it('PDF button toasts error when server returns 500', async () => {
    server.use(
      http.post(`${API}/reports/json`, () =>
        HttpResponse.json(
          { error: { code: 'INTERNAL', message: 'fail' } },
          { status: 500 },
        ),
      ),
    )
    render(wrap(<ExportButtons filters={filters} />))
    fireEvent.click(screen.getByLabelText('Exportar PDF'))
    await waitFor(() => expect(toastError).toHaveBeenCalled())
    expect(toastError).toHaveBeenCalledWith('Error al generar el PDF')
  })
})
