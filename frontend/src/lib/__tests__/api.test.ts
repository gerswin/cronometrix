import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  apiRetry: vi.fn(
    (config: { headers: Record<string, string>; url: string; _retry: boolean }) =>
      Promise.resolve({
        data: {
          ok: true,
          retried: config._retry,
          authorization: config.headers.Authorization,
        },
      })
  ),
  axiosCreate: vi.fn(),
  axiosCreateConfigs: [] as unknown[],
  redirectHref: vi.fn(),
  refreshPost: vi.fn(),
  toastError: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: mocks.toastError } }))

vi.mock('axios', () => {
  const requestInterceptors: Array<(config: unknown) => unknown> = []
  const responseInterceptors: Array<{
    fulfilled: (response: unknown) => unknown
    rejected: (error: unknown) => unknown
  }> = []

  const apiClient = Object.assign(
    (config: { headers: Record<string, string>; url: string; _retry: boolean }) =>
      mocks.apiRetry(config),
    {
      interceptors: {
        request: {
          use: (handler: (config: unknown) => unknown) => requestInterceptors.push(handler),
        },
        response: {
          use: (
            fulfilled: (response: unknown) => unknown,
            rejected: (error: unknown) => unknown
          ) => responseInterceptors.push({ fulfilled, rejected }),
        },
      },
      __triggerRequest: (config: unknown) =>
        requestInterceptors.reduce((current, handler) => handler(current), config),
      __triggerResponseError: (error: unknown) => responseInterceptors[0].rejected(error),
    }
  )

  const refreshClient = {
    post: (...args: unknown[]) => mocks.refreshPost(...args),
  }

  let createCount = 0

  return {
    default: {
      create: (...args: unknown[]) => {
        mocks.axiosCreate(...args)
        mocks.axiosCreateConfigs.push(args[0])
        createCount += 1
        return createCount === 1 ? apiClient : refreshClient
      },
      post: (...args: unknown[]) => mocks.refreshPost(...args),
    },
  }
})

type ApiShim = {
  __triggerRequest: (config: unknown) => unknown
  __triggerResponseError: (error: unknown) => Promise<unknown>
}

function unauthorizedError(path: string, retry = false) {
  return {
    response: { status: 401 },
    config: {
      headers: { Authorization: 'Bearer old-token' },
      url: path,
      _retry: retry,
    },
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}

const originalWindow = globalThis.window

beforeEach(async () => {
  vi.clearAllMocks()
  mocks.refreshPost.mockReset()
  vi.useFakeTimers()

  let href = ''
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: {
      location: {
        pathname: '/dashboard',
        search: '?filter=late',
        get href() {
          return href
        },
        set href(value: string) {
          href = value
          mocks.redirectHref(value)
        },
      },
    },
    writable: true,
  })

  const { setAccessToken } = await import('../api')
  setAccessToken('__test-reset__')
  setAccessToken(null)
})

afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: originalWindow,
    writable: true,
  })
})

