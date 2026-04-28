/**
 * Coverage for the useSSE hook (src/hooks/use-sse.ts).
 *
 * Strategy: stub global EventSource with a controllable fake so we can
 * deterministically drive open / message / error events. This is faster
 * and more reliable than msw's EventSource shim, which doesn't simulate
 * the auto-reconnect closure path of an `EventSource` instance.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useSSE } from '../use-sse'

const { setAccessTokenMock, getAccessTokenMock } = vi.hoisted(() => ({
  setAccessTokenMock: vi.fn(),
  getAccessTokenMock: vi.fn(),
}))
vi.mock('@/lib/api', () => ({
  setAccessToken: (...a: unknown[]) => setAccessTokenMock(...a),
  getAccessToken: () => getAccessTokenMock(),
}))

interface FakeESInstance {
  url: string
  closed: boolean
  onopen: (() => void) | null
  onmessage: ((e: { data: string }) => void) | null
  onerror: (() => void) | null
  close(): void
}

let createdInstances: FakeESInstance[] = []

class FakeEventSource implements FakeESInstance {
  url: string
  closed = false
  onopen: (() => void) | null = null
  onmessage: ((e: { data: string }) => void) | null = null
  onerror: (() => void) | null = null
  constructor(url: string) {
    this.url = url
    createdInstances.push(this)
  }
  close() { this.closed = true }
}

beforeEach(() => {
  vi.clearAllMocks()
  createdInstances = []
  ;(globalThis as unknown as { EventSource: typeof FakeEventSource }).EventSource = FakeEventSource
  getAccessTokenMock.mockReturnValue('tok-A')
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useSSE', () => {
  it('does not open a connection when token is null (logout race)', () => {
    getAccessTokenMock.mockReturnValueOnce(null)
    const onMessage = vi.fn()
    const { result } = renderHook(() => useSSE<{ x: number }>('/events/stream', onMessage))
    expect(createdInstances).toHaveLength(0)
    expect(result.current.connected).toBe(false)
    expect(result.current.reconnecting).toBe(false)
  })

  it('opens an EventSource with the token in the URL when token is present', () => {
    const onMessage = vi.fn()
    renderHook(() => useSSE('/events/stream', onMessage))
    expect(createdInstances).toHaveLength(1)
    expect(createdInstances[0].url).toContain('/events/stream')
    expect(createdInstances[0].url).toContain('token=tok-A')
  })

  it('on open: sets connected=true and clears reconnecting', () => {
    const onMessage = vi.fn()
    const { result } = renderHook(() => useSSE('/events/stream', onMessage))
    act(() => { createdInstances[0].onopen?.() })
    expect(result.current.connected).toBe(true)
    expect(result.current.reconnecting).toBe(false)
  })

  it('parses a JSON message and forwards the payload to onMessage', () => {
    const onMessage = vi.fn()
    renderHook(() => useSSE<{ id: string }>('/events/stream', onMessage))
    act(() => {
      createdInstances[0].onopen?.()
      createdInstances[0].onmessage?.({ data: JSON.stringify({ id: 'x' }) })
    })
    expect(onMessage).toHaveBeenCalledWith({ id: 'x' })
  })

  it('malformed message JSON is silently skipped (no exception bubbles)', () => {
    const onMessage = vi.fn()
    renderHook(() => useSSE('/events/stream', onMessage))
    act(() => {
      createdInstances[0].onmessage?.({ data: 'not-json' })
    })
    expect(onMessage).not.toHaveBeenCalled()
  })

  it('error: closes ES, sets reconnecting=true, schedules a retry; subsequent connect re-uses fresh token', () => {
    vi.useFakeTimers()
    getAccessTokenMock.mockReturnValueOnce('tok-A').mockReturnValueOnce('tok-B')
    const onMessage = vi.fn()
    const { result } = renderHook(() => useSSE('/events/stream', onMessage))
    expect(createdInstances).toHaveLength(1)

    act(() => { createdInstances[0].onerror?.() })
    expect(createdInstances[0].closed).toBe(true)
    expect(result.current.reconnecting).toBe(true)

    // First backoff is 1000ms — advance and verify a fresh ES was opened
    act(() => { vi.advanceTimersByTime(1000) })
    expect(createdInstances).toHaveLength(2)
    expect(createdInstances[1].url).toContain('token=tok-B')
  })

  it('repeated errors apply progressive backoff (1s, 2s, 4s, 8s, 30s) and cap at 30s', () => {
    vi.useFakeTimers()
    const onMessage = vi.fn()
    renderHook(() => useSSE('/events/stream', onMessage))
    const expected = [1000, 2000, 4000, 8000, 30000]
    for (let i = 0; i < expected.length; i++) {
      act(() => { createdInstances[i].onerror?.() })
      act(() => { vi.advanceTimersByTime(expected[i]) })
      expect(createdInstances).toHaveLength(i + 2)
    }
    // 6th error: backoff is still 30s (capped)
    act(() => { createdInstances[expected.length].onerror?.() })
    act(() => { vi.advanceTimersByTime(30000) })
    expect(createdInstances).toHaveLength(expected.length + 2)
  })

  it('cleanup on unmount: closes the active ES and clears the pending reconnect timer', () => {
    vi.useFakeTimers()
    const onMessage = vi.fn()
    const { unmount } = renderHook(() => useSSE('/events/stream', onMessage))
    act(() => { createdInstances[0].onerror?.() })
    // pending reconnect scheduled
    unmount()
    expect(createdInstances[0].closed).toBe(true)
    // advance past the backoff — should NOT create a new ES because the timer was cleared
    act(() => { vi.advanceTimersByTime(60000) })
    expect(createdInstances).toHaveLength(1)
  })
})
