import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { HttpResponse, http } from 'msw'
import { setupServer } from 'msw/node'
import type { Department, GlobalRules } from '@/types/api'

const { toastSuccess, toastError } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}))
vi.mock('sonner', () => ({ toast: { success: toastSuccess, error: toastError } }))

import { RulesForm } from '../rules-form'

const API = 'http://localhost:3001/api/v1'
const server = setupServer()

const initial: GlobalRules = {
  late_arrival_tolerance_min: 20,
  early_departure_tolerance_min: 10,
  bonus_minutes: 5,
  effective_from: '2026-04-01T12:00:00Z',
  version: 4,
  updated_at: '2026-04-02T12:00:00Z',
}

const department: Department = {
  id: 'd1', name: 'Operaciones', base_salary_cents: 0,
  shift_start_time: '08:00', shift_end_time: '17:00', lunch_mode: 'fixed',
  lunch_duration_min: 60, status: 'active', deleted_at: null, version: 1,
  created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z',
}

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => server.resetHandlers())

let queryClient: QueryClient
beforeEach(() => {
  vi.clearAllMocks()
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  server.use(
    http.get(`${API}/departments`, () => HttpResponse.json({ data: [department], total: 1, limit: 1000, offset: 0 })),
  )
})

function renderForm(props: { initialData?: GlobalRules; canEdit?: boolean } = {}) {
  return render(
    <QueryClientProvider client={queryClient}>
      <RulesForm initialData={props.initialData ?? initial} canEdit={props.canEdit ?? true} />
    </QueryClientProvider>,
  )
}

describe('RulesForm', () => {
  it('loads the active department, updates all rule controls and submits the versioned payload', async () => {
    let body: unknown
    server.use(
      http.patch(`${API}/rules`, async ({ request }) => {
        body = await request.json()
        return HttpResponse.json({ ok: true })
      }),
    )
    renderForm()
    await screen.findByText('Jornada Ideal')

    fireEvent.change(screen.getByLabelText('Entrada Arriba'), { target: { value: '25' } })
    fireEvent.change(screen.getByLabelText('Salida Anticipada'), { target: { value: '15' } })
    fireEvent.change(screen.getByLabelText('Minutos Libres'), { target: { value: '8' } })
    expect(screen.getByText('25 min')).toBeVisible()
    expect(screen.getByText('15 min')).toBeVisible()
    expect(screen.getByText('8', { selector: 'span' })).toBeVisible()

    fireEvent.submit(screen.getByRole('form', { name: 'Reglas Globales' }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('Reglas actualizadas'))
    expect(body).toEqual({
      late_arrival_tolerance_min: 25,
      early_departure_tolerance_min: 15,
      bonus_minutes: 8,
      version: 4,
    })
  })

  it('clamps bonus input to its range and resets when upstream rules change', async () => {
    const { rerender } = renderForm()
    const bonus = screen.getByLabelText('Minutos Libres')
    fireEvent.change(bonus, { target: { value: '99' } })
    expect(bonus).toHaveValue(60)
    fireEvent.change(bonus, { target: { value: '-3' } })
    expect(bonus).toHaveValue(0)
    fireEvent.change(bonus, { target: { value: '' } })
    expect(bonus).toHaveValue(0)
    fireEvent.change(bonus, { target: { value: 'not-a-number' } })

    rerender(
      <QueryClientProvider client={queryClient}>
        <RulesForm initialData={{ ...initial, bonus_minutes: 12, version: 5 }} canEdit />
      </QueryClientProvider>,
    )
    expect(screen.getByLabelText('Minutos Libres')).toHaveValue(12)
    expect(screen.getByText('Versión:').parentElement).toHaveTextContent('5')
  })

  it('uses the first department when none is active and handles an empty department list', async () => {
    server.use(
      http.get(`${API}/departments`, () => HttpResponse.json({ data: [{ ...department, status: 'inactive' }], total: 1, limit: 1000, offset: 0 })),
    )
    const { unmount } = renderForm()
    expect(await screen.findByText('Jornada Ideal')).toBeVisible()
    unmount()

    queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    server.use(
      http.get(`${API}/departments`, () => HttpResponse.json({ data: [], total: 0, limit: 1000, offset: 0 })),
    )
    renderForm()
    expect(await screen.findByText(/Configure al menos un departamento/)).toBeVisible()
  })

  it('reports optimistic-lock conflicts and refreshes the rules query', async () => {
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    server.use(http.patch(`${API}/rules`, () => HttpResponse.json({}, { status: 409 })))
    renderForm()
    fireEvent.submit(screen.getByRole('form', { name: 'Reglas Globales' }))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Otro admin acaba de cambiar las reglas; recargando…'))
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['rules'] })
  })

  it('reports a generic save failure', async () => {
    server.use(http.patch(`${API}/rules`, () => HttpResponse.json({}, { status: 500 })))
    renderForm()
    fireEvent.submit(screen.getByRole('form', { name: 'Reglas Globales' }))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Error al guardar'))
  })

  it('disables every editable control for read-only users', () => {
    renderForm({ canEdit: false })
    expect(screen.getByLabelText('Entrada Arriba')).toBeDisabled()
    expect(screen.getByLabelText('Salida Anticipada')).toBeDisabled()
    expect(screen.getByLabelText('Minutos Libres')).toBeDisabled()
  })
})
