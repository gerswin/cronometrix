'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { startOfWeek, endOfWeek, format } from 'date-fns'
import { Plus } from 'lucide-react'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { WeekNavigator } from '@/components/timesheet/week-navigator'
import { TimesheetTable } from '@/components/timesheet/timesheet-table'
import { NovedadModal } from '@/components/timesheet/novedad-modal'
import { useAuth } from '@/hooks/use-auth'
import { PrimaryButton } from '@/components/ui/primary-button'
import type { PaginatedResponse, DailyRecord, Employee } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const PAGE_SIZE = 50

export default function TimesheetPage() {
  const { role } = useAuth()
  const [currentDate, setCurrentDate] = useState(new Date())
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: PAGE_SIZE,
  })
  const [selectedRecord, setSelectedRecord] = useState<DailyRecord | null>(null)
  const [modalOpen, setModalOpen] = useState(false)

  // Always use Monday as week start (Pitfall 7 — LOTTT work week)
  const weekStart = format(startOfWeek(currentDate, { weekStartsOn: 1 }), 'yyyy-MM-dd')
  const weekEnd = format(endOfWeek(currentDate, { weekStartsOn: 1 }), 'yyyy-MM-dd')

  const { data, isLoading } = useQuery<PaginatedResponse<DailyRecord>>({
    queryKey: ['daily-records', weekStart, weekEnd, pagination.pageIndex],
    queryFn: () =>
      api
        .get('/daily-records', {
          params: {
            from_date: weekStart,
            to_date: weekEnd,
            limit: PAGE_SIZE,
            offset: pagination.pageIndex * PAGE_SIZE,
          },
        })
        .then((r) => r.data),
  })

  // The /daily-records payload carries employee_id but not employee_name, so we
  // join it client-side against the employee directory (paginated to cover the
  // full roster, since the list endpoint caps each page at 100). Without this the
  // table falls back to rendering the raw employee_id (issue #3).
  const { data: employeeNames } = useQuery<Map<string, string>>({
    queryKey: ['employee-name-map'],
    queryFn: async () => {
      const map = new Map<string, string>()
      const limit = 100
      let offset = 0
      for (;;) {
        const page = await api
          .get('/employees', { params: { limit, offset } })
          .then((r) => r.data as PaginatedResponse<Employee>)
        for (const emp of page.data) map.set(emp.id, emp.name)
        offset += limit
        if (page.data.length === 0 || offset >= page.total) break
      }
      return map
    },
  })

  const records: DailyRecord[] = (data?.data ?? []).map((rec) => ({
    ...rec,
    employee_name: rec.employee_name ?? employeeNames?.get(rec.employee_id),
  }))

  const handleEditClick = (record: DailyRecord) => {
    setSelectedRecord(record)
    setModalOpen(true)
  }

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Marcaciones" />
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <WeekNavigator currentDate={currentDate} onChange={setCurrentDate} />
          {role === 'admin' && (
            <PrimaryButton
              size="sm"
              icon={Plus}
              data-testid="open-novedad-modal"
              onClick={() => {
                setSelectedRecord(null)
                setModalOpen(true)
              }}
            >
              Registrar Novedad
            </PrimaryButton>
          )}
        </div>

        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          {isLoading ? (
            <div className="p-8 text-center text-slate-400 text-sm">
              Cargando…
            </div>
          ) : (
            <TimesheetTable
              data={records}
              total={data?.total ?? 0}
              pagination={pagination}
              onPaginationChange={setPagination}
              onEditClick={handleEditClick}
            />
          )}
        </div>
      </div>

      <NovedadModal
        open={modalOpen}
        record={selectedRecord}
        onClose={() => {
          setModalOpen(false)
          setSelectedRecord(null)
        }}
      />
    </div>
  )
}
