'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { format, startOfMonth, endOfMonth } from 'date-fns'
import { api } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { TopBar } from '@/components/layout/top-bar'
import { PeriodPicker } from '@/components/reports/period-picker'
import { FiltersBar } from '@/components/reports/filters-bar'
import { SummaryTable } from '@/components/reports/summary-table'
import { DrillDownDialog } from '@/components/reports/drill-down-dialog'
import { ExportButtons } from '@/components/reports/export-buttons'
import type {
  ReportPayload,
  ReportFilters,
  DeptSummary,
  PaginatedResponse,
  Department,
} from '@/types/api'

export default function ReportsPage() {
  const { role } = useAuth()
  const today = new Date()
  const [filters, setFilters] = useState<ReportFilters>({
    period_type: 'monthly',
    from_date: format(startOfMonth(today), 'yyyy-MM-dd'),
    to_date: format(endOfMonth(today), 'yyyy-MM-dd'),
    include_inactive: false,
  })
  const [drillDownEmployeeId, setDrillDownEmployeeId] =
    useState<string | null>(null)

  const departmentsQ = useQuery<DeptSummary[]>({
    queryKey: ['departments-list'],
    queryFn: () =>
      api
        .get<PaginatedResponse<Department>>('/departments')
        .then((r) =>
          r.data.data.map((d) => ({ id: d.id, name: d.name })),
        ),
    staleTime: 300_000,
  })

  const reportQ = useQuery<ReportPayload>({
    queryKey: ['reports', filters],
    queryFn: () =>
      api.post<ReportPayload>('/reports/json', filters).then((r) => r.data),
    enabled: false,
  })

  const canExport = role === 'admin' || role === 'supervisor'

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Reportes" />
      <div className="p-6 space-y-4">
        {/* Filter row */}
        <div className="flex items-end gap-3 flex-wrap">
          <PeriodPicker value={filters} onChange={setFilters} />
          <FiltersBar
            value={filters}
            onChange={setFilters}
            departments={departmentsQ.data ?? []}
          />
          <div className="flex-1" />
          {canExport && (
            <button
              type="button"
              onClick={() => reportQ.refetch()}
              disabled={reportQ.isFetching}
              className="px-4 py-2 bg-slate-900 text-white text-sm rounded-md hover:bg-slate-800 disabled:opacity-50"
            >
              {reportQ.isFetching ? 'Generando…' : 'Emitir Reporte'}
            </button>
          )}
          {canExport && reportQ.data && (
            <ExportButtons filters={filters} payload={reportQ.data} />
          )}
        </div>

        {/* Summary table */}
        <SummaryTable
          payload={reportQ.data}
          isLoading={reportQ.isFetching}
          onDrillDown={setDrillDownEmployeeId}
        />
      </div>

      <DrillDownDialog
        employeeId={drillDownEmployeeId}
        from={filters.from_date}
        to={filters.to_date}
        onClose={() => setDrillDownEmployeeId(null)}
      />
    </div>
  )
}
