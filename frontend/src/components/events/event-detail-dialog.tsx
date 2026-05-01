'use client'
import Link from 'next/link'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { RawAttendanceEvent } from '@/types/api'
import { fmtDateTime } from '@/lib/format/datetime'
import { EventPhoto } from './event-photo'

interface Props {
  eventId: string | null
  onClose: () => void
}

export function EventDetailDialog({ eventId, onClose }: Props) {
  const open = eventId !== null
  const { data, isLoading, error } = useQuery<RawAttendanceEvent>({
    queryKey: ['events', eventId],
    queryFn: () => api.get(`/events/${eventId}`).then((r) => r.data),
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
          <DialogTitle>Detalle del evento</DialogTitle>
        </DialogHeader>

        {isLoading && (
          <div className="text-sm text-slate-500 py-4">Cargando…</div>
        )}
        {error && (
          <div className="text-sm text-red-600 py-4">
            Error al cargar el evento.
          </div>
        )}
        {data && (
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="col-span-2">
              <EventPhoto
                eventId={data.id}
                hasPhoto={!!data.photo_path}
                className="w-48 h-48 rounded mx-auto"
                alt="Foto del evento"
              />
            </div>
            <Field label="Capturado" value={fmtDateTime(data.captured_at)} />
            <Field
              label="Dirección"
              value={data.direction === 'entry' ? 'Entrada' : 'Salida'}
            />
            <Field
              label="Empleado"
              value={data.is_unknown ? 'Desconocido' : data.employee_id ?? '—'}
            />
            <Field label="Dispositivo" value={data.device_id} />
            <Field
              label="Face ID"
              value={data.face_id ?? '—'}
            />
            <Field
              label="Empleado (string Hikvision)"
              value={data.employee_no_string ?? '—'}
            />
            <Field label="Registrado" value={fmtDateTime(data.created_at)} />
            <div className="col-span-2 pt-2 border-t border-slate-100">
              <Link
                href={`/timesheet?from_date=${data.captured_at.slice(0, 10)}&to_date=${data.captured_at.slice(0, 10)}${data.employee_id ? `&employee_id=${data.employee_id}` : ''}`}
                className="text-xs text-blue-600 hover:underline"
              >
                Ir a marcaciones de esa fecha →
              </Link>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-slate-500 uppercase tracking-wide">
        {label}
      </div>
      <div className="text-slate-700 truncate" title={value}>
        {value}
      </div>
    </div>
  )
}
