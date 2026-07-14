import { QueryClient } from '@tanstack/react-query'
import axios from 'axios'
import { toast } from 'sonner'

// `undefined` (var not set) → dev fallback localhost.
// `""` (empty string) → relative URL, resolves against the page origin so a
// reverse proxy (Caddy / Cloudflare tunnel) can serve API + frontend on the
// same host. Distinct from `||` so an explicit empty string is honored.
const RAW_API_BASE = process.env.NEXT_PUBLIC_API_URL
export const API_BASE =
  RAW_API_BASE === undefined ? 'http://localhost:3001' : RAW_API_BASE

export const api = axios.create({
  baseURL: `${API_BASE}/api/v1`,
  withCredentials: true, // Send cookies for refresh token (SameSite=Lax allows this)
})

// Refresh must not pass through `api`'s response interceptor: a rejected
// refresh is terminal for that attempt and must never recursively refresh.
const refreshClient = axios.create({
  baseURL: `${API_BASE}/api/v1`,
  withCredentials: true,
})

// Attach access token from memory
let accessToken: string | null = null
let requestTimeExpiryHandled = false
let requestTimeExpiryRedirectTimer: ReturnType<typeof setTimeout> | null = null
let refreshPromise: Promise<string> | null = null
let sessionGeneration = 0
let loginGeneration: number | null = null

class SessionSupersededError extends Error {
  constructor() {
    super('session superseded')
    this.name = 'SessionSupersededError'
  }
}

export function isSessionSupersededError(error: unknown): boolean {
  return error instanceof SessionSupersededError
}

// WR-05: pub-sub for token changes so AuthContext can re-decode claims after
// login / refresh / logout instead of going stale until the next page reload.
type Listener = () => void
const tokenListeners = new Set<Listener>()

export function setAccessToken(token: string | null) {
  accessToken = token
  if (token !== null) {
    requestTimeExpiryHandled = false
    if (requestTimeExpiryRedirectTimer !== null) {
      clearTimeout(requestTimeExpiryRedirectTimer)
      requestTimeExpiryRedirectTimer = null
    }
  }
  for (const listener of tokenListeners) {
    try { listener() } catch { /* listener errors must not break the setter */ }
  }
}

export function getAccessToken(): string | null {
  return accessToken
}

/**
 * Subscribe to access-token changes. Returns an unsubscribe function.
 * Used by `AuthProvider` to keep decoded JWT claims in sync.
 */
export function onAccessTokenChange(listener: Listener): () => void {
  tokenListeners.add(listener)
  return () => { tokenListeners.delete(listener) }
}

/**
 * Exchange the httpOnly refresh cookie for one access token per JS realm.
 * This function intentionally does not mutate the in-memory token: callers
 * decide whether the asynchronous result is still current before applying it.
 */
export function refreshAccessToken(): Promise<string> {
  if (loginGeneration !== null) {
    return Promise.reject(new SessionSupersededError())
  }
  if (!refreshPromise) {
    const refreshGeneration = sessionGeneration
    refreshPromise = refreshClient
      .post<{ access_token: string }>('/auth/refresh', {})
      .then(
        ({ data }) => {
          if (refreshGeneration !== sessionGeneration) {
            throw new SessionSupersededError()
          }
          return data.access_token
        },
        (error: unknown) => {
          if (refreshGeneration !== sessionGeneration) {
            throw new SessionSupersededError()
          }
          throw error
        },
      )
      .finally(() => {
        refreshPromise = null
      })
  }

  return refreshPromise
}

type LoginResult = {
  access_token: string
  user: { id: string; username?: string; full_name?: string; role?: string }
}

/**
 * Serialize an interactive login behind any refresh response already in flight.
 * Browsers apply httpOnly Set-Cookie headers before JavaScript sees a response,
 * so generation checks alone cannot protect cookie ordering: the login request
 * itself must be sent after the older refresh has settled.
 */
export async function loginWithCredentials(
  username: string,
  password: string,
): Promise<LoginResult> {
  const generation = ++sessionGeneration
  loginGeneration = generation
  const pendingRefresh = refreshPromise

  try {
    await pendingRefresh?.catch(() => undefined)
    const { data } = await axios.post<LoginResult>(
      `${API_BASE}/api/v1/auth/login`,
      { username, password },
      { withCredentials: true },
    )
    if (generation !== sessionGeneration) {
      throw new Error('login attempt superseded')
    }
    setAccessToken(data.access_token)
    return data
  } catch (error) {
    if (generation === sessionGeneration) setAccessToken(null)
    throw error
  } finally {
    if (loginGeneration === generation) loginGeneration = null
  }
}

api.interceptors.request.use((config) => {
  if (accessToken) {
    config.headers.Authorization = `Bearer ${accessToken}`
  }
  return config
})

function handleRequestTimeExpiry() {
  if (requestTimeExpiryHandled) return
  requestTimeExpiryHandled = true
  setAccessToken(null)

  if (typeof window === 'undefined') return

  toast.error('Tu sesión ha expirado', { duration: 3000 })
  const redirect = `${window.location.pathname}${window.location.search}`
  requestTimeExpiryRedirectTimer = setTimeout(() => {
    requestTimeExpiryRedirectTimer = null
    // SessionGate normally navigates first. Keep this fallback for requests
    // made outside the protected tree without causing a second navigation.
    if (window.location.pathname !== '/login') {
      window.location.href = `/login?redirect=${encodeURIComponent(redirect)}`
    }
  }, 3000)
}

// Auto-refresh on 401
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    if (error.response?.status === 401 && !error.config._retry) {
      if (loginGeneration !== null) return Promise.reject(error)
      error.config._retry = true
      const refreshGeneration = sessionGeneration
      let token: string
      try {
        token = await refreshAccessToken()
      } catch {
        if (refreshGeneration === sessionGeneration && loginGeneration === null) {
          handleRequestTimeExpiry()
        }
        return Promise.reject(error)
      }
      if (refreshGeneration !== sessionGeneration || loginGeneration !== null) {
        return Promise.reject(error)
      }
      setAccessToken(token)
      error.config.headers.Authorization = `Bearer ${token}`
      return api(error.config)
    }
    return Promise.reject(error)
  }
)

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5 * 60 * 1000 },
  },
})
