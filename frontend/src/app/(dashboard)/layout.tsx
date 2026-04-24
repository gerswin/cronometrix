import { AuthProvider } from '@/contexts/auth-context'
import { Sidebar } from '@/components/layout/sidebar'
import { Toaster } from 'sonner'

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <AuthProvider>
      <div className="flex min-h-screen min-w-[1280px] bg-slate-50">
        <Sidebar />
        <main className="flex-1 overflow-auto">{children}</main>
      </div>
      <Toaster position="top-right" />
    </AuthProvider>
  )
}
