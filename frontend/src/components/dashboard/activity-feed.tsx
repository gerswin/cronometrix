'use client'
import { useState, useEffect, useCallback } from 'react'
import { format } from 'date-fns'
import { useSSE } from '@/hooks/use-sse'
import { addToRingBuffer } from '@/lib/ring-buffer'
import { SSEReconnectBanner } from './sse-reconnect-banner'
import { AttendanceEventSSEPayload } from '@/types/api'
import { api } from '@/lib/api'
import Link from 'next/link'

function EventAvatar({ event }: { event: AttendanceEventSSEPayload }) {
  const initials = (event.employee_name ?? '?').split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase()
  // WR-04: GET /events/{id}/photo requires Authorization: Bearer. A naive
  //        <img src> tag issues the request without the JWT and gets 401.
  //        Fetch the blob through the axios `api` instance (which attaches
  //        the bearer token via interceptor) and feed it to the <img> as a
  //        blob URL.
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    if (!event.has_photo) {
      setSrc(null)
      return
    }
    let cancelled = false
    let url: string | null = null
    api
      .get(`/events/${event.id}/photo`, { responseType: 'blob' })
      .then(r => {
        if (cancelled) return
        url = URL.createObjectURL(r.data as Blob)
        setSrc(url)
      })
      .catch(() => {
        if (!cancelled) setSrc(null)
      })
    return () => {
      cancelled = true
      if (url) URL.revokeObjectURL(url)
    }
  }, [event.id, event.has_photo])

  if (event.has_photo && src) {
    return (
      <img
        data-testid="photo-img"
        src={src}
        alt={event.employee_name ?? 'evento'}
        className="w-10 h-10 rounded-full object-cover shrink-0"
      />
    )
  }
  return (
    <div
      data-testid="photo-fallback"
      className="w-10 h-10 rounded-full bg-slate-300 flex items-center justify-center text-xs font-semibold text-slate-700 shrink-0"
    >
      {initials}
    </div>
  )
}

export function ActivityFeed() {
  const [events, setEvents] = useState<AttendanceEventSSEPayload[]>([])

  const handleMessage = useCallback((payload: AttendanceEventSSEPayload) => {
    setEvents(prev => addToRingBuffer(prev, payload, 20))
  }, [])

  const { reconnecting } = useSSE<AttendanceEventSSEPayload>('/events/stream', handleMessage)

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold text-slate-700">Actividad en Vivo</h2>
        <Link
          href={`/timesheet?from_date=${format(new Date(), 'yyyy-MM-dd')}&to_date=${format(new Date(), 'yyyy-MM-dd')}`}
          className="text-xs text-blue-600 hover:underline"
        >
          Ver todo
        </Link>
      </div>
      <SSEReconnectBanner reconnecting={reconnecting} />
      <ul data-testid="ring-buffer" className="space-y-2 overflow-y-auto flex-1">
        {events.length === 0 && (
          <li className="text-xs text-slate-400 py-4 text-center">Sin actividad reciente</li>
        )}
        {events.map(event => (
          <li key={event.id} data-testid={`ring-row-${event.id}`} className="flex items-center gap-3">
            <EventAvatar event={event} />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-slate-800 truncate">
                {event.employee_name ?? 'Empleado desconocido'}
              </p>
              <p className="text-xs text-slate-500">
                {event.department ?? '—'} · {format(new Date(event.captured_at), 'HH:mm')}
              </p>
            </div>
            <span className={`text-xs font-medium px-2 py-0.5 rounded-full shrink-0 ${
              event.direction === 'entry'
                ? 'bg-green-100 text-green-700'
                : 'bg-blue-100 text-blue-700'
            }`}>
              {event.direction === 'entry' ? 'Entrada' : 'Salida'}
            </span>
          </li>
        ))}
      </ul>
    </div>
  )
}
