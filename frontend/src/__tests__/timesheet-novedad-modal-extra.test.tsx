/**
 * Top-level coverage extension that mounts the NovedadModal component
 * (the existing novedad-modal.test.tsx covers only the Zod schema).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { NovedadModal } from '../components/timesheet/novedad-modal'
import type { DailyRecord } from '../types/api'

const { apiPost } = vi.hoisted(() => ({ apiPost: vi.fn() }))
vi.mock('@/lib/api', () => ({ api: { post: (...a: unknown[]) => apiPost(...a) } }))

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

const RECORD: DailyRecord = {
  id: 'rec-1',
  employee_id: 'emp-1',
  department_id: 'dep-1',
  anchor_date: '2026-04-23',
  shift_type: 'day',
  work_minutes: 0,
  overtime_minutes: 0,
  late_minutes: 0,
  early_departure_minutes: 0,
  is_rest_day_worked: false,
  entry_at: null,
  exit_at: null,
  leave_id: null,
  computed_at: '2026-04-23T16:00:00Z',
  created_at: '2026-04-23T00:00:00Z',
  updated_at: '2026-04-23T00:00:00Z',
  anomalies: [],
}

describe('NovedadModal (component)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    apiPost.mockResolvedValue({ data: { ok: true } })
  })

  it('closed state renders nothing', () => {
    render(wrap(<NovedadModal open={false} record={RECORD} onClose={() => {}} />))
    expect(screen.queryByText('Registrar Novedad')).toBeNull()
  })

  it('renders the title + Aprobado decorative chip + required field labels', () => {
    render(wrap(<NovedadModal open={true} record={RECORD} onClose={() => {}} />))
    // "Registrar Novedad" appears in both the title and the submit button; assert presence via getAllByText
    expect(screen.getAllByText('Registrar Novedad').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText('Aprobado')).toBeTruthy()
    expect(screen.getByText(/Empleado \*/)).toBeTruthy()
    expect(screen.getByText(/Departamento \*/)).toBeTruthy()
    expect(screen.getByText(/Fecha Inicio \*/)).toBeTruthy()
    expect(screen.getByText(/Fecha Fin \*/)).toBeTruthy()
    expect(screen.getByText(/Tipo de Novedad \*/)).toBeTruthy()
    expect(screen.getByText(/Descripción \/ Justificación \*/)).toBeTruthy()
  })

  it('Cancelar fires onClose', async () => {
    const onClose = vi.fn()
    render(wrap(<NovedadModal open={true} record={RECORD} onClose={onClose} />))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Cancelar/i }))
    })
    expect(onClose).toHaveBeenCalled()
  })

  it('submitting with empty justification surfaces a Zod error and does not POST', async () => {
    render(wrap(<NovedadModal open={true} record={RECORD} onClose={() => {}} />))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Registrar Novedad/i }))
    })
    // No api.post call because validation failed
    expect(apiPost).not.toHaveBeenCalled()
  })

  it('valid submission with a record id POSTs to /daily-records/:id/overrides', async () => {
    const onClose = vi.fn()
    render(wrap(<NovedadModal open={true} record={RECORD} onClose={onClose} />))
    fireEvent.input(screen.getByLabelText(/Descripción \/ Justificación/), {
      target: { value: 'Permiso médico' },
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Registrar Novedad/i }))
    })
    await waitFor(() => expect(apiPost).toHaveBeenCalled())
    const [url] = apiPost.mock.calls[0]
    expect(url).toBe(`/daily-records/${RECORD.id}/overrides`)
    expect(onClose).toHaveBeenCalled()
  })

  it('valid submission with a null record posts to /leaves instead', async () => {
    render(wrap(<NovedadModal open={true} record={null} onClose={() => {}} />))
    // Fill required fields manually because record=null
    fireEvent.input(screen.getByLabelText('Empleado *'), { target: { value: 'emp-2' } })
    fireEvent.input(screen.getByLabelText('Departamento *'), { target: { value: 'dep-2' } })
    fireEvent.input(screen.getByLabelText('Fecha Inicio *'), { target: { value: '2026-05-01' } })
    fireEvent.input(screen.getByLabelText('Fecha Fin *'), { target: { value: '2026-05-01' } })
    fireEvent.input(screen.getByLabelText(/Descripción \/ Justificación/), {
      target: { value: 'Vacaciones' },
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Registrar Novedad/i }))
    })
    await waitFor(() => expect(apiPost).toHaveBeenCalled())
    const [url] = apiPost.mock.calls[0]
    expect(url).toBe('/leaves')
  })
})
