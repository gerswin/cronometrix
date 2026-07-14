'use client'
import { CheckCircle2, Loader2, XCircle } from 'lucide-react'
import { Skeleton } from '@/components/ui/skeleton'
import type { FrameAnalysis } from '@/lib/face-detection'

interface ValidationPanelProps {
  analysis: FrameAnalysis | null
  analyzing: boolean
}

type PillState = 'ok' | 'fail'

function pillClass(state: PillState): string {
  return state === 'ok' ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'
}

export function ValidationPanel({ analysis, analyzing }: ValidationPanelProps) {
  const rows = analysis
    ? [
        { label: 'Rostro Detectado', state: analysis.faceDetected ? 'ok' : 'fail' },
        { label: 'Buena Iluminación', state: analysis.luminanceOk ? 'ok' : 'fail' },
        { label: 'Resolución Óptima', state: analysis.sizeOk ? 'ok' : 'fail' },
      ] satisfies Array<{ label: string; state: PillState }>
    : []

  return (
    <div className="space-y-2">
      <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">
        Validación de IA
      </p>

      {analyzing ? (
        <div className="space-y-2">
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-full" />
          <Skeleton className="h-6 w-full" />
          <p className="text-xs text-slate-400 text-center flex items-center justify-center gap-1">
            <Loader2 size={10} className="animate-spin" />
            Analizando foto…
          </p>
        </div>
      ) : !analysis ? (
        <p className="text-xs text-slate-400">Captura una foto para evaluar.</p>
      ) : (
        <div className="space-y-1.5">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center justify-between">
              <span className="text-xs text-slate-700">{row.label}</span>
              <span
                className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${pillClass(row.state)}`}
              >
                {row.state === 'ok' ? <CheckCircle2 size={10} /> : <XCircle size={10} />}
                {row.state === 'ok' ? 'OK' : 'Falla'}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
