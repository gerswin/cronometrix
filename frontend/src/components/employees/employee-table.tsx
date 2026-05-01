'use client'
import {
  useReactTable, getCoreRowModel, flexRender,
  type ColumnDef, type PaginationState,
} from '@tanstack/react-table'
import { format } from 'date-fns'
import { Pencil, ChevronRight, UserPlus, UserX } from 'lucide-react'
import { useAuth } from '@/hooks/use-auth'
import type { Employee } from '@/types/api'

function StatusBadge({ status }: { status: Employee['status'] }) {
  const map = {
    active: 'bg-green-100 text-green-700',
    pending: 'bg-yellow-100 text-yellow-700',
    inactive: 'bg-slate-100 text-slate-600',
  }
  const labels = { active: 'Activo', pending: 'Pendiente', inactive: 'Inactivo' }
  return (
    <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${map[status]}`}>
      {labels[status]}
    </span>
  )
}

interface EmployeeTableProps {
  data: Employee[]
  total: number
  pagination: PaginationState
  onPaginationChange: (p: PaginationState) => void
  onEnrollClick?: (employee: Employee) => void
  onEditClick?: (employee: Employee) => void
  onDeactivateClick?: (employee: Employee) => void
}

const PAGE_SIZE = 10

export function EmployeeTable({
  data,
  total,
  pagination,
  onPaginationChange,
  onEnrollClick,
  onEditClick,
  onDeactivateClick,
}: EmployeeTableProps) {
  const { role } = useAuth()

  const columns: ColumnDef<Employee>[] = [
    { accessorKey: 'name', header: 'Nombre' },
    {
      accessorKey: 'employee_code',
      header: 'Identificativo',
      cell: ({ getValue }) => (getValue() as string | undefined) || '—',
    },
    {
      accessorKey: 'department_name',
      header: 'Departamento',
      cell: ({ getValue }) => (getValue() as string | undefined) ?? '—',
    },
    {
      accessorKey: 'position',
      header: 'Cargo',
      cell: ({ getValue }) => (getValue() as string | undefined) || '—',
    },
    {
      accessorKey: 'hire_date',
      header: 'Fecha Ingreso',
      cell: ({ getValue }) => {
        const v = getValue() as string | null | undefined
        if (!v) return '—'
        try { return format(new Date(v), 'dd/MM/yyyy') } catch { return '—' }
      },
    },
    {
      id: 'status',
      header: 'Estatus',
      cell: ({ row }) => <StatusBadge status={row.original.status} />,
    },
    {
      id: 'actions',
      header: 'Acciones',
      cell: ({ row }) => (
        <div className="flex items-center gap-1" data-testid={`emp-actions-${row.original.id}`}>
          {role === 'admin' && (
            <button
              className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700"
              aria-label="Editar empleado"
              data-testid={`emp-action-edit-${row.original.id}`}
              onClick={() => onEditClick ? onEditClick(row.original) : alert(`Editar: ${row.original.id}`)}
            >
              <Pencil size={14} />
            </button>
          )}
          {role === 'admin' && row.original.status === 'active' && onDeactivateClick && (
            <button
              className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-red-600"
              aria-label="Desactivar empleado"
              data-testid={`emp-action-deactivate-${row.original.id}`}
              onClick={() => onDeactivateClick(row.original)}
            >
              <UserX size={14} />
            </button>
          )}
          {role === 'admin' && onEnrollClick && (
            <button
              className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700"
              aria-label="Enrolar Rostro"
              onClick={() => onEnrollClick(row.original)}
            >
              <UserPlus size={14} />
            </button>
          )}
          <button
            className="p-1 rounded hover:bg-slate-100 text-slate-500 hover:text-slate-700"
            aria-label="Ver detalles"
          >
            <ChevronRight size={14} />
          </button>
        </div>
      ),
    },
  ]

  const table = useReactTable({
    data,
    columns,
    pageCount: Math.ceil(total / PAGE_SIZE),
    state: { pagination },
    onPaginationChange: (updater) => {
      const next = typeof updater === 'function' ? updater(pagination) : updater
      onPaginationChange(next)
    },
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    manualFiltering: true,
  })

  const pageCount = Math.ceil(total / PAGE_SIZE)
  const currentPage = pagination.pageIndex + 1

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map(hg => (
              <tr key={hg.id} className="border-b border-slate-200">
                {hg.headers.map(h => (
                  <th key={h.id} className="px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">
                    {flexRender(h.column.columnDef.header, h.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map(row => (
              <tr key={row.id} className="border-b border-slate-100 hover:bg-slate-50">
                {row.getVisibleCells().map(cell => (
                  <td key={cell.id} className="px-3 py-2 text-slate-700">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
            {data.length === 0 && (
              <tr>
                <td colSpan={columns.length} className="px-3 py-8 text-center text-slate-400 text-xs">
                  Sin empleados para los filtros seleccionados
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination — D-11: Anterior / page numbers / Siguiente */}
      {pageCount > 1 && (
        <div className="flex items-center justify-between px-3 py-3 border-t border-slate-100">
          <button
            disabled={pagination.pageIndex === 0}
            onClick={() => onPaginationChange({ ...pagination, pageIndex: pagination.pageIndex - 1 })}
            className="text-xs px-3 py-1 rounded border border-slate-200 disabled:opacity-40 hover:bg-slate-50"
          >
            Anterior
          </button>
          <span className="text-xs text-slate-500">
            Página {currentPage} de {pageCount} ({total} total)
          </span>
          <button
            disabled={pagination.pageIndex >= pageCount - 1}
            onClick={() => onPaginationChange({ ...pagination, pageIndex: pagination.pageIndex + 1 })}
            className="text-xs px-3 py-1 rounded border border-slate-200 disabled:opacity-40 hover:bg-slate-50"
          >
            Siguiente
          </button>
        </div>
      )}
    </div>
  )
}
