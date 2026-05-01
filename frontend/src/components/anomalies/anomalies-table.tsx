'use client'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type PaginationState,
} from '@tanstack/react-table'
import type { Anomaly } from '@/types/api'
import { fmtDateTime } from '@/lib/format/datetime'

interface Props {
  data: Anomaly[]
  total: number
  pagination: PaginationState
  onPaginationChange: (p: PaginationState) => void
  onView: (anomaly: Anomaly) => void
  employeeNameById: Map<string, string>
  isLoading?: boolean
  pageSize: number
}

export function AnomaliesTable({
  data,
  total,
  pagination,
  onPaginationChange,
  onView,
  employeeNameById,
  isLoading,
  pageSize,
}: Props) {
  const columns: ColumnDef<Anomaly>[] = [
    {
      accessorKey: 'anchor_date',
      header: 'Fecha',
    },
    {
      accessorKey: 'employee_id',
      header: 'Empleado',
      cell: ({ getValue }) => {
        const id = getValue() as string
        return employeeNameById.get(id) ?? id
      },
    },
    {
      accessorKey: 'code',
      header: 'Código',
      cell: ({ getValue }) => (
        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700">
          {getValue() as string}
        </span>
      ),
    },
    {
      accessorKey: 'detail',
      header: 'Detalle',
      cell: ({ getValue }) => (getValue() as string | null) ?? '—',
    },
    {
      accessorKey: 'created_at',
      header: 'Detectado',
      cell: ({ getValue }) => fmtDateTime(getValue() as string),
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <button
          type="button"
          data-testid={`anomaly-view-${row.original.id}`}
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
        Cargando anomalías…
      </div>
    )
  }

  const currentPage = pagination.pageIndex + 1

  return (
    <div data-testid="anomalies-table">
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
              <tr data-testid="anomalies-empty">
                <td
                  colSpan={columns.length}
                  className="px-3 py-8 text-center text-slate-400 text-xs"
                >
                  Sin anomalías para los filtros seleccionados
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-between px-3 py-3 border-t border-slate-100">
        <button
          data-testid="anomalies-pagination-prev"
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
          data-testid="anomalies-pagination-next"
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
