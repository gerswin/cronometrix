'use client'
import { createContext, useContext, useEffect, useState, ReactNode } from 'react'
import { JWTClaims } from '@/types/api'
import { getAccessToken } from '@/lib/api'

function decodeJwtPayload(token: string): JWTClaims | null {
  try {
    const [, payload] = token.split('.')
    return JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
  } catch {
    return null
  }
}

interface AuthContextValue {
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
