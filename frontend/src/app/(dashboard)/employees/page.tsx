'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  Plus,
  Pencil,
  ScanFace,
  Search,
  Trash2,
  LogOut,
  Loader2,
  Save,
} from 'lucide-react'
import { api, logoutCurrentSession } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { EnrollmentModal } from '@/components/enrollment/enrollment-modal'
import {
  NewEmployeeDialog,
  SALARY_KIND_OPTIONS,
  SALARY_KIND_AMOUNT_LABEL,
} from '@/components/employees/new-employee-dialog'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { PrimaryButton } from '@/components/ui/primary-button'
import { fmtDate } from '@/lib/format/datetime'
import type {
  PaginatedResponse,
  Employee,
  Department,
  CreateEmployeeRequest,
  SalaryKind,
} from '@/types/api'

const PAGE_SIZE = 10

// ── Schemas ──────────────────────────────────────────────────────────────────

// Optional currency override (major units), held as a string so the raw input
// reaches validation. Empty → blank so the employee falls back to the department
// salary. Accepts ONLY plain digits with up to two decimals — alphanumeric input
// (letters, scientific notation like "1e5", signs) is rejected instead of being
// silently coerced, which previously fed NaN/bogus values into the payroll cents
// calculation. Convert to a number at submit time via parseSalaryCents().
const optionalSalary = z
  .string()
  .trim()
  .refine((v) => v === '' || /^\d+(\.\d{1,2})?$/.test(v), {
    message: 'Sueldo debe ser un número válido (solo dígitos, máx. 2 decimales)',
  })
  .optional()

// Returns base_salary_cents for a validated salary string, or undefined when the
// field is blank (no override → department salary applies).
function parseSalaryCents(value: string | undefined): number | undefined {
  if (value === undefined || value.trim() === '') return undefined
  return Math.round(Number(value) * 100)
}

// H-08: the create form's own schema (including the mandatory, no-default
// `salary_kind` unit) lives in NewEmployeeDialog
// (`@/components/employees/new-employee-dialog`), not here — POST /employees
// requires salary_kind while PATCH /employees/:id treats it as optional
// (omit to leave the employee's existing unit unchanged), so the edit
// form's schema stays independent.
const editEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
  base_salary: optionalSalary,
  // No default value here or in the <select> below (H-08). This field
  // cannot be schema-mandated the way the create form's is: `base_salary`
  // above is pre-filled with the employee's CURRENT amount, so "is
  // base_salary non-empty" is true on every edit, not just ones that change
  // the amount. A zod-level requirement on that condition would block
  // *every* save. The actual "only required when the amount is actually
  // being changed" rule is enforced in the submit handler below via
  // react-hook-form's dirtyFields, which zod's schema-level validation has
  // no access to. (Critical 1 fix: GET /employees now round-trips
  // salary_kind, so `handleEditClick` below prefills this field with the
  // employee's current unit instead of leaving it blank.)
  salary_kind: z.enum(['hourly', 'daily', 'monthly']).optional().or(z.literal('')),
})
type EditEmployeeFormData = z.infer<typeof editEmployeeSchema>

// ── Avatar palette (deterministic) ───────────────────────────────────────────

const AVATAR_PALETTE = [
  '#D4E8F7', '#FDE8D8', '#E8D4F7', '#D4F7D4',
  '#F7E8D4', '#D4E0F7', '#E8F7D4', '#F7D4E8',
]
function avatarColor(seed: string): string {
  let h = 0
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0
  return AVATAR_PALETTE[h % AVATAR_PALETTE.length]
}
function initialsFor(name: string): string {
  return name
    .split(' ')
    .filter(Boolean)
    .map((p) => p[0])
    .slice(0, 2)
    .join('')
    .toUpperCase()
}

// ── Page ─────────────────────────────────────────────────────────────────────

