'use client'
import { useAuth } from '@/hooks/use-auth'

interface TopBarProps { title: string }

export function TopBar({ title }: TopBarProps) {
  const { role, sub } = useAuth()
  return (
    <header className="h-14 border-b border-slate-200 px-6 flex items-center justify-between bg-white">
      <h1 className="text-base font-semibold text-slate-800">{title}</h1>
      <div className="text-xs text-slate-500 capitalize">{role} · {sub}</div>
    </header>
  )
}
