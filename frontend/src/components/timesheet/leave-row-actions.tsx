'use client'
import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Paperclip, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import { openBlobInNewTab } from '@/lib/file-download'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import { useAuth } from '@/hooks/use-auth'
import type { Leave } from '@/types/api'

interface Props {
  leaveId: string
}

// Per-row actions for a daily-record that has an associated leave. The leave
// is fetched lazily on hover so the table doesn't N+1 GET /leaves/{id} for
// every row at mount time. TanStack Query caches per-id, so subsequent
// hovers/clicks on the same row reuse the result.
export function LeaveRowActions({ leaveId }: Props) {
  const { role } = useAuth()
  const queryClient = useQueryClient()
  const [shouldFetch, setShouldFetch] = useState(false)
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [downloading, setDownloading] = useState(false)

  const { data: leave } = useQuery<Leave>({
    queryKey: ['leaves', leaveId],
    queryFn: () => api.get(`/leaves/${leaveId}`).then((r) => r.data),
    enabled: shouldFetch,
    staleTime: 30_000,
  })

  const cancelMutation = useMutation({
    mutationFn: async () => {
      if (!leave) throw new Error('Leave not loaded')
      await api.delete(`/leaves/${leaveId}`, {
        params: { version: leave.version },
      })
    },
    onSuccess: () => {
      toast.success('Novedad cancelada')
      setConfirmOpen(false)
      queryClient.invalidateQueries({ queryKey: ['daily-records'] })
      queryClient.invalidateQueries({ queryKey: ['leaves'] })
      queryClient.invalidateQueries({ queryKey: ['leaves', leaveId] })
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response
        ?.status
      if (status === 409) {
        toast.error('Esta novedad cambió; recarga e intenta de nuevo')
        queryClient.invalidateQueries({ queryKey: ['leaves', leaveId] })
      } else {
        toast.error('No se pudo cancelar la novedad')
      }
    },
  })

  async function handleEvidenceDownload() {
    if (downloading) return
    setDownloading(true)
    try {
      const resp = await api.get(`/leaves/${leaveId}/evidence`, {
        responseType: 'blob',
      })
      const contentType =
        (resp.headers as Record<string, string>)['content-type'] ??
        'application/octet-stream'
      openBlobInNewTab(new Blob([resp.data as BlobPart], { type: contentType }))
    } catch (err) {
      const status = (err as { response?: { status?: number } })?.response
        ?.status
      if (status === 404) {
        toast.error('Esta novedad no tiene evidencia adjunta')
      } else {
        toast.error('No se pudo descargar la evidencia')
      }
    } finally {
      setDownloading(false)
    }
  }

  // Trigger lazy fetch on first hover. After fetch resolves we know whether
  // the leave has evidence_path and what version to send to DELETE.
  const onMouseEnter = () => setShouldFetch(true)

  const hasEvidence = !!leave?.evidence_path
  const isCancelled = leave?.status === 'cancelled'

  return (
    <span
      className="inline-flex items-center gap-1"
      onMouseEnter={onMouseEnter}
      onFocus={onMouseEnter}
    >
      {hasEvidence && (
        <button
          type="button"
          data-testid="leave-evidence-button"
          onClick={handleEvidenceDownload}
          disabled={downloading}
          className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700 disabled:opacity-50"
          aria-label="Ver evidencia"
          title="Ver evidencia"
        >
          <Paperclip size={14} />
        </button>
      )}
      {role === 'admin' && !isCancelled && (
        <>
          <button
            type="button"
            data-testid="leave-cancel-button"
            onClick={() => setConfirmOpen(true)}
            disabled={!leave}
            className="p-1 rounded hover:bg-red-50 text-slate-500 hover:text-red-600 disabled:opacity-50"
            aria-label="Cancelar novedad"
            title="Cancelar novedad"
          >
            <Trash2 size={14} />
          </button>
          <Dialog
            open={confirmOpen}
            onOpenChange={(o: boolean) => {
              if (!o) setConfirmOpen(false)
            }}
          >
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Cancelar novedad</DialogTitle>
              </DialogHeader>
              <p className="text-sm text-slate-600">
                ¿Cancelar esta novedad? El registro diario volverá a calcularse
                sin el justificativo.
              </p>
              <DialogFooter className="gap-2 mt-4">
                <PrimaryButton
                  type="button"
                  variant="outline"
                  size="md"
                  onClick={() => setConfirmOpen(false)}
                >
                  Volver
                </PrimaryButton>
                <PrimaryButton
                  type="button"
                  variant="danger"
                  size="md"
                  icon={Trash2}
                  onClick={() => cancelMutation.mutate()}
                  disabled={cancelMutation.isPending}
                >
                  {cancelMutation.isPending ? 'Cancelando…' : 'Cancelar novedad'}
                </PrimaryButton>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </>
      )}
    </span>
  )
}
