'use client'
/**
 * "Nuevo Empleado" creation dialog — extracted from
 * `app/(dashboard)/employees/page.tsx` (Pencil F93Iv design) so the H-08/C-03
 * salary contract can be unit-tested (`src/app/**` is outside the frontend
 * coverage `include` glob; `src/components/**` is inside it).
 *
 * H-08 / C-03 context: the backend now REQUIRES both `base_salary_cents`
 * (a positive amount) and `salary_kind` (its unit) on POST /employees —
 * `employees::service::create_queued` rejects a missing salary with
 * `SALARY_REQUIRED`/`SALARY_INVALID` and a missing unit with
 * `SALARY_KIND_REQUIRED`. Before this file existed the form sent neither
 * field, so every employee-creation request returned 422.
 *
 * The unit `<select>` has NO default/preselected option on purpose: a
 * default is exactly how the original ambiguity (a monthly figure paid out
 * as if it were daily, multiplying the period by ~30) comes back — someone
 * accepts the form without looking and the amount once again has a unit
 * nobody chose.
 */
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Save, User, Briefcase, X } from 'lucide-react'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import type { CreateEmployeeRequest, Department, SalaryKind } from '@/types/api'

// ── Schema ───────────────────────────────────────────────────────────────

// Held as a string so the raw input reaches validation. Accepts ONLY plain
// digits with up to two decimals (no letters, scientific notation, signs) —
// feeding NaN/bogus values into the payroll cents calculation is the failure
// mode this refine exists to rule out. Converted to cents at submit time via
// parseSalaryCents().
const salaryAmountSchema = z
  .string()
  .trim()
  .min(1, 'Sueldo es requerido')
  .refine((v) => /^\d+(\.\d{1,2})?$/.test(v), {
    message: 'Sueldo debe ser un número válido (solo dígitos, máx. 2 decimales)',
  })
  .refine((v) => Number(v) > 0, { message: 'Sueldo debe ser mayor que 0' })

export const SALARY_KIND_OPTIONS: { value: SalaryKind; label: string }[] = [
  { value: 'hourly', label: 'Por hora' },
  { value: 'daily', label: 'Diario' },
  { value: 'monthly', label: 'Mensual' },
]

// H-08: once a unit is picked, the amount field's label says what the amount
// means instead of the old, ambiguous "Sueldo Base (USD)".
const SALARY_KIND_AMOUNT_LABEL: Record<SalaryKind, string> = {
  hourly: 'Monto por hora ($)',
  daily: 'Monto por día ($)',
  monthly: 'Monto por mes ($)',
}

const newEmployeeSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido'),
  employee_code: z.string().min(1, 'Cédula es requerida'),
  department_id: z.string().min(1, 'Departamento es requerido'),
  position: z.string().optional(),
  hire_date: z.string().optional(),
  base_salary: salaryAmountSchema,
  // No default value anywhere in this schema or the <select> below — an
  // empty string fails z.enum validation, which is precisely how "the user
  // must pick" is enforced (D-01 in the task brief).
  salary_kind: z.enum(['hourly', 'daily', 'monthly'], {
    error: 'Selecciona la unidad del sueldo',
  }),
})
export type NewEmployeeFormData = z.infer<typeof newEmployeeSchema>

function parseSalaryCents(value: string): number {
  return Math.round(Number(value) * 100)
}

// ── Component ────────────────────────────────────────────────────────────

interface NewEmployeeDialogProps {
  open: boolean
  departments?: Department[]
  enrollAfterSave: boolean
  onEnrollAfterSaveChange: (checked: boolean) => void
  onClose: () => void
  onSubmit: (payload: CreateEmployeeRequest) => void
  isPending: boolean
}

export function NewEmployeeDialog({
  open,
  departments,
  enrollAfterSave,
  onEnrollAfterSaveChange,
  onClose,
  onSubmit,
  isPending,
}: NewEmployeeDialogProps) {
  const {
    register,
    handleSubmit,
    reset,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<NewEmployeeFormData>({ resolver: zodResolver(newEmployeeSchema) })

  // The Dialog portal unmounts its content when closed (base-ui
  // `keepMounted` defaults to false), but this component itself — and the
  // useForm() instance living in it — stays mounted across open/close
  // cycles because the parent renders it unconditionally. Without this,
  // values from a cancelled or just-saved form would still be sitting in
  // react-hook-form's internal state the next time the dialog opens.
  useEffect(() => {
    if (!open) reset()
  }, [open, reset])

  const selectedKind = watch('salary_kind')
  const amountLabel = selectedKind
    ? SALARY_KIND_AMOUNT_LABEL[selectedKind as SalaryKind]
    : 'Sueldo Base ($)'

  function submit(values: NewEmployeeFormData) {
    onSubmit({
      employee_code: values.employee_code,
      name: values.name,
      department_id: values.department_id,
      ...(values.position && { position: values.position }),
      ...(values.hire_date && { hire_date: values.hire_date }),
      base_salary_cents: parseSalaryCents(values.base_salary),
      salary_kind: values.salary_kind,
    })
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o: boolean) => {
        if (!o) onClose()
      }}
    >
      <DialogContent
        className="max-w-[700px] p-0 overflow-hidden"
        data-testid="new-employee-form"
      >
        <form onSubmit={handleSubmit(submit)} className="flex flex-col">
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
              onClick={onClose}
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
                    {...register('name')}
                    placeholder="Ana Pérez González"
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
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Cédula<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    {...register('employee_code')}
                    placeholder="V-12345678"
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.employee_code ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  />
                  {errors.employee_code && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.employee_code.message}
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
                    {...register('department_id')}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.department_id ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  >
                    <option value="">Seleccionar…</option>
                    {departments?.map((d) => (
                      <option key={d.id} value={d.id}>
                        {d.name}
                      </option>
                    ))}
                  </select>
                  {errors.department_id && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.department_id.message}
                    </span>
                  )}
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">Cargo</span>
                  <input
                    {...register('position')}
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
                    {...register('hire_date')}
                    className="w-full px-3 py-2 rounded text-[13px] border border-[#EEF0F2] bg-white focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Unidad del Sueldo<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <select
                    {...register('salary_kind')}
                    data-testid="new-employee-salary-kind"
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.salary_kind ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  >
                    <option value="">Seleccionar…</option>
                    {SALARY_KIND_OPTIONS.map((o) => (
                      <option key={o.value} value={o.value}>
                        {o.label}
                      </option>
                    ))}
                  </select>
                  {errors.salary_kind && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.salary_kind.message}
                    </span>
                  )}
                </label>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    {amountLabel}<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    type="text"
                    inputMode="decimal"
                    placeholder="0.00"
                    data-testid="new-employee-base-salary"
                    {...register('base_salary')}
                    className={`w-full px-3 py-2 rounded text-[13px] border bg-white ${
                      errors.base_salary ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                  />
                  {errors.base_salary && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.base_salary.message}
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
                onChange={(e) => onEnrollAfterSaveChange(e.target.checked)}
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
                onClick={onClose}
                className="px-6 py-2.5 rounded text-[13px] font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50 transition-colors"
              >
                Cancelar
              </button>
              <PrimaryButton
                type="submit"
                size="md"
                icon={Save}
                data-testid="new-employee-submit"
                disabled={isSubmitting || isPending}
              >
                {isPending ? 'Guardando…' : 'Guardar Empleado'}
              </PrimaryButton>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
