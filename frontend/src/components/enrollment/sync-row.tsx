'use client'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { CheckCircle2, XCircle, Loader2, Clock, RotateCcw } from 'lucide-react'
import { retryEnrollmentPush } from '@/lib/enrollment-api'
import { PrimaryButton } from '@/components/ui/primary-button'
import { Progress } from '@/components/ui/progress'
import type { EnrollmentDevicePush } from '@/types/api'

interface SyncRowProps {
  push: EnrollmentDevicePush
  enrollmentId: string
}

function StatusPill({
  status,
  deviceId,
}: {
  status: EnrollmentDevicePush['status']
  deviceId: string
}) {
  const map: Record<EnrollmentDevicePush['status'], { cls: string; icon: React.ReactNode; label: string }> = {
    pending:     { cls: 'bg-slate-100 text-slate-600',  icon: <Clock size={10} />,             label: 'Esperando' },
    in_progress: { cls: 'bg-slate-100 text-slate-600',  icon: <Loader2 size={10} className="animate-spin" />, label: 'Enviando' },
    success:     { cls: 'bg-green-100 text-green-700',  icon: <CheckCircle2 size={10} />,       label: 'Sincronizado' },
    failed:      { cls: 'bg-red-100 text-red-700',      icon: <XCircle size={10} />,             label: 'Falló' },
  }
  const { cls, icon, label } = map[status]
  return (
    <span
      data-testid={`enrollment-push-status-${deviceId}`}
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${cls}`}
    >
      {icon}
      {label}
    </span>
  )
}

export function SyncRow({ push, enrollmentId }: SyncRowProps) {
  const queryClient = useQueryClient()
  const retryMutation = useMutation({
    mutationFn: () => retryEnrollmentPush(enrollmentId, push.device_id),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['enrollment', enrollmentId] }),
        queryClient.invalidateQueries({ queryKey: ['enrollments', 'in_progress'] }),
      ])
    },
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
      data-testid={`enrollment-push-row-${push.device_id}`}
      className="flex flex-col gap-1.5 py-2 border-b border-slate-100 last:border-0"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium text-slate-700 truncate">{push.device_name}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <StatusPill status={push.status} deviceId={push.device_id} />
          {push.status === 'failed' && (
            <PrimaryButton
              size="sm"
              variant="outline"
              type="button"
              icon={retryMutation.isPending ? Loader2 : RotateCcw}
              disabled={retryMutation.isPending}
              onClick={() => retryMutation.mutate()}
              data-testid={`enrollment-retry-${push.device_id}`}
            >
              {retryMutation.isPending ? 'Reintentando…' : 'Reintentar'}
            </PrimaryButton>
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
