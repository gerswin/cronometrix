'use client'
import { useCallback, useEffect, useRef, useState } from 'react'
import { AlertTriangle, Camera, Check, RotateCcw } from 'lucide-react'
import type { CapturedPhotoCandidate } from '@/lib/enrollment-api'
import { PrimaryButton } from '@/components/ui/primary-button'

interface WebcamCaptureTabProps {
  onCaptured: (candidate: CapturedPhotoCandidate) => void
  onCleared: () => void
}

const MEDIA_CONSTRAINTS: MediaStreamConstraints = {
  video: { width: { ideal: 640 }, height: { ideal: 480 }, facingMode: 'user' },
  audio: false,
}

export function WebcamCaptureTab({ onCaptured, onCleared }: WebcamCaptureTabProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const captureCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const mountedRef = useRef(false)
  const streamGenerationRef = useRef(0)
  const captureGenerationRef = useRef(0)
  const previewUrlRef = useRef<string | null>(null)
  const [permissionDenied, setPermissionDenied] = useState(false)
  const [frozenBlob, setFrozenBlob] = useState<Blob | null>(null)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)

  const requestStream = useCallback(async (reportAnyError: boolean) => {
    const generation = ++streamGenerationRef.current
    try {
      const stream = await navigator.mediaDevices.getUserMedia(MEDIA_CONSTRAINTS)
      if (!mountedRef.current || generation !== streamGenerationRef.current) {
        stream.getTracks().forEach((track) => track.stop())
        return
      }

      streamRef.current?.getTracks().forEach((track) => track.stop())
      streamRef.current = stream
      const video = videoRef.current
      if (video) {
        video.srcObject = stream
        video.play().catch(() => {})
      }
    } catch (error) {
      if (!mountedRef.current || generation !== streamGenerationRef.current) return
      if (reportAnyError || error instanceof DOMException) setPermissionDenied(true)
    }
  }, [])

  useEffect(() => {
    mountedRef.current = true
    queueMicrotask(() => { void requestStream(false) })
    return () => {
      mountedRef.current = false
      streamGenerationRef.current += 1
      captureGenerationRef.current += 1
      streamRef.current?.getTracks().forEach((track) => track.stop())
      streamRef.current = null
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
      previewUrlRef.current = null
    }
  }, [requestStream])

  function captureFrame() {
    const video = videoRef.current
    if (!video) return

    const canvas = captureCanvasRef.current ?? document.createElement('canvas')
    captureCanvasRef.current = canvas
    canvas.width = 640
    canvas.height = 480
    const context = canvas.getContext('2d')
    if (!context) return
    context.drawImage(video, 0, 0, 640, 480)
    const generation = ++captureGenerationRef.current

    canvas.toBlob((blob) => {
      if (!blob || !mountedRef.current || generation !== captureGenerationRef.current) return
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
      const objectUrl = URL.createObjectURL(blob)
      previewUrlRef.current = objectUrl
      setFrozenBlob(blob)
      setPreviewUrl(objectUrl)
      streamRef.current?.getTracks().forEach((track) => track.stop())
      streamRef.current = null
    }, 'image/jpeg', 0.92)
  }

  function accept() {
    if (!frozenBlob) return
    onCaptured({ blob: frozenBlob, capturedVia: 'webcam', sourceDeviceId: null })
  }

  function retake() {
    captureGenerationRef.current += 1
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
    previewUrlRef.current = null
    setFrozenBlob(null)
    setPreviewUrl(null)
    setPermissionDenied(false)
    onCleared()
    void requestStream(true)
  }

  if (permissionDenied) {
    return (
      <div
        role="alert"
        className="flex items-start gap-3 rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700"
      >
        <AlertTriangle size={16} className="mt-0.5 shrink-0" />
        <span>El navegador bloqueó la cámara. Habilita el acceso y vuelve a intentar.</span>
      </div>
    )
  }

  const frozen = frozenBlob !== null

  return (
    <div className="space-y-3">
      <div className="relative rounded-md overflow-hidden bg-slate-900" style={{ width: 320, height: 240 }}>
        <video
          ref={videoRef}
          className={`w-full h-full object-cover ${frozen ? 'hidden' : ''}`}
          muted
          playsInline
        />
        {frozen && previewUrl && (
          // Blob-backed user preview cannot use the Next image optimizer.
          // eslint-disable-next-line @next/next/no-img-element
          <img src={previewUrl} alt="Captura" className="w-full h-full object-cover" />
        )}
        <canvas ref={captureCanvasRef} className="hidden" />
      </div>

      {!frozen ? (
        <PrimaryButton size="sm" icon={Camera} onClick={captureFrame} type="button">
          Capturar Rostro
        </PrimaryButton>
      ) : (
        <div className="flex gap-2">
          <PrimaryButton size="sm" icon={Check} onClick={accept} type="button">
            Aceptar
          </PrimaryButton>
          <PrimaryButton size="sm" variant="outline" icon={RotateCcw} onClick={retake} type="button">
            Recapturar
          </PrimaryButton>
        </div>
      )}
    </div>
  )
}
