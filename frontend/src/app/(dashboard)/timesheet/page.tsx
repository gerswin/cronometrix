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
import type { PaginatedResponse, DailyRecord } from '@/types/api'
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
              data={data?.data ?? []}
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
