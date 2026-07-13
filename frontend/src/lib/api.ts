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
let refreshPromise: Promise<string> | null = null

// WR-05: pub-sub for token changes so AuthContext can re-decode claims after
// login / refresh / logout instead of going stale until the next page reload.
type Listener = () => void
const tokenListeners = new Set<Listener>()

export function setAccessToken(token: string | null) {
  accessToken = token
  if (token !== null) {
    requestTimeExpiryHandled = false
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
  if (!refreshPromise) {
    refreshPromise = refreshClient
      .post<{ access_token: string }>('/auth/refresh', {})
      .then(({ data }) => data.access_token)
      .finally(() => {
        refreshPromise = null
      })
  }

  return refreshPromise
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
  setTimeout(() => {
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
      error.config._retry = true
      try {
        const token = await refreshAccessToken()
        setAccessToken(token)
        error.config.headers.Authorization = `Bearer ${token}`
        return api(error.config)
      } catch {
        handleRequestTimeExpiry()
      }
    }
    return Promise.reject(error)
  }
)

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5 * 60 * 1000 },
  },
})
