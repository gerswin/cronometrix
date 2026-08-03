/**
 * Task 5 (frontend per-file coverage floor): this component sat under
 * 30% line coverage because it was only ever exercised indirectly through
 * TimesheetTable's tests, which never hover/click into a row's leave
 * actions. These tests mount it directly and drive the lazy-fetch, evidence
 * download, and cancel-mutation branches so the file clears the 70/60/70/70
 * per-file floor enforced by scripts/enforce-frontend-file-floor.mjs.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { LeaveRowActions } from '../leave-row-actions'
import type { Leave } from '@/types/api'

const { useAuthMock, getMock, deleteMock, toastSuccess, toastError, openBlobInNewTabMock } = vi.hoisted(() => ({
  useAuthMock: vi.fn(),
  getMock: vi.fn(),
  deleteMock: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  openBlobInNewTabMock: vi.fn(),
}))
vi.mock('@/hooks/use-auth', () => ({ useAuth: () => useAuthMock() }))
vi.mock('@/lib/api', () => ({
  api: {
    get: (...a: unknown[]) => getMock(...a),
    delete: (...a: unknown[]) => deleteMock(...a),
  },
}))
vi.mock('@/lib/file-download', () => ({ openBlobInNewTab: (...a: unknown[]) => openBlobInNewTabMock(...a) }))
vi.mock('sonner', () => ({ toast: { success: toastSuccess, error: toastError } }))

function makeLeave(over: Partial<Leave> = {}): Leave {
  return {
    id: 'leave-1',
    employee_id: 'emp-1',
    from_date: '2026-04-23',
    to_date: '2026-04-23',
    leave_type: 'medical',
    justification: 'Consulta médica',
    evidence_path: null,
    created_by: 'u1',
    status: 'active',
    version: 3,
    created_at: '2026-04-23T08:00:00-04:00',
    updated_at: '2026-04-23T08:00:00-04:00',
    ...over,
  }
}

function renderActions(leaveId = 'leave-1') {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return render(
    <QueryClientProvider client={client}>
      <LeaveRowActions leaveId={leaveId} />
    </QueryClientProvider>,
  )
}

// The lazy-fetch hook is `onMouseEnter`/`onFocus` on the wrapping <span>.
// Neither role nor testid is on it, so grab the container's own first child.
function hoverRoot(container: HTMLElement) {
  fireEvent.mouseEnter(container.firstChild as Element)
}

describe('LeaveRowActions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    getMock.mockResolvedValue({ data: makeLeave() })
  })

  it('renders nothing before hover: no evidence button, cancel button disabled pending leave fetch', () => {
    renderActions()
    expect(screen.queryByTestId('leave-evidence-button')).toBeNull()
    expect(getMock).not.toHaveBeenCalled()
    expect(screen.getByTestId('leave-cancel-button')).toBeDisabled()
  })

  it('non-admin never sees the cancel button, even after the leave loads', async () => {
    useAuthMock.mockReturnValue({ role: 'supervisor', sub: 'u1', claims: null })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(getMock).toHaveBeenCalledWith('/leaves/leave-1'))
    expect(screen.queryByTestId('leave-cancel-button')).toBeNull()
  })

  it('hover triggers the lazy GET and enables the cancel button once loaded', async () => {
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    expect(getMock).toHaveBeenCalledWith('/leaves/leave-1')
  })

  it('shows the evidence button once the loaded leave has an evidence_path', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-evidence-button')).toBeTruthy())
  })

  it('a cancelled leave hides the cancel button even for an admin', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ status: 'cancelled' }) })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(getMock).toHaveBeenCalled())
    expect(screen.queryByTestId('leave-cancel-button')).toBeNull()
  })

  it('clicking the evidence button downloads the blob and opens it in a new tab', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    getMock.mockResolvedValueOnce({
      data: new Blob(['pdf-bytes']),
      headers: { 'content-type': 'application/pdf' },
    })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    const evidenceButton = await screen.findByTestId('leave-evidence-button')
    await act(async () => fireEvent.click(evidenceButton))
    await waitFor(() => expect(openBlobInNewTabMock).toHaveBeenCalled())
    expect(getMock).toHaveBeenCalledWith('/leaves/leave-1/evidence', { responseType: 'blob' })
  })

  it('falls back to application/octet-stream when the response has no content-type header', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    getMock.mockResolvedValueOnce({ data: new Blob(['bytes']), headers: {} })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    const evidenceButton = await screen.findByTestId('leave-evidence-button')
    await act(async () => fireEvent.click(evidenceButton))
    await waitFor(() => expect(openBlobInNewTabMock).toHaveBeenCalled())
    const blobArg = openBlobInNewTabMock.mock.calls[0][0] as Blob
    expect(blobArg.type).toBe('application/octet-stream')
  })

  it('toasts a specific message when evidence download 404s', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    getMock.mockRejectedValueOnce({ response: { status: 404 } })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    const evidenceButton = await screen.findByTestId('leave-evidence-button')
    await act(async () => fireEvent.click(evidenceButton))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Esta novedad no tiene evidencia adjunta'))
    expect(openBlobInNewTabMock).not.toHaveBeenCalled()
  })

  it('toasts a generic message when evidence download fails for another reason', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    getMock.mockRejectedValueOnce({ response: { status: 500 } })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    const evidenceButton = await screen.findByTestId('leave-evidence-button')
    await act(async () => fireEvent.click(evidenceButton))
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('No se pudo descargar la evidencia'))
  })

  it('the evidence button is disabled while a download is in flight', async () => {
    getMock.mockResolvedValueOnce({ data: makeLeave({ evidence_path: '/tmp/evidence.pdf' }) })
    let resolveDownload: (v: unknown) => void = () => {}
    getMock.mockImplementationOnce(
      () => new Promise((resolve) => { resolveDownload = resolve }),
    )
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    const evidenceButton = await screen.findByTestId('leave-evidence-button')
    fireEvent.click(evidenceButton)
    await waitFor(() => expect(evidenceButton).toBeDisabled())
    await act(async () => resolveDownload({ data: new Blob(['x']), headers: {} }))
  })

  it('clicking the cancel button opens the confirm dialog', async () => {
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    fireEvent.click(screen.getByTestId('leave-cancel-button'))
    expect(screen.getByRole('heading', { name: 'Cancelar novedad' })).toBeTruthy()
    expect(screen.getByText(/El registro diario volverá a calcularse/)).toBeTruthy()
  })

  it('"Volver" closes the dialog without mutating', async () => {
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    fireEvent.click(screen.getByTestId('leave-cancel-button'))
    fireEvent.click(screen.getByRole('button', { name: 'Volver' }))
    expect(deleteMock).not.toHaveBeenCalled()
  })

  it('confirming cancel DELETEs with the loaded leave version and toasts success', async () => {
    deleteMock.mockResolvedValueOnce({ data: {} })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    fireEvent.click(screen.getByTestId('leave-cancel-button'))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^Cancelar novedad$/ }))
    })
    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith('/leaves/leave-1', { params: { version: 3 } }))
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith('Novedad cancelada'))
  })

  it('a 409 conflict on cancel shows the stale-data message and refetches', async () => {
    deleteMock.mockRejectedValueOnce({ response: { status: 409 } })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    fireEvent.click(screen.getByTestId('leave-cancel-button'))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^Cancelar novedad$/ }))
    })
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Esta novedad cambió; recarga e intenta de nuevo'))
  })

  it('a non-409 failure on cancel shows a generic error message', async () => {
    deleteMock.mockRejectedValueOnce({ response: { status: 500 } })
    const { container } = renderActions()
    await act(async () => hoverRoot(container))
    await waitFor(() => expect(screen.getByTestId('leave-cancel-button')).not.toBeDisabled())
    fireEvent.click(screen.getByTestId('leave-cancel-button'))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^Cancelar novedad$/ }))
    })
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('No se pudo cancelar la novedad'))
  })
})
