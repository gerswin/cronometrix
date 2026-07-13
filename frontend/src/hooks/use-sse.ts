'use client'
import { useEffect, useRef, useState } from 'react'
import { API_BASE, getAccessToken, onAccessTokenChange } from '@/lib/api'

const BACKOFF_DELAYS = [1000, 2000, 4000, 8000, 30000]

export function useSSE<T>(
  path: string,
  onMessage: (data: T) => void,
) {
  const esRef = useRef<EventSource | null>(null)
  const attemptRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onMessageRef = useRef(onMessage)
  const tokenRef = useRef<string | null | undefined>(undefined)
  const generationRef = useRef(0)
  const [connected, setConnected] = useState(false)
  const [reconnecting, setReconnecting] = useState(false)

  useEffect(() => { onMessageRef.current = onMessage })

  useEffect(() => {
    let disposed = false

    const clearRetry = () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
    }

    const closeSource = () => {
      esRef.current?.close()
      esRef.current = null
    }

    const connect = (token: string, generation: number) => {
      if (
        disposed ||
        generation !== generationRef.current ||
        tokenRef.current !== token
      ) return

      clearRetry()
      const url = `${API_BASE}/api/v1${path}?token=${encodeURIComponent(token)}`
      const source = new EventSource(url)
      esRef.current = source

      const isCurrent = () =>
        !disposed &&
        generation === generationRef.current &&
        tokenRef.current === token &&
        esRef.current === source

      source.onopen = () => {
        if (!isCurrent()) return
        attemptRef.current = 0
        setConnected(true)
        setReconnecting(false)
      }
      source.onmessage = (event) => {
        if (!isCurrent()) return
        try { onMessageRef.current(JSON.parse(event.data)) } catch { /* skip malformed */ }
      }
      source.onerror = () => {
        if (!isCurrent()) return
        source.close()
        esRef.current = null
        setConnected(false)
        setReconnecting(true)
        const delay = BACKOFF_DELAYS[
          Math.min(attemptRef.current, BACKOFF_DELAYS.length - 1)
        ]
        attemptRef.current += 1
        timerRef.current = setTimeout(() => {
          timerRef.current = null
          connect(token, generation)
        }, delay)
      }
    }

    const handleTokenChange = () => {
      const nextToken = getAccessToken()
      if (nextToken === tokenRef.current) return

      tokenRef.current = nextToken
      generationRef.current += 1
      const generation = generationRef.current
      clearRetry()
      closeSource()
      attemptRef.current = 0
      setConnected(false)
      setReconnecting(false)

      if (nextToken) connect(nextToken, generation)
    }

    // Subscribe first so a token mutation cannot be missed between the
    // initial read and listener registration.
    const unsubscribe = onAccessTokenChange(handleTokenChange)
    handleTokenChange()

    return () => {
      disposed = true
      generationRef.current += 1
      tokenRef.current = undefined
      unsubscribe()
      clearRetry()
      closeSource()
    }
  }, [path])

  return { connected, reconnecting }
}
