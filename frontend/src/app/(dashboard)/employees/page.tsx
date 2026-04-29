'use client'
import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { EmployeeTable } from '@/components/employees/employee-table'
import { EnrollmentModal } from '@/components/enrollment/enrollment-modal'
import { useAuth } from '@/hooks/use-auth'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import type { PaginatedResponse, Employee, Department } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const PAGE_SIZE = 10

// ── Schemas ────────────────────────────────────────────────────────────────

const newEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  employee_code: z.string().min(1, 'Código es requerido'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
})
type NewEmployeeFormData = z.infer<typeof newEmployeeSchema>

const editEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
})
type EditEmployeeFormData = z.infer<typeof editEmployeeSchema>

// ── Page ───────────────────────────────────────────────────────────────────

export default function EmployeesPage() {
  const { role } = useAuth()
  const queryClient = useQueryClient()

  const [pagination, setPagination] = useState<PaginationState>({ pageIndex: 0, pageSize: PAGE_SIZE })
  const [enrollmentEmployee, setEnrollmentEmployee] = useState<Employee | null>(null)
  const [search, setSearch] = useState('')
  const [deptFilter, setDeptFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState('')

  // Dialog state
  const [newEmpOpen, setNewEmpOpen] = useState(false)
  const [editEmployee, setEditEmployee] = useState<Employee | null>(null)
  const [deactivateEmployee, setDeactivateEmployee] = useState<Employee | null>(null)

  // ── Queries ──────────────────────────────────────────────────────────────

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

  // ── Create form ──────────────────────────────────────────────────────────

  const {
    register: registerNew,
    handleSubmit: handleSubmitNew,
    reset: resetNew,
    formState: { errors: errorsNew, isSubmitting: isSubmittingNew },
  } = useForm<NewEmployeeFormData>({
    resolver: zodResolver(newEmployeeSchema),
  })

  const createMutation = useMutation({
    mutationFn: async (values: NewEmployeeFormData) => {
      await api.post('/employees', {
        employee_code: values.employee_code,
        name: values.name,
        department_id: values.department_id,
        ...(values.position && { position: values.position }),
        ...(values.hire_date && { hire_date: values.hire_date }),
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      resetNew()
      setNewEmpOpen(false)
    },
  })

  // ── Edit form ────────────────────────────────────────────────────────────

  const {
    register: registerEdit,
    handleSubmit: handleSubmitEdit,
    reset: resetEdit,
    formState: { errors: errorsEdit, isSubmitting: isSubmittingEdit },
  } = useForm<EditEmployeeFormData>({
    resolver: zodResolver(editEmployeeSchema),
  })

  const updateMutation = useMutation({
    mutationFn: async ({ id, version, values }: { id: string; version: number; values: EditEmployeeFormData }) => {
      await api.patch(`/employees/${id}`, {
        name: values.name,
        department_id: values.department_id,
        ...(values.position !== undefined && { position: values.position }),
        ...(values.hire_date && { hire_date: values.hire_date }),
        version,
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      resetEdit()
      setEditEmployee(null)
    },
  })

  // ── Deactivate mutation ──────────────────────────────────────────────────

  const deactivateMutation = useMutation({
    mutationFn: async (id: string) => {
      await api.delete(`/employees/${id}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      setDeactivateEmployee(null)
    },
  })

  // ── Handlers ─────────────────────────────────────────────────────────────

  const handleEditClick = (emp: Employee) => {
    setEditEmployee(emp)
    resetEdit({
      name: emp.name,
      department_id: emp.department_id,
      position: emp.position ?? '',
      hire_date: emp.hire_date ?? '',
    })
  }

  // ── Render ────────────────────────────────────────────────────────────────

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
              onClick={() => setNewEmpOpen(true)}
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
              onEditClick={handleEditClick}
              onDeactivateClick={setDeactivateEmployee}
            />
          )}
        </div>
      </div>

      <EnrollmentModal
        open={!!enrollmentEmployee}
        employee={enrollmentEmployee}
        onClose={() => setEnrollmentEmployee(null)}
      />

      {/* ── New Employee Dialog ─────────────────────────────────────────── */}
      <Dialog
        open={newEmpOpen}
        onOpenChange={(o: boolean) => {
          if (!o) { resetNew(); setNewEmpOpen(false) }
        }}
      >
        <DialogContent data-testid="new-employee-form">
          <DialogHeader>
            <DialogTitle>Nuevo Empleado</DialogTitle>
          </DialogHeader>

          <form
            onSubmit={handleSubmitNew((v) => createMutation.mutate(v))}
            className="space-y-4"
          >
            <div>
              <Label htmlFor="new-emp-name">Nombre *</Label>
              <Input id="new-emp-name" {...registerNew('name')} placeholder="Nombre completo" />
              {errorsNew.name && (
                <p role="alert" className="text-xs text-destructive mt-1">{errorsNew.name.message}</p>
              )}
            </div>

            <div>
              <Label htmlFor="new-emp-code">Código / Identificación *</Label>
              <Input id="new-emp-code" {...registerNew('employee_code')} placeholder="EMP001" />
              {errorsNew.employee_code && (
                <p role="alert" className="text-xs text-destructive mt-1">{errorsNew.employee_code.message}</p>
              )}
            </div>

            <div>
              <Label htmlFor="new-emp-dept">Departamento *</Label>
              <select
                id="new-emp-dept"
                {...registerNew('department_id')}
                className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              >
                <option value="">Seleccionar departamento…</option>
                {departments?.data.map(d => (
                  <option key={d.id} value={d.id}>{d.name}</option>
                ))}
              </select>
              {errorsNew.department_id && (
                <p role="alert" className="text-xs text-destructive mt-1">{errorsNew.department_id.message}</p>
              )}
            </div>

            <div>
              <Label htmlFor="new-emp-position">Cargo (opcional)</Label>
              <Input id="new-emp-position" {...registerNew('position')} />
            </div>

            <div>
              <Label htmlFor="new-emp-hire-date">Fecha Ingreso (opcional)</Label>
              <Input id="new-emp-hire-date" type="date" {...registerNew('hire_date')} />
            </div>

            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => { resetNew(); setNewEmpOpen(false) }}>
                Cancelar
              </Button>
              <Button
                type="submit"
                data-testid="new-employee-submit"
                disabled={isSubmittingNew || createMutation.isPending}
              >
                {createMutation.isPending ? 'Guardando…' : 'Guardar'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* ── Edit Employee Dialog ────────────────────────────────────────── */}
      <Dialog
        open={!!editEmployee}
        onOpenChange={(o: boolean) => {
          if (!o) { resetEdit(); setEditEmployee(null) }
        }}
      >
        <DialogContent data-testid="edit-employee-form">
          <DialogHeader>
            <DialogTitle>Editar Empleado</DialogTitle>
          </DialogHeader>

          <form
            onSubmit={handleSubmitEdit((v) =>
              updateMutation.mutate({ id: editEmployee!.id, version: editEmployee!.version, values: v })
            )}
            className="space-y-4"
          >
            <div>
              <Label htmlFor="edit-emp-name">Nombre *</Label>
              <Input id="edit-emp-name" {...registerEdit('name')} />
              {errorsEdit.name && (
                <p role="alert" className="text-xs text-destructive mt-1">{errorsEdit.name.message}</p>
              )}
            </div>

            <div>
              <Label htmlFor="edit-emp-dept">Departamento *</Label>
              <select
                id="edit-emp-dept"
                {...registerEdit('department_id')}
                className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              >
                <option value="">Seleccionar departamento…</option>
                {departments?.data.map(d => (
                  <option key={d.id} value={d.id}>{d.name}</option>
                ))}
              </select>
              {errorsEdit.department_id && (
                <p role="alert" className="text-xs text-destructive mt-1">{errorsEdit.department_id.message}</p>
              )}
            </div>

            <div>
              <Label htmlFor="edit-emp-position">Cargo (opcional)</Label>
              <Input id="edit-emp-position" {...registerEdit('position')} />
            </div>

            <div>
              <Label htmlFor="edit-emp-hire-date">Fecha Ingreso (opcional)</Label>
              <Input id="edit-emp-hire-date" type="date" {...registerEdit('hire_date')} />
            </div>

            <DialogFooter className="gap-2">
              <Button type="button" variant="outline" onClick={() => { resetEdit(); setEditEmployee(null) }}>
                Cancelar
              </Button>
              <Button
                type="submit"
                disabled={isSubmittingEdit || updateMutation.isPending}
              >
                {updateMutation.isPending ? 'Guardando…' : 'Guardar'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* ── Deactivate Confirm Dialog ───────────────────────────────────── */}
      <Dialog
        open={!!deactivateEmployee}
        onOpenChange={(o: boolean) => {
          if (!o) setDeactivateEmployee(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Desactivar Empleado</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-slate-600">
            ¿Desactivar a <strong>{deactivateEmployee?.name}</strong>? Esta acción puede revertirse.
          </p>
          <DialogFooter className="gap-2 mt-4">
            <Button type="button" variant="outline" onClick={() => setDeactivateEmployee(null)}>
              Cancelar
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => deactivateEmployee && deactivateMutation.mutate(deactivateEmployee.id)}
              disabled={deactivateMutation.isPending}
            >
              {deactivateMutation.isPending ? 'Desactivando…' : 'Desactivar'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
