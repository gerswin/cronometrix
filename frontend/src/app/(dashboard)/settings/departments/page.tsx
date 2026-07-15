'use client'

import { useState, useMemo } from 'react'
import { useRouter } from 'next/navigation'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { Plus, X, LogOut, Loader2, Building2, Clock4, Calendar, Save } from 'lucide-react'

import { api, logoutCurrentSession } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import {
  Dialog,
  DialogContent,
} from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import {
  departmentFormSchema,
  type DepartmentFormValues,
} from '@/lib/validations'
import type { Department, Employee, PaginatedResponse } from '@/types/api'

// ── Helpers ──────────────────────────────────────────────────────────────────

function fmtSalaryWhole(cents: number): string {
  // Pencil shows "$450,000" — whole dollars with thousands separators.
  return `$${Math.round(cents / 100).toLocaleString('en-US')}`
}

const LUNCH_BADGE_CONFIG = {
  punch: { bg: '#22C55E', label: 'Obligatorio' },   // empleados marcan
  fixed: { bg: '#F59E0B', label: 'Descuento' },     // deducción automática
} as const

const LUNCH_LABELS = {
  punch: 'Marcaje Obligatorio',
  fixed: 'Descuento Auto.',
} as const

// ── Page ─────────────────────────────────────────────────────────────────────

