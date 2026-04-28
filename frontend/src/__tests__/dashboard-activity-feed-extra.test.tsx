/**
 * Top-level coverage extension that mounts the ActivityFeed component
 * (existing src/__tests__/activity-feed.test.ts covers only the
 * ring-buffer helper).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, act, waitFor } from '@testing-library/react'
import { ActivityFeed } from '../components/dashboard/activity-feed'

vi.mock('next/link', () => ({
  default: ({ href, children, className }: { href: string; children: React.ReactNode; className?: string }) => (
    <a href={href} className={className}>{children}</a>
  ),
}))

const { apiGetMock, useSSEMock } = vi.hoisted(() => ({
  apiGetMock: vi.fn(),
  useSSEMock: vi.fn(),
}))
vi.mock('@/lib/api', () => ({
  api: { get: (...a: unknown[]) => apiGetMock(...a) },
}))
vi.mock('@/hooks/use-sse', () => ({
  useSSE: <T,>(path: string, onMessage: (d: T) => void) => useSSEMock(path, onMessage),
}))

describe('ActivityFeed (component)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useSSEMock.mockReturnValue({ connected: true, reconnecting: false })
    apiGetMock.mockResolvedValue({ data: new Blob(['fake']) })
    globalThis.URL.createObjectURL = vi.fn(() => 'blob:test-feed')
    globalThis.URL.revokeObjectURL = vi.fn()
  })

  it('renders the section header and Ver todo link', () => {
    render(<ActivityFeed />)
    expect(screen.getByText(/Actividad en Vivo/)).toBeTruthy()
    const verTodo = screen.getByText('Ver todo') as HTMLAnchorElement
    expect(verTodo.getAttribute('href')).toContain('/timesheet?from_date=')
  })

  it('empty state shows the Spanish copy', () => {
    render(<ActivityFeed />)
    expect(screen.getByText(/Sin actividad reciente/)).toBeTruthy()
  })

  it('SSE reconnect banner shown when useSSE reports reconnecting=true', () => {
    useSSEMock.mockReturnValue({ connected: false, reconnecting: true })
    render(<ActivityFeed />)
    expect(screen.getByText(/Conexión perdida — reconectando…/)).toBeTruthy()
  })

  it('renders an event row when SSE delivers a message; entry direction shows Entrada chip', async () => {
    let pushMessage: ((m: unknown) => void) | null = null
    useSSEMock.mockImplementation((_path: string, onMessage: (d: unknown) => void) => {
      pushMessage = onMessage
      return { connected: true, reconnecting: false }
    })
    render(<ActivityFeed />)
    expect(pushMessage).toBeTruthy()
    await act(async () => {
      pushMessage!({
        id: 'e1',
        employee_id: 'emp-1',
        employee_name: 'Ana García',
        department: 'Operaciones',
        captured_at: '2026-04-28T08:30:00-04:00',
        direction: 'entry',
        has_photo: false,
      })
    })
    await waitFor(() => expect(screen.getByText('Ana García')).toBeTruthy())
    expect(screen.getByText('Entrada')).toBeTruthy()
    // Empty state copy must NOT be present anymore
    expect(screen.queryByText(/Sin actividad reciente/)).toBeNull()
  })

  it('exit direction renders the Salida chip and falls back to em-dash for missing department', async () => {
    let pushMessage: ((m: unknown) => void) | null = null
    useSSEMock.mockImplementation((_path: string, onMessage: (d: unknown) => void) => {
      pushMessage = onMessage
      return { connected: true, reconnecting: false }
    })
    render(<ActivityFeed />)
    await act(async () => {
      pushMessage!({
        id: 'e2',
        employee_id: 'emp-2',
        employee_name: null,
        department: null,
        captured_at: '2026-04-28T17:00:00-04:00',
        direction: 'exit',
        has_photo: false,
      })
    })
    await waitFor(() => expect(screen.getByText('Salida')).toBeTruthy())
    // Fallback for null employee name
    expect(screen.getByText('Empleado desconocido')).toBeTruthy()
    // Department em-dash separator: appears in 'em-dash · HH:mm'
    expect(screen.getByText(/—/)).toBeTruthy()
  })

  it('photo branch: has_photo=true triggers api.get with /events/:id/photo and responseType blob', async () => {
    let pushMessage: ((m: unknown) => void) | null = null
    useSSEMock.mockImplementation((_path: string, onMessage: (d: unknown) => void) => {
      pushMessage = onMessage
      return { connected: true, reconnecting: false }
    })
    render(<ActivityFeed />)
    await act(async () => {
      pushMessage!({
        id: 'evt-photo',
        employee_id: 'emp-3',
        employee_name: 'Iñaki Núñez',
        department: 'TI',
        captured_at: '2026-04-28T09:00:00-04:00',
        direction: 'entry',
        has_photo: true,
      })
    })
    await waitFor(() =>
      expect(apiGetMock).toHaveBeenCalledWith('/events/evt-photo/photo', { responseType: 'blob' })
    )
  })

  it('photo branch with rejected api.get falls back to initials avatar without throwing', async () => {
    apiGetMock.mockRejectedValueOnce(new Error('401'))
    let pushMessage: ((m: unknown) => void) | null = null
    useSSEMock.mockImplementation((_path: string, onMessage: (d: unknown) => void) => {
      pushMessage = onMessage
      return { connected: true, reconnecting: false }
    })
    render(<ActivityFeed />)
    await act(async () => {
      pushMessage!({
        id: 'evt-photo-fail',
        employee_id: 'emp-4',
        employee_name: 'Maria Lopez',
        department: 'RH',
        captured_at: '2026-04-28T10:00:00-04:00',
        direction: 'entry',
        has_photo: true,
      })
    })
    await waitFor(() => expect(apiGetMock).toHaveBeenCalled())
    // Component still renders without crashing
    expect(screen.getByText('Maria Lopez')).toBeTruthy()
  })
})
