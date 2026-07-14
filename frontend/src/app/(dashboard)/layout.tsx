import { Suspense } from 'react'
import { AuthProvider } from '@/contexts/auth-context'
import { SessionGate } from '@/components/auth/session-gate'
import { Sidebar } from '@/components/layout/sidebar'
import { Toaster } from 'sonner'

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <AuthProvider>
      <Suspense fallback={<div data-testid="session-initializing" />}>
        <SessionGate>
          <div className="flex min-h-screen min-w-[1280px] bg-slate-50">
            <Sidebar />
            <main className="flex-1 overflow-auto">{children}</main>
          </div>
        </SessionGate>
      </Suspense>
      <Toaster position="top-right" />
    </AuthProvider>
  )
}