export default function DepartmentsPage() {
  const router = useRouter()
  const { role } = useAuth()
  const queryClient = useQueryClient()
  const isAdmin = role === 'admin'

  const [createOpen, setCreateOpen] = useState(false)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  // Departments — full list
  const { data: deptsData, isLoading, error } =
    useQuery<PaginatedResponse<Department>>({
      queryKey: ['departments', 'all'],
      queryFn: () =>
        api.get('/departments', { params: { limit: 200 } }).then((r) => r.data),
    })

  // Employees — single fetch, grouped client-side per design decision
  const { data: employeesData } =
    useQuery<PaginatedResponse<Employee>>({
      queryKey: ['employees', 'all-active'],
      queryFn: () =>
        api
          .get('/employees', { params: { status: 'active', limit: 10000 } })
          .then((r) => r.data),
      staleTime: 5 * 60_000,
    })

  const departments = useMemo(
    () =>
      (deptsData?.data ?? []).filter(
        (d) => d.status === 'active' || d.status === undefined,
      ),
    [deptsData],
  )

  const empCountByDept = useMemo(() => {
    const map = new Map<string, number>()
    for (const e of employeesData?.data ?? []) {
      map.set(e.department_id, (map.get(e.department_id) ?? 0) + 1)
    }
    return map
  }, [employeesData])

  const empListByDept = useMemo(() => {
    const map = new Map<string, Employee[]>()
    for (const e of employeesData?.data ?? []) {
      const list = map.get(e.department_id) ?? []
      list.push(e)
      map.set(e.department_id, list)
    }
    return map
  }, [employeesData])

  const selected = useMemo(
    () => departments.find((d) => d.id === selectedId) ?? null,
    [departments, selectedId],
  )

  // ── Logout ─────────────────────────────────────────────────────────────────

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await logoutCurrentSession()
    } finally {
      router.push('/login')
    }
  }

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div className="flex flex-col h-full bg-[#F8F9FA]">
      {/* ── Header ────────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-6 py-4">
        <div className="flex flex-col gap-1">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Inicio / Departamentos
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Gestión de Departamentos
          </h1>
        </div>

        <div className="flex items-center gap-3">
          {isAdmin && (
            <PrimaryButton
              type="button"
              size="sm"
              icon={Plus}
              data-testid="new-department-trigger"
              onClick={() => setCreateOpen(true)}
            >
              Nuevo Depto.
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

      {/* ── Main content: table + side panel ──────────────────────────── */}
      <div className="flex-1 overflow-hidden flex gap-6 p-6">
        {/* Departments table */}
        <section
          className="flex-1 bg-white rounded border border-[#EEF0F2] overflow-hidden flex flex-col"
          style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
        >
          {/* Table header */}
          <div
            className="flex items-center px-4 py-3 bg-[#F8F9FA] border-b border-[#EEF0F2]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            <div className="flex-1 text-[12px] text-[#666666] tracking-wide">
              Nombre
            </div>
            <div className="w-[120px] text-[12px] text-[#666666] tracking-wide">
              Tipo Almuerzo
            </div>
            <div className="w-[100px] text-[12px] text-[#666666] tracking-wide">
              Min. Almuerzo
            </div>
            <div className="w-[120px] text-[12px] text-[#666666] tracking-wide">
              Sueldo Base
            </div>
            <div className="w-[80px] text-[12px] text-[#666666] tracking-wide">
              Empleados
            </div>
          </div>

          {/* Table body */}
          <div className="flex-1 overflow-auto">
            {isLoading && (
              <div className="flex items-center gap-2 px-4 py-6 text-[13px] text-[#666666]">
                <Loader2 size={14} className="animate-spin" />
                Cargando…
              </div>
            )}
            {error && (
              <div className="px-4 py-6 text-[13px] text-red-600">
                Error al cargar departamentos
              </div>
            )}
            {!isLoading && departments.length === 0 && (
              <div className="px-4 py-12 text-center text-[13px] text-[#666666]">
                Aún no hay departamentos. Crea el primero para asignar empleados.
              </div>
            )}
            {departments.map((dept) => {
              const lunchCfg = LUNCH_BADGE_CONFIG[dept.lunch_mode]
              const isSelected = dept.id === selectedId
              const empCount = empCountByDept.get(dept.id) ?? 0

              return (
                <button
                  key={dept.id}
                  type="button"
                  onClick={() => setSelectedId(dept.id)}
                  data-testid={`dept-row-${dept.id}`}
                  className={[
                    'flex items-center w-full px-4 py-3 border-b border-[#EEF0F2] text-left transition-colors',
                    isSelected
                      ? 'bg-[#1E3FB8] text-white'
                      : 'bg-white text-[#1A1A1A] hover:bg-slate-50',
                  ].join(' ')}
                >
                  <div className="flex-1 text-[13px] font-medium">
                    {dept.name}
                  </div>
                  <div className="w-[120px]">
                    <span
                      className="rounded px-2 py-0.5 text-[11px] font-medium text-white"
                      style={{ backgroundColor: lunchCfg.bg }}
                    >
                      {lunchCfg.label}
                    </span>
                  </div>
                  <div
                    className="w-[100px] text-[13px]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {dept.lunch_mode === 'fixed'
                      ? `${dept.lunch_duration_min ?? 0} min`
                      : '—'}
                  </div>
                  <div
                    className="w-[120px] text-[13px]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {fmtSalaryWhole(dept.base_salary_cents)}
                  </div>
                  <div
                    className="w-[80px] text-[13px] font-semibold"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {empCount}
                  </div>
                </button>
              )
            })}
          </div>
        </section>

        {/* Side panel (only when a row is selected) */}
        {selected && (
          <ConfigPanel
            key={`${selected.id}:${selected.version}`}
            department={selected}
            employees={empListByDept.get(selected.id) ?? []}
            canEdit={isAdmin}
            onClose={() => setSelectedId(null)}
            onSaved={() =>
              queryClient.invalidateQueries({ queryKey: ['departments'] })
            }
          />
        )}
      </div>

      {/* Create modal — only for new departments */}
      {createOpen && (
        <DepartmentCreateDialog
          onClose={() => setCreateOpen(false)}
          onSuccess={() => {
            queryClient.invalidateQueries({ queryKey: ['departments'] })
            setCreateOpen(false)
          }}
        />
      )}
    </div>
  )
}

// ── Side panel — inline edit ─────────────────────────────────────────────────

