'use client'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type PaginationState,
} from '@tanstack/react-table'
import { format } from 'date-fns'
import type { AuditEntry } from '@/types/audit'
import { DiffCell } from './diff-cell'

/** Color badge per operation type */
function OperationBadge({ op }: { op: string }) {
  const map: Record<string, string> = {
    INSERT: 'bg-emerald-100 text-emerald-700',
    UPDATE: 'bg-amber-100 text-amber-700',
    DELETE: 'bg-rose-100 text-rose-700',
  }
  return (
    <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${map[op] ?? 'bg-slate-100 text-slate-600'}`}>
      {op}
    </span>
  )
}

interface AuditTableProps {
  data: AuditEntry[]
  total: number
  pagination: PaginationState
  onPaginationChange: (p: PaginationState) => void
  isLoading?: boolean
}

const PAGE_SIZE = 20

export function AuditTable({ data, total, pagination, onPaginationChange, isLoading }: AuditTableProps) {
  const columns: ColumnDef<AuditEntry>[] = [
    {
      accessorKey: 'created_at',
      header: 'Timestamp',
      cell: ({ getValue }) => {
        try {
          return format(new Date((getValue() as number) * 1000), 'dd/MM/yyyy HH:mm:ss')
        } catch {
          return '—'
        }
      },
    },
    {
      accessorKey: 'actor_id',
      header: 'Actor',
      cell: ({ getValue }) => (getValue() as string | null) ?? '—',
    },
    {
      accessorKey: 'table_name',
      header: 'Tabla',
    },
    {
      accessorKey: 'operation',
      header: 'Operación',
      cell: ({ getValue }) => <OperationBadge op={getValue() as string} />,
    },
    {
      accessorKey: 'record_id',
      header: 'ID Registro',
    },
    {
      id: 'diff',
      header: 'Cambios',
      cell: ({ row }) => (
        <DiffCell
          operation={row.original.operation}
          old_data={row.original.old_data}
          new_data={row.original.new_data}
        />
      ),
    },
  ]

  const pageCount = Math.ceil(total / PAGE_SIZE)

  const table = useReactTable({
    data,
    columns,
    pageCount,
    state: { pagination },
    onPaginationChange: (updater) => {
      const next = typeof updater === 'function' ? updater(pagination) : updater
      onPaginationChange(next)
    },
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    manualFiltering: true,
  })

  const currentPage = pagination.pageIndex + 1

  if (isLoading) {
    return (
      <div className="p-8 text-center text-slate-400 text-sm">
        Cargando auditoría…
      </div>
    )
  }

  return (
    <div data-testid="audit-table">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map(hg => (
              <tr key={hg.id} className="border-b border-slate-200">
                {hg.headers.map(h => (
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
            {table.getRowModel().rows.map(row => (
              <tr
                key={row.id}
                data-testid={`audit-row-${row.original.id}`}
                className="border-b border-slate-100 hover:bg-slate-50"
              >
                {row.getVisibleCells().map(cell => (
                  <td key={cell.id} className="px-3 py-2 text-slate-700">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
            {data.length === 0 && (
              <tr data-testid="audit-empty">
                <td
                  colSpan={columns.length}
                  className="px-3 py-8 text-center text-slate-400 text-xs"
                >
                  Sin entradas para los filtros seleccionados
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination — always visible so tests can interact with it */}
      <div className="flex items-center justify-between px-3 py-3 border-t border-slate-100">
        <button
          data-testid="audit-pagination-prev"
          disabled={pagination.pageIndex === 0}
          onClick={() =>
            onPaginationChange({ ...pagination, pageIndex: pagination.pageIndex - 1 })
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
          data-testid="audit-pagination-next"
          disabled={pagination.pageIndex >= pageCount - 1}
          onClick={() =>
            onPaginationChange({ ...pagination, pageIndex: pagination.pageIndex + 1 })
          }
          className="text-xs px-3 py-1 rounded border border-slate-200 disabled:opacity-40 hover:bg-slate-50"
        >
          Siguiente
        </button>
      </div>
    </div>
  )
}
