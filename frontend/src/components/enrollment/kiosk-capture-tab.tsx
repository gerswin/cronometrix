'use client'
import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { Loader2, AlertTriangle } from 'lucide-react'
import { api } from '@/lib/api'
import { Button } from '@/components/ui/button'
import type { Device, CaptureFromDeviceState } from '@/types/api'

interface KioskCaptureTabProps {
  employeeId: string
  onCaptured: (blob: Blob) => void
}

type KioskState = 'idle' | 'waiting' | 'captured' | 'timeout'

const COUNTDOWN_SECONDS = 30

export function KioskCaptureTab({ employeeId, onCaptured }: KioskCaptureTabProps) {
  const [selectedDeviceId, setSelectedDeviceId] = useState<string>('')
  const [captureId, setCaptureId] = useState<string | null>(null)
  const [kioskState, setKioskState] = useState<KioskState>('idle')
  const [countdown, setCountdown] = useState(COUNTDOWN_SECONDS)
  const [previewBlob, setPreviewBlob] = useState<Blob | null>(null)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const countdownRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Fetch active devices for the Select dropdown
  const { data: devicesData } = useQuery<{ data: Device[] }>({
    queryKey: ['devices-active'],
    queryFn: () => api.get('/devices?status=active').then(r => r.data),
  })
  const devices = devicesData?.data ?? []

  // Start capture mutation
  const startCaptureMutation = useMutation({
    mutationFn: () =>
      api.post('/enrollments/capture-from-device', {
        device_id: selectedDeviceId,
        employee_id: employeeId,
      }).then(r => r.data as { capture_id: string; status: string }),
    onSuccess: (data) => {
      setCaptureId(data.capture_id)
      setKioskState('waiting')
      setCountdown(COUNTDOWN_SECONDS)
      // Start countdown
      if (countdownRef.current) clearInterval(countdownRef.current)
      countdownRef.current = setInterval(() => {
        setCountdown(prev => {
          if (prev <= 1) {
            if (countdownRef.current) clearInterval(countdownRef.current)
            return 0
          }
          return prev - 1
        })
      }, 1000)
    },
    onError: (err: unknown) => {
      const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message
        ?? 'No se pudo iniciar la captura.'
      toast.error(msg)
    },
  })

  // Poll capture status
  const { data: captureState } = useQuery<CaptureFromDeviceState>({
    queryKey: ['capture', captureId],
    queryFn: () => api.get(`/enrollments/captures/${captureId}`).then(r => r.data),
    enabled: !!captureId && kioskState === 'waiting',
    refetchInterval: (q) => {
      const d = q.state.data as CaptureFromDeviceState | undefined
      if (!d) return 1500
      const terminal = d.status === 'captured' || d.status === 'timeout' || d.status === 'error'
      return terminal ? false : 1500
    },
  })

  // Decode photo_b64 → Blob when captured (contract reconciled with 07-01 Task 4)
  useEffect(() => {
    if (captureState?.status !== 'captured' || !captureState.photo_b64) return

    // Clear countdown timer
    if (countdownRef.current) {
      clearInterval(countdownRef.current)
      countdownRef.current = null
    }

    const bin = atob(captureState.photo_b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    const blob = new Blob([bytes], { type: 'image/jpeg' })
    setPreviewBlob(blob)
    const url = URL.createObjectURL(blob)
    setPreviewUrl(url)
    setKioskState('captured')

    return () => URL.revokeObjectURL(url)
  }, [captureState?.status, captureState?.photo_b64])

  // Handle timeout/error from poll
  useEffect(() => {
    if (captureState?.status === 'timeout' || captureState?.status === 'error') {
      if (countdownRef.current) {
        clearInterval(countdownRef.current)
        countdownRef.current = null
      }
      setKioskState('timeout')
    }
  }, [captureState?.status])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (countdownRef.current) clearInterval(countdownRef.current)
      if (previewUrl) URL.revokeObjectURL(previewUrl)
    }
  }, [previewUrl])

  function handleAccept() {
    if (previewBlob) {
      onCaptured(previewBlob)
    }
  }

  function handleRetry() {
    setCaptureId(null)
    setKioskState('idle')
    setPreviewBlob(null)
    if (previewUrl) {
      URL.revokeObjectURL(previewUrl)
      setPreviewUrl(null)
    }
  }

  function handleCancel() {
    if (countdownRef.current) clearInterval(countdownRef.current)
    setCaptureId(null)
    setKioskState('idle')
  }

  return (
    <div className="space-y-4">
      {/* Idle state */}
      {kioskState === 'idle' && (
        <div className="space-y-3">
          <div>
            <label className="text-xs text-slate-500 font-medium uppercase tracking-wide">
              Dispositivo Hikvision
            </label>
            <select
              value={selectedDeviceId}
              onChange={e => setSelectedDeviceId(e.target.value)}
              className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              aria-label="Seleccionar dispositivo"
            >
              <option value="">Selecciona un dispositivo…</option>
              {devices.map(d => (
                <option key={d.id} value={d.id}>
                  {d.name} ({d.ip_address})
                </option>
              ))}
            </select>
          </div>
          <Button
            size="sm"
            type="button"
            disabled={!selectedDeviceId || startCaptureMutation.isPending}
            onClick={() => startCaptureMutation.mutate()}
          >
            {startCaptureMutation.isPending ? (
              <><Loader2 size={14} className="animate-spin" /> Iniciando…</>
            ) : (
              'Iniciar Captura'
            )}
          </Button>
        </div>
      )}

      {/* Waiting state */}
      {kioskState === 'waiting' && (
        <div className="space-y-3">
          <div className="flex items-center gap-3 rounded-md bg-blue-50 border border-blue-200 px-4 py-3 text-sm text-blue-700">
            <Loader2 size={16} className="animate-spin shrink-0" />
            <span>Esperando captura en el dispositivo… ({countdown}s)</span>
          </div>
          <Button size="sm" variant="outline" type="button" onClick={handleCancel}>
            Cancelar
          </Button>
        </div>
      )}

      {/* Captured state — preview + Aceptar */}
      {kioskState === 'captured' && previewUrl && (
        <div className="space-y-3">
          <img
            src={previewUrl}
            alt="Captura del dispositivo"
            className="rounded-md border border-slate-200 object-cover"
            style={{ width: 160, height: 160 }}
          />
          <div className="flex gap-2">
            <Button size="sm" type="button" onClick={handleAccept}>
              Aceptar
            </Button>
            <Button size="sm" variant="outline" type="button" onClick={handleRetry}>
              Recapturar
            </Button>
          </div>
        </div>
      )}

      {/* Timeout state */}
      {kioskState === 'timeout' && (
        <div className="space-y-3">
          <div
            role="alert"
            className="flex items-start gap-3 rounded-md bg-amber-50 border border-amber-200 px-4 py-3 text-sm text-amber-700"
          >
            <AlertTriangle size={16} className="mt-0.5 shrink-0" />
            <span>No se detectó captura. El dispositivo no respondió a tiempo.</span>
          </div>
          <Button size="sm" variant="outline" type="button" onClick={handleRetry}>
            Reintentar
          </Button>
        </div>
      )}
    </div>
  )
}
