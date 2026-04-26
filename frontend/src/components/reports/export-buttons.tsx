'use client'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import { renderReportPdf } from '@/lib/reports/pdf'
import type { ReportFilters, ReportPayload } from '@/types/api'

interface Props {
  filters: ReportFilters
  payload?: ReportPayload
}

export function ExportButtons({ filters, payload: _payload }: Props) {
  // _payload is accepted as a prop so the screen can pass the in-memory
  // data (e.g. for a future "Export current view" optimisation), but
  // Excel/PDF both re-request from the backend so the export reflects
  // the freshest data and shares the audit-log entry.
  void _payload

  const exportExcelMutation = useMutation({
    mutationFn: async () => {
      const resp = await api.post('/reports/excel', filters, {
        responseType: 'blob',
      })
      const blob = new Blob([resp.data], {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `prenomina_${filters.from_date}_${filters.to_date}.xlsx`
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
    },
    onSuccess: () => toast.success('Reporte Excel descargado'),
    onError: () => toast.error('Error al generar el Excel'),
  })

  const exportPdfMutation = useMutation({
    mutationFn: async () => {
      const resp = await api.post<ReportPayload>('/reports/json', filters)
      renderReportPdf(resp.data)
    },
    onSuccess: () => toast.success('Reporte PDF generado'),
    onError: () => toast.error('Error al generar el PDF'),
  })

  return (
    <div className="flex items-end gap-2">
      <button
        type="button"
        onClick={() => exportExcelMutation.mutate()}
        disabled={exportExcelMutation.isPending}
        aria-label="Exportar Excel"
        className="px-4 py-2 border border-slate-200 text-sm rounded-md hover:bg-slate-50 disabled:opacity-50"
      >
        {exportExcelMutation.isPending ? 'Generando…' : 'Exportar Excel'}
      </button>
      <button
        type="button"
        onClick={() => exportPdfMutation.mutate()}
        disabled={exportPdfMutation.isPending}
        aria-label="Exportar PDF"
        className="px-4 py-2 border border-slate-200 text-sm rounded-md hover:bg-slate-50 disabled:opacity-50"
      >
        {exportPdfMutation.isPending ? 'Generando…' : 'Exportar PDF'}
      </button>
    </div>
  )
}
