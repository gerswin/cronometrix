'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import {
  LayoutDashboard, Clock, Users, Cpu, UserCheck, BarChart2, ShieldCheck, Settings
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuth } from '@/hooks/use-auth'

type Role = 'admin' | 'supervisor' | 'viewer'

interface NavItem {
  href: string
  icon: typeof LayoutDashboard
  label: string
  roles?: Role[]
}

const NAV_ITEMS: NavItem[] = [
  { href: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { href: '/timesheet', icon: Clock, label: 'Marcaciones' },
  { href: '/employees', icon: Users, label: 'Empleados' },
  { href: '/devices', icon: Cpu, label: 'Dispositivos' },
  { href: '/enrollment', icon: UserCheck, label: 'Enrolamiento' },
  { href: '/reports', icon: BarChart2, label: 'Reportes' },
  { href: '/audit', icon: ShieldCheck, label: 'Auditoría' },
  { href: '/settings/tenant-info', icon: Settings, label: 'Configuración', roles: ['admin'] },
]

export function Sidebar() {
  const pathname = usePathname()
  const { role } = useAuth()
  // Filter role-gated entries. Items without `roles` are always visible.
  // Backend remains the authoritative RBAC source — this only hides
  // controls the user cannot exercise (CR-04).
  const visibleItems = NAV_ITEMS.filter(
    (item) => !item.roles || (role && item.roles.includes(role as Role))
  )
  return (
    <aside className="w-60 min-h-screen bg-slate-900 text-slate-100 flex flex-col shrink-0">
      <div className="px-6 py-5 border-b border-slate-700">
        <span className="text-lg font-semibold tracking-tight">Cronometrix</span>
      </div>
      <nav className="flex-1 py-4 space-y-1 px-3">
        {visibleItems.map(({ href, icon: Icon, label }) => {
          // WR-07: exact match for the leaf path, prefix match only for sub-routes
          // (e.g. `/timesheet/edit/123` should still highlight Marcaciones, but
          // a future `/reports-archive` must not light up `/reports`).
          const isActive =
            href === '/'
              ? pathname === '/'
              : pathname === href || pathname.startsWith(href + '/')
          return (
            <Link
              key={href}
              href={href}
              className={cn(
                'flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors',
                isActive
                  ? 'bg-slate-700 text-white'
                  : 'text-slate-400 hover:bg-slate-800 hover:text-slate-100'
              )}
            >
              <Icon size={16} />
              {label}
            </Link>
          )
        })}
      </nav>
    </aside>
  )
}
