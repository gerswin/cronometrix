'use client'
import { useEffect, useState } from 'react'
import { api } from '@/lib/api'
import { cn } from '@/lib/utils'

// Lazy-loads /events/{id}/photo via the authed axios instance and renders the
// blob into an <img>. Falls back to a placeholder when no photo is available
// or the request fails. Reused by the events list (small thumbnail) and the
// detail dialog (larger preview).

interface Props {
  eventId: string
  hasPhoto: boolean
  className?: string
  alt?: string
}

export function EventPhoto({ eventId, hasPhoto, className, alt }: Props) {
  const [src, setSrc] = useState<string | null>(null)

  useEffect(() => {
    if (!hasPhoto) {
      setSrc(null)
      return
    }
    let cancelled = false
    let url: string | null = null
    api
      .get(`/events/${eventId}/photo`, { responseType: 'blob' })
      .then((r) => {
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
  }, [eventId, hasPhoto])

  if (hasPhoto && src) {
    return (
      <img
        src={src}
        alt={alt ?? 'evento'}
        className={cn('object-cover', className)}
      />
    )
  }
  return (
    <div
      className={cn(
        'bg-slate-200 flex items-center justify-center text-slate-400 text-xs',
        className,
      )}
      aria-label="Sin foto"
    >
      —
    </div>
  )
}
