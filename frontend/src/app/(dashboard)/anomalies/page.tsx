'use client'
import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import type { PaginationState } from '@tanstack/react-table'
import { format, startOfMonth, endOfMonth } from 'date-fns'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { AccessRestricted } from '@/components/common/access-restricted'
import { useAuth } from '@/hooks/use-auth'
import { AnomaliesTable } from '@/components/anomalies/anomalies-table'
import {
  AnomaliesFilters,
  type AnomaliesFilterState,
} from '@/components/anomalies/anomalies-filters'
import { DailyRecordDialog } from '@/components/daily-records/daily-record-dialog'
import type {
  Anomaly,
  Employee,
  PaginatedResponse,
} from '@/types/api'

const PAGE_SIZE = 20

function currentMonthRange(): AnomaliesFilterState {
  const now = new Date()
  return {
    from_date: format(startOfMonth(now), 'yyyy-MM-dd'),
    to_date: format(endOfMonth(now), 'yyyy-MM-dd'),
  }
}

export default function AnomaliesPage() {
  const { role } = useAuth()
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: PAGE_SIZE,
  })
  const [filters, setFilters] = useState<AnomaliesFilterState>(currentMonthRange)
  const [openRecordId, setOpenRecordId] = useState<string | null>(null)

  const canRead = role === 'admin' || role === 'supervisor'

  const { data, isLoading } = useQuery<PaginatedResponse<Anomaly>>({
    queryKey: ['anomalies', pagination.pageIndex, filters],
    queryFn: () =>
      api
        .get('/anomalies', {
          params: {
            limit: PAGE_SIZE,
            offset: pagination.pageIndex * PAGE_SIZE,
            ...(filters.code && { code: filters.code }),
            ...(filters.employee_id && { employee_id: filters.employee_id }),
            ...(filters.from_date && { from_date: filters.from_date }),
            ...(filters.to_date && { to_date: filters.to_date }),
          },
        })
        .then((r) => r.data),
    enabled: canRead,
  })

  const { data: employeesData } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', 'all-active'],
    queryFn: () =>
      api
        .get('/employees', { params: { status: 'active', limit: 1000 } })
        .then((r) => r.data),
    enabled: canRead,
    staleTime: 5 * 60 * 1000,
  })

  const employeeOptions = useMemo(
    () =>
      (employeesData?.data ?? []).map((e) => ({
        id: e.id,
        label: e.name,
        sublabel: e.employee_code,
      })),
    [employeesData],
  )

  const employeeNameById = useMemo(() => {
    const m = new Map<string, string>()
    for (const e of employeesData?.data ?? []) m.set(e.id, e.name)
    return m
  }, [employeesData])

  if (!canRead) return <AccessRestricted />

  return (
    <div className="flex flex-col h-full" data-testid="anomalies-page">
      <TopBar title="Anomalías" />
      <div className="p-6 space-y-4">
        <AnomaliesFilters
          value={filters}
          onChange={(next) => {
            setFilters(next)
            setPagination((p) => ({ ...p, pageIndex: 0 }))
          }}
          employees={employeeOptions}
        />
        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          <AnomaliesTable
            data={data?.data ?? []}
            total={data?.total ?? 0}
            pagination={pagination}
            onPaginationChange={setPagination}
            onView={(a) => setOpenRecordId(a.daily_record_id)}
            employeeNameById={employeeNameById}
            isLoading={isLoading}
            pageSize={PAGE_SIZE}
          />
        </div>
      </div>
      <DailyRecordDialog
        recordId={openRecordId}
        onClose={() => setOpenRecordId(null)}
      />
    </div>
  )
}
