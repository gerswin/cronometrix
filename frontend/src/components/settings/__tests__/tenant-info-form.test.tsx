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

const { toastSuccess, toastError } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: { success: toastSuccess, error: toastError },
}))

import { TenantInfoForm } from '../tenant-info-form'
import type { TenantInfo } from '@/types/api'

const API = 'http://localhost:3001/api/v1'

let lastPatchBody: { client_name?: string; client_rif?: string; address?: string; version: number } | null = null

const server = setupServer(
  http.patch(`${API}/tenant-info`, async ({ request }) => {
    const body = (await request.json()) as {
      client_name?: string
      client_rif?: string
      address?: string
      version: number
    }
    lastPatchBody = body
    if (body.version !== 1) {
      return HttpResponse.json(
        { error: { code: 'VERSION_CONFLICT', message: 'stale' } },
        { status: 409 },
      )
    }
    return HttpResponse.json({
      client_name: body.client_name ?? '',
      client_rif: body.client_rif ?? '',
      address: body.address ?? '',
      version: 2,
      updated_at: '2026-04-25T01:00:00Z',
    })
  }),
)

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))
afterAll(() => server.close())
afterEach(() => {
  server.resetHandlers()
  vi.clearAllMocks()
  lastPatchBody = null
})

let qc: QueryClient
beforeEach(() => {
  qc = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  })
})

const initialData: TenantInfo = {
  client_name: 'Acme',
  client_rif: 'J-12345678-9',
  address: 'Caracas',
  version: 1,
  updated_at: '2026-04-25T00:00:00Z',
}

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('<TenantInfoForm>', () => {
  it('admin sees enabled inputs and submit button', () => {
    render(wrap(<TenantInfoForm initialData={initialData} canEdit={true} />))
    expect(screen.getByLabelText('Nombre del Cliente')).not.toBeDisabled()
    expect(screen.getByLabelText('RIF')).not.toBeDisabled()
    expect(screen.getByLabelText('Dirección')).not.toBeDisabled()
    expect(
      screen.getByRole('button', { name: /Guardar Cambios/ }),
    ).toBeInTheDocument()
  })

  it('viewer sees disabled inputs and no submit button', () => {
    render(wrap(<TenantInfoForm initialData={initialData} canEdit={false} />))
    expect(screen.getByLabelText('Nombre del Cliente')).toBeDisabled()
    expect(screen.getByLabelText('RIF')).toBeDisabled()
    expect(screen.getByLabelText('Dirección')).toBeDisabled()
    expect(
      screen.queryByRole('button', { name: /Guardar Cambios/ }),
    ).toBeNull()
  })

  it('supervisor sees disabled inputs and no submit button', () => {
    // Same path as viewer — non-admin = canEdit:false from the page
    render(wrap(<TenantInfoForm initialData={initialData} canEdit={false} />))
    expect(screen.getByLabelText('Nombre del Cliente')).toBeDisabled()
    expect(
      screen.queryByRole('button', { name: /Guardar Cambios/ }),
    ).toBeNull()
  })

  it('successful submit posts version field and toasts success', async () => {
    render(wrap(<TenantInfoForm initialData={initialData} canEdit={true} />))
    fireEvent.change(screen.getByLabelText('Nombre del Cliente'), {
      target: { value: 'Acme Updated' },
    })
    fireEvent.click(
      screen.getByRole('button', { name: /Guardar Cambios/ }),
    )
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())
    expect(lastPatchBody).not.toBeNull()
    expect(lastPatchBody!.version).toBe(1)
    expect(lastPatchBody!.client_name).toBe('Acme Updated')
    expect(toastSuccess).toHaveBeenCalledWith('Datos actualizados')
  })

  it('409 conflict shows reload toast', async () => {
    render(
      wrap(
        <TenantInfoForm
          initialData={{ ...initialData, version: 99 }}
          canEdit={true}
        />,
      ),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /Guardar Cambios/ }),
    )
    await waitFor(() => expect(toastError).toHaveBeenCalled())
    expect(toastError).toHaveBeenCalledWith(
      'Esta información fue modificada por otro usuario; recargando…',
    )
  })

  it('rejects malformed RIF via zod refine', async () => {
    render(wrap(<TenantInfoForm initialData={initialData} canEdit={true} />))
    fireEvent.change(screen.getByLabelText('RIF'), {
      target: { value: 'no-format' },
    })
    fireEvent.click(
      screen.getByRole('button', { name: /Guardar Cambios/ }),
    )
    await waitFor(() =>
      expect(
        screen.getByText(/RIF inválido/),
      ).toBeInTheDocument(),
    )
    // No PATCH happened
    expect(lastPatchBody).toBeNull()
  })
})
