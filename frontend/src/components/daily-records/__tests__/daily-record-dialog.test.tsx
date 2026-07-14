import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { HttpResponse, delay, http } from 'msw'
import { setupServer } from 'msw/node'
import type { DailyRecordDetail } from '@/types/api'
import { DailyRecordDialog } from '../daily-record-dialog'

const API = 'http://localhost:3001/api/v1'
const server = setupServer()
const record: DailyRecordDetail = {
  id: 'r1', employee_id: 'e1', department_id: 'd1', anchor_date: '2026-04-23',
  shift_type: 'day', work_minutes: 480, overtime_minutes: 30, late_minutes: 12,
  early_departure_minutes: 0, is_rest_day_worked: true,
  entry_at: '2026-04-23T12:00:00Z', exit_at: '2026-04-23T21:00:00Z',
  leave_id: 'l1', computed_at: '2026-04-23T22:00:00Z',
  created_at: '2026-04-23T22:00:00Z', updated_at: '2026-04-23T22:01:00Z',
  anomalies: ['LATE', 'OVERTIME'],
}

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())

function renderDialog(recordId: string | null, onClose = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return {
    onClose,
    ...render(
      <QueryClientProvider client={client}>
        <DailyRecordDialog recordId={recordId} onClose={onClose} />
      </QueryClientProvider>,
    ),
  }
}

describe('DailyRecordDialog', () => {
  it('stays closed and does not request a record for a null id', () => {
    const request = vi.fn()
    server.use(http.get(`${API}/daily-records/:id`, () => { request(); return HttpResponse.json(record) }))
    renderDialog(null)
    expect(screen.queryByText('Detalle del registro diario')).toBeNull()
    expect(request).not.toHaveBeenCalled()
  })

  it('shows loading then all computed details, hints and anomaly badges', async () => {
    server.use(http.get(`${API}/daily-records/r1`, async () => { await delay(30); return HttpResponse.json(record) }))
    renderDialog('r1')
    expect(screen.getByText('Cargando…')).toBeVisible()
    expect(await screen.findByText('2026-04-23')).toBeVisible()
    expect(screen.getByText('8:00')).toHaveAttribute('title', '480 min')
    expect(screen.getByText('0:30')).toHaveAttribute('title', '30 min')
    expect(screen.getByText('Sí')).toBeVisible()
    expect(screen.getByText('l1')).toBeVisible()
    expect(screen.getByText('LATE')).toBeVisible()
    expect(screen.getByText('OVERTIME')).toBeVisible()
  })

  it('renders empty clock/leave/anomaly values and a normal workday', async () => {
    server.use(http.get(`${API}/daily-records/r2`, () => HttpResponse.json({
      ...record, id: 'r2', entry_at: null, exit_at: null, leave_id: null,
      is_rest_day_worked: false, work_minutes: 0, overtime_minutes: 0,
      late_minutes: 0, anomalies: [],
    })))
    renderDialog('r2')
    expect(await screen.findByText('No')).toBeVisible()
    expect(screen.getAllByText('—').length).toBeGreaterThanOrEqual(6)
  })

  it('shows an API error and closes on Escape', async () => {
    server.use(http.get(`${API}/daily-records/bad`, () => HttpResponse.json({}, { status: 500 })))
    const { onClose } = renderDialog('bad')
    expect(await screen.findByText('Error al cargar el detalle.')).toBeVisible()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalled()
  })
})
