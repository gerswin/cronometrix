/**
 * Coverage for src/lib/api.ts:
 *  - setAccessToken / getAccessToken / onAccessTokenChange (pub-sub)
 *  - request interceptor: attaches Bearer when token present, omits otherwise
 *  - response interceptor: 401 → POST /auth/refresh → retry with new token
 *  - 401 + refresh failure → toast + setAccessToken(null) + redirect schedule
 *  - non-401 error pass-through
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

const { toastError, axiosPostMock } = vi.hoisted(() => ({
  toastError: vi.fn(),
  axiosPostMock: vi.fn(),
}))
vi.mock('sonner', () => ({ toast: { error: toastError } }))

// Mock the axios module so we can:
//  - construct the api instance via api.create() returning a fresh shim
//  - intercept axios.post (the standalone one used for /auth/refresh)
vi.mock('axios', () => {
  const requestInterceptors: Array<(c: unknown) => unknown> = []
  const responseInterceptors: Array<{
    fulfilled: (r: unknown) => unknown
    rejected: (e: unknown) => unknown
  }> = []
  const apiShim = {
    interceptors: {
      request: { use: (fn: (c: unknown) => unknown) => requestInterceptors.push(fn) },
      response: {
        use: (fulfilled: (r: unknown) => unknown, rejected: (e: unknown) => unknown) =>
          responseInterceptors.push({ fulfilled, rejected }),
      },
    },
    // Expose for tests
    __triggerRequest: (config: unknown) => {
      let cur = config
      for (const fn of requestInterceptors) cur = fn(cur)
      return cur
    },
    __triggerResponseError: (err: unknown) => {
      const i = responseInterceptors[0]
      return i.rejected(err)
    },
  }
  // Make api(error.config) callable so the retry path can call into us
  const apiCallable = (
    cfg: { headers: Record<string, string>; url: string; _retry: boolean }
  ) => Promise.resolve({ data: { ok: true, retried: cfg._retry, hdr: cfg.headers.Authorization } })
  Object.assign(apiCallable, apiShim)
  return {
    default: {
      create: () => apiCallable,
      post: (...a: unknown[]) => axiosPostMock(...a),
    },
  }
})

// Stub window.location for the refresh-fail redirect path. The api module
// reads `window.location.pathname` and assigns to `window.location.href`.
beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  Object.defineProperty(globalThis, 'window', {
    value: {
      location: { pathname: '/dashboard', href: '' },
    },
    writable: true,
    configurable: true,
  })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('lib/api', () => {
  it('setAccessToken + getAccessToken round-trip', async () => {
    const mod = await import('../api')
    mod.setAccessToken('abc')
    expect(mod.getAccessToken()).toBe('abc')
    mod.setAccessToken(null)
    expect(mod.getAccessToken()).toBeNull()
  })

  it('onAccessTokenChange listener fires on every set; unsubscribe stops further notifications', async () => {
    const mod = await import('../api')
    const fn = vi.fn()
    const unsubscribe = mod.onAccessTokenChange(fn)
    mod.setAccessToken('a')
    mod.setAccessToken('b')
    expect(fn).toHaveBeenCalledTimes(2)
    unsubscribe()
    mod.setAccessToken('c')
    expect(fn).toHaveBeenCalledTimes(2)
  })

  it('listener errors do not break the setter', async () => {
    const mod = await import('../api')
    const goodListener = vi.fn()
    const badListener = vi.fn(() => { throw new Error('listener exploded') })
    mod.onAccessTokenChange(badListener)
    mod.onAccessTokenChange(goodListener)
    expect(() => mod.setAccessToken('x')).not.toThrow()
    expect(goodListener).toHaveBeenCalled()
  })

  it('request interceptor attaches Bearer header when token present', async () => {
    const mod = await import('../api')
    mod.setAccessToken('tok-1')
    type ApiShim = {
      __triggerRequest: (cfg: unknown) => unknown
    }
    const apiAny = mod.api as unknown as ApiShim
    const cfg = apiAny.__triggerRequest({ headers: {} })
    expect((cfg as { headers: Record<string, string> }).headers.Authorization).toBe('Bearer tok-1')
  })

  it('request interceptor leaves Authorization absent when token is null', async () => {
    const mod = await import('../api')
    mod.setAccessToken(null)
    type ApiShim = { __triggerRequest: (cfg: unknown) => unknown }
    const apiAny = mod.api as unknown as ApiShim
    const cfg = apiAny.__triggerRequest({ headers: {} }) as { headers: Record<string, string> }
    expect(cfg.headers.Authorization).toBeUndefined()
  })

  it('401 → /auth/refresh succeeds → request retried with new Bearer token', async () => {
    axiosPostMock.mockResolvedValueOnce({ data: { access_token: 'new-tok' } })
    const mod = await import('../api')
    type ApiShim = { __triggerResponseError: (err: unknown) => Promise<unknown> }
    const apiAny = mod.api as unknown as ApiShim
    const errorObj = {
      response: { status: 401 },
      config: { headers: { Authorization: 'Bearer old-tok' }, url: '/protected', _retry: false },
    }
    const result = await apiAny.__triggerResponseError(errorObj)
    expect(axiosPostMock).toHaveBeenCalled()
    expect((result as { data: { retried: boolean } }).data.retried).toBe(true)
    // The retry-config Authorization header was rewritten to the new token
    expect((errorObj.config.headers as Record<string, string>).Authorization).toBe('Bearer new-tok')
    // Token was stored
    expect(mod.getAccessToken()).toBe('new-tok')
  })

  it('401 + refresh fails → toast.error + setAccessToken(null) + redirect scheduled', async () => {
    axiosPostMock.mockRejectedValueOnce(new Error('refresh dead'))
    const mod = await import('../api')
    mod.setAccessToken('still-here')
    type ApiShim = { __triggerResponseError: (err: unknown) => Promise<unknown> }
    const apiAny = mod.api as unknown as ApiShim
    const errorObj = {
      response: { status: 401 },
      config: { headers: { Authorization: 'Bearer x' }, url: '/protected', _retry: false },
    }
    await expect(apiAny.__triggerResponseError(errorObj)).rejects.toBe(errorObj)
    expect(mod.getAccessToken()).toBeNull()
    expect(toastError).toHaveBeenCalledWith(
      'Tu sesión ha expirado',
      expect.objectContaining({ duration: 3000 })
    )
    // Advance past the 3s redirect timer
    vi.advanceTimersByTime(3500)
    type WindowShim = { location: { href: string; pathname: string } }
    const w = globalThis.window as unknown as WindowShim
    expect(w.location.href).toBe('/login?redirect=%2Fdashboard')
  })

  it('401 with _retry already true does NOT trigger a second refresh', async () => {
    const mod = await import('../api')
    type ApiShim = { __triggerResponseError: (err: unknown) => Promise<unknown> }
    const apiAny = mod.api as unknown as ApiShim
    const errorObj = {
      response: { status: 401 },
      config: { headers: { Authorization: 'Bearer x' }, url: '/protected', _retry: true },
    }
    await expect(apiAny.__triggerResponseError(errorObj)).rejects.toBe(errorObj)
    expect(axiosPostMock).not.toHaveBeenCalled()
  })

  it('non-401 errors (e.g. 500) bypass the refresh path and propagate', async () => {
    const mod = await import('../api')
    type ApiShim = { __triggerResponseError: (err: unknown) => Promise<unknown> }
    const apiAny = mod.api as unknown as ApiShim
    const errorObj = {
      response: { status: 500 },
      config: { headers: {}, url: '/x', _retry: false },
    }
    await expect(apiAny.__triggerResponseError(errorObj)).rejects.toBe(errorObj)
    expect(axiosPostMock).not.toHaveBeenCalled()
  })

  it('queryClient is exported with sane retry+staleTime defaults', async () => {
    const mod = await import('../api')
    const opts = mod.queryClient.getDefaultOptions()
    expect(opts.queries?.retry).toBe(1)
    expect(opts.queries?.staleTime).toBe(5 * 60 * 1000)
  })
})
