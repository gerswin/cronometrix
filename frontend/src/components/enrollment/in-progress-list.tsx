'use client'
import { Badge } from '@/components/ui/badge'
import type { Employee, Enrollment } from '@/types/api'

interface InProgressListProps {
  activeEnrollmentId?: string | null
  activeEmployee?: Employee | null
  enrollment?: Enrollment | null
  onReopen?: () => void
}

/**
 * V1 implementation: reflects the modal's currently-tracked enrollment (session-scoped).
 * A full "list past enrollments" feature requires a GET /enrollments?status=in_progress
 * endpoint not in 07-01 scope — tracked as known gap in 07-02-SUMMARY.md.
 */
export function InProgressList({
  activeEnrollmentId,
  activeEmployee,
  enrollment,
  onReopen,
}: InProgressListProps) {
  if (!activeEnrollmentId || !activeEmployee || !enrollment) {
    return null
  }

  const successCount = enrollment.device_pushes.filter(p => p.status === 'success').length
  const totalCount = enrollment.device_pushes.length
  const allTerminal = enrollment.device_pushes.every(
    p => p.status === 'success' || p.status === 'failed'
  )

  // Don't show once fully terminal
  if (allTerminal) return null

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
      <p className="text-sm font-semibold text-slate-700 mb-2">Enrolamientos en curso</p>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-sm text-slate-700">{activeEmployee.name}</span>
          <Badge variant="outline">
            {successCount}/{totalCount} dispositivos
          </Badge>
        </div>
        {onReopen && (
          <button
            type="button"
            onClick={onReopen}
            className="text-xs text-blue-600 hover:underline"
          >
            Ver detalles
          </button>
        )}
      </div>
    </div>
  )
}
