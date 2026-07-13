'use client'
import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { AlertTriangle, Camera, Check, Loader2, RotateCcw } from 'lucide-react'
import { api } from '@/lib/api'
import {
  getDeviceCapture,
  startDeviceCapture,
  type CapturedPhotoCandidate,
} from '@/lib/enrollment-api'
import { PrimaryButton } from '@/components/ui/primary-button'
import type { CaptureFromDeviceState, Device } from '@/types/api'

interface KioskCaptureTabProps {
  employeeId: string
  onCaptured: (candidate: CapturedPhotoCandidate) => void
  onCleared: () => void
}

type KioskState = 'idle' | 'waiting' | 'captured' | 'timeout'

const COUNTDOWN_SECONDS = 30

export function KioskCaptureTab({ employeeId, onCaptured, onCleared }: KioskCaptureTabProps) {
  const queryClient = useQueryClient()
  const [selectedDeviceId, setSelectedDeviceId] = useState('')
  const [captureId, setCaptureId] = useState<string | null>(null)
  const [captureSourceDeviceId, setCaptureSourceDeviceId] = useState<string | null>(null)
  const [kioskState, setKioskState] = useState<KioskState>('idle')
  const [countdown, setCountdown] = useState(COUNTDOWN_SECONDS)
  const [previewBlob, setPreviewBlob] = useState<Blob | null>(null)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const countdownRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const previewUrlRef = useRef<string | null>(null)
  const generationRef = useRef(0)
  const previousEmployeeRef = useRef(employeeId)

  const { data: devicesData } = useQuery<{ data: Device[] }>({
    queryKey: ['devices-active'],
    queryFn: () => api.get('/devices?status=active').then((response) => response.data),
  })
  const devices = devicesData?.data ?? []

  function stopCountdown() {
    if (!countdownRef.current) return
    clearInterval(countdownRef.current)
    countdownRef.current = null
  }

  function clearPreview() {
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
    previewUrlRef.current = null
    setPreviewUrl(null)
    setPreviewBlob(null)
    setCaptureSourceDeviceId(null)
  }

  useEffect(() => {
    if (previousEmployeeRef.current === employeeId) return
    previousEmployeeRef.current = employeeId
    generationRef.current += 1
    if (captureId) {
      void queryClient.cancelQueries({ queryKey: ['capture', captureId] })
    }
    stopCountdown()
    clearPreview()
    setSelectedDeviceId('')
    setCaptureId(null)
    setKioskState('idle')
    setCountdown(COUNTDOWN_SECONDS)
    onCleared()
  // Reset is intentionally tied only to the employee/session identity.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [employeeId])

  useEffect(() => {
    return () => {
      generationRef.current += 1
      stopCountdown()
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
      previewUrlRef.current = null
    }
  }, [])

  const startCaptureMutation = useMutation({
    mutationFn: (request: { deviceId: string; employeeId: string; generation: number }) =>
      startDeviceCapture(request),
    onSuccess: (data, request) => {
      if (request.generation !== generationRef.current || request.employeeId !== employeeId) return
      setCaptureId(data.capture_id)
      setCaptureSourceDeviceId(data.source_device_id)
      setKioskState('waiting')
      setCountdown(COUNTDOWN_SECONDS)
      stopCountdown()
      countdownRef.current = setInterval(() => {
        setCountdown((previous) => {
          if (previous <= 1) {
            stopCountdown()
            return 0
          }
          return previous - 1
        })
      }, 1000)
    },
    onError: (error: unknown, request) => {
      if (request.generation !== generationRef.current || request.employeeId !== employeeId) return
      const responseData = (error as {
        response?: { data?: { error?: { message?: string }; message?: string } }
      })?.response?.data
      const message = responseData?.error?.message
        ?? responseData?.message
        ?? 'No se pudo iniciar la captura.'
      toast.error(message)
    },
  })

  const { data: captureState } = useQuery<CaptureFromDeviceState>({
    queryKey: ['capture', captureId],
    queryFn: () => getDeviceCapture(captureId as string),
    enabled: captureId !== null,
    refetchInterval: (query) => {
      const state = query.state.data as CaptureFromDeviceState | undefined
      if (!state) return 1500
      return state.status === 'capturing' ? 1500 : false
    },
  })

  useEffect(() => {
    if (
      !captureId
      || captureState?.capture_id !== captureId
      || captureState.status !== 'captured'
      || !captureState.photo_b64
    ) return

    stopCountdown()
    const binary = atob(captureState.photo_b64)
    const bytes = new Uint8Array(binary.length)
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index)
    }
    const blob = new Blob([bytes], { type: 'image/jpeg' })
    clearPreview()
    const objectUrl = URL.createObjectURL(blob)
    previewUrlRef.current = objectUrl
    setPreviewBlob(blob)
    setPreviewUrl(objectUrl)
    setCaptureSourceDeviceId(captureState.source_device_id)
    setKioskState('captured')
  }, [captureId, captureState])

  useEffect(() => {
    if (!captureId || captureState?.capture_id !== captureId) return
    if (captureState.status !== 'timeout' && captureState.status !== 'error') return
    stopCountdown()
    setKioskState('timeout')
  }, [captureId, captureState])

  function beginCapture() {
    const generation = ++generationRef.current
    clearPreview()
    onCleared()
    startCaptureMutation.mutate({
      deviceId: selectedDeviceId,
      employeeId,
      generation,
    })
  }

  function resetCapture() {
    generationRef.current += 1
    if (captureId) {
      void queryClient.cancelQueries({ queryKey: ['capture', captureId] })
    }
    stopCountdown()
    clearPreview()
    setCaptureId(null)
    setKioskState('idle')
    setCountdown(COUNTDOWN_SECONDS)
    onCleared()
  }

  function handleAccept() {
    if (!previewBlob || !captureSourceDeviceId) return
    onCaptured({
      blob: previewBlob,
      capturedVia: 'device',
      sourceDeviceId: captureSourceDeviceId,
    })
  }

  return (
    <div className="space-y-4">
      {kioskState === 'idle' && (
        <div className="space-y-3">
          <div>
            <label className="text-xs text-slate-500 font-medium uppercase tracking-wide">
              Dispositivo Hikvision
            </label>
            <select
              value={selectedDeviceId}
              onChange={(event) => setSelectedDeviceId(event.target.value)}
              className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              aria-label="Seleccionar dispositivo"
            >
              <option value="">Selecciona un dispositivo…</option>
              {devices.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.name} ({device.ip})
                </option>
              ))}
            </select>
          </div>
          <PrimaryButton
            size="sm"
            type="button"
            icon={startCaptureMutation.isPending ? Loader2 : Camera}
            disabled={!selectedDeviceId || startCaptureMutation.isPending}
            onClick={beginCapture}
          >
            {startCaptureMutation.isPending ? 'Iniciando…' : 'Iniciar Captura'}
          </PrimaryButton>
        </div>
      )}

      {kioskState === 'waiting' && (
        <div className="space-y-3">
          <div className="flex items-center gap-3 rounded-md bg-blue-50 border border-blue-200 px-4 py-3 text-sm text-blue-700">
            <Loader2 size={16} className="animate-spin shrink-0" />
            <span>Esperando captura en el dispositivo… ({countdown}s)</span>
          </div>
          <PrimaryButton size="sm" variant="outline" type="button" onClick={resetCapture}>
            Cancelar
          </PrimaryButton>
        </div>
      )}

      {kioskState === 'captured' && previewUrl && (
        <div className="space-y-3">
          {/* Blob-backed device preview cannot use the Next image optimizer. */}
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src={previewUrl}
            alt="Captura del dispositivo"
            className="rounded-md border border-slate-200 object-cover"
            style={{ width: 160, height: 160 }}
          />
          <div className="flex gap-2">
            <PrimaryButton size="sm" icon={Check} type="button" onClick={handleAccept}>
              Aceptar
            </PrimaryButton>
            <PrimaryButton size="sm" variant="outline" icon={RotateCcw} type="button" onClick={resetCapture}>
              Recapturar
            </PrimaryButton>
          </div>
        </div>
      )}

      {kioskState === 'timeout' && (
        <div className="space-y-3">
          <div
            role="alert"
            className="flex items-start gap-3 rounded-md bg-amber-50 border border-amber-200 px-4 py-3 text-sm text-amber-700"
          >
            <AlertTriangle size={16} className="mt-0.5 shrink-0" />
            <span>No se detectó captura. El dispositivo no respondió a tiempo.</span>
          </div>
          <PrimaryButton size="sm" variant="outline" icon={RotateCcw} type="button" onClick={resetCapture}>
            Reintentar
          </PrimaryButton>
        </div>
      )}
    </div>
  )
}
