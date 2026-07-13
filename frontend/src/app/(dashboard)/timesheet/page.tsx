'use client'
import { Suspense, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { startOfWeek, endOfWeek, format, isValid, parseISO } from 'date-fns'
import { useRouter, useSearchParams } from 'next/navigation'
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

function parseAnchorDate(value: string | null): Date | null {
  if (!value) return null
  const parsed = parseISO(value)
  return isValid(parsed) && format(parsed, 'yyyy-MM-dd') === value ? parsed : null
}

function TimesheetContent() {
  const searchParams = useSearchParams()
  const employeeId = searchParams.get('employee_id')?.trim() || null
  const requestedAnchor = searchParams.get('anchor_date')
  const parsedAnchor = parseAnchorDate(requestedAnchor)
  const anchorDate = parsedAnchor ? format(parsedAnchor, 'yyyy-MM-dd') : null
  const currentDate = parsedAnchor ?? new Date()
  const filterKey = `${employeeId ?? ''}:${anchorDate ?? ''}`

  return (
    <TimesheetView
      key={filterKey}
      employeeId={employeeId}
      anchorDate={anchorDate}
      currentDate={currentDate}
      searchParamsString={searchParams.toString()}
    />
  )
}

interface TimesheetViewProps {
  employeeId: string | null
  anchorDate: string | null
  currentDate: Date
  searchParamsString: string
}

function TimesheetView({
  employeeId,
  anchorDate,
  currentDate,
  searchParamsString,
}: TimesheetViewProps) {
  const { role } = useAuth()
  const router = useRouter()
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
    queryKey: ['daily-records', employeeId, anchorDate, weekStart, weekEnd, pagination.pageIndex],
    queryFn: () =>
      api
        .get('/daily-records', {
          params: {
            from_date: weekStart,
            to_date: weekEnd,
            ...(employeeId ? { employee_id: employeeId } : {}),
            limit: PAGE_SIZE,
            offset: pagination.pageIndex * PAGE_SIZE,
          },
        })
        .then((r) => r.data),
  })

  const records: DailyRecord[] = data?.data ?? []

  const handleWeekChange = (date: Date) => {
    const next = new URLSearchParams(searchParamsString)
    next.set('anchor_date', format(date, 'yyyy-MM-dd'))
    router.replace(`/timesheet?${next.toString()}`)
  }

  const handlePaginationChange = (next: PaginationState) => {
    setPagination(next)
  }

  const handleEditClick = (record: DailyRecord) => {
    setSelectedRecord(record)
    setModalOpen(true)
  }

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Marcaciones" />
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <WeekNavigator currentDate={currentDate} onChange={handleWeekChange} />
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
              onPaginationChange={handlePaginationChange}
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

export default function TimesheetPage() {
  return (
    <Suspense fallback={<div className="p-8 text-center text-slate-400 text-sm">Cargando…</div>}>
      <TimesheetContent />
    </Suspense>
  )
}
