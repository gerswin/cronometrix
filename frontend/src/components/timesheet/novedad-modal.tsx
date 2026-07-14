'use client'
import { useEffect, useState } from 'react'
import { useForm, Controller } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { AlertCircle, X, User, FileText, Paperclip, Upload, Save } from 'lucide-react'
import { api } from '@/lib/api'
import { novedadSchema, type NovedadFormData } from '@/lib/validations'
import type { DailyRecord, Department, Employee, PaginatedResponse } from '@/types/api'

import { Dialog, DialogContent } from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import { SearchableSelect } from '@/components/ui/searchable-select'

interface NovedadModalProps {
  open: boolean
  record: DailyRecord | null
  onClose: () => void
}

export function NovedadModal({ open, record, onClose }: NovedadModalProps) {
  const queryClient = useQueryClient()
  const [serverError, setServerError] = useState<string | null>(null)

  const {
    register,
    handleSubmit,
    control,
    reset,
    setValue,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<NovedadFormData>({
    resolver: zodResolver(novedadSchema),
    defaultValues: {
      tipo_novedad: 'manual',
      notificar_supervisor: false,
      employee_id: record?.employee_id ?? '',
      department_id: record?.department_id ?? '',
    },
  })

  const employeeId = watch('employee_id')
  const departmentId = watch('department_id')
  const evidence = watch('evidence')

  useEffect(() => {
    if (!open) return
    reset({
      tipo_novedad: 'manual',
      notificar_supervisor: false,
      employee_id: record?.employee_id ?? '',
      department_id: record?.department_id ?? '',
      fecha_inicio: record?.anchor_date ?? '',
      fecha_fin: record?.anchor_date ?? '',
      justification: '',
      motivo: '',
      impacto_nomina: '',
      evidence: undefined,
    })
  }, [open, record, reset])

  const { data: employees, isLoading: loadingEmployees } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', 'searchable'],
    queryFn: () => api.get('/employees', { params: { limit: 500 } }).then((r) => r.data),
    staleTime: 60_000,
    enabled: open,
  })

  const { data: departments, isLoading: loadingDepartments } = useQuery<PaginatedResponse<Department>>({
    queryKey: ['departments', 'searchable'],
    queryFn: () => api.get('/departments', { params: { limit: 200 } }).then((r) => r.data),
    staleTime: 300_000,
    enabled: open,
  })

  const employeeOptions = (employees?.data ?? []).map((e) => ({
    id: e.id,
    label: e.name,
    sublabel: [e.employee_code, e.department_name].filter(Boolean).join(' · '),
  }))

  const departmentOptions = (departments?.data ?? []).map((d) => ({
    id: d.id,
    label: d.name,
  }))

  const mutation = useMutation({
    mutationFn: async (values: NovedadFormData) => {
      const fd = new FormData()
      fd.append('employee_id', values.employee_id)
      fd.append('department_id', values.department_id)
      fd.append('from_date', values.fecha_inicio)
      fd.append('to_date', values.fecha_fin)
      fd.append('leave_type', values.tipo_novedad)
      fd.append('justification', values.justification)
      if (values.motivo) fd.append('motivo', values.motivo)
      if (values.evidence) fd.append('evidence', values.evidence)

      if (record?.id) {
        await api.post(`/daily-records/${record.id}/overrides`, fd, {
          headers: { 'Content-Type': 'multipart/form-data' },
        })
      } else {
        await api.post('/leaves', fd, {
          headers: { 'Content-Type': 'multipart/form-data' },
        })
      }
    },
    onMutate: () => {
      setServerError(null)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['daily-records'] })
      queryClient.invalidateQueries({ queryKey: ['leaves'] })
      toast.success('Novedad registrada')
      reset()
      onClose()
    },
    onError: (err: unknown) => {
      const data = (err as { response?: { data?: { error?: { code?: string; message?: string } } } })?.response?.data?.error
      const msg = data?.message ?? 'No se pudo registrar la novedad. Intenta de nuevo.'
      setServerError(msg)
      toast.error(msg)
    },
  })

  const handleClose = () => {
    reset()
    setServerError(null)
    onClose()
  }

  const inputCls = (hasError: boolean) =>
    `w-full px-3 py-2 rounded text-[13px] border bg-white ${
      hasError ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
    } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) handleClose()
      }}
    >
      <DialogContent
        className="max-w-[640px] p-0 overflow-hidden"
        data-testid="novedad-modal"
      >
        <form
          onSubmit={handleSubmit((v) => mutation.mutate(v))}
          className="flex flex-col"
        >
          {/* ── Header ────────────────────────────────────────── */}
          <div className="flex items-center justify-between px-7 py-4 border-b border-[#EEF0F2]">
            <div className="flex flex-col gap-0.5">
              <h2
                className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                Registrar Novedad
              </h2>
              <p
                className="text-[12px] italic text-[#666666]"
                style={{ fontFamily: 'var(--font-serif)' }}
              >
                Justifique una ausencia, retardo o incidencia del empleado
              </p>
            </div>
            <button
              type="button"
              aria-label="Cerrar"
              onClick={handleClose}
              className="flex items-center justify-center w-8 h-8 rounded bg-[#F3F4F6] hover:bg-[#E5E7EB] transition-colors"
            >
              <X size={18} className="text-[#666666]" />
            </button>
          </div>

          {/* ── Body ──────────────────────────────────────────── */}
          <div className="px-7 py-5 flex flex-col gap-4 max-h-[65vh] overflow-y-auto">
            {serverError && (
              <div
                role="alert"
                data-testid="novedad-server-error"
                className="flex items-start gap-2 p-3 rounded border-l-4 border-[#DC2626] bg-[#FEF2F2] text-[12px]"
              >
                <AlertCircle className="size-4 shrink-0 mt-0.5 text-[#DC2626]" />
                <span className="text-[#DC2626]">{serverError}</span>
              </div>
            )}

            {/* Sec 1 — Empleado y Fecha */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <User size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Empleado y Fecha
                </h3>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Empleado<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <SearchableSelect
                    id="employee_id"
                    data-testid="novedad-employee"
                    value={employeeId || null}
                    onChange={(id) => {
                      setValue('employee_id', id, { shouldValidate: true })
                      const emp = employees?.data.find((e) => e.id === id)
                      if (emp?.department_id) {
                        setValue('department_id', emp.department_id, { shouldValidate: true })
                      }
                    }}
                    options={employeeOptions}
                    placeholder="Buscar empleado…"
                    loading={loadingEmployees}
                  />
                  <input type="hidden" {...register('employee_id')} />
                  {errors.employee_id && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.employee_id.message}
                    </span>
                  )}
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Departamento<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <SearchableSelect
                    id="department_id"
                    data-testid="novedad-department"
                    value={departmentId || null}
                    onChange={(id) => setValue('department_id', id, { shouldValidate: true })}
                    options={departmentOptions}
                    placeholder="Buscar departamento…"
                    loading={loadingDepartments}
                  />
                  <input type="hidden" {...register('department_id')} />
                  {errors.department_id && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.department_id.message}
                    </span>
                  )}
                </label>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Fecha Inicio<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    type="date"
                    {...register('fecha_inicio')}
                    defaultValue={record?.anchor_date ?? ''}
                    className={inputCls(!!errors.fecha_inicio)}
                  />
                  {errors.fecha_inicio && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.fecha_inicio.message}
                    </span>
                  )}
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Fecha Fin<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <input
                    type="date"
                    {...register('fecha_fin')}
                    defaultValue={record?.anchor_date ?? ''}
                    className={inputCls(!!errors.fecha_fin)}
                  />
                  {errors.fecha_fin && (
                    <span role="alert" className="text-[11px] text-[#DC2626]">
                      {errors.fecha_fin.message}
                    </span>
                  )}
                </label>
              </div>
            </div>

            <div className="h-px bg-[#EEF0F2] -mx-7" />

            {/* Sec 2 — Tipo y Detalle */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <FileText size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Tipo y Detalle de la Novedad
                </h3>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Tipo<span className="text-[#DC2626] ml-0.5">*</span>
                  </span>
                  <select
                    {...register('tipo_novedad')}
                    className={inputCls(false)}
                  >
                    <option value="medical">Médica</option>
                    <option value="vacation">Vacaciones</option>
                    <option value="unpaid">Sin Goce</option>
                    <option value="manual">Manual</option>
                  </select>
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">Motivo</span>
                  <input
                    {...register('motivo')}
                    placeholder="Ej: Cita médica"
                    className={inputCls(false)}
                  />
                </label>
              </div>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] font-medium text-[#1A1A1A]">
                  Descripción / Justificación<span className="text-[#DC2626] ml-0.5">*</span>
                </span>
                <textarea
                  data-testid="novedad-justification"
                  {...register('justification')}
                  rows={3}
                  placeholder="Describa la razón de la novedad…"
                  className={`w-full px-3 py-2 rounded text-[13px] border bg-white resize-none ${
                    errors.justification ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                  } focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:border-transparent`}
                />
                {errors.justification && (
                  <span role="alert" className="text-[11px] text-[#DC2626]">
                    {errors.justification.message}
                  </span>
                )}
              </label>
            </div>

            <div className="h-px bg-[#EEF0F2] -mx-7" />

            {/* Sec 3 — Soporte y Opciones */}
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <Paperclip size={16} className="text-[#1E3FB8]" />
                <h3
                  className="text-[14px] font-bold text-[#1A1A1A]"
                  style={{ fontFamily: 'var(--font-sans)' }}
                >
                  Soporte y Opciones
                </h3>
              </div>

              {/* Upload zone */}
              <Controller
                name="evidence"
                control={control}
                render={({ field }) => (
                  <label
                    className={`flex flex-col items-center justify-center gap-1 py-3 rounded border bg-[#FAFBFC] cursor-pointer hover:bg-[#F3F4F6] transition-colors ${
                      errors.evidence ? 'border-[#DC2626]' : 'border-[#EEF0F2]'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <Upload size={18} className="text-[#1E3FB8]" />
                      <span className="text-[13px] font-medium text-[#1E3FB8]">
                        {evidence
                          ? (evidence as File).name
                          : 'Adjuntar soporte (reposo, permiso, etc.)'}
                      </span>
                    </div>
                    <span className="text-[11px] text-[#666666]">
                      PDF, JPG, PNG — Máx. 5MB
                    </span>
                    <input
                      type="file"
                      accept=".pdf,.jpg,.jpeg,.png"
                      data-testid="novedad-evidence"
                      onChange={(e) => field.onChange(e.target.files?.[0] ?? undefined)}
                      className="hidden"
                    />
                  </label>
                )}
              />
              {errors.evidence && (
                <span role="alert" className="text-[11px] text-[#DC2626]">
                  {errors.evidence.message as string}
                </span>
              )}

              {/* Impacto + Estado */}
              <div className="grid grid-cols-2 gap-4">
                <label className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Impacto en nómina
                  </span>
                  <select
                    {...register('impacto_nomina')}
                    className={inputCls(false)}
                  >
                    <option value="">No descontar</option>
                    <option value="full">Día completo</option>
                    <option value="partial">Parcial</option>
                  </select>
                </label>
                <div className="flex flex-col gap-1">
                  <span className="text-[12px] font-medium text-[#1A1A1A]">
                    Estado Inicial
                  </span>
                  <div className="flex items-center px-3 py-2">
                    <span className="text-[12px] font-medium px-2.5 py-0.5 rounded-full bg-[#DCFCE7] text-[#15803D]">
                      Aprobado
                    </span>
                  </div>
                </div>
              </div>

              {/* Notify supervisor */}
              <label className="flex items-start gap-3 py-1 cursor-pointer">
                <input
                  type="checkbox"
                  {...register('notificar_supervisor')}
                  data-testid="notify-supervisor"
                  className="mt-0.5 h-[18px] w-[18px] rounded border-[#D1D5DB] text-[#1E3FB8] focus:ring-[#1E3FB8] focus:ring-offset-0"
                />
                <div className="flex flex-col gap-0.5">
                  <span className="text-[13px] font-medium text-[#1A1A1A]">
                    Notificar al supervisor
                  </span>
                  <span className="text-[11px] text-[#666666]">
                    Se enviará un correo informando del registro de novedad.
                  </span>
                </div>
              </label>
            </div>
          </div>

          {/* ── Footer ────────────────────────────────────────── */}
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
                onClick={handleClose}
                className="px-6 py-2.5 rounded text-[13px] font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50 transition-colors"
              >
                Cancelar
              </button>
              <PrimaryButton
                type="submit"
                size="md"
                icon={Save}
                disabled={isSubmitting || mutation.isPending}
                data-testid="novedad-submit"
              >
                {mutation.isPending ? 'Registrando…' : 'Registrar Novedad'}
              </PrimaryButton>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
