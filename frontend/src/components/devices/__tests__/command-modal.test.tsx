import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { CommandModal } from '../command-modal'
import type { Device } from '@/types/api'

const { toastSuccess, toastError, postMock } = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  postMock: vi.fn(),
}))
vi.mock('sonner', () => ({ toast: { success: toastSuccess, error: toastError } }))
vi.mock('@/lib/api', () => ({ api: { post: (...a: unknown[]) => postMock(...a) } }))

const DEVICE: Device = {
  id: 'dev-1',
  name: 'Entrada Principal',
  ip_address: '10.0.0.10',
  direction: 'entry',
  status: 'online',
  last_seen_at: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
}

describe('CommandModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    postMock.mockResolvedValue({ data: { ok: true } })
  })

  it('renders title and device name + ip when open', () => {
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    expect(screen.getByText('Enviar Comando ISAPI')).toBeTruthy()
    expect(screen.getByText('Entrada Principal')).toBeTruthy()
    expect(screen.getByText(/10.0.0.10/)).toBeTruthy()
  })

  it('renders all three Spanish command labels in the select', () => {
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    expect(screen.getByText('Abrir Puerta')).toBeTruthy()
    expect(screen.getByText('Reiniciar Dispositivo')).toBeTruthy()
    expect(screen.getByText('Modo Enrolamiento')).toBeTruthy()
  })

  it('shows the reboot warning copy only when reboot is selected', () => {
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    // Default: door_open. No warning.
    expect(screen.queryByText(/dispositivo perderá conexión/i)).toBeNull()
    // Switch to reboot
    const select = screen.getByRole('combobox') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'reboot' } })
    expect(screen.getByText(/dispositivo perderá conexión/i)).toBeTruthy()
  })

  it('clicking Enviar Comando POSTs /devices/:id/commands with payload', async () => {
    const onClose = vi.fn()
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={onClose} />))
    const submit = screen.getByRole('button', { name: /Enviar Comando/i })
    await act(async () => { fireEvent.click(submit) })
    await waitFor(() => expect(postMock).toHaveBeenCalled())
    expect(postMock).toHaveBeenCalledWith('/devices/dev-1/commands', { command: 'door_open' })
    await waitFor(() => expect(toastSuccess).toHaveBeenCalled())
    expect(onClose).toHaveBeenCalled()
  })

  it('toasts the server error message on failure', async () => {
    postMock.mockRejectedValueOnce({ response: { data: { message: 'Device offline' } } })
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    const submit = screen.getByRole('button', { name: /Enviar Comando/i })
    await act(async () => { fireEvent.click(submit) })
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Device offline'))
  })

  it('toasts a generic error message when the server reply has no message', async () => {
    postMock.mockRejectedValueOnce({})
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    const submit = screen.getByRole('button', { name: /Enviar Comando/i })
    await act(async () => { fireEvent.click(submit) })
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Error al enviar comando'))
  })

  it('Cancel button calls onClose without submitting', () => {
    const onClose = vi.fn()
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={onClose} />))
    fireEvent.click(screen.getByRole('button', { name: /Cancelar/i }))
    expect(onClose).toHaveBeenCalled()
    expect(postMock).not.toHaveBeenCalled()
  })

  it('selecting enrollment_mode and submitting POSTs the enrollment_mode payload', async () => {
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    const select = screen.getByRole('combobox') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'enrollment_mode' } })
    const submit = screen.getByRole('button', { name: /Enviar Comando/i })
    await act(async () => { fireEvent.click(submit) })
    await waitFor(() => expect(postMock).toHaveBeenCalled())
    expect(postMock).toHaveBeenCalledWith('/devices/dev-1/commands', { command: 'enrollment_mode' })
  })

  it('selecting reboot then switching back to door_open hides the warning copy again', () => {
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    const select = screen.getByRole('combobox') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'reboot' } })
    expect(screen.getByText(/dispositivo perderá conexión/i)).toBeTruthy()
    fireEvent.change(select, { target: { value: 'door_open' } })
    expect(screen.queryByText(/dispositivo perderá conexión/i)).toBeNull()
  })

  it('closed state (open=false) renders nothing — exercises the early dialog-content guard', () => {
    render(wrap(<CommandModal open={false} device={DEVICE} onClose={() => {}} />))
    expect(screen.queryByText('Enviar Comando ISAPI')).toBeNull()
  })

  it('button shows Enviando… while pending; remains disabled until resolution', async () => {
    let resolveCommand: (v: unknown) => void = () => {}
    postMock.mockImplementationOnce(() => new Promise((r) => { resolveCommand = r }))
    render(wrap(<CommandModal open={true} device={DEVICE} onClose={() => {}} />))
    const submit = screen.getByRole('button', { name: /Enviar Comando/i }) as HTMLButtonElement
    await act(async () => { fireEvent.click(submit) })
    await waitFor(() => expect(submit.disabled).toBe(true))
    expect(submit.textContent).toContain('Enviando')
    resolveCommand?.({ data: { ok: true } })
  })
})
