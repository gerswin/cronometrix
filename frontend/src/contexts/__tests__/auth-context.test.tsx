import { act, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const authApi = vi.hoisted(() => {
  let token: string | null = null
  const listeners = new Set<() => void>()

  const getAccessToken = vi.fn(() => token)
  const onAccessTokenChange = vi.fn((listener: () => void) => {
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  })
  const refreshAccessToken = vi.fn<() => Promise<string>>()
  const isSessionSupersededError = vi.fn(
    (error: unknown) => error instanceof Error && error.message === 'session superseded'
  )
  const setAccessToken = vi.fn((nextToken: string | null) => {
    token = nextToken
    for (const listener of listeners) listener()
  })

  return {
    getAccessToken,
    onAccessTokenChange,
    refreshAccessToken,
    isSessionSupersededError,
    setAccessToken,
    currentToken: () => token,
    reset: () => {
      token = null
      listeners.clear()
      getAccessToken.mockClear()
      onAccessTokenChange.mockClear()
      refreshAccessToken.mockReset()
      setAccessToken.mockClear()
    },
    seedToken: (nextToken: string | null) => {
      token = nextToken
    },
  }
})

vi.mock('@/lib/api', () => ({
  getAccessToken: authApi.getAccessToken,
  onAccessTokenChange: authApi.onAccessTokenChange,
  refreshAccessToken: authApi.refreshAccessToken,
  isSessionSupersededError: authApi.isSessionSupersededError,
  setAccessToken: authApi.setAccessToken,
}))

import { AuthProvider, useAuth } from '../auth-context'

type TokenClaims = {
  sub: string
  role: 'admin' | 'supervisor' | 'viewer'
  exp: number
  iat: number
  jti: string
  token_type: string
}

function base64Url(value: object) {
  return btoa(JSON.stringify(value))
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
}

function makeToken(overrides: Partial<TokenClaims> = {}) {
  const now = Math.floor(Date.now() / 1000)
  const claims: TokenClaims = {
    sub: 'user-1',
    role: 'admin',
    exp: now + 600,
    iat: now,
    jti: 'jti-1',
    token_type: 'access',
    ...overrides,
  }
  return `${base64Url({ alg: 'HS256', typ: 'JWT' })}.${base64Url(claims)}.signature`
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

function AuthState() {
  const { claims, role, status, sub } = useAuth()
  return (
    <output data-testid="auth-state">
      {JSON.stringify({ jti: claims?.jti ?? null, role, status, sub })}
    </output>
  )
}

function renderProvider() {
  return render(
    <AuthProvider>
      <AuthState />
    </AuthProvider>
  )
}

function expectState(expected: {
  jti: string | null
  role: string | null
  status: string
  sub: string | null
}) {
  expect(screen.getByTestId('auth-state')).toHaveTextContent(JSON.stringify(expected))
}

beforeEach(() => {
  authApi.reset()
  authApi.refreshAccessToken.mockReturnValue(new Promise(() => {}))
})

describe('AuthProvider bootstrap', () => {
  it('starts initializing, subscribes first, refreshes the cookie, and becomes authenticated', async () => {
    const refresh = deferred<string>()
    const refreshedToken = makeToken({ sub: 'refreshed-user', jti: 'refreshed-jti' })
    authApi.refreshAccessToken.mockReturnValueOnce(refresh.promise)

    renderProvider()

    expectState({ jti: null, role: null, status: 'initializing', sub: null })
    expect(authApi.onAccessTokenChange).toHaveBeenCalledOnce()
    expect(authApi.refreshAccessToken).toHaveBeenCalledOnce()
    expect(authApi.onAccessTokenChange.mock.invocationCallOrder[0]).toBeLessThan(
      authApi.refreshAccessToken.mock.invocationCallOrder[0]
    )

    await act(async () => {
      refresh.resolve(refreshedToken)
      await refresh.promise
    })

    await waitFor(() =>
      expectState({
        jti: 'refreshed-jti',
        role: 'admin',
        status: 'authenticated',
        sub: 'refreshed-user',
      })
    )
    expect(authApi.currentToken()).toBe(refreshedToken)
  })

  it('settles anonymous when the httpOnly-cookie refresh fails', async () => {
    const refresh = deferred<string>()
    authApi.refreshAccessToken.mockReturnValueOnce(refresh.promise)

    renderProvider()
    expectState({ jti: null, role: null, status: 'initializing', sub: null })

    await act(async () => {
      refresh.reject(new Error('no cookie'))
      await refresh.promise.catch(() => undefined)
    })

    await waitFor(() =>
      expectState({ jti: null, role: null, status: 'anonymous', sub: null })
    )
    expect(authApi.currentToken()).toBeNull()
  })

  it('skips cookie refresh only for a decodable, unexpired access token', async () => {
    authApi.seedToken(
      makeToken({ sub: 'memory-user', role: 'viewer', jti: 'memory-jti', token_type: 'access' })
    )

    renderProvider()

    await waitFor(() =>
      expectState({
        jti: 'memory-jti',
        role: 'viewer',
        status: 'authenticated',
        sub: 'memory-user',
      })
    )
    expect(authApi.refreshAccessToken).not.toHaveBeenCalled()
  })

  it.each([
    ['malformed', 'not-a-jwt'],
    ['expired', makeToken({ exp: Math.floor(Date.now() / 1000) - 1 })],
    ['wrong-type', makeToken({ token_type: 'refresh' })],
  ])('attempts cookie refresh for a %s memory token', (_label, token) => {
    authApi.seedToken(token)

    renderProvider()

    expect(authApi.refreshAccessToken).toHaveBeenCalledOnce()
    expectState({ jti: null, role: null, status: 'initializing', sub: null })
  })

  it('does not overwrite a login token when an older bootstrap refresh resolves later', async () => {
    const refresh = deferred<string>()
    const loginToken = makeToken({ sub: 'login-user', role: 'supervisor', jti: 'login-jti' })
    const staleBootstrapToken = makeToken({ sub: 'stale-user', jti: 'stale-jti' })
    authApi.refreshAccessToken.mockReturnValueOnce(refresh.promise)
    renderProvider()
    expect(authApi.refreshAccessToken).toHaveBeenCalledOnce()

    act(() => {
      authApi.setAccessToken(loginToken)
    })
    await waitFor(() =>
      expectState({
        jti: 'login-jti',
        role: 'supervisor',
        status: 'authenticated',
        sub: 'login-user',
      })
    )

    authApi.setAccessToken.mockClear()
    await act(async () => {
      refresh.resolve(staleBootstrapToken)
      await refresh.promise
    })

    expect(authApi.setAccessToken).not.toHaveBeenCalled()
    expect(authApi.currentToken()).toBe(loginToken)
    expectState({
      jti: 'login-jti',
      role: 'supervisor',
      status: 'authenticated',
      sub: 'login-user',
    })
  })

  it('ignores a superseded bootstrap failure until the newer login publishes its token', async () => {
    const refresh = deferred<string>()
    authApi.refreshAccessToken.mockReturnValueOnce(refresh.promise)
    renderProvider()

    await act(async () => {
      refresh.reject(new Error('session superseded'))
      await refresh.promise.catch(() => undefined)
    })

    expectState({ jti: null, role: null, status: 'initializing', sub: null })
    expect(authApi.setAccessToken).not.toHaveBeenCalled()

    const loginToken = makeToken({ sub: 'login-user', jti: 'login-jti' })
    act(() => authApi.setAccessToken(loginToken))
    await waitFor(() =>
      expectState({
        jti: 'login-jti',
        role: 'admin',
        status: 'authenticated',
        sub: 'login-user',
      })
    )
  })
})