interface ConfigPanelProps {
  department: Department
  employees: Employee[]
  canEdit: boolean
  onClose: () => void
  onSaved: () => void
}

function ConfigPanel({
  department,
  employees,
  canEdit,
  onClose,
  onSaved,
}: ConfigPanelProps) {
  const queryClient = useQueryClient()
  const router = useRouter()
  const totalEmployees = employees.length
  const visibleEmployees = employees.slice(0, 5)
  const remaining = Math.max(0, totalEmployees - visibleEmployees.length)

  // Local edit state — pre-primed from the department prop. Re-syncs whenever
  // the parent passes a different `department` (key={id} forces a fresh mount).
  const [lunchMode, setLunchMode] =
    useState<Department['lunch_mode']>(department.lunch_mode)
  const [lunchDuration, setLunchDuration] = useState<number>(
    department.lunch_duration_min ?? 60,
  )
  const [baseSalary, setBaseSalary] = useState<number>(
    department.base_salary_cents / 100,
  )
  const [shiftStart, setShiftStart] = useState(department.shift_start_time)
  const [shiftEnd, setShiftEnd] = useState(department.shift_end_time)

  const isDirty =
    lunchMode !== department.lunch_mode ||
    (lunchMode === 'fixed' && lunchDuration !== (department.lunch_duration_min ?? 0)) ||
    Math.round(baseSalary * 100) !== department.base_salary_cents ||
    shiftStart !== department.shift_start_time ||
    shiftEnd !== department.shift_end_time

  const mutation = useMutation({
    mutationFn: async () => {
      await api.patch(`/departments/${department.id}`, {
        name: department.name,
        base_salary_cents: Math.round(baseSalary * 100),
        shift_start_time: shiftStart,
        shift_end_time: shiftEnd,
        lunch_mode: lunchMode,
        lunch_duration_min: lunchMode === 'fixed' ? lunchDuration : null,
        version: department.version,
      })
    },
    onSuccess: () => {
      toast.success('Departamento actualizado')
      onSaved()
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response
        ?.status
      if (status === 409) {
        toast.error('Otro admin lo modificó; recargando…')
        queryClient.invalidateQueries({ queryKey: ['departments'] })
      } else {
        const msg =
          (err as { response?: { data?: { error?: { message?: string } } } })
            ?.response?.data?.error?.message ?? 'Error al guardar'
        toast.error(msg)
      }
    },
  })

  return (
    <aside
      className="w-[340px] shrink-0 bg-white rounded border border-[#EEF0F2] flex flex-col"
      style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
      data-testid="dept-config-panel"
    >
      {/* Panel header */}
      <div className="flex items-center justify-between px-4 py-[14px] border-b border-[#EEF0F2]">
        <span className="text-[15px] font-semibold text-[#1A1A1A]">
          Configurar: {department.name}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="Cerrar panel"
          className="text-[#666666] hover:text-[#1A1A1A] transition-colors"
        >
          <X size={16} />
        </button>
      </div>

      {/* Panel body */}
      <div className="flex-1 overflow-auto p-5 flex flex-col gap-5">
        {/* Tipo de Almuerzo (toggle) */}
        <div className="flex flex-col gap-2">
          <span
            className="text-[12px] text-[#666666] tracking-wide"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Tipo de Almuerzo
          </span>
          <div className="flex items-center rounded border border-[#EEF0F2] overflow-hidden">
            <button
              type="button"
              disabled={!canEdit}
              onClick={() => setLunchMode('punch')}
              className={[
                'flex-1 px-3 py-2.5 text-[12px] font-medium text-center transition-colors',
                lunchMode === 'punch'
                  ? 'bg-[#1E3FB8] text-white'
                  : 'bg-white text-[#1A1A1A] hover:bg-slate-50',
                !canEdit ? 'cursor-not-allowed opacity-60' : '',
              ].join(' ')}
            >
              {LUNCH_LABELS.punch}
            </button>
            <button
              type="button"
              disabled={!canEdit}
              onClick={() => setLunchMode('fixed')}
              className={[
                'flex-1 px-3 py-2.5 text-[12px] font-medium text-center transition-colors',
                lunchMode === 'fixed'
                  ? 'bg-[#1E3FB8] text-white'
                  : 'bg-white text-[#1A1A1A] hover:bg-slate-50',
                !canEdit ? 'cursor-not-allowed opacity-60' : '',
              ].join(' ')}
            >
              {LUNCH_LABELS.fixed}
            </button>
          </div>
        </div>

        {/* Minutos de Almuerzo (only meaningful in 'fixed') */}
        <div className="flex flex-col gap-2">
          <span
            className="text-[12px] text-[#666666] tracking-wide"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Minutos de Almuerzo
          </span>
          <input
            type="number"
            min={0}
            max={240}
            step={1}
            value={lunchDuration}
            disabled={!canEdit || lunchMode !== 'fixed'}
            onChange={(e) => setLunchDuration(Number(e.target.value))}
            className="rounded border border-[#EEF0F2] px-3 py-2.5 text-[14px] text-[#1A1A1A] disabled:bg-slate-50 disabled:text-slate-500"
            style={{ fontFamily: 'var(--font-mono)' }}
          />
        </div>

        {/* Sueldo Base */}
        <div className="flex flex-col gap-2">
          <span
            className="text-[12px] text-[#666666] tracking-wide"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Sueldo Base ($)
          </span>
          <input
            type="number"
            min={0}
            step={0.01}
            value={baseSalary}
            disabled={!canEdit}
            onChange={(e) => setBaseSalary(Number(e.target.value))}
            className="rounded border border-[#EEF0F2] px-3 py-2.5 text-[14px] text-[#1A1A1A] disabled:bg-slate-50 disabled:text-slate-500"
            style={{ fontFamily: 'var(--font-mono)' }}
          />
        </div>

        {/* Shift times — extra fields not in Pencil but required by backend */}
        <div className="flex gap-3">
          <div className="flex-1 flex flex-col gap-2">
            <span
              className="text-[12px] text-[#666666] tracking-wide"
              style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
            >
              Inicio Turno
            </span>
            <input
              type="time"
              value={shiftStart}
              disabled={!canEdit}
              onChange={(e) => setShiftStart(e.target.value)}
              className="rounded border border-[#EEF0F2] px-3 py-2.5 text-[14px] text-[#1A1A1A] disabled:bg-slate-50 disabled:text-slate-500"
              style={{ fontFamily: 'var(--font-mono)' }}
            />
          </div>
          <div className="flex-1 flex flex-col gap-2">
            <span
              className="text-[12px] text-[#666666] tracking-wide"
              style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
            >
              Fin Turno
            </span>
            <input
              type="time"
              value={shiftEnd}
              disabled={!canEdit}
              onChange={(e) => setShiftEnd(e.target.value)}
              className="rounded border border-[#EEF0F2] px-3 py-2.5 text-[14px] text-[#1A1A1A] disabled:bg-slate-50 disabled:text-slate-500"
              style={{ fontFamily: 'var(--font-mono)' }}
            />
          </div>
        </div>

        {/* Empleados Vinculados */}
        <div className="flex flex-col gap-2 flex-1 min-h-0">
          <div className="flex items-center justify-between">
            <span
              className="text-[12px] text-[#666666] tracking-wide"
              style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
            >
              Empleados Vinculados
            </span>
            <span
              className="text-[12px] font-semibold text-[#1E3FB8]"
              style={{ fontFamily: 'var(--font-mono)' }}
            >
              {totalEmployees}
            </span>
          </div>
          <div className="flex flex-col gap-1 overflow-auto">
            {visibleEmployees.length === 0 && (
              <span className="text-[12px] text-[#666666] py-2">
                Sin empleados asignados
              </span>
            )}
            {visibleEmployees.map((emp) => (
              <div
                key={emp.id}
                className="flex items-center gap-2.5 bg-[#F8F9FA] rounded px-2 py-1.5"
              >
                <span
                  className="w-6 h-6 rounded-full bg-[#1E3FB8] text-white text-[10px] font-semibold flex items-center justify-center shrink-0"
                  aria-hidden="true"
                >
                  {emp.name
                    .split(' ')
                    .map((p) => p[0])
                    .join('')
                    .slice(0, 2)
                    .toUpperCase()}
                </span>
                <span className="text-[12px] text-[#1A1A1A] truncate">
                  {emp.name}
                </span>
              </div>
            ))}
            {remaining > 0 && (
              <button
                type="button"
                onClick={() =>
                  router.push(
                    `/employees?department_id=${department.id}&status=active`,
                  )
                }
                className="text-left text-[12px] text-[#1E3FB8] hover:underline mt-1 px-2"
              >
                + {remaining} más…
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Footer save button */}
      {canEdit && (
        <div className="px-5 py-4 border-t border-[#EEF0F2] flex justify-end">
          <button
            type="button"
            onClick={() => mutation.mutate()}
            disabled={!isDirty || mutation.isPending}
            className="inline-flex items-center gap-1.5 rounded px-4 py-2 text-[13px] font-medium text-white bg-[#1E3FB8] hover:bg-[#1835A0] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            {mutation.isPending ? 'Guardando…' : 'Guardar Cambios'}
          </button>
        </div>
      )}
    </aside>
  )
}

// ── Create modal — minimal, only for new departments ─────────────────────────

function DepartmentCreateDialog({
  onClose,
  onSuccess,
}: {
  onClose: () => void
  onSuccess: () => void
}) {
  const {
    register,
    handleSubmit,
    watch,
    setValue,
    formState: { errors, isSubmitting },
  } = useForm<DepartmentFormValues>({
    resolver: zodResolver(departmentFormSchema),
    defaultValues: {
      name: '',
      base_salary: 0,
      shift_start_time: '08:00',
      shift_end_time: '17:00',
      lunch_mode: 'fixed',
      lunch_duration_min: 60,
    },
  })

  const lunchMode = watch('lunch_mode')

  const mutation = useMutation({
    mutationFn: async (values: DepartmentFormValues) => {
      await api.post('/departments', {
        name: values.name,
        base_salary_cents: Math.round(values.base_salary * 100),
        shift_start_time: values.shift_start_time,
        shift_end_time: values.shift_end_time,
        lunch_mode: values.lunch_mode,
        lunch_duration_min:
          values.lunch_mode === 'fixed' ? values.lunch_duration_min ?? 0 : null,
      })
    },
    onSuccess: () => {
      toast.success('Departamento creado')
      onSuccess()
    },
    onError: (err: unknown) => {
      const msg =
        (err as { response?: { data?: { error?: { message?: string } } } })
          ?.response?.data?.error?.message ?? 'Error al crear'
      toast.error(msg)
    },
  })

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent
        className="max-w-[640px] p-0 overflow-hidden"
        data-testid="department-form"
      >
        <form
          onSubmit={handleSubmit((v) => mutation.mutate(v))}
          className="flex flex-col"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-7 py-4 border-b border-[#EEF0F2]">
            <div className="flex flex-col gap-0.5">
              <h2
                className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                Crear Departamento
              </h2>
              <p
                className="text-[12px] italic text-[#666666]"
                style={{ fontFamily: 'var(--font-serif)' }}
              >
                Configure un nuevo departamento y sus reglas de asistencia
              </p>
            </div>
            <button
              type="button"
              aria-label="Cerrar"
              onClick={onClose}
              className="flex items-center justify-center w-8 h-8 rounded bg-[#F3F4F6] hover:bg-[#E5E7EB] transition-colors"
            >
              <X size={18} className="text-[#666666]" />
            </button>
          </div>

          {/* Body */}
          <div className="px-7 py-5 flex flex-col gap-4 max-h-[60vh] overflow-y-auto">
            {/* Sec 1 — Información General */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <Building2 size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Información General
                </h3>
              </div>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] font-medium text-[#1A1A1A]">
                  Nombre<span className="text-[#DC2626] ml-0.5">*</span>
                </span>
                <input
                  {...register('name')}
                  placeholder="Producción"
                  className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                    errors.name ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                  } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                />
                {errors.name && (
                  <span role="alert" className="text-[11px] text-[#DC2626]">
                    {errors.name.message}
                  </span>
                )}
              </label>
            </div>

            <div className="h-px bg-[#EEF0F2] -mx-7" />

            {/* Sec 2 — Reglas de Asistencia */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <Clock4 size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Reglas de Asistencia
                </h3>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Tipo de Almuerzo<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <select
                    {...register('lunch_mode', {
                      onChange: (e) => {
                        if (e.target.value === 'punch')
                          setValue('lunch_duration_min', null)
                        else if (watch('lunch_duration_min') === null)
                          setValue('lunch_duration_min', 60)
                      },
                    })}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.lunch_mode ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  >
                    <option value="punch">Marcaje Obligatorio</option>
                    <option value="fixed">Descuento Auto.</option>
                  </select>
                  {errors.lunch_mode && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.lunch_mode.message}
                    </span>
                  )}
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Minutos de Almuerzo
                    {lunchMode === 'fixed' && (
                      <span className="text-[#DC2626] ml-0.5">*</span>
                    )}
                  </span>
                  <input
                    type="number"
                    min="0"
                    step="1"
                    disabled={lunchMode !== 'fixed'}
                    {...register('lunch_duration_min', { valueAsNumber: true })}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.lunch_duration_min ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent disabled:bg-[#F8F9FA] disabled:text-[#999999]`}
                  />
                  {errors.lunch_duration_min && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.lunch_duration_min.message}
                    </span>
                  )}
                </label>
              </div>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] font-medium text-[#1A1A1A]">
                  Sueldo Base (USD)<span className="text-[#DC2626] ml-0.5">*</span>
                </span>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder="0.00"
                  {...register('base_salary', { valueAsNumber: true })}
                  className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                    errors.base_salary ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                  } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                />
                {errors.base_salary ? (
                  <span role="alert" className="text-[11px] text-[#DC2626]">
                    {errors.base_salary.message}
                  </span>
                ) : (
                  <span className="text-[11px] text-[#666666]">
                    Sueldo sugerido por defecto. Cada empleado puede tener su propio sueldo.
                  </span>
                )}
              </label>
            </div>

            <div className="h-px bg-[#EEF0F2] -mx-7" />

            {/* Sec 3 — Horario por Defecto */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <Calendar size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Horario por Defecto
                </h3>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Hora de Entrada<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    type="time"
                    {...register('shift_start_time')}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.shift_start_time ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  />
                  {errors.shift_start_time && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.shift_start_time.message}
                    </span>
                  )}
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Hora de Salida<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    type="time"
                    {...register('shift_end_time')}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.shift_end_time ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  />
                  {errors.shift_end_time && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.shift_end_time.message}
                    </span>
                  )}
                </label>
              </div>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between px-7 py-3 border-t border-[#EEF0F2] bg-[#FAFBFC]">
            <div className="flex items-center gap-1">
              <span className="text-[14px] font-bold text-[#DC2626]">*</span>
              <span className="text-[11px] text-[#666666]">
                Campos obligatorios
              </span>
            </div>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={onClose}
                className="px-6 py-2.5 rounded text-[13px] font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50 transition-colors"
              >
                Cancelar
              </button>
              <PrimaryButton
                type="submit"
                size="md"
                icon={Save}
                disabled={isSubmitting || mutation.isPending}
              >
                {mutation.isPending ? 'Guardando…' : 'Crear Departamento'}
              </PrimaryButton>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
