/**
 * Branch-coverage extension for ExportButtons. Existing test covers happy
 * + 500 paths. This file adds: in-flight disabled state, Generando…
 * label transition, payload prop is accepted but unused (line 18: `void
 * _payload`), accessibility — the buttons are real <button type="button">.
 */
import { describe, it, expect, vi, beforeEach, afterEach, beforeAll, afterAll } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ExportButtons } from '../export-buttons'
import type { ReportFilters, ReportPayload } from '@/types/api'

const { toastSuccess, toastError, renderReportPdfMock } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  renderReportPdfMock: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: { success: toastSuccess, error: toastError },
}))
vi.mock('@/lib/reports/pdf', () => ({
  renderReportPdf: (...a: unknown[]) => renderReportPdfMock(...a),
}))

Object.defineProperty(URL, 'createObjectURL', { value: vi.fn(() => 'blob:x'), writable: true })
Object.defineProperty(URL, 'revokeObjectURL', { value: vi.fn(), writable: true })

const API = 'http://localhost:3001/api/v1'

let resolveExcel: ((res: Response) => void) | null = null

const server = setupServer(
  http.post(`${API}/reports/excel`, async () =>
    new Promise<Response>((res) => { resolveExcel = res })
  ),
  http.post(`${API}/reports/json`, async () =>
    HttpResponse.json({
      header: { client_name: '', client_rif: '', from_date: '2026-04-01', to_date: '2026-04-30', generated_at_iso: '2026-04-25T18:00:00Z' },
      rows: [], dept_subtotals: [], grand_total: zeros(), departments_in_order: [],
    } as ReportPayload)
  ),
)

function zeros() {
  return {
    work_min: 0, ot_min: 0, late_min: 0, days_worked: 0, days_absent: 0,
    work_pay_cents: 0, ot_pay_cents: 0, night_premium_cents: 0,
    rest_day_surcharge_cents: 0, late_deduction_cents: 0, total_a_pagar_cents: 0,
    days_ivss: 0, days_vacation: 0, days_permission: 0, days_unpaid: 0,
  }
}

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => { server.resetHandlers(); vi.clearAllMocks(); resolveExcel = null })

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
})

const filters: ReportFilters = {
  period_type: 'monthly', from_date: '2026-04-01', to_date: '2026-04-30', include_inactive: false,
}

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('<ExportButtons> — extra branches', () => {
  it('shows the Generando… label and disables the Excel button while in flight', async () => {
    render(wrap(<ExportButtons filters={filters} />))
    const btn = screen.getByLabelText('Exportar Excel') as HTMLButtonElement
    fireEvent.click(btn)
    await waitFor(() => expect(btn.disabled).toBe(true))
    expect(btn.textContent).toContain('Generando')
    // Resolve the pending excel response so we don't hang afterEach.
    resolveExcel?.(new Response(new Blob(['x']), { status: 200 }))
  })

  it('payload prop is accepted but ignored (still re-fetches from server)', async () => {
    const fakePayload: ReportPayload = {
      header: { client_name: 'Cached', client_rif: 'J-1', from_date: '2026-04-01', to_date: '2026-04-30', generated_at_iso: '2026-04-25T18:00:00Z' },
      rows: [], dept_subtotals: [], grand_total: zeros(), departments_in_order: [],
    }
    render(wrap(<ExportButtons filters={filters} payload={fakePayload} />))
    fireEvent.click(screen.getByLabelText('Exportar PDF'))
    await waitFor(() => expect(renderReportPdfMock).toHaveBeenCalled())
    // The arg is the FRESH payload from the server (client_name=''), NOT the cached one.
    const arg = renderReportPdfMock.mock.calls[0][0] as ReportPayload
    expect(arg.header.client_name).toBe('')
  })

  it('both buttons render with type="button" so they cannot accidentally submit a parent form', () => {
    render(wrap(<ExportButtons filters={filters} />))
    expect((screen.getByLabelText('Exportar Excel') as HTMLButtonElement).type).toBe('button')
    expect((screen.getByLabelText('Exportar PDF') as HTMLButtonElement).type).toBe('button')
  })

  it('Excel filename uses the from/to date range', async () => {
    // Use a more deterministic filter
    const local: ReportFilters = { ...filters, from_date: '2026-05-01', to_date: '2026-05-31' }
    server.use(
      http.post(`${API}/reports/excel`, () =>
        new HttpResponse(new Blob(['xlsx-bytes']), { status: 200 })
      ),
    )
    const createSpy = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fname')
    const appendSpy = vi.spyOn(document.body, 'appendChild')
    render(wrap(<ExportButtons filters={local} />))
    await act(async () => { fireEvent.click(screen.getByLabelText('Exportar Excel')) })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())
    // The component appends an <a> with download attr containing the date range
    const appendedAnchor = appendSpy.mock.calls.find(
      ([n]) => (n as HTMLAnchorElement).tagName === 'A'
    )?.[0] as HTMLAnchorElement | undefined
    expect(appendedAnchor?.download).toBe('prenomina_2026-05-01_2026-05-31.xlsx')
    createSpy.mockRestore()
    appendSpy.mockRestore()
  })
})