describe('lib/api access-token state', () => {
  it('round-trips the in-memory access token', async () => {
    const { getAccessToken, setAccessToken } = await import('../api')
    setAccessToken('abc')
    expect(getAccessToken()).toBe('abc')
    setAccessToken(null)
    expect(getAccessToken()).toBeNull()
  })

  it('notifies token listeners until they unsubscribe', async () => {
    const { onAccessTokenChange, setAccessToken } = await import('../api')
    const listener = vi.fn()
    const unsubscribe = onAccessTokenChange(listener)

    setAccessToken('a')
    setAccessToken('b')
    expect(listener).toHaveBeenCalledTimes(2)

    unsubscribe()
    setAccessToken('c')
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it('does not let a failing listener break the token setter', async () => {
    const { onAccessTokenChange, setAccessToken } = await import('../api')
    const goodListener = vi.fn()
    const badListener = vi.fn(() => {
      throw new Error('listener exploded')
    })
    const unsubscribeBad = onAccessTokenChange(badListener)
    const unsubscribeGood = onAccessTokenChange(goodListener)

    expect(() => setAccessToken('x')).not.toThrow()
    expect(goodListener).toHaveBeenCalledOnce()

    unsubscribeBad()
    unsubscribeGood()
  })

  it('attaches the current access token to requests', async () => {
    const { api, setAccessToken } = await import('../api')
    setAccessToken('tok-1')

    const config = (api as unknown as ApiShim).__triggerRequest({ headers: {} }) as {
      headers: Record<string, string>
    }

    expect(config.headers.Authorization).toBe('Bearer tok-1')
  })

  it('leaves Authorization absent when no access token exists', async () => {
    const { api } = await import('../api')

    const config = (api as unknown as ApiShim).__triggerRequest({ headers: {} }) as {
      headers: Record<string, string>
    }

    expect(config.headers.Authorization).toBeUndefined()
  })
})

describe('lib/api single-flight refresh', () => {
  it('shares one refresh across simultaneous 401s and retries both once with the same token', async () => {
    const pendingRefresh = deferred<{ data: { access_token: string } }>()
    mocks.refreshPost.mockReturnValueOnce(pendingRefresh.promise)
    const { api, getAccessToken } = await import('../api')
    const client = api as unknown as ApiShim
    const firstError = unauthorizedError('/employees')
    const secondError = unauthorizedError('/devices')

    const firstRetry = client.__triggerResponseError(firstError)
    const secondRetry = client.__triggerResponseError(secondError)
    const retries = Promise.allSettled([firstRetry, secondRetry])

    pendingRefresh.resolve({ data: { access_token: 'shared-new-token' } })
    const [firstResult, secondResult] = await retries

    expect(mocks.refreshPost).toHaveBeenCalledTimes(1)
    expect(mocks.refreshPost).toHaveBeenCalledWith('/auth/refresh', {})
    expect(mocks.apiRetry).toHaveBeenCalledTimes(2)
    expect(firstError.config._retry).toBe(true)
    expect(secondError.config._retry).toBe(true)
    expect(firstResult).toEqual({
      status: 'fulfilled',
      value: expect.objectContaining({
        data: expect.objectContaining({ authorization: 'Bearer shared-new-token' }),
      }),
    })
    expect(secondResult).toEqual({
      status: 'fulfilled',
      value: expect.objectContaining({
        data: expect.objectContaining({ authorization: 'Bearer shared-new-token' }),
      }),
    })
    expect(getAccessToken()).toBe('shared-new-token')
  })

  it('deduplicates token clearing, the request-time expiry toast, and redirect on shared failure', async () => {
    const pendingRefresh = deferred<never>()
    mocks.refreshPost.mockReturnValueOnce(pendingRefresh.promise)
    const { api, getAccessToken, setAccessToken } = await import('../api')
    const client = api as unknown as ApiShim
    setAccessToken('old-token')
    const firstError = unauthorizedError('/employees')
    const secondError = unauthorizedError('/devices')

    const firstRetry = client.__triggerResponseError(firstError)
    const secondRetry = client.__triggerResponseError(secondError)
    const retries = Promise.allSettled([firstRetry, secondRetry])
    pendingRefresh.reject(new Error('refresh rejected'))

    await expect(retries).resolves.toEqual([
      { status: 'rejected', reason: firstError },
      { status: 'rejected', reason: secondError },
    ])
    expect(mocks.refreshPost).toHaveBeenCalledTimes(1)
    expect(getAccessToken()).toBeNull()
    expect(mocks.toastError).toHaveBeenCalledTimes(1)
    expect(mocks.toastError).toHaveBeenCalledWith(
      'Tu sesión ha expirado',
      expect.objectContaining({ duration: 3000 })
    )

    await vi.advanceTimersByTimeAsync(3000)
    expect(mocks.redirectHref).toHaveBeenCalledOnce()
    expect(mocks.redirectHref).toHaveBeenCalledWith(
      '/login?redirect=%2Fdashboard%3Ffilter%3Dlate'
    )
  })

  it('allows one new expiry notification after a later successful token set', async () => {
    const { api, setAccessToken } = await import('../api')
    const client = api as unknown as ApiShim
    mocks.refreshPost.mockRejectedValueOnce(new Error('first refresh rejected'))

    await expect(client.__triggerResponseError(unauthorizedError('/employees'))).rejects.toBeTruthy()
    await vi.advanceTimersByTimeAsync(3000)
    expect(mocks.toastError).toHaveBeenCalledTimes(1)
    expect(mocks.redirectHref).toHaveBeenCalledTimes(1)

    setAccessToken('new-login-token')
    mocks.refreshPost.mockRejectedValueOnce(new Error('second refresh rejected'))
    await expect(client.__triggerResponseError(unauthorizedError('/devices'))).rejects.toBeTruthy()
    await vi.advanceTimersByTimeAsync(3000)

    expect(mocks.toastError).toHaveBeenCalledTimes(2)
    expect(mocks.redirectHref).toHaveBeenCalledTimes(2)
  })

  it('never refreshes a request already marked _retry=true', async () => {
    const { api } = await import('../api')
    const client = api as unknown as ApiShim
    const error = unauthorizedError('/protected', true)

    await expect(client.__triggerResponseError(error)).rejects.toBe(error)
    expect(mocks.refreshPost).not.toHaveBeenCalled()
    expect(mocks.apiRetry).not.toHaveBeenCalled()
  })

  it('propagates non-401 errors without refreshing', async () => {
    const { api } = await import('../api')
    const client = api as unknown as ApiShim
    const error = {
      response: { status: 500 },
      config: { headers: {}, url: '/health', _retry: false },
    }

    await expect(client.__triggerResponseError(error)).rejects.toBe(error)
    expect(mocks.refreshPost).not.toHaveBeenCalled()
  })

  it('uses a separate credentialed client for refresh requests', async () => {
    await import('../api')

    expect(mocks.axiosCreateConfigs).toHaveLength(2)
    expect(mocks.axiosCreateConfigs[1]).toEqual(
      expect.objectContaining({ withCredentials: true })
    )
  })

  it('keeps direct bootstrap refresh failures silent', async () => {
    mocks.refreshPost.mockRejectedValueOnce(new Error('no refresh cookie'))
    const { refreshAccessToken } = await import('../api')

    await expect(refreshAccessToken()).rejects.toThrow('no refresh cookie')
    await vi.advanceTimersByTimeAsync(3000)

    expect(mocks.toastError).not.toHaveBeenCalled()
    expect(mocks.redirectHref).not.toHaveBeenCalled()
  })
})

describe('lib/api query client', () => {
  it('exports the expected retry and stale-time defaults', async () => {
    const { queryClient } = await import('../api')
    const options = queryClient.getDefaultOptions()

    expect(options.queries?.retry).toBe(1)
    expect(options.queries?.staleTime).toBe(5 * 60 * 1000)
  })
})
