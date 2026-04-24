'use client'
import { startOfWeek, endOfWeek, addWeeks, subWeeks, format } from 'date-fns'

interface WeekNavigatorProps {
  currentDate: Date
  onChange: (date: Date) => void
}

export function WeekNavigator({ currentDate, onChange }: WeekNavigatorProps) {
  // Pitfall 7: always weekStartsOn: 1 (Monday) — Venezuela LOTTT work week
  const weekStart = startOfWeek(currentDate, { weekStartsOn: 1 })
  const weekEnd = endOfWeek(currentDate, { weekStartsOn: 1 })

  return (
    <div className="flex items-center gap-3">
      <button
        onClick={() => onChange(subWeeks(currentDate, 1))}
        className="p-1 rounded hover:bg-slate-100 text-slate-600"
        aria-label="Semana anterior"
      >
        ←
      </button>
      <span className="text-sm font-medium text-slate-700 min-w-[200px] text-center">
        {format(weekStart, 'dd MMM')} – {format(weekEnd, 'dd MMM yyyy')}
      </span>
      <button
        onClick={() => onChange(addWeeks(currentDate, 1))}
        className="p-1 rounded hover:bg-slate-100 text-slate-600"
        aria-label="Semana siguiente"
      >
        →
      </button>
    </div>
  )
}
