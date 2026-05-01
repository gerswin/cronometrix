'use client'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { Info } from 'lucide-react'
import { api } from '@/lib/api'
import { rulesFormSchema, type RulesFormValues } from '@/lib/validations'
import type { GlobalRules, Department, PaginatedResponse } from '@/types/api'
import { fmtDateTime } from '@/lib/format/datetime'
import { ToleranceSimulator } from '@/components/settings/tolerance-simulator'

interface Props {
  initialData: GlobalRules
  canEdit: boolean
}

// ── Tolerance slider ─────────────────────────────────────────────────────────

interface SliderFieldProps {
  label: string
  value: number
  onChange: (v: number) => void
  disabled: boolean
}

function ToleranceSlider({ label, value, onChange, disabled }: SliderFieldProps) {
  const fillPct = (value / 60) * 100

  return (
    <div className="flex flex-col gap-2">
      {/* Header row: label + live badge */}
      <div className="flex items-center justify-between">
        <span className="text-[13px] font-medium text-[#1A1A1A]">{label}</span>
        <span
          className="text-[13px] font-semibold text-[#1A1A1A]"
          style={{ fontFamily: 'var(--font-mono)' }}
        >
          {value} min
        </span>
      </div>

      {/* Slider track wrapper */}
      <div className="relative h-[6px] rounded-[3px] bg-[#EEF0F2]">
        {/* Filled portion */}
        <div
          className="absolute left-0 top-0 h-full rounded-[3px] bg-[#1E3FB8] pointer-events-none"
          style={{ width: `${fillPct}%` }}
        />
        {/* Native range input — sits on top, fully transparent */}
        <input
          type="range"
          min={0}
          max={60}
          step={1}
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(Number(e.target.value))}
          className={[
            'tolerance-slider',
            'absolute inset-0 w-full h-full opacity-0 cursor-pointer',
            disabled ? 'cursor-not-allowed' : '',
          ]
            .filter(Boolean)
            .join(' ')}
          aria-label={label}
        />
        {/* Visible thumb */}
        <div
          className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-4 h-4 rounded-full bg-white border-2 border-[#1E3FB8] pointer-events-none"
          style={{
            left: `${fillPct}%`,
            boxShadow: '0 1px 4px rgba(0,0,0,0.18)',
            opacity: disabled ? 0.5 : 1,
          }}
        />
      </div>
    </div>
  )
}

// ── Main form component ──────────────────────────────────────────────────────

