'use client'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type PaginationState,
} from '@tanstack/react-table'
import { format, parseISO } from 'date-fns'
import { Pencil } from 'lucide-react'
import { useAuth } from '@/hooks/use-auth'
import { dailyRecordKey } from '@/lib/daily-record-key'
import type { DailyRecord } from '@/types/api'
import { LeaveRowActions } from './leave-row-actions'

function getStatusBadge(record: DailyRecord) {
  if (record.leave_id) {
    if (record.work_minutes === 0) {
      return (
        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700">
          Ausente Justificado
        </span>
      )
    }
    return (
      <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-700">
        Justificado
      </span>
    )
  }
  if (record.work_minutes === 0) {
    return (
      <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700">
        Ausente
      </span>
    )
  }
  return (
    <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700">
      Normal
    </span>
  )
}

function formatTime(iso: string | null | undefined) {
  if (!iso) return '—'
  try {
    return format(new Date(iso), 'HH:mm')
  } catch {
    return '—'
  }
}

interface TimesheetTableProps {
  data: DailyRecord[]
  total: number
  pagination: PaginationState
  onPaginationChange: (p: PaginationState) => void
  onEditClick: (record: DailyRecord) => void
}

const PAGE_SIZE = 50

export function TimesheetTable({
  data,
  total,
  pagination,
  onPaginationChange,
  onEditClick,
}: TimesheetTableProps) {
  const { role } = useAuth()

  const columns: ColumnDef<DailyRecord>[] = [
    {
      accessorKey: 'anchor_date',
      header: 'Fecha',
      cell: ({ getValue }) => format(parseISO(getValue() as string), 'dd/MM/yyyy'),
    },
    {
      accessorKey: 'employee_id',
      header: 'Empleado',
      cell: ({ row }) => row.original.employee_name ?? row.original.employee_id,
    },
    {
      accessorKey: 'department_id',
      header: 'Departamento',
      cell: ({ row }) => row.original.department_name ?? row.original.department_id,
    },
    {
      accessorKey: 'entry_at',
      header: 'Entrada',
      cell: ({ getValue }) => formatTime(getValue() as string | null),
    },
    {
      accessorKey: 'exit_at',
      header: 'Salida',
      cell: ({ getValue }) => formatTime(getValue() as string | null),
    },
    {
      id: 'estado',
      header: 'Novedades / Estado',
      cell: ({ row }) => getStatusBadge(row.original),
    },
    {
      id: 'actions',
      header: 'Acciones',
      cell: ({ row }: { row: { original: DailyRecord } }) => (
        <span className="inline-flex items-center gap-1">
          {role === 'admin' && (
            <button
              data-testid="open-novedad-modal"
              onClick={() => onEditClick(row.original)}
              className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700"
              aria-label="Registrar novedad"
            >
              <Pencil size={14} />
            </button>
          )}
          {row.original.leave_id && (
            <LeaveRowActions leaveId={row.original.leave_id} />
          )}
        </span>
      ),
    } as ColumnDef<DailyRecord>,
  ]

  const table = useReactTable({
    data,
    columns,
    pageCount: Math.ceil(total / PAGE_SIZE),
    state: { pagination },
    onPaginationChange: (updater) => {
      const next =
        typeof updater === 'function' ? updater(pagination) : updater
      onPaginationChange(next)
    },
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    manualFiltering: true,
    getRowId: dailyRecordKey,
  })

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b border-slate-200">
              {hg.headers.map((header) => (
                <th
                  key={header.id}
                  className="px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide"
                >
                  {flexRender(
                    header.column.columnDef.header,
                    header.getContext()
                  )}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr
              key={row.id}
              data-testid={`timesheet-row-${row.id}`}
              className="border-b border-slate-100 hover:bg-slate-50"
            >
              {row.getVisibleCells().map((cell) => (
                <td key={cell.id} className="px-3 py-2 text-slate-700">
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
          {data.length === 0 && (
            <tr>
              <td
                colSpan={columns.length}
                className="px-3 py-8 text-center text-slate-400 text-xs"
              >
                Sin registros para esta semana
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  )
}
