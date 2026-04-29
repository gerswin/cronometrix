'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { EmployeeTable } from '@/components/employees/employee-table'
import { EnrollmentModal } from '@/components/enrollment/enrollment-modal'
import { useAuth } from '@/hooks/use-auth'
import type { PaginatedResponse, Employee, Department } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const PAGE_SIZE = 10

export default function EmployeesPage() {
  const { role } = useAuth()
  const [pagination, setPagination] = useState<PaginationState>({ pageIndex: 0, pageSize: PAGE_SIZE })
  const [enrollmentEmployee, setEnrollmentEmployee] = useState<Employee | null>(null)
  const [search, setSearch] = useState('')
  const [deptFilter, setDeptFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState('')

  const { data: employees, isLoading } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', pagination.pageIndex, search, deptFilter, statusFilter],
    queryFn: () =>
      api.get('/employees', {
        params: {
          ...(search && { name: search }),
          ...(deptFilter && { department_id: deptFilter }),
          ...(statusFilter && { status: statusFilter }),
          limit: PAGE_SIZE,
          offset: pagination.pageIndex * PAGE_SIZE,
        },
      }).then(r => r.data),
  })

  const { data: departments } = useQuery<PaginatedResponse<Department>>({
    queryKey: ['departments'],
    queryFn: () => api.get('/departments').then(r => r.data),
    staleTime: 300_000,
  })

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Empleados" />
      <div className="p-6 space-y-4">
        {/* Filters row */}
        <div className="flex items-center gap-3 flex-wrap">
          <input
            type="search"
            placeholder="Buscar empleado…"
            value={search}
            onChange={e => { setSearch(e.target.value); setPagination(p => ({ ...p, pageIndex: 0 })) }}
            className="rounded-md border border-slate-200 px-3 py-2 text-sm w-52"
          />
          <select
            value={deptFilter}
            onChange={e => { setDeptFilter(e.target.value); setPagination(p => ({ ...p, pageIndex: 0 })) }}
            className="rounded-md border border-slate-200 px-3 py-2 text-sm"
          >
            <option value="">Todos los departamentos</option>
            {departments?.data.map(d => (
              <option key={d.id} value={d.id}>{d.name}</option>
            ))}
          </select>
          <select
            value={statusFilter}
            onChange={e => { setStatusFilter(e.target.value); setPagination(p => ({ ...p, pageIndex: 0 })) }}
            className="rounded-md border border-slate-200 px-3 py-2 text-sm"
          >
            <option value="">Todos los estatus</option>
            <option value="active">Activo</option>
            <option value="pending">Pendiente</option>
            <option value="inactive">Inactivo</option>
          </select>

          {/* Spacer */}
          <div className="flex-1" />

          {/* D-14: "Emitir Reporte" visible to Admin and Supervisor, hidden for Viewer */}
          {(role === 'admin' || role === 'supervisor') && (
            <button className="px-4 py-2 border border-slate-200 text-sm rounded-md hover:bg-slate-50">
              Emitir Reporte
            </button>
          )}
          {/* D-14: "Nuevo Empleado" visible only to Admin */}
          {role === 'admin' && (
            <button
              data-testid="new-employee-button"
              className="px-4 py-2 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700"
            >
              Nuevo Empleado
            </button>
          )}
        </div>

        {/* Table */}
        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          {isLoading ? (
            <div className="p-8 text-center text-slate-400 text-sm">Cargando empleados…</div>
          ) : (
            <EmployeeTable
              data={employees?.data ?? []}
              total={employees?.total ?? 0}
              pagination={pagination}
              onPaginationChange={setPagination}
              onEnrollClick={setEnrollmentEmployee}
            />
          )}
        </div>
      </div>

      <EnrollmentModal
        open={!!enrollmentEmployee}
        employee={enrollmentEmployee}
        onClose={() => setEnrollmentEmployee(null)}
      />
    </div>
  )
}
