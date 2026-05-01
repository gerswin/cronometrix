'use client'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type PaginationState,
} from '@tanstack/react-table'
import type { RawAttendanceEvent } from '@/types/api'
import { fmtDateTime } from '@/lib/format/datetime'
import { EventPhoto } from './event-photo'

interface Props {
  data: RawAttendanceEvent[]
  total: number
  pagination: PaginationState
  onPaginationChange: (p: PaginationState) => void
  onView: (event: RawAttendanceEvent) => void
  employeeNameById: Map<string, string>
  deviceNameById: Map<string, string>
  isLoading?: boolean
  pageSize: number
}

export function EventsTable({
  data,
  total,
  pagination,
  onPaginationChange,
  onView,
  employeeNameById,
  deviceNameById,
  isLoading,
  pageSize,
}: Props) {
  const columns: ColumnDef<RawAttendanceEvent>[] = [
    {
      id: 'photo',
      header: '',
      cell: ({ row }) => (
        <EventPhoto
          eventId={row.original.id}
          hasPhoto={!!row.original.photo_path}
          className="w-9 h-9 rounded"
        />
      ),
    },
    {
      accessorKey: 'captured_at',
      header: 'Capturado',
      cell: ({ getValue }) => fmtDateTime(getValue() as string),
    },
    {
      accessorKey: 'direction',
      header: 'Dirección',
      cell: ({ getValue }) => {
        const d = getValue() as 'entry' | 'exit'
        return (
          <span
            className={`px-2 py-0.5 rounded-full text-xs font-medium ${
              d === 'entry'
                ? 'bg-green-100 text-green-700'
                : 'bg-blue-100 text-blue-700'
            }`}
          >
            {d === 'entry' ? 'Entrada' : 'Salida'}
          </span>
        )
      },
    },
    {
      id: 'employee',
      header: 'Empleado',
      cell: ({ row }) => {
        const r = row.original
        if (r.is_unknown) {
          return <span className="text-slate-400">Desconocido</span>
        }
        if (r.employee_id) {
          return (
            employeeNameById.get(r.employee_id) ??
            r.employee_no_string ??
            r.employee_id
          )
        }
        return r.face_id ?? '—'
      },
    },
    {
      accessorKey: 'device_id',
      header: 'Dispositivo',
      cell: ({ getValue }) => {
        const id = getValue() as string
        return deviceNameById.get(id) ?? id
      },
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <button
          type="button"
          data-testid={`event-view-${row.original.id}`}
          onClick={() => onView(row.original)}
          className="text-xs px-2 py-1 rounded border border-slate-200 hover:bg-slate-50"
        >
          Ver
        </button>
      ),
    },
  ]

  const pageCount = Math.ceil(total / pageSize)
  const table = useReactTable({
    data,
    columns,
    pageCount,
    state: { pagination },
    onPaginationChange: (updater) => {
      const next =
        typeof updater === 'function' ? updater(pagination) : updater
      onPaginationChange(next)
    },
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    manualFiltering: true,
  })

  if (isLoading) {
    return (
      <div className="p-8 text-center text-slate-400 text-sm">
        Cargando eventos…
      </div>
    )
  }

  const currentPage = pagination.pageIndex + 1

  return (
    <div data-testid="events-table">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id} className="border-b border-slate-200">
                {hg.headers.map((h) => (
                  <th
                    key={h.id}
                    className="px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide"
                  >
                    {flexRender(h.column.columnDef.header, h.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
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
              <tr data-testid="events-empty">
                <td
                  colSpan={columns.length}
                  className="px-3 py-8 text-center text-slate-400 text-xs"
                >
                  Sin eventos para los filtros seleccionados
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-between px-3 py-3 border-t border-slate-100">
        <button
          data-testid="events-pagination-prev"
          disabled={pagination.pageIndex === 0}
          onClick={() =>
            onPaginationChange({
              ...pagination,
              pageIndex: pagination.pageIndex - 1,
            })
          }
          className="text-xs px-3 py-1 rounded border border-slate-200 disabled:opacity-40 hover:bg-slate-50"
        >
          Anterior
        </button>
        <span className="text-xs text-slate-500">
          {pageCount > 0
            ? `Página ${currentPage} de ${pageCount} (${total} total)`
            : `${total} entradas`}
        </span>
        <button
          data-testid="events-pagination-next"
          disabled={pagination.pageIndex >= pageCount - 1}
          onClick={() =>
            onPaginationChange({
              ...pagination,
              pageIndex: pagination.pageIndex + 1,
            })
          }
          className="text-xs px-3 py-1 rounded border border-slate-200 disabled:opacity-40 hover:bg-slate-50"
        >
          Siguiente
        </button>
      </div>
    </div>
  )
}
