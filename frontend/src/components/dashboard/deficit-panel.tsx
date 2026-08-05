'use client'
import { fmtMinutes } from '@/lib/format/minutes'
import type { PresenceRow } from '@/types/api'

interface Props {
  rows: PresenceRow[]
}

export function DeficitPanel({ rows }: Props) {
  // C-02: solo quien YA SALIÓ puede haber incumplido la jornada. La jornada
  // esperada es la del día completo, no prorrateada a la hora actual, así que
  // sin el filtro por `status` todo empleado presente aparecía aquí con un
  // déficit grande y rojo a media mañana. `deficit_min` es una regla válida
  // sobre un periodo cerrado (reportes), no sobre un día en curso.
  const short = rows
    .filter(r => r.status === 'left' && r.deficit_min > 0)
    .sort((a, b) => b.deficit_min - a.deficit_min)

  return (
    <div className="bg-white rounded-lg border border-[#EEF0F2]">
      <div className="px-4 py-[14px] border-b border-[#EEF0F2]">
        <span className="text-[15px] font-semibold text-[#1A1A1A]">Jornada incumplida hoy</span>
      </div>

      {short.length === 0 ? (
        <p data-testid="deficit-empty" className="px-4 py-8 text-center text-[13px] text-[#666666]">
          Todos cumplieron su jornada
        </p>
      ) : (
        <ul>
          {short.map(r => (
            <li
              key={r.employee_id}
              data-testid={`deficit-row-${r.employee_id}`}
              className="flex items-center justify-between px-4 py-[10px] border-t border-[#EEF0F2]"
            >
              <span className="text-[14px] text-[#1A1A1A]">{r.employee_name}</span>
              <span className="text-[13px] font-medium text-[#EF4444]">
                {fmtMinutes(r.deficit_min)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
