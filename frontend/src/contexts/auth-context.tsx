'use client'

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import {
  getAccessToken,
  isSessionSupersededError,
  onAccessTokenChange,
  refreshAccessToken,
  setAccessToken,
} from '@/lib/api'
import type { JWTClaims } from '@/types/api'

export type AuthStatus = 'initializing' | 'authenticated' | 'anonymous'

export interface AuthContextValue {
  status: AuthStatus
  /** Display-hint role from unverified JWT decode. Backend is authoritative. */
  role: 'admin' | 'supervisor' | 'viewer' | null
  sub: string | null
  claims: JWTClaims | null
}

const AuthContext = createContext<AuthContextValue>({
  status: 'initializing',
  role: null,
  sub: null,
  claims: null,
})

function isJwtClaims(value: unknown): value is JWTClaims {
  if (!value || typeof value !== 'object') return false

  const claims = value as Record<string, unknown>
  return (
    typeof claims.sub === 'string' &&
    (claims.role === 'admin' || claims.role === 'supervisor' || claims.role === 'viewer') &&
    typeof claims.exp === 'number' &&
    Number.isFinite(claims.exp) &&
    typeof claims.iat === 'number' &&
    Number.isFinite(claims.iat) &&
    typeof claims.jti === 'string' &&
    (claims.token_type === 'access' || claims.token_type === 'refresh')
  )
}

/**
 * CR-04: This decode does NOT verify the JWT signature — it cannot be the
 * authority for any security decision. The backend (`AuthUser` extractor)
 * remains the authoritative RBAC boundary for every protected request.
 */
function decodeJwtPayload(token: string): JWTClaims | null {
  try {
    const parts = token.split('.')
    if (parts.length !== 3 || !parts[1]) return null

    const base64 = parts[1].replace(/-/g, '+').replace(/_/g, '/')
    const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, '=')
    const decoded: unknown = JSON.parse(atob(padded))
    return isJwtClaims(decoded) ? decoded : null
  } catch {
    return null
  }
}

function usableAccessClaims(token: string | null): JWTClaims | null {
  if (!token) return null

  const claims = decodeJwtPayload(token)
  if (!claims || claims.token_type !== 'access') return null
  if (claims.exp <= Math.floor(Date.now() / 1000)) return null
  return claims
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus>('initializing')
  const [claims, setClaims] = useState<JWTClaims | null>(null)
  const tokenGeneration = useRef(0)

  useEffect(() => {
    let active = true

    const syncFromMemory = () => {
      tokenGeneration.current += 1
      if (!active) return

      const nextClaims = usableAccessClaims(getAccessToken())
      setClaims(nextClaims)
      setStatus(nextClaims ? 'authenticated' : 'anonymous')
    }

    // Subscribe before reading memory or starting refresh so a login/logout
    // between those operations cannot be lost.
    const unsubscribe = onAccessTokenChange(syncFromMemory)
    const bootstrapGeneration = tokenGeneration.current
    const bootstrapToken = getAccessToken()
    const memoryClaims = usableAccessClaims(bootstrapToken)

    if (memoryClaims) {
      syncFromMemory()
    } else {
      void refreshAccessToken()
        .then((token) => {
          if (
            !active ||
            tokenGeneration.current !== bootstrapGeneration ||
            getAccessToken() !== bootstrapToken
          ) {
            return
          }

          if (!usableAccessClaims(token)) {
            setAccessToken(null)
            return
          }

          setAccessToken(token)
        })
        .catch((error) => {
          if (isSessionSupersededError(error)) return
          if (
            !active ||
            tokenGeneration.current !== bootstrapGeneration ||
            getAccessToken() !== bootstrapToken
          ) {
            return
          }

          // Bootstrap failures are deliberately silent. The gate will route
          // the now-anonymous session without showing an expiry toast.
          setAccessToken(null)
        })
    }

    return () => {
      active = false
      unsubscribe()
    }
  }, [])

  return (
    <AuthContext.Provider
      value={{
        status,
        role: claims?.role ?? null,
        sub: claims?.sub ?? null,
        claims,
      }}
    >
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  return useContext(AuthContext)
}
