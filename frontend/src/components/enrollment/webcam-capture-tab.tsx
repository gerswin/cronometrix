'use client'
import { useEffect, useRef, useState } from 'react'
import { Camera, Check, RotateCcw, AlertTriangle } from 'lucide-react'
import { PrimaryButton } from '@/components/ui/primary-button'

interface WebcamCaptureTabProps {
  onCaptured: (blob: Blob) => void
  onValidationChange: (allGreen: boolean) => void
}

export function WebcamCaptureTab({ onCaptured, onValidationChange }: WebcamCaptureTabProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const captureCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const [permissionDenied, setPermissionDenied] = useState(false)
  const [frozen, setFrozen] = useState(false)
  const [frozenBlob, setFrozenBlob] = useState<Blob | null>(null)

  // Start webcam on mount, stop on unmount (Pitfall 6)
  useEffect(() => {
    let cancelled = false

    navigator.mediaDevices
      .getUserMedia({
        video: { width: { ideal: 640 }, height: { ideal: 480 }, facingMode: 'user' },
        audio: false,
      })
      .then((stream) => {
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop())
          return
        }
        streamRef.current = stream
        if (videoRef.current) {
          videoRef.current.srcObject = stream
          videoRef.current.play().catch(() => {})
        }
      })
      .catch((err) => {
        if (!cancelled && err instanceof DOMException) {
          setPermissionDenied(true)
        }
      })

    return () => {
      cancelled = true
      // Pitfall 6: stop all tracks on unmount
      streamRef.current?.getTracks().forEach((t) => t.stop())
      streamRef.current = null
    }
  }, [])

  function captureFrame() {
    const video = videoRef.current
    if (!video) return

    const canvas = captureCanvasRef.current ?? document.createElement('canvas')
    captureCanvasRef.current = canvas
    canvas.width = 640
    canvas.height = 480
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(video, 0, 0, 640, 480)

    canvas.toBlob(
      (blob) => {
        if (!blob) return
        setFrozenBlob(blob)
        setFrozen(true)
        // Stop the stream so the camera light turns off
        streamRef.current?.getTracks().forEach((t) => t.stop())
        streamRef.current = null
      },
      'image/jpeg',
      0.92
    )
  }

  function accept() {
    if (frozenBlob) {
      onCaptured(frozenBlob)
    }
  }

  function retake() {
    setFrozen(false)
    setFrozenBlob(null)
    onValidationChange(false)

    // Restart stream
    navigator.mediaDevices
      .getUserMedia({
        video: { width: { ideal: 640 }, height: { ideal: 480 }, facingMode: 'user' },
        audio: false,
      })
      .then((stream) => {
        streamRef.current = stream
        if (videoRef.current) {
          videoRef.current.srcObject = stream
          videoRef.current.play().catch(() => {})
        }
      })
      .catch(() => setPermissionDenied(true))
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

  return (
    <div className="space-y-3">
      <div className="relative rounded-md overflow-hidden bg-slate-900" style={{ width: 320, height: 240 }}>
        {/* Live preview */}
        <video
          ref={videoRef}
          className={`w-full h-full object-cover ${frozen ? 'hidden' : ''}`}
          muted
          playsInline
        />
        {/* Frozen frame preview */}
        {frozen && frozenBlob && (
          <img
            src={URL.createObjectURL(frozenBlob)}
            alt="Captura"
            className="w-full h-full object-cover"
          />
        )}
        {/* Hidden capture canvas */}
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
