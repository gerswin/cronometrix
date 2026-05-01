'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { LogOut } from 'lucide-react'
import { useAuth } from '@/hooks/use-auth'
import { api, setAccessToken } from '@/lib/api'

interface TopBarProps { title: string }

export function TopBar({ title }: TopBarProps) {
  const { role, sub } = useAuth()
  const router = useRouter()
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  // CR-04: role/sub are display hints from an unverified JWT decode. The
  // backend is the authoritative RBAC source; this label is non-load-bearing.

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      // Backend clears the refresh cookie + invalidates the refresh-token hash.
      // Failure is non-fatal: we still clear local state and redirect.
      await api.post('/auth/logout').catch(() => undefined)
    } finally {
      setAccessToken(null)
      router.push('/login')
    }
  }

  return (
    <header className="h-14 border-b border-slate-200 px-6 flex items-center justify-between bg-white">
      <h1 className="text-base font-semibold text-slate-800">{title}</h1>
      <div className="flex items-center gap-3">
        <span className="text-xs text-slate-500 capitalize" title="Local display only — backend enforces role">
          {role} · {sub}
        </span>
        <button
          type="button"
          onClick={handleLogout}
          disabled={isLoggingOut}
          aria-label="Cerrar sesión"
          data-testid="logout-button"
          className="inline-flex items-center gap-1.5 text-xs text-slate-600 hover:text-slate-900 px-2.5 py-1.5 rounded-md border border-slate-200 hover:bg-slate-50 disabled:opacity-50 transition-colors"
        >
          <LogOut size={14} />
          {isLoggingOut ? 'Saliendo…' : 'Salir'}
        </button>
      </div>
    </header>
  )
}
