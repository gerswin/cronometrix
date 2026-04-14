import { QueryClient } from '@tanstack/react-query'
import axios from 'axios'

export const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'

export const api = axios.create({
  baseURL: `${API_BASE}/api/v1`,
  withCredentials: true, // Send cookies for refresh token (SameSite=Lax allows this)
})

// Attach access token from memory
let accessToken: string | null = null

export function setAccessToken(token: string | null) {
  accessToken = token
}

export function getAccessToken(): string | null {
  return accessToken
}

api.interceptors.request.use((config) => {
  if (accessToken) {
    config.headers.Authorization = `Bearer ${accessToken}`
  }
  return config
})

// Auto-refresh on 401
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    if (error.response?.status === 401 && !error.config._retry) {
      error.config._retry = true
      try {
        const { data } = await axios.post(
          `${API_BASE}/api/v1/auth/refresh`,
          {},
          { withCredentials: true }
        )
        setAccessToken(data.access_token)
        error.config.headers.Authorization = `Bearer ${data.access_token}`
        return api(error.config)
      } catch {
        setAccessToken(null)
        if (typeof window !== 'undefined') {
          window.location.href = '/login'
        }
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
