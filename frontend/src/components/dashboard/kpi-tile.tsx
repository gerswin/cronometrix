import { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface KPITileProps {
  title: string
  value: string | number
  sub?: ReactNode
  variant?: 'default' | 'warning' | 'danger'
}

export function KPITile({ title, value, sub, variant = 'default' }: KPITileProps) {
  return (
    <div className={cn(
      'rounded-xl border p-4 bg-white shadow-sm',
      variant === 'warning' && 'border-yellow-300',
      variant === 'danger' && 'border-red-300',
    )}>
      <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">{title}</p>
      <p className="text-3xl font-bold text-slate-800">{value}</p>
      {sub && <div className="mt-1">{sub}</div>}
    </div>
  )
}
