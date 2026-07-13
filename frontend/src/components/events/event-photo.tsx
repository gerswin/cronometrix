'use client'
import { ReactNode, useEffect, useState } from 'react'
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
  fallback?: ReactNode
}

export function EventPhoto({ eventId, hasPhoto, className, alt, fallback }: Props) {
  const [photo, setPhoto] = useState<{ eventId: string; src: string } | null>(null)

  useEffect(() => {
    if (!hasPhoto) {
      return
    }
    let cancelled = false
    let url: string | null = null
    api
      .get(`/events/${encodeURIComponent(eventId)}/photo`, { responseType: 'blob' })
      .then((r) => {
        if (cancelled) return
        url = URL.createObjectURL(r.data as Blob)
        setPhoto({ eventId, src: url })
      })
      .catch(() => {
        if (!cancelled) {
          setPhoto((current) => current?.eventId === eventId ? null : current)
        }
      })
    return () => {
      cancelled = true
      if (url) URL.revokeObjectURL(url)
    }
  }, [eventId, hasPhoto])

  if (hasPhoto && photo?.eventId === eventId) {
    return (
      <img
        data-testid="photo-img"
        src={photo.src}
        alt={alt ?? 'evento'}
        className={cn('object-cover', className)}
      />
    )
  }
  return (
    <div
      data-testid="photo-fallback"
      className={cn(
        fallback === undefined &&
          'bg-slate-200 flex items-center justify-center text-slate-400 text-xs',
        className,
      )}
      aria-label={fallback === undefined ? 'Sin foto' : alt}
    >
      {fallback ?? '—'}
    </div>
  )
}
