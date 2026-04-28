'use client'
import { useEffect, useRef, useState } from 'react'
import { CheckCircle2, XCircle, Loader2 } from 'lucide-react'
import { Skeleton } from '@/components/ui/skeleton'
import { loadFaceApi, analyzeFrame, type FrameAnalysis } from '@/lib/face-detection'

interface ValidationPanelProps {
  videoRef: React.RefObject<HTMLVideoElement | null>
  onValidationChange: (allGreen: boolean) => void
  active: boolean  // true when webcam tab is active
}

type PillState = 'evaluating' | 'ok' | 'fail'

interface CheckRow {
  label: string
  state: PillState
}

const INTERVAL_MS = 500

export function ValidationPanel({ videoRef, onValidationChange, active }: ValidationPanelProps) {
  const [loadingModel, setLoadingModel] = useState(true)
  const [analysis, setAnalysis] = useState<FrameAnalysis | null>(null)
  const faceapiRef = useRef<typeof import('@vladmandic/face-api') | null>(null)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  // Lazy-load model on mount
  useEffect(() => {
    let cancelled = false
    loadFaceApi().then((fa) => {
      if (!cancelled) {
        faceapiRef.current = fa
        setLoadingModel(false)
      }
    }).catch(() => {
      if (!cancelled) setLoadingModel(false)
    })
    return () => { cancelled = true }
  }, [])

  // Per-frame analysis loop
  useEffect(() => {
    if (loadingModel || !active || !faceapiRef.current) return

    // Create offscreen canvas for luminance sampling
    if (!canvasRef.current) {
      canvasRef.current = document.createElement('canvas')
    }
    const canvas = canvasRef.current

    const id = setInterval(async () => {
      const video = videoRef.current
      if (!video || video.readyState < 2) return
      if (!faceapiRef.current) return
      try {
        const result = await analyzeFrame(video, canvas, faceapiRef.current)
        setAnalysis(result)
        onValidationChange(result.faceDetected && result.luminanceOk && result.sizeOk)
      } catch {
        // ignore frame errors
      }
    }, INTERVAL_MS)

    return () => clearInterval(id)
  }, [loadingModel, active, videoRef, onValidationChange])

  const pillClass = (state: PillState) => {
    switch (state) {
      case 'ok': return 'bg-green-100 text-green-700'
      case 'fail': return 'bg-red-100 text-red-700'
      default: return 'bg-slate-100 text-slate-600'
    }
  }

  const rows: CheckRow[] = analysis
    ? [
        { label: 'Rostro Detectado', state: analysis.faceDetected ? 'ok' : 'fail' },
        { label: 'Buena Iluminación', state: analysis.luminanceOk ? 'ok' : 'fail' },
        { label: 'Resolución Óptima', state: analysis.sizeOk ? 'ok' : 'fail' },
      ]
    : [
        { label: 'Rostro Detectado', state: 'evaluating' },
        { label: 'Buena Iluminación', state: 'evaluating' },
        { label: 'Resolución Óptima', state: 'evaluating' },
      ]

  return (
    <div className="space-y-2">
      <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Validación de IA</p>

      {loadingModel ? (
        <div className="space-y-2">
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-full" />
          <p className="text-xs text-slate-400 text-center">Cargando modelo de IA…</p>
        </div>
      ) : !active ? (
        <p className="text-xs text-slate-400">Inicia la captura para evaluar.</p>
      ) : (
        <div className="space-y-1.5">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center justify-between">
              <span className="text-xs text-slate-700">{row.label}</span>
              <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${pillClass(row.state)}`}>
                {row.state === 'evaluating' && <Loader2 size={10} className="animate-spin" />}
                {row.state === 'ok' && <CheckCircle2 size={10} />}
                {row.state === 'fail' && <XCircle size={10} />}
                {row.state === 'evaluating' ? 'Evaluando…' : row.state === 'ok' ? 'OK' : 'Falla'}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
