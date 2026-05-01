'use client'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { DailyRecordDetail } from '@/types/api'
import { fmtTime, fmtDateTime } from '@/lib/format/datetime'
import { fmtMin } from '@/lib/format/duration'

interface Props {
  recordId: string | null
  onClose: () => void
}

export function DailyRecordDialog({ recordId, onClose }: Props) {
  const open = recordId !== null

  const { data, isLoading, error } = useQuery<DailyRecordDetail>({
    queryKey: ['daily-records', 'detail', recordId],
    queryFn: () =>
      api.get(`/daily-records/${recordId}`).then((r) => r.data),
    enabled: open,
  })

  return (
    <Dialog
      open={open}
      onOpenChange={(o: boolean) => {
        if (!o) onClose()
      }}
    >
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Detalle del registro diario</DialogTitle>
        </DialogHeader>

        {isLoading && (
          <div className="text-sm text-slate-500 py-4">Cargando…</div>
        )}
        {error && (
          <div className="text-sm text-red-600 py-4">
            Error al cargar el detalle.
          </div>
        )}
        {data && (
          <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <Field label="Fecha" value={data.anchor_date} />
            <Field label="Tipo de jornada" value={data.shift_type} />
            <Field label="Entrada" value={fmtTime(data.entry_at)} />
            <Field label="Salida" value={fmtTime(data.exit_at)} />
            <Field
              label="Trabajado"
              value={fmtMin(data.work_minutes)}
              hint={`${data.work_minutes} min`}
            />
            <Field
              label="Extras"
              value={fmtMin(data.overtime_minutes)}
              hint={`${data.overtime_minutes} min`}
            />
            <Field
              label="Retraso"
              value={fmtMin(data.late_minutes)}
              hint={`${data.late_minutes} min`}
            />
            <Field
              label="Salida anticipada"
              value={fmtMin(data.early_departure_minutes)}
              hint={`${data.early_departure_minutes} min`}
            />
            <Field
              label="Día de descanso trabajado"
              value={data.is_rest_day_worked ? 'Sí' : 'No'}
            />
            <Field
              label="Novedad asociada"
              value={data.leave_id ?? '—'}
            />
            <Field label="Calculado" value={fmtDateTime(data.computed_at)} />
            <Field label="Actualizado" value={fmtDateTime(data.updated_at)} />
            <div className="col-span-2">
              <dt className="text-xs text-slate-500 uppercase tracking-wide">
                Anomalías
              </dt>
              <dd className="mt-1 flex flex-wrap gap-1">
                {data.anomalies.length === 0 ? (
                  <span className="text-slate-500">—</span>
                ) : (
                  data.anomalies.map((c) => (
                    <span
                      key={c}
                      className="px-2 py-0.5 rounded bg-amber-100 text-amber-800 text-xs"
                    >
                      {c}
                    </span>
                  ))
                )}
              </dd>
            </div>
          </dl>
        )}
      </DialogContent>
    </Dialog>
  )
}

function Field({
  label,
  value,
  hint,
}: {
  label: string
  value: string
  hint?: string
}) {
  return (
    <div>
      <dt className="text-xs text-slate-500 uppercase tracking-wide">
        {label}
      </dt>
      <dd className="text-slate-700" title={hint}>
        {value}
      </dd>
    </div>
  )
}
