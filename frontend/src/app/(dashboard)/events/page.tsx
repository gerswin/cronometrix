'use client'
import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import type { PaginationState } from '@tanstack/react-table'
import { startOfMonth, endOfMonth } from 'date-fns'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { EventsTable } from '@/components/events/events-table'
import {
  EventsFilters,
  type EventsFilterState,
} from '@/components/events/events-filters'
import { EventDetailDialog } from '@/components/events/event-detail-dialog'
import type {
  RawAttendanceEvent,
  Employee,
  Device,
  PaginatedResponse,
} from '@/types/api'

const PAGE_SIZE = 50

function currentMonthEpochRange(): EventsFilterState {
  const now = new Date()
  return {
    from: Math.floor(startOfMonth(now).getTime() / 1000),
    to: Math.floor(endOfMonth(now).getTime() / 1000),
  }
}

export default function EventsPage() {
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: PAGE_SIZE,
  })
  const [filters, setFilters] = useState<EventsFilterState>(currentMonthEpochRange)
  const [openEventId, setOpenEventId] = useState<string | null>(null)

  const { data, isLoading } = useQuery<PaginatedResponse<RawAttendanceEvent>>({
    queryKey: ['raw-events', pagination.pageIndex, filters],
    queryFn: () =>
      api
        .get('/events', {
          params: {
            limit: PAGE_SIZE,
            offset: pagination.pageIndex * PAGE_SIZE,
            ...(filters.employee_id && { employee_id: filters.employee_id }),
            ...(filters.device_id && { device_id: filters.device_id }),
            ...(filters.from !== undefined && { from: filters.from }),
            ...(filters.to !== undefined && { to: filters.to }),
            ...(filters.include_unknown && { include_unknown: true }),
          },
        })
        .then((r) => r.data),
  })

  const { data: employeesData } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', 'all-active'],
    queryFn: () =>
      api
        .get('/employees', { params: { status: 'active', limit: 1000 } })
        .then((r) => r.data),
    staleTime: 5 * 60 * 1000,
  })

  const { data: devicesData } = useQuery<PaginatedResponse<Device>>({
    queryKey: ['devices', 'all'],
    queryFn: () =>
      api.get('/devices').then((r) => r.data),
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
  const deviceOptions = useMemo(
    () =>
      (devicesData?.data ?? []).map((d) => ({ id: d.id, label: d.name })),
    [devicesData],
  )

  const employeeNameById = useMemo(() => {
    const m = new Map<string, string>()
    for (const e of employeesData?.data ?? []) m.set(e.id, e.name)
    return m
  }, [employeesData])
  const deviceNameById = useMemo(() => {
    const m = new Map<string, string>()
    for (const d of devicesData?.data ?? []) m.set(d.id, d.name)
    return m
  }, [devicesData])

  return (
    <div className="flex flex-col h-full" data-testid="events-page">
      <TopBar title="Eventos" />
      <div className="p-6 space-y-4">
        <EventsFilters
          value={filters}
          onChange={(next) => {
            setFilters(next)
            setPagination((p) => ({ ...p, pageIndex: 0 }))
          }}
          employees={employeeOptions}
          devices={deviceOptions}
        />
        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          <EventsTable
            data={data?.data ?? []}
            total={data?.total ?? 0}
            pagination={pagination}
            onPaginationChange={setPagination}
            onView={(e) => setOpenEventId(e.id)}
            employeeNameById={employeeNameById}
            deviceNameById={deviceNameById}
            isLoading={isLoading}
            pageSize={PAGE_SIZE}
          />
        </div>
      </div>
      <EventDetailDialog
        eventId={openEventId}
        onClose={() => setOpenEventId(null)}
      />
    </div>
  )
}
