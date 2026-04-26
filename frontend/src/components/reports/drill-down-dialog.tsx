'use client'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { PaginatedResponse, DailyRecord } from '@/types/api'

interface Props {
  employeeId: string | null
  from: string
  to: string
  onClose: () => void
}

function fmtTime(iso: string | null | undefined) {
  if (!iso) return '—'
  try {
    return iso.slice(11, 16)
  } catch {
    return '—'
  }
}

export function DrillDownDialog({ employeeId, from, to, onClose }: Props) {
  const isOpen = employeeId !== null

  const { data, isLoading, error } = useQuery<PaginatedResponse<DailyRecord>>({
    queryKey: ['daily-records', employeeId, from, to],
    queryFn: () =>
      api
        .get('/daily-records', {
          params: { employee_id: employeeId, from_date: from, to_date: to },
        })
        .then((r) => r.data),
    enabled: !!employeeId,
  })

  return (
    <Dialog open={isOpen} onOpenChange={(o: boolean) => { if (!o) onClose() }}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>Detalle por Día</DialogTitle>
        </DialogHeader>

        {isLoading && (
          <div className="text-sm text-slate-500 py-4">Cargando…</div>
        )}
        {error && (
          <div className="text-sm text-red-600 py-4">
            Error al cargar el detalle.
          </div>
        )}
        {data && data.data.length === 0 && (
          <div className="text-sm text-slate-500 py-4">
            Sin registros para este período.
          </div>
        )}
        {data && data.data.length > 0 && (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-200 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">
                  <th className="px-2 py-2">Fecha</th>
                  <th className="px-2 py-2">Entrada</th>
                  <th className="px-2 py-2">Salida</th>
                  <th className="px-2 py-2">Trab</th>
                  <th className="px-2 py-2">Extra</th>
                  <th className="px-2 py-2">Retraso</th>
                  <th className="px-2 py-2">Anomalías</th>
                </tr>
              </thead>
              <tbody>
                {data.data.map((r) => (
                  <tr key={r.id} className="border-b border-slate-100">
                    <td className="px-2 py-2 text-slate-700">{r.anchor_date}</td>
                    <td className="px-2 py-2 text-slate-700">
                      {fmtTime(r.entry_at)}
                    </td>
                    <td className="px-2 py-2 text-slate-700">
                      {fmtTime(r.exit_at)}
                    </td>
                    <td className="px-2 py-2 text-slate-700">
                      {r.work_minutes}
                    </td>
                    <td className="px-2 py-2 text-slate-700">
                      {r.overtime_minutes}
                    </td>
                    <td className="px-2 py-2 text-slate-700">
                      {r.late_minutes}
                    </td>
                    <td className="px-2 py-2 text-slate-700">
                      {r.leave_id ? (
                        <span className="px-1.5 py-0.5 rounded text-xs bg-amber-100 text-amber-700">
                          Novedad
                        </span>
                      ) : r.anomalies && r.anomalies.length > 0 ? (
                        <span className="text-xs text-amber-700">
                          {r.anomalies.join(', ')}
                        </span>
                      ) : (
                        ''
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
