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

const { apiBaseState, getAccessTokenMock, onAccessTokenChangeMock } = vi.hoisted(() => ({
  apiBaseState: { value: 'https://api.example.test' },
  getAccessTokenMock: vi.fn(),
  onAccessTokenChangeMock: vi.fn(),
}))
vi.mock('@/lib/api', () => ({
  get API_BASE() { return apiBaseState.value },
  getAccessToken: () => getAccessTokenMock(),
  onAccessTokenChange: (listener: () => void) => onAccessTokenChangeMock(listener),
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
let tokenListener: (() => void) | null = null
let unsubscribeMock: ReturnType<typeof vi.fn>

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
  apiBaseState.value = 'https://api.example.test'
  createdInstances = []
  tokenListener = null
  unsubscribeMock = vi.fn()
  ;(globalThis as unknown as { EventSource: typeof FakeEventSource }).EventSource = FakeEventSource
  getAccessTokenMock.mockReturnValue('tok-A')
  onAccessTokenChangeMock.mockImplementation((listener: () => void) => {
    tokenListener = listener
    return unsubscribeMock
  })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useSSE', () => {
  it('does not open a connection when token is null (logout race)', () => {
    getAccessTokenMock.mockReturnValue(null)
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
    expect(createdInstances[0].url).toBe(
      'https://api.example.test/api/v1/events/stream?token=tok-A',
    )
    expect(onAccessTokenChangeMock.mock.invocationCallOrder[0]).toBeLessThan(
      getAccessTokenMock.mock.invocationCallOrder[0],
    )
  })

  it('uses a relative same-origin URL when the public API base is explicitly empty', () => {
    apiBaseState.value = ''
    getAccessTokenMock.mockReturnValue('test-token')

    renderHook(() => useSSE('/events/stream', vi.fn()))

    expect(createdInstances).toHaveLength(1)
    expect(createdInstances[0].url).toBe(
      '/api/v1/events/stream?token=test-token',
    )
  })

  it('connects when the token transitions from null to A', () => {
    getAccessTokenMock.mockReturnValue(null)
    renderHook(() => useSSE('/events/stream', vi.fn()))
    expect(createdInstances).toHaveLength(0)

    getAccessTokenMock.mockReturnValue('tok-A')
    act(() => tokenListener?.())

    expect(createdInstances).toHaveLength(1)
    expect(createdInstances[0].url).toContain('token=tok-A')
  })

  it('replaces A with B and ignores stale callbacks from A', () => {
    vi.useFakeTimers()
    const onMessage = vi.fn()
    const { result } = renderHook(() => useSSE('/events/stream', onMessage))
    const sourceA = createdInstances[0]

    act(() => sourceA.onerror?.())

    getAccessTokenMock.mockReturnValue('tok-B')
    act(() => tokenListener?.())

    expect(sourceA.closed).toBe(true)
    expect(createdInstances).toHaveLength(2)
    expect(createdInstances[1].url).toContain('token=tok-B')

    act(() => {
      sourceA.onopen?.()
      sourceA.onmessage?.({ data: JSON.stringify({ stale: true }) })
      sourceA.onerror?.()
    })
    expect(result.current.connected).toBe(false)
    expect(result.current.reconnecting).toBe(false)
    expect(onMessage).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(60_000))
    expect(createdInstances).toHaveLength(2)
  })

  it('closes and cancels retry when token transitions from A to null', () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useSSE('/events/stream', vi.fn()))
    act(() => createdInstances[0].onerror?.())
    expect(result.current.reconnecting).toBe(true)

    getAccessTokenMock.mockReturnValue(null)
    act(() => tokenListener?.())
    expect(createdInstances[0].closed).toBe(true)
    expect(result.current.connected).toBe(false)
    expect(result.current.reconnecting).toBe(false)

    act(() => vi.advanceTimersByTime(60_000))
    expect(createdInstances).toHaveLength(1)
  })

  it('treats repeated notification for the same token as a no-op', () => {
    renderHook(() => useSSE('/events/stream', vi.fn()))
    act(() => tokenListener?.())
    expect(createdInstances).toHaveLength(1)
    expect(createdInstances[0].closed).toBe(false)
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
    getAccessTokenMock.mockReturnValue('tok-A')
    const onMessage = vi.fn()
    const { result } = renderHook(() => useSSE('/events/stream', onMessage))
    expect(createdInstances).toHaveLength(1)

    act(() => { createdInstances[0].onerror?.() })
    expect(createdInstances[0].closed).toBe(true)
    expect(result.current.reconnecting).toBe(true)

    // First backoff is 1000ms — advance and verify the same active token is used.
    act(() => { vi.advanceTimersByTime(1000) })
    expect(createdInstances).toHaveLength(2)
    expect(createdInstances[1].url).toContain('token=tok-A')
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
    expect(unsubscribeMock).toHaveBeenCalledOnce()
    // advance past the backoff — should NOT create a new ES because the timer was cleared
    act(() => { vi.advanceTimersByTime(60000) })
    expect(createdInstances).toHaveLength(1)
  })
})
