'use client'

import { useEffect, useRef, type ReactNode } from 'react'
import { usePathname, useRouter, useSearchParams } from 'next/navigation'
import { useAuth } from '@/contexts/auth-context'

export function SessionGate({ children }: { children: ReactNode }) {
  const { status } = useAuth()
  const pathname = usePathname()
  const router = useRouter()
  const searchParams = useSearchParams()
  const serializedSearch = searchParams.toString()
  const lastRedirect = useRef<string | null>(null)

  useEffect(() => {
    if (status !== 'anonymous') {
      if (status === 'authenticated') lastRedirect.current = null
      return
    }

    const returnTo = `${pathname}${serializedSearch ? `?${serializedSearch}` : ''}`
    const target = `/login?redirect=${encodeURIComponent(returnTo)}`
    if (lastRedirect.current === target) return

    lastRedirect.current = target
    router.replace(target)
  }, [pathname, router, serializedSearch, status])

  if (status === 'initializing') {
    return <div data-testid="session-initializing" />
  }

  if (status === 'anonymous') return null

  return children
}
