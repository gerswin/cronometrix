'use client'
import { useState, useCallback } from 'react'
import Link from 'next/link'
import { useSSE } from '@/hooks/use-sse'
import { addToRingBuffer } from '@/lib/ring-buffer'
import { SSEReconnectBanner } from './sse-reconnect-banner'
import { fmtTime } from '@/lib/format/datetime'
import { AttendanceEventSSEPayload } from '@/types/api'
import { EventPhoto } from '@/components/events/event-photo'

const AVATAR_PALETTE = [
  '#3B82F6',
  '#A855F7',
  '#22C55E',
  '#F59E0B',
  '#EF4444',
  '#06B6D4',
  '#84CC16',
]

/** Deterministic color from palette based on a string seed. */
function avatarColor(seed: string): string {
  let hash = 0
  for (let i = 0; i < seed.length; i++) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0
  }
  return AVATAR_PALETTE[hash % AVATAR_PALETTE.length]
}

/** Extracts up to 2 uppercase initials from a name; returns '?' for unknown. */
function initials(name: string | null): string {
  if (!name) return '?'
  return name
    .split(' ')
    .map(w => w[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()
}

interface EventAvatarProps {
  event: AttendanceEventSSEPayload
}

function EventAvatar({ event }: EventAvatarProps) {
  const seed = event.employee_id ?? event.id
  const bg = avatarColor(seed)
  const label = initials(event.employee_name)

  return (
    <div
      className="w-8 h-8 rounded-full flex items-center justify-center text-[11px] font-semibold text-white shrink-0"
      style={{ backgroundColor: bg }}
      aria-label={event.employee_name ?? 'Empleado desconocido'}
    >
      {label}
    </div>
  )
}

// SSE payload does not carry device_id; the `department` field is used as
// the location context in the subtitle (HH:MM · Department).
export function ActivityFeed() {
  const [events, setEvents] = useState<AttendanceEventSSEPayload[]>([])

  const handleMessage = useCallback((payload: AttendanceEventSSEPayload) => {
    setEvents(prev => addToRingBuffer(prev, payload, 20))
  }, [])

  const { reconnecting } = useSSE<AttendanceEventSSEPayload>('/events/stream', handleMessage)

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-[14px] border-b border-[#EEF0F2]">
        <span className="text-[15px] font-semibold text-[#1A1A1A]">Actividad en Vivo</span>
        <Link href="/events" className="text-[12px] text-[#1E3FB8] hover:underline">
          Ver todo
        </Link>
      </div>

      <SSEReconnectBanner reconnecting={reconnecting} />

      {/* Event list */}
      <ul data-testid="ring-buffer" className="flex-1 overflow-y-auto">
        {events.length === 0 && (
          <li className="flex items-center justify-center h-full text-[13px] text-[#666666] py-8">
            Sin actividad reciente
          </li>
        )}
        {events.map((event, idx) => {
          // SSE payload has `department` (name string) but no device_id.
          // Show department as location context; fall back to '—'.
          const locationLabel = event.department ?? '—'
          const isLast = idx === events.length - 1

          return (
            <li
              key={event.id}
              data-testid={`ring-row-${event.id}`}
              className={`flex items-center gap-3 px-4 py-[10px] ${isLast ? '' : 'border-b border-[#EEF0F2]'}`}
            >
              <EventPhoto
                eventId={event.id}
                hasPhoto={event.has_photo}
                className="w-8 h-8 rounded-full shrink-0"
                alt={event.employee_name ?? 'Empleado desconocido'}
                fallback={<EventAvatar event={event} />}
              />

              {/* Name + time · device */}
              <div className="flex-1 min-w-0 flex flex-col gap-0.5">
                <span className="text-[14px] font-medium text-[#1A1A1A] truncate">
                  {event.employee_name ?? 'Empleado desconocido'}
                </span>
                <span className="text-[11px] text-[#666666] truncate">
                  {fmtTime(event.captured_at)} · {locationLabel}
                </span>
              </div>

              {/* Direction badge */}
              <span
                className={`text-[11px] font-medium px-2 py-0.5 rounded-full shrink-0 ${
                  event.direction === 'entry'
                    ? 'bg-green-100 text-green-700'
                    : 'bg-blue-100 text-blue-700'
                }`}
              >
                {event.direction === 'entry' ? 'Entrada' : 'Salida'}
              </span>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