export function RulesForm({ initialData, canEdit }: Props) {
  const queryClient = useQueryClient()

  const {
    handleSubmit,
    reset,
    watch,
    setValue,
    formState: { isDirty },
  } = useForm<RulesFormValues>({
    resolver: zodResolver(rulesFormSchema),
    defaultValues: {
      late_arrival_tolerance_min: initialData.late_arrival_tolerance_min,
      early_departure_tolerance_min: initialData.early_departure_tolerance_min,
      bonus_minutes: initialData.bonus_minutes,
    },
  })

  // Re-prime when upstream data changes (post-save or 409 refetch)
  useEffect(() => {
    reset({
      late_arrival_tolerance_min: initialData.late_arrival_tolerance_min,
      early_departure_tolerance_min: initialData.early_departure_tolerance_min,
      bonus_minutes: initialData.bonus_minutes,
    })
  }, [initialData, reset])

  const mutation = useMutation({
    mutationFn: async (values: RulesFormValues) => {
      const resp = await api.patch('/rules', {
        ...values,
        version: initialData.version,
      })
      return resp.data
    },
    onSuccess: () => {
      toast.success('Reglas actualizadas')
      queryClient.invalidateQueries({ queryKey: ['rules'] })
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response?.status
      if (status === 409) {
        toast.error('Otro admin acaba de cambiar las reglas; recargando…')
        queryClient.invalidateQueries({ queryKey: ['rules'] })
      } else {
        toast.error('Error al guardar')
      }
    },
  })

  // Live field values for slider + simulator
  const lateArrival = watch('late_arrival_tolerance_min')
  const earlyDep = watch('early_departure_tolerance_min')
  const bonusMin = watch('bonus_minutes')

  // Departments query — take the first active one for the simulator
  const { data: departmentsData } = useQuery<PaginatedResponse<Department>>({
    queryKey: ['departments'],
    queryFn: () =>
      api.get('/departments', { params: { limit: 1000 } }).then((r) => r.data),
    staleTime: 10 * 60_000,
  })

  const firstDept =
    departmentsData?.data?.find((d) => d.status === 'active') ??
    departmentsData?.data?.[0] ??
    null

  const cardClass =
    'bg-white rounded border border-[#EEF0F2] flex flex-col'
  const cardShadow = {
    boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d',
  }

  return (
    <form
      id="rules-form"
      onSubmit={handleSubmit((v) => mutation.mutate(v))}
      aria-label="Reglas Globales"
      className="flex flex-col gap-6"
    >
      {/* ── Top row: tolerance card + bonus card ──────────────────────── */}
      <div className="flex gap-6">
        {/* Left card — Regla de los 20 Minutos */}
        <div className={`${cardClass} flex-1`} style={cardShadow}>
          <div className="px-4 py-[14px] border-b border-[#EEF0F2]">
            <span className="text-[15px] font-semibold text-[#1A1A1A]">
              Regla de los 20 Minutos
            </span>
          </div>
          <div className="p-5 flex flex-col gap-5">
            <ToleranceSlider
              label="Entrada Arriba"
              value={lateArrival}
              onChange={(v) =>
                setValue('late_arrival_tolerance_min', v, {
                  shouldDirty: true,
                  shouldValidate: true,
                })
              }
              disabled={!canEdit}
            />
            {/*
              SPEC DEVIATION: Pencil node JH3OT shows a second slider labelled
              "Entrada Abajo". Our backend models this as early_departure_tolerance_min
              (salida anticipada), so we relabel it "Salida Anticipada" to
              accurately reflect its meaning. The third design slot "Salida" is
              omitted here because bonus_minutes lives in the "Bolsa de Regalo"
              card — rendering it twice would be confusing.
            */}
            <ToleranceSlider
              label="Salida Anticipada"
              value={earlyDep}
              onChange={(v) =>
                setValue('early_departure_tolerance_min', v, {
                  shouldDirty: true,
                  shouldValidate: true,
                })
              }
              disabled={!canEdit}
            />
          </div>
        </div>

        {/* Right card — Bolsa de Regalo (fixed width) */}
        <div className={`${cardClass} shrink-0`} style={{ ...cardShadow, width: 300 }}>
          <div className="px-4 py-[14px] border-b border-[#EEF0F2]">
            <span className="text-[15px] font-semibold text-[#1A1A1A]">
              Bolsa de Regalo
            </span>
          </div>
          <div className="p-5 flex flex-col gap-4">
            <p className="text-[13px] text-[#666666] leading-snug">
              Minutos gratis antes de aplicar descuento al empleado.
            </p>

            {/* Bonus minutes number input */}
            <div className="flex flex-col gap-2">
              <label
                htmlFor="bonus_minutes_input"
                className="text-[12px] text-[#666666] tracking-[0.5px]"
                style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
              >
                Minutos Libres
              </label>
              <div className="flex items-center px-3 py-2.5 border border-[#EEF0F2] rounded">
                <input
                  id="bonus_minutes_input"
                  type="number"
                  min={0}
                  max={60}
                  step={1}
                  value={bonusMin}
                  disabled={!canEdit}
                  onChange={(e) => {
                    const raw = e.target.value
                    const num = raw === '' ? 0 : Math.max(0, Math.min(60, parseInt(raw, 10)))
                    if (!isNaN(num)) {
                      setValue('bonus_minutes', num, {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                  }}
                  aria-label="Minutos Libres"
                  className={[
                    'w-full bg-transparent text-[14px] font-semibold text-[#1A1A1A] outline-none',
                    '[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none',
                    !canEdit ? 'opacity-50 cursor-not-allowed' : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  style={{ fontFamily: 'var(--font-mono)' }}
                />
              </div>
            </div>

            {/* Info note */}
            <div className="flex items-start gap-2 px-3 py-2.5 rounded bg-[#DCEAFD]">
              <Info size={16} className="text-[#1E3FB8] shrink-0 mt-[1px]" />
              <p className="text-[12px] text-[#1A1A1A] leading-snug">
                Los primeros{' '}
                <span className="font-semibold" style={{ fontFamily: 'var(--font-mono)' }}>
                  {bonusMin}
                </span>{' '}
                min de retraso acumulado no se descuentan.
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* ── Simulator card ─────────────────────────────────────────────── */}
      <div className={cardClass} style={cardShadow}>
        <div className="px-4 py-[14px] border-b border-[#EEF0F2]">
          <span className="text-[15px] font-semibold text-[#1A1A1A]">
            Simulador de Tolerancias
          </span>
        </div>
        <div className="p-5">
          <ToleranceSimulator
            lateMin={lateArrival}
            earlyMin={earlyDep}
            bonusMin={bonusMin}
            department={firstDept}
          />
        </div>
      </div>

      {/* ── Metadata footer ─────────────────────────────────────────────── */}
      <div className="text-[12px] text-[#666666] border-t border-[#EEF0F2] pt-3 flex flex-col gap-1">
        <div>
          Vigentes desde:{' '}
          <span className="text-[#1A1A1A]">{fmtDateTime(initialData.effective_from)}</span>
        </div>
        <div>
          Última actualización:{' '}
          <span className="text-[#1A1A1A]">{fmtDateTime(initialData.updated_at)}</span>
        </div>
        <div>
          Versión: <span className="text-[#1A1A1A]">{initialData.version}</span>
        </div>
      </div>

      {/*
        Hidden submit — the real submit button lives in the page header
        and targets this form via form="rules-form". This hidden button
        is a fallback for keyboard Enter submission inside the form.
      */}
      <button type="submit" className="sr-only" tabIndex={-1} aria-hidden="true">
        Guardar
      </button>
    </form>
  )
}
