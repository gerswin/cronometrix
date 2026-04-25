'use client'
import { useEffect, useRef, useState, useCallback } from 'react'
import { getAccessToken } from '@/lib/api'

const BACKOFF_DELAYS = [1000, 2000, 4000, 8000, 30000]

export function useSSE<T>(
  path: string,
  onMessage: (data: T) => void,
) {
  const esRef = useRef<EventSource | null>(null)
  const attemptRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onMessageRef = useRef(onMessage)
  const [connected, setConnected] = useState(false)
  const [reconnecting, setReconnecting] = useState(false)

  useEffect(() => { onMessageRef.current = onMessage })

  const connect = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current)
    // CR-01: always re-read the token at connect time so reconnects after a
    //        refresh use the new token (the closure variable would be stale).
    // WR-01: if no token is available (logout race, pre-auth mount), bail out
    //        instead of opening an EventSource with `?token=` blank — keeps
    //        the reconnect banner from being stuck and avoids a guaranteed-401.
    const token = getAccessToken()
    if (!token) {
      setConnected(false)
      setReconnecting(false)
      return
    }
    const url = `${process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:3001'}/api/v1${path}?token=${encodeURIComponent(token)}`
    const es = new EventSource(url)
    esRef.current = es

    es.onopen = () => {
      attemptRef.current = 0
      setConnected(true)
      setReconnecting(false)
    }
    es.onmessage = (e) => {
      try { onMessageRef.current(JSON.parse(e.data)) } catch { /* skip malformed */ }
    }
    es.onerror = () => {
      es.close()
      setConnected(false)
      setReconnecting(true)
      const delay = BACKOFF_DELAYS[Math.min(attemptRef.current, BACKOFF_DELAYS.length - 1)]
      attemptRef.current++
      timerRef.current = setTimeout(connect, delay)
    }
  }, [path])

  useEffect(() => {
    connect()
    return () => {
      esRef.current?.close()
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [connect])

  return { connected, reconnecting }
}
