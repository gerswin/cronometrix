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
  X,
  User,
  Briefcase,
  Save,
} from 'lucide-react'
import { api, setAccessToken } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { EnrollmentModal } from '@/components/enrollment/enrollment-modal'
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
import type { PaginatedResponse, Employee, Department } from '@/types/api'

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

const newEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  employee_code: z.string().min(1, 'Cédula es requerida'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
  base_salary: optionalSalary,
})
type NewEmployeeFormData = z.infer<typeof newEmployeeSchema>

const editEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
  base_salary: optionalSalary,
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
    mutationFn: async (values: NewEmployeeFormData) => {
      const r = await api.post<Employee>('/employees', {
        employee_code: values.employee_code,
        name: values.name,
        department_id: values.department_id,
        ...(values.position && { position: values.position }),
        ...(values.hire_date && { hire_date: values.hire_date }),
        ...(parseSalaryCents(values.base_salary) !== undefined && {
          base_salary_cents: parseSalaryCents(values.base_salary),
        }),
      })
      return r.data
    },
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] })
      resetNew()
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
      setDeactivateEmployee(null)
    },
  })

  // ── Forms ─────────────────────────────────────────────────────────────────

  const {
    register: registerNew,
    handleSubmit: handleSubmitNew,
    reset: resetNew,
    formState: { errors: errorsNew, isSubmitting: isSubmittingNew },
  } = useForm<NewEmployeeFormData>({ resolver: zodResolver(newEmployeeSchema) })

  const {
    register: registerEdit,
    handleSubmit: handleSubmitEdit,
    reset: resetEdit,
    formState: { errors: errorsEdit, isSubmitting: isSubmittingEdit },
  } = useForm<EditEmployeeFormData>({ resolver: zodResolver(editEmployeeSchema) })

  function handleEditClick(emp: Employee) {
    setEditEmployee(emp)
    resetEdit({
      name: emp.name,
      department_id: emp.department_id,
      position: emp.position ?? '',
      hire_date: emp.hire_date ?? '',
      base_salary: emp.base_salary_cents != null ? String(emp.base_salary_cents / 100) : '',
    })
  }

  // ── Logout ────────────────────────────────────────────────────────────────

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await api.post('/auth/logout').catch(() => undefined)
    } finally {
      setAccessToken(null)
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

      {/* New Employee */}
      {/* New Employee — Pencil F93Iv design */}
      <Dialog
        open={newEmpOpen}
        onOpenChange={(o: boolean) => {
          if (!o) {
            resetNew()
            setNewEmpOpen(false)
          }
        }}
      >
        <DialogContent
          className="max-w-[700px] p-0 overflow-hidden"
          data-testid="new-employee-form"
        >
          <form
            onSubmit={handleSubmitNew((v) => createMutation.mutate(v))}
            className="flex flex-col"
          >
            {/* Header */}
            <div className="flex items-center justify-between px-7 py-4 border-b border-[#EEF0F2]">
              <div className="flex flex-col gap-0.5">
                <h2
                  className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Registrar Nuevo Empleado
                </h2>
                <p
                  className="text-[12px] italic text-[#666666]"
                  style={{ fontFamily: 'var(--font-serif)' }}
                >
                  Complete la ficha técnica del personal
                </p>
              </div>
              <button
                type="button"
                aria-label="Cerrar"
                onClick={() => {
                  resetNew()
                  setNewEmpOpen(false)
                }}
                className="flex items-center justify-center w-8 h-8 rounded bg-[#F3F4F6] hover:bg-[#E5E7EB] transition-colors"
              >
                <X size={18} className="text-[#666666]" />
              </button>
            </div>

            {/* Body */}
            <div className="px-7 py-5 flex flex-col gap-4 max-h-[60vh] overflow-y-auto">
              {/* Section 1 — Datos Personales */}
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-2">
                  <User size={16} className="text-[#1E3FB8]" />
                  <h3
                    className="text-[14px] font-bold text-[#1A1A1A]"
                    style={{ fontFamily: 'var(--font-sans)' }}
                  >
                    Datos Personales
                  </h3>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">
                      Nombre completo<span className="text-[#DC2626] ml-0.5">*</span>
                    </span>
                    <input
                      {...registerNew('name')}
                      placeholder="Ana Pérez González"
                      className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                        errorsNew.name ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                      } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                    />
                    {errorsNew.name && (
                      <span role="alert" className="text-[11px] text-[#DC2626]">
                        {errorsNew.name.message}
                      </span>
                    )}
                  </label>
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">
                      Cédula<span className="text-[#DC2626] ml-0.5">*</span>
                    </span>
                    <input
                      {...registerNew('employee_code')}
                      placeholder="V-12345678"
                      className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                        errorsNew.employee_code ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                      } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                    />
                    {errorsNew.employee_code && (
                      <span role="alert" className="text-[11px] text-[#DC2626]">
                        {errorsNew.employee_code.message}
                      </span>
                    )}
                  </label>
                </div>
              </div>

              <div className="h-px bg-[#EEF0F2] -mx-7" />

              {/* Section 2 — Datos Laborales */}
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-2">
                  <Briefcase size={16} className="text-[#1E3FB8]" />
                  <h3
                    className="text-[14px] font-bold text-[#1A1A1A]"
                    style={{ fontFamily: 'var(--font-sans)' }}
                  >
                    Datos Laborales
                  </h3>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">
                      Departamento<span className="text-[#DC2626] ml-0.5">*</span>
                    </span>
                    <select
                      {...registerNew('department_id')}
                      className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                        errorsNew.department_id ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                      } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                    >
                      <option value="">Seleccionar…</option>
                      {departments?.data.map((d) => (
                        <option key={d.id} value={d.id}>
                          {d.name}
                        </option>
                      ))}
                    </select>
                    {errorsNew.department_id && (
                      <span role="alert" className="text-[11px] text-[#DC2626]">
                        {errorsNew.department_id.message}
                      </span>
                    )}
                  </label>
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">Cargo</span>
                    <input
                      {...registerNew('position')}
                      placeholder="Ej: Operario"
                      className="w-full px-3 py-2 rounded text-[13px] border border-[#EEF0F2] bg-white focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent"
                    />
                  </label>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">
                      Fecha de Ingreso
                    </span>
                    <input
                      type="date"
                      {...registerNew('hire_date')}
                      className="w-full px-3 py-2 rounded text-[13px] border border-[#EEF0F2] bg-white focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent"
                    />
                  </label>
                  <label className="flex flex-col gap-1">
                    <span className="text-[12px] font-medium text-[#1A1A1A]">
                      Sueldo Base ($)
                    </span>
                    <input
                      type="text"
                      inputMode="decimal"
                      placeholder="0.00"
                      {...registerNew('base_salary')}
                      className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                        errorsNew.base_salary ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                      } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                    />
                    {errorsNew.base_salary ? (
                      <span role="alert" className="text-[11px] text-[#DC2626]">
                        {errorsNew.base_salary.message}
                      </span>
                    ) : (
                      <span className="text-[11px] text-[#666666]">
                        Sustituye al sueldo del departamento.
                      </span>
                    )}
                  </label>
                </div>
              </div>

              <div className="h-px bg-[#EEF0F2] -mx-7" />

              {/* Enrollment checkbox */}
              <label className="flex items-start gap-3 py-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={enrollAfterSave}
                  onChange={(e) => setEnrollAfterSave(e.target.checked)}
                  className="mt-0.5 h-[18px] w-[18px] rounded border-[#D1D5DB] text-[#1E3FB8] focus:ring-[#1E3FB8] focus:ring-offset-0"
                  data-testid="enroll-after-save"
                />
                <div className="flex flex-col gap-0.5">
                  <span className="text-[13px] font-medium text-[#1A1A1A]">
                    Iniciar enrolamiento facial al guardar
                  </span>
                  <span className="text-[11px] text-[#666666]">
                    Se abrirá el sincronizador biométrico para capturar la foto del empleado.
                  </span>
                </div>
              </label>
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between px-7 py-3 border-t border-[#EEF0F2] bg-[#FAFBFC]">
              <div className="flex items-center gap-1">
                <span className="text-[14px] font-bold text-[#DC2626]">*</span>
                <span className="text-[11px] text-[#666666]">Campos obligatorios</span>
              </div>
              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={() => {
                    resetNew()
                    setNewEmpOpen(false)
                  }}
                  className="px-6 py-2.5 rounded text-[13px] font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50 transition-colors"
                >
                  Cancelar
                </button>
                <PrimaryButton
                  type="submit"
                  size="md"
                  icon={Save}
                  data-testid="new-employee-submit"
                  disabled={isSubmittingNew || createMutation.isPending}
                >
                  {createMutation.isPending ? 'Guardando…' : 'Guardar Empleado'}
                </PrimaryButton>
              </div>
            </div>
          </form>
        </DialogContent>
      </Dialog>

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
          <form
            onSubmit={handleSubmitEdit((v) =>
              updateMutation.mutate({
                id: editEmployee!.id,
                version: editEmployee!.version,
                values: v,
              }),
            )}
            className="space-y-4"
          >
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
              <Label htmlFor="edit-emp-salary">Sueldo Base ($)</Label>
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
