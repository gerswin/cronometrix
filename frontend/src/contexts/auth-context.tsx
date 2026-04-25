'use client'
import { createContext, useContext, useEffect, useState, ReactNode } from 'react'
import { JWTClaims } from '@/types/api'
import { getAccessToken } from '@/lib/api'

/**
 * CR-04: This decode does NOT verify the JWT signature — it cannot be the
 * authority for any security decision. The backend (`AuthUser` extractor)
 * is the only authoritative RBAC source: every mutation re-validates the
 * signed JWT and rejects requests that lack the required role.
 *
 * The values returned here are display hints only. They are used to:
 *  - hide UI controls the current user cannot use (UX, not security)
 *  - render `role · sub` in the top bar
 *
 * A user who tampers with their JWT in browser memory can flip their role
 * client-side, but every protected endpoint will still reject their forged
 * requests at the network boundary.
 */
function decodeJwtPayload(token: string): JWTClaims | null {
  try {
    const [, payload] = token.split('.')
    return JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
  } catch {
    return null
  }
}

interface AuthContextValue {
  /** Display-hint role from unverified JWT decode. Backend is authoritative. */
  role: 'admin' | 'supervisor' | 'viewer' | null
  sub: string | null
  claims: JWTClaims | null
}

const AuthContext = createContext<AuthContextValue>({ role: null, sub: null, claims: null })

export function AuthProvider({ children }: { children: ReactNode }) {
  const [claims, setClaims] = useState<JWTClaims | null>(null)

  useEffect(() => {
    // Decode token from memory; Providers component sets it before mounting
    const token = getAccessToken()
    if (token) setClaims(decodeJwtPayload(token))
  }, [])

  return (
    <AuthContext.Provider value={{ role: claims?.role ?? null, sub: claims?.sub ?? null, claims }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  return useContext(AuthContext)
}
