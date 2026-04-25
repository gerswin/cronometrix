'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import {
  LayoutDashboard, Clock, Users, Cpu, UserCheck, BarChart2, ShieldCheck
} from 'lucide-react'
import { cn } from '@/lib/utils'

const NAV_ITEMS = [
  { href: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { href: '/timesheet', icon: Clock, label: 'Marcaciones' },
  { href: '/employees', icon: Users, label: 'Empleados' },
  { href: '/devices', icon: Cpu, label: 'Dispositivos' },
  { href: '/enrollment', icon: UserCheck, label: 'Enrolamiento' },
  { href: '/reports', icon: BarChart2, label: 'Reportes' },
  { href: '/audit', icon: ShieldCheck, label: 'Auditoría' },
]

export function Sidebar() {
  const pathname = usePathname()
  return (
    <aside className="w-60 min-h-screen bg-slate-900 text-slate-100 flex flex-col shrink-0">
      <div className="px-6 py-5 border-b border-slate-700">
        <span className="text-lg font-semibold tracking-tight">Cronometrix</span>
      </div>
      <nav className="flex-1 py-4 space-y-1 px-3">
        {NAV_ITEMS.map(({ href, icon: Icon, label }) => {
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
