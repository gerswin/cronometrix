import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { HttpResponse, delay, http } from 'msw'
import { setupServer } from 'msw/node'
import type { RawAttendanceEvent } from '@/types/api'
import { EventDetailDialog } from '../event-detail-dialog'

const API = 'http://localhost:3001/api/v1'
const server = setupServer()
const event: RawAttendanceEvent = {
  id: 'ev-1', employee_id: 'e1', device_id: 'd1', direction: 'entry',
  captured_at: '2026-04-23T12:00:00Z', is_unknown: false, face_id: 'face-1',
  employee_no_string: '001', photo_path: null, created_at: '2026-04-23T12:00:01Z',
}

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())
beforeEach(() => vi.clearAllMocks())

function renderDialog(eventId: string | null, onClose = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return {
    onClose,
    ...render(
      <QueryClientProvider client={client}>
        <EventDetailDialog eventId={eventId} onClose={onClose} />
      </QueryClientProvider>,
    ),
  }
}

describe('EventDetailDialog', () => {
  it('does not fetch or open when no event is selected', () => {
    const request = vi.fn()
    server.use(http.get(`${API}/events/:id`, () => { request(); return HttpResponse.json(event) }))
    renderDialog(null)
    expect(screen.queryByText('Detalle del evento')).toBeNull()
    expect(request).not.toHaveBeenCalled()
  })

  it('shows loading and then renders event fields and a filtered timesheet link', async () => {
    server.use(http.get(`${API}/events/ev-1`, async () => { await delay(30); return HttpResponse.json(event) }))
    renderDialog('ev-1')
    expect(screen.getByText('Cargando…')).toBeVisible()
    expect(await screen.findByText('face-1')).toBeVisible()
    expect(screen.getByText('Entrada')).toBeVisible()
    expect(screen.getByText('e1')).toBeVisible()
    expect(screen.getByRole('link', { name: /Ir a marcaciones/ })).toHaveAttribute(
      'href',
      '/timesheet?from_date=2026-04-23&to_date=2026-04-23&employee_id=e1',
    )
  })

  it('renders exit, unknown and missing optional identifiers', async () => {
    server.use(http.get(`${API}/events/ev-2`, () => HttpResponse.json({
      ...event, id: 'ev-2', direction: 'exit', employee_id: null, is_unknown: true,
      face_id: null, employee_no_string: null,
    })))
    renderDialog('ev-2')
    expect(await screen.findByText('Salida')).toBeVisible()
    expect(screen.getByText('Desconocido')).toBeVisible()
    expect(screen.getAllByText('—')).toHaveLength(3)
    expect(screen.getByRole('link', { name: /Ir a marcaciones/ }).getAttribute('href')).not.toContain('employee_id')
  })

  it('shows a request error and closes on Escape', async () => {
    server.use(http.get(`${API}/events/ev-bad`, () => HttpResponse.json({}, { status: 500 })))
    const { onClose } = renderDialog('ev-bad')
    expect(await screen.findByText('Error al cargar el evento.')).toBeVisible()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalled()
  })
})