export default function EmployeesPage() {
  const router = useRouter()
  const { role } = useAuth()
  const queryClient = useQueryClient()
  const isAdmin = role === 'admin'

  const [pageIndex, setPageIndex] = useState(0)
  const [search, setSearch] = useState('')
  const [deptFilter, setDeptFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  const [enrollmentEmployee, setEnrollmentEmployee] = useState<Employee | null>(null)
  const [newEmpOpen, setNewEmpOpen] = useState(false)
  const [enrollAfterSave, setEnrollAfterSave] = useState(true)
  const [editEmployee, setEditEmployee] = useState<Employee | null>(null)
  const [deactivateEmployee, setDeactivateEmployee] = useState<Employee | null>(null)

  // ── Queries ───────────────────────────────────────────────────────────────

  const { data: employees, isLoading } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', pageIndex, search, deptFilter, statusFilter],
    queryFn: () =>
      api
        .get('/employees', {
          params: {
            ...(search && { name: search }),
            ...(deptFilter && { department_id: deptFilter }),
            ...(statusFilter && { status: statusFilter }),
            limit: PAGE_SIZE,
            offset: pageIndex * PAGE_SIZE,
          },
        })
        .then((r) => r.data),
  })

  const { data: departments } = useQuery<PaginatedResponse<Department>>({
    queryKey: ['departments'],
    queryFn: () => api.get('/departments').then((r) => r.data),
    staleTime: 300_000,
  })

  // ── Mutations ─────────────────────────────────────────────────────────────

  const createMutation = useMutation({
    mutationFn: async (payload: CreateEmployeeRequest) => {
      const r = await api.post<Employee>('/employees', payload)
      return r.data
    },
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      queryClient.invalidateQueries({ queryKey: ['employees-total-active'] })
      setNewEmpOpen(false)
      if (enrollAfterSave) {
        setEnrollmentEmployee(created)
      }
    },
  })

  const updateMutation = useMutation({
    mutationFn: async ({
      id,
      version,
      values,
    }: {
      id: string
      version: number
      values: EditEmployeeFormData
    }) => {
      await api.patch(`/employees/${id}`, {
        name: values.name,
        department_id: values.department_id,
        ...(values.position !== undefined && { position: values.position }),
        ...(values.hire_date && { hire_date: values.hire_date }),
        ...(parseSalaryCents(values.base_salary) !== undefined && {
          base_salary_cents: parseSalaryCents(values.base_salary),
        }),
        // H-08: only sent when the operator picked one — omitting it leaves
        // the employee's existing salary_kind unchanged (PATCH semantics).
        ...(values.salary_kind && { salary_kind: values.salary_kind }),
        version,
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      resetEdit()
      setEditEmployee(null)
    },
  })

  const deactivateMutation = useMutation({
    mutationFn: async (id: string) => {
      await api.delete(`/employees/${id}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      queryClient.invalidateQueries({ queryKey: ['employees-total-active'] })
      setDeactivateEmployee(null)
    },
  })

  // ── Forms ─────────────────────────────────────────────────────────────────
  // The "Nuevo Empleado" form owns its own useForm() instance inside
  // NewEmployeeDialog (see import above) so the H-08/C-03 salary contract is
  // unit-testable; only the edit form's react-hook-form wiring stays here.

  const {
    register: registerEdit,
    handleSubmit: handleSubmitEdit,
    reset: resetEdit,
    setError: setErrorEdit,
    watch: watchEdit,
    formState: { errors: errorsEdit, isSubmitting: isSubmittingEdit, dirtyFields: dirtyFieldsEdit },
  } = useForm<EditEmployeeFormData>({ resolver: zodResolver(editEmployeeSchema) })

  // H-08 / Critical 1: label the amount field with the unit currently
  // selected, the same way NewEmployeeDialog does — falls back to the
  // ambiguous generic label only while no unit is known/selected yet.
  const selectedEditKind = watchEdit('salary_kind')
  const editAmountLabel = selectedEditKind
    ? SALARY_KIND_AMOUNT_LABEL[selectedEditKind as SalaryKind]
    : 'Sueldo Base ($)'

  // H-08: salary_kind is only required when the operator actually changes
  // base_salary — see the long comment on editEmployeeSchema above for why
  // that can't be expressed at the zod level (base_salary is pre-filled
  // with the employee's current amount, so "non-empty" is always true).
  function submitEdit(values: EditEmployeeFormData) {
    if (dirtyFieldsEdit.base_salary && values.base_salary?.trim() && !values.salary_kind) {
      setErrorEdit('salary_kind', { message: 'Selecciona la unidad del sueldo' })
      return
    }
    updateMutation.mutate({
      id: editEmployee!.id,
      version: editEmployee!.version,
      values,
    })
  }

  function handleEditClick(emp: Employee) {
    setEditEmployee(emp)
    resetEdit({
      name: emp.name,
      department_id: emp.department_id,
      position: emp.position ?? '',
      hire_date: emp.hire_date ?? '',
      base_salary: emp.base_salary_cents != null ? String(emp.base_salary_cents / 100) : '',
      // Critical 1 / H-08: GET /employees now round-trips salary_kind, so
      // the selector opens already showing the employee's current unit
      // instead of forcing a blind re-pick on every edit (raising the
      // amount used to risk re-interpreting a monthly salary as daily).
      salary_kind: emp.salary_kind ?? '',
    })
  }

  // ── Logout ────────────────────────────────────────────────────────────────

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await logoutCurrentSession()
    } finally {
      router.push('/login')
    }
  }

  // ── Derived ───────────────────────────────────────────────────────────────

  const total = employees?.total ?? 0
  const rows = employees?.data ?? []
  const startIndex = total === 0 ? 0 : pageIndex * PAGE_SIZE + 1
  const endIndex = Math.min((pageIndex + 1) * PAGE_SIZE, total)
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const deptNameById = new Map<string, string>(
    (departments?.data ?? []).map((d) => [d.id, d.name]),
  )

  function resetPage<T>(setter: (v: T) => void): (v: T) => void {
    return (v: T) => {
      setter(v)
      setPageIndex(0)
    }
  }

  // Compact pagination: at most 5 numbered buttons centered around current.
  function buildPageNumbers(): number[] {
    if (pageCount <= 5) return Array.from({ length: pageCount }, (_, i) => i)
    const start = Math.max(0, Math.min(pageCount - 5, pageIndex - 2))
    return Array.from({ length: 5 }, (_, i) => start + i)
  }

  return (
    <div className="flex flex-col h-full bg-[#F8F9FA]">
      {/* ── Header ─────────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-8 py-4">
        <div className="flex flex-col gap-1">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Inicio / Empleados
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Gestión de Empleados
          </h1>
        </div>
        <div className="flex items-center gap-3">
          {isAdmin && (
            <PrimaryButton
              type="button"
              size="sm"
              icon={Plus}
              data-testid="new-employee-button"
              onClick={() => setNewEmpOpen(true)}
            >
              Nuevo Empleado
            </PrimaryButton>
          )}
          <button
            type="button"
            onClick={handleLogout}
            disabled={isLoggingOut}
            aria-label="Cerrar sesión"
            data-testid="logout-button"
            className="inline-flex items-center gap-1.5 text-xs text-[#666666] hover:text-[#1A1A1A] px-2.5 py-1.5 rounded-md border border-[#EEF0F2] hover:bg-slate-50 disabled:opacity-50 transition-colors"
          >
            <LogOut size={14} aria-hidden="true" />
            {isLoggingOut ? 'Saliendo…' : 'Salir'}
          </button>
        </div>
      </header>

      {/* ── Body ───────────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto px-8 py-6 flex flex-col gap-5">
        {/* Filter bar */}
        <div className="flex items-center gap-3 flex-wrap">
          {/* Departamento dropdown */}
          <select
            value={deptFilter}
            onChange={(e) => resetPage(setDeptFilter)(e.target.value)}
            className="rounded border border-[#EEF0F2] bg-white px-3 py-2 text-[13px] text-[#1A1A1A]"
            style={{ fontFamily: 'var(--font-sans)' }}
            data-testid="filter-department"
          >
            <option value="">Departamento</option>
            {departments?.data.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>

          {/* Estatus dropdown */}
          <select
            value={statusFilter}
            onChange={(e) => resetPage(setStatusFilter)(e.target.value)}
            className="rounded border border-[#EEF0F2] bg-white px-3 py-2 text-[13px] text-[#1A1A1A]"
            style={{ fontFamily: 'var(--font-sans)' }}
            data-testid="filter-status"
          >
            <option value="">Estatus</option>
            <option value="active">Activo</option>
            <option value="pending">Pendiente</option>
            <option value="inactive">Inactivo</option>
          </select>

          <div className="flex-1" />

          {/* Search */}
          <div className="relative">
            <span className="pointer-events-none absolute inset-y-0 left-3 flex items-center">
              <Search size={14} className="text-[#666666]" />
            </span>
            <input
              type="search"
              value={search}
              onChange={(e) => resetPage(setSearch)(e.target.value)}
              placeholder="Buscar empleado..."
              data-testid="filter-search"
              className="w-[260px] rounded border border-[#EEF0F2] bg-white pl-9 pr-3 py-2 text-[13px] text-[#1A1A1A] placeholder:text-[#999999]"
              style={{ fontFamily: 'var(--font-sans)' }}
            />
          </div>

          {/* Enrolar Rostro shortcut */}
          <button
            type="button"
            onClick={() => router.push('/enrollment')}
            data-testid="enroll-shortcut"
            className="inline-flex items-center gap-1.5 rounded border border-[#1E3FB8] bg-[#EBF5FB] px-3 py-2 text-[13px] font-medium text-[#1E3FB8] hover:bg-[#DDEBF6] transition-colors"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <ScanFace size={14} aria-hidden="true" />
            Enrolar Rostro
          </button>
        </div>

        {/* Table */}
        <section
          className="bg-white rounded border border-[#EEF0F2] overflow-hidden flex flex-col"
          style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
          data-testid="employees-table"
        >
          {/* Column headers */}
          <div className="flex items-center bg-[#F8F9FA] border-b border-[#EEF0F2] px-4 py-2.5">
            <div className="w-[44px]" aria-hidden="true" />
            <div className="flex-1 text-[12px] font-semibold text-[#666666]">Nombre</div>
            <div className="w-[120px] text-[12px] font-semibold text-[#666666]">Cédula</div>
            <div className="w-[150px] text-[12px] font-semibold text-[#666666]">Departamento</div>
            <div className="w-[150px] text-[12px] font-semibold text-[#666666]">Cargo</div>
            <div className="w-[120px] text-[12px] font-semibold text-[#666666]">Fecha Ingreso</div>
            <div className="w-[90px] text-[12px] font-semibold text-[#666666]">Estatus</div>
            <div className="w-[90px] text-[12px] font-semibold text-[#666666] text-center">Acciones</div>
          </div>

          {/* Rows */}
          <div className="flex-1 overflow-auto">
            {isLoading && (
              <div className="flex items-center gap-2 px-4 py-8 text-[13px] text-[#666666]">
                <Loader2 size={14} className="animate-spin" />
                Cargando empleados…
              </div>
            )}
            {!isLoading && rows.length === 0 && (
              <div className="px-4 py-12 text-center text-[13px] text-[#666666]">
                Sin empleados para los filtros seleccionados.
              </div>
            )}
            {!isLoading &&
              rows.map((e) => {
                const deptName =
                  e.department_name ?? deptNameById.get(e.department_id) ?? '—'
                const statusCfg =
                  e.status === 'active'
                    ? { bg: '#DCFCE7', text: '#22C55E', label: 'Activo' }
                    : e.status === 'pending'
                      ? { bg: '#FEF3C7', text: '#D97706', label: 'Pendiente' }
                      : { bg: '#FEE2E2', text: '#EF4444', label: 'Inactivo' }
                return (
                  <div
                    key={e.id}
                    className="flex items-center px-4 py-2.5 border-b border-[#EEF0F2] hover:bg-slate-50"
                    data-testid={`employee-row-${e.id}`}
                  >
                    <span
                      className="w-8 h-8 rounded-full flex items-center justify-center text-[11px] font-semibold text-[#1A1A1A] shrink-0"
                      style={{ backgroundColor: avatarColor(e.id) }}
                      aria-hidden="true"
                    >
                      {initialsFor(e.name)}
                    </span>
                    <div className="flex-1 ml-3 text-[13px] font-medium text-[#1A1A1A] truncate">
                      {e.name}
                    </div>
                    <div
                      className="w-[120px] text-[12px] text-[#1A1A1A] truncate"
                      style={{ fontFamily: 'var(--font-mono)' }}
                    >
                      {e.employee_code || '—'}
                    </div>
                    <div className="w-[150px] text-[13px] text-[#1A1A1A] truncate">
                      {deptName}
                    </div>
                    <div className="w-[150px] text-[13px] text-[#1A1A1A] truncate">
                      {e.position || '—'}
                    </div>
                    <div
                      className="w-[120px] text-[12px] text-[#666666]"
                      style={{ fontFamily: 'var(--font-mono)' }}
                    >
                      {e.hire_date ? fmtDate(e.hire_date) : '—'}
                    </div>
                    <div className="w-[90px]">
                      <span
                        className="inline-flex items-center justify-center rounded-full px-2 py-0.5 text-[11px] font-medium"
                        style={{ backgroundColor: statusCfg.bg, color: statusCfg.text }}
                        data-testid={`employee-status-${e.id}`}
                      >
                        {statusCfg.label}
                      </span>
                    </div>
                    <div className="w-[90px] flex items-center justify-center gap-2">
                      {isAdmin && (
                        <button
                          type="button"
                          onClick={() => handleEditClick(e)}
                          aria-label={`Editar ${e.name}`}
                          data-testid={`employee-edit-${e.id}`}
                          className="p-1 rounded hover:bg-slate-100 text-[#666666] hover:text-[#1A1A1A] transition-colors"
                        >
                          <Pencil size={16} />
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => setEnrollmentEmployee(e)}
                        aria-label={`Enrolar rostro de ${e.name}`}
                        data-testid={`employee-enroll-${e.id}`}
                        className="p-1 rounded hover:bg-blue-50 text-[#1E3FB8] hover:text-[#1835A0] transition-colors"
                      >
                        <ScanFace size={16} />
                      </button>
                      {isAdmin && e.status === 'active' && (
                        <button
                          type="button"
                          onClick={() => setDeactivateEmployee(e)}
                          aria-label={`Desactivar ${e.name}`}
                          data-testid={`employee-deactivate-${e.id}`}
                          className="p-1 rounded hover:bg-red-50 text-[#EF4444] hover:text-[#DC2626] transition-colors"
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
                    </div>
                  </div>
                )
              })}
          </div>

          {/* Footer pagination */}
          <div className="flex items-center justify-between bg-[#F8F9FA] border-t border-[#EEF0F2] px-4 py-3">
            <span className="text-[12px] text-[#666666]">
              {total === 0
                ? 'Sin empleados'
                : `Mostrando ${startIndex}-${endIndex} de ${total} empleados`}
            </span>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setPageIndex((p) => Math.max(0, p - 1))}
                disabled={pageIndex === 0}
                data-testid="pagination-prev"
                className="rounded border border-[#EEF0F2] bg-white px-2.5 py-1 text-[12px] text-[#1A1A1A] hover:bg-slate-50 disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Anterior
              </button>
              {buildPageNumbers().map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => setPageIndex(p)}
                  data-testid={`pagination-page-${p + 1}`}
                  className={[
                    'rounded px-2.5 py-1 text-[12px] font-medium transition-colors',
                    p === pageIndex
                      ? 'bg-[#1E3FB8] text-white'
                      : 'bg-white border border-[#EEF0F2] text-[#1A1A1A] hover:bg-slate-50',
                  ].join(' ')}
                >
                  {p + 1}
                </button>
              ))}
              <button
                type="button"
                onClick={() => setPageIndex((p) => Math.min(pageCount - 1, p + 1))}
                disabled={pageIndex >= pageCount - 1}
                data-testid="pagination-next"
                className="rounded border border-[#EEF0F2] bg-white px-2.5 py-1 text-[12px] text-[#1A1A1A] hover:bg-slate-50 disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Siguiente
              </button>
            </div>
          </div>
        </section>
      </div>

      {/* ── Modals (preserved) ─────────────────────────────────────────── */}

      <EnrollmentModal
        open={!!enrollmentEmployee}
        employee={enrollmentEmployee}
        onClose={() => setEnrollmentEmployee(null)}
      />

      {/* New Employee — Pencil F93Iv design; extracted to
          NewEmployeeDialog so the H-08/C-03 salary contract is unit-tested
          under src/components (see @/components/employees/new-employee-dialog). */}
      <NewEmployeeDialog
        open={newEmpOpen}
        departments={departments?.data}
        enrollAfterSave={enrollAfterSave}
        onEnrollAfterSaveChange={setEnrollAfterSave}
        onClose={() => setNewEmpOpen(false)}
        onSubmit={(payload) => createMutation.mutate(payload)}
        isPending={createMutation.isPending}
      />

      {/* Edit Employee */}
      <Dialog
        open={!!editEmployee}
        onOpenChange={(o: boolean) => {
          if (!o) {
            resetEdit()
            setEditEmployee(null)
          }
        }}
      >
        <DialogContent data-testid="edit-employee-form">
          <DialogHeader>
            <DialogTitle>Editar Empleado</DialogTitle>
          </DialogHeader>
          <form onSubmit={handleSubmitEdit(submitEdit)} className="space-y-4">
            <div>
              <Label htmlFor="edit-emp-name">Nombre *</Label>
              <Input id="edit-emp-name" {...registerEdit('name')} />
              {errorsEdit.name && (
                <p role="alert" className="text-xs text-destructive mt-1">
                  {errorsEdit.name.message}
                </p>
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
                {departments?.data.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
              </select>
              {errorsEdit.department_id && (
                <p role="alert" className="text-xs text-destructive mt-1">
                  {errorsEdit.department_id.message}
                </p>
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
            <div>
              <Label htmlFor="edit-emp-salary">{editAmountLabel} (opcional)</Label>
              <Input
                id="edit-emp-salary"
                type="text"
                inputMode="decimal"
                {...registerEdit('base_salary')}
              />
              {errorsEdit.base_salary && (
                <p role="alert" className="text-xs text-destructive mt-1">
                  {errorsEdit.base_salary.message}
                </p>
              )}
            </div>
            <div>
              <Label htmlFor="edit-emp-salary-kind">Unidad del Sueldo</Label>
              {/* H-08: prefilled from the employee's current unit by
                  handleEditClick above (falls back to blank only when the
                  employee has none set yet); required (via the submit-time
                  check below) only when Sueldo Base above is also being
                  set/changed, matching PATCH /employees/:id, which treats
                  salary_kind as independently optional. */}
              <select
                id="edit-emp-salary-kind"
                {...registerEdit('salary_kind')}
                className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              >
                <option value="">Seleccionar…</option>
                {SALARY_KIND_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
              {errorsEdit.salary_kind ? (
                <p role="alert" className="text-xs text-destructive mt-1">
                  {errorsEdit.salary_kind.message}
                </p>
              ) : (
                <p className="text-xs text-muted-foreground mt-1">
                  Requerida solo si cambia el Sueldo Base.
                </p>
              )}
            </div>
            <DialogFooter className="gap-2">
              <PrimaryButton
                type="button"
                variant="outline"
                size="md"
                onClick={() => {
                  resetEdit()
                  setEditEmployee(null)
                }}
              >
                Cancelar
              </PrimaryButton>
              <PrimaryButton
                type="submit"
                size="md"
                icon={Save}
                disabled={isSubmittingEdit || updateMutation.isPending}
              >
                {updateMutation.isPending ? 'Guardando…' : 'Guardar'}
              </PrimaryButton>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Deactivate confirm */}
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
            ¿Desactivar a <strong>{deactivateEmployee?.name}</strong>? Esta acción
            puede revertirse.
          </p>
          <DialogFooter className="gap-2 mt-4">
            <PrimaryButton
              type="button"
              variant="outline"
              size="md"
              onClick={() => setDeactivateEmployee(null)}
            >
              Cancelar
            </PrimaryButton>
            <PrimaryButton
              type="button"
              variant="danger"
              size="md"
              icon={Trash2}
              onClick={() =>
                deactivateEmployee &&
                deactivateMutation.mutate(deactivateEmployee.id)
              }
              disabled={deactivateMutation.isPending}
            >
              {deactivateMutation.isPending ? 'Desactivando…' : 'Desactivar'}
            </PrimaryButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
