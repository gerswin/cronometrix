'use client'
import { useQuery } from '@tanstack/react-query'
import { listInProgressEnrollments } from '@/lib/enrollment-api'
import { Badge } from '@/components/ui/badge'

interface InProgressListProps {
  onReopen: (enrollmentId: string) => void
}

export function InProgressList({ onReopen }: InProgressListProps) {
  const { data } = useQuery({
    queryKey: ['enrollments', 'in_progress'],
    queryFn: () => listInProgressEnrollments({ limit: 100 }),
  })
  const enrollments = data?.data ?? []

  if (enrollments.length === 0) return null

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
      <p className="text-sm font-semibold text-slate-700 mb-2">Enrolamientos en curso</p>
      <div className="divide-y divide-slate-100">
        {enrollments.map((enrollment) => {
          const successCount = enrollment.device_pushes.filter(
            (push) => push.status === 'success',
          ).length
          return (
            <div
              key={enrollment.id}
              data-testid={`enrollment-row-${enrollment.id}`}
              className="flex items-center justify-between py-2 first:pt-0 last:pb-0"
            >
              <div className="flex items-center gap-2">
                <span className="text-sm text-slate-700">{enrollment.employee_name}</span>
                <span className="text-xs text-slate-400">{enrollment.employee_code}</span>
                <Badge variant="outline">
                  {successCount}/{enrollment.device_pushes.length} dispositivos
                </Badge>
              </div>
              <button
                type="button"
                onClick={() => onReopen(enrollment.id)}
                data-testid={`enrollment-reopen-${enrollment.id}`}
                className="text-xs text-blue-600 hover:underline"
              >
                Ver detalles
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
