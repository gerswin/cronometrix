import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { CreateDeviceModal } from '../create-device-modal'

const { postMock, toastError, toastSuccess } = vi.hoisted(() => ({
  postMock: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}))

vi.mock('@/lib/api', () => ({
  api: { post: (...args: unknown[]) => postMock(...args) },
}))
vi.mock('sonner', () => ({
  toast: { error: toastError, success: toastSuccess },
}))

function renderModal(open = true, onClose = vi.fn()) {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  })
  const view = render(
    <QueryClientProvider client={queryClient}>
      <CreateDeviceModal open={open} onClose={onClose} />
    </QueryClientProvider>,
  )
  return { ...view, onClose, queryClient }
}

function fillRequiredFields() {
  fireEvent.change(screen.getByTestId('device-name'), {
    target: { value: 'Salida Almacén' },
  })
  fireEvent.change(screen.getByTestId('device-ip'), {
    target: { value: 'reader-02.local' },
  })
  fireEvent.change(screen.getByTestId('device-port'), {
    target: { value: '8443' },
  })
  fireEvent.change(screen.getByTestId('device-scheme'), {
    target: { value: 'https' },
  })
  fireEvent.change(screen.getByTestId('device-username'), {
    target: { value: 'operator' },
  })
  fireEvent.change(screen.getByTestId('device-password'), {
    target: { value: 'super-secret' },
  })
  fireEvent.change(screen.getByTestId('device-direction'), {
    target: { value: 'exit' },
  })
  fireEvent.click(screen.getByTestId('device-insecure-tls'))
}

describe('CreateDeviceModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    postMock.mockResolvedValue({ data: { id: 'dev-new' } })
  })

  it('renders nothing while closed and exposes canonical defaults when opened', () => {
    const { rerender, queryClient, onClose } = renderModal(false)
    expect(screen.queryByTestId('create-device-modal')).not.toBeInTheDocument()

    rerender(
      <QueryClientProvider client={queryClient}>
        <CreateDeviceModal open={true} onClose={onClose} />
      </QueryClientProvider>,
    )

    expect(screen.getByTestId('device-name')).toHaveValue('')
    expect(screen.getByTestId('device-ip')).toHaveValue('')
    expect(screen.getByTestId('device-port')).toHaveValue(80)
    expect(screen.getByTestId('device-scheme')).toHaveValue('http')
    expect(screen.getByTestId('device-username')).toHaveValue('admin')
    expect(screen.getByTestId('device-password')).toHaveValue('')
    expect(screen.getByTestId('device-direction')).toHaveValue('entry')
    expect(screen.getByTestId('device-insecure-tls')).not.toBeChecked()
  })

  it('resets changed fields to canonical defaults whenever it is reopened', async () => {
    const { rerender, queryClient, onClose } = renderModal()
    fillRequiredFields()
    expect(screen.getByTestId('device-name')).toHaveValue('Salida Almacén')

    rerender(
      <QueryClientProvider client={queryClient}>
        <CreateDeviceModal open={false} onClose={onClose} />
      </QueryClientProvider>,
    )
    rerender(
      <QueryClientProvider client={queryClient}>
        <CreateDeviceModal open={true} onClose={onClose} />
      </QueryClientProvider>,
    )

    await waitFor(() => expect(screen.getByTestId('device-name')).toHaveValue(''))
    expect(screen.getByTestId('device-port')).toHaveValue(80)
    expect(screen.getByTestId('device-scheme')).toHaveValue('http')
    expect(screen.getByTestId('device-username')).toHaveValue('admin')
    expect(screen.getByTestId('device-direction')).toHaveValue('entry')
    expect(screen.getByTestId('device-insecure-tls')).not.toBeChecked()
  })

  it('shows client errors for missing and invalid required values without submitting', async () => {
    renderModal()
    fireEvent.change(screen.getByTestId('device-ip'), {
      target: { value: 'invalid host!' },
    })
    fireEvent.change(screen.getByTestId('device-port'), {
      target: { value: '70000' },
    })
    fireEvent.change(screen.getByTestId('device-username'), {
      target: { value: '' },
    })

    fireEvent.click(screen.getByTestId('save-device-btn'))

    expect(await screen.findByText('Nombre es requerido')).toBeInTheDocument()
    expect(screen.getByText('IP o hostname inválido')).toBeInTheDocument()
    expect(screen.getByText('Máximo 65535')).toBeInTheDocument()
    expect(screen.getByText('Usuario es requerido')).toBeInTheDocument()
    expect(screen.getByText('Contraseña es requerida')).toBeInTheDocument()
    expect(postMock).not.toHaveBeenCalled()
  })

  it('posts the exact canonical body, then toasts, invalidates devices, and closes', async () => {
    const onClose = vi.fn()
    const { queryClient } = renderModal(true, onClose)
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    fillRequiredFields()

    fireEvent.click(screen.getByTestId('save-device-btn'))

    await waitFor(() => {
      expect(postMock).toHaveBeenCalledWith('/devices', {
        allow_insecure_tls: true,
        direction: 'exit',
        ip: 'reader-02.local',
        name: 'Salida Almacén',
        password: 'super-secret',
        port: 8443,
        scheme: 'https',
        username: 'operator',
      })
    })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('Dispositivo creado'))
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['devices'] })
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('shows the canonical nested backend error message', async () => {
    postMock.mockRejectedValueOnce({
      response: { data: { error: { message: 'El dispositivo ya existe' } } },
    })
    renderModal()
    fillRequiredFields()

    fireEvent.click(screen.getByTestId('save-device-btn'))

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith('El dispositivo ya existe')
    })
  })

  it('uses the generic error fallback for an unstructured failure', async () => {
    postMock.mockRejectedValueOnce(new Error('network down'))
    renderModal()
    fillRequiredFields()

    fireEvent.click(screen.getByTestId('save-device-btn'))

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith('Error al crear dispositivo')
    })
  })

  it('disables submit and shows pending copy until the request resolves', async () => {
    let resolveRequest: (value: unknown) => void = () => undefined
    postMock.mockImplementationOnce(
      () => new Promise((resolve) => { resolveRequest = resolve }),
    )
    renderModal()
    fillRequiredFields()

    const submit = screen.getByTestId('save-device-btn')
    fireEvent.click(submit)

    await waitFor(() => expect(submit).toBeDisabled())
    expect(submit).toHaveTextContent('Guardando…')

    await act(async () => {
      resolveRequest({ data: { id: 'dev-new' } })
    })
  })

  it.each([
    ['Cancel button', 'cancel-device-btn'],
    ['dialog close button', 'Cerrar'],
  ])('%s closes without submitting', (_label, control) => {
    const onClose = vi.fn()
    renderModal(true, onClose)

    if (control === 'Cerrar') {
      fireEvent.click(screen.getByRole('button', { name: control }))
    } else {
      fireEvent.click(screen.getByTestId(control))
    }

    expect(onClose).toHaveBeenCalledTimes(1)
    expect(postMock).not.toHaveBeenCalled()
  })
})
