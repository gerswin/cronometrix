'use client'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { CheckCircle2, XCircle, Loader2, Clock } from 'lucide-react'
import { api } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import type { EnrollmentDevicePush } from '@/types/api'

interface SyncRowProps {
  push: EnrollmentDevicePush
  enrollmentId: string
}

function StatusPill({ status }: { status: EnrollmentDevicePush['status'] }) {
  const map: Record<EnrollmentDevicePush['status'], { cls: string; icon: React.ReactNode; label: string }> = {
    pending:     { cls: 'bg-slate-100 text-slate-600',  icon: <Clock size={10} />,             label: 'Esperando' },
    in_progress: { cls: 'bg-slate-100 text-slate-600',  icon: <Loader2 size={10} className="animate-spin" />, label: 'Enviando' },
    success:     { cls: 'bg-green-100 text-green-700',  icon: <CheckCircle2 size={10} />,       label: 'Sincronizado' },
    failed:      { cls: 'bg-red-100 text-red-700',      icon: <XCircle size={10} />,             label: 'Falló' },
  }
  const { cls, icon, label } = map[status]
  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${cls}`}>
      {icon}
      {label}
    </span>
  )
}

export function SyncRow({ push, enrollmentId }: SyncRowProps) {
  const retryMutation = useMutation({
    mutationFn: () =>
      api.post(`/enrollments/${enrollmentId}/devices/${push.device_id}/retry`).then(r => r.data),
    onError: (err: unknown) => {
      const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message
        ?? 'No se pudo reintentar la sincronización.'
      toast.error(msg)
    },
  })

  const progressValue = push.status === 'success' ? 100 : 0

  return (
    <div
      aria-live="polite"
      className="flex flex-col gap-1.5 py-2 border-b border-slate-100 last:border-0"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium text-slate-700 truncate">{push.device_name}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <StatusPill status={push.status} />
          {push.status === 'failed' && (
            <Button
              size="sm"
              variant="outline"
              type="button"
              disabled={retryMutation.isPending}
              onClick={() => retryMutation.mutate()}
            >
              {retryMutation.isPending ? <Loader2 size={12} className="animate-spin" /> : 'Reintentar'}
            </Button>
          )}
        </div>
      </div>

      <Progress value={progressValue} className="h-1" />

      {push.error_message && (
        <p className="text-xs text-red-700">{push.error_message}</p>
      )}
    </div>
  )
}
