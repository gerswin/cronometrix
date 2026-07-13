'use client'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import {
  X,
  MonitorSmartphone,
  Wifi,
  Settings,
  Save,
} from 'lucide-react'
import { api } from '@/lib/api'
import {
  createDeviceFormSchema,
  type CreateDeviceFormValues,
} from '@/lib/validations'
import {
  Dialog,
  DialogContent,
  DialogClose,
} from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'

interface CreateDeviceModalProps {
  open: boolean
  onClose: () => void
}

const DEFAULT_VALUES: CreateDeviceFormValues = {
  name: '',
  ip: '',
  port: 80,
  scheme: 'http',
  username: 'admin',
  password: '',
  direction: 'entry',
  allow_insecure_tls: false,
}

export function CreateDeviceModal({ open, onClose }: CreateDeviceModalProps) {
  const qc = useQueryClient()

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateDeviceFormValues>({
    resolver: zodResolver(createDeviceFormSchema),
    defaultValues: DEFAULT_VALUES,
  })

  useEffect(() => {
    if (open) reset(DEFAULT_VALUES)
  }, [open, reset])

  const mutation = useMutation({
    mutationFn: (body: CreateDeviceFormValues) =>
      api.post('/devices', body).then((r) => r.data),
    onSuccess: () => {
      toast.success('Dispositivo creado')
      qc.invalidateQueries({ queryKey: ['devices'] })
      onClose()
    },
    onError: (err: unknown) => {
      const msg =
        (err as { response?: { data?: { error?: { message?: string } } } })
          ?.response?.data?.error?.message ?? 'Error al crear dispositivo'
      toast.error(msg)
    },
  })

  function onSubmit(values: CreateDeviceFormValues) {
    mutation.mutate(values)
  }

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose() }}>
      <DialogContent
        className="max-w-[680px] p-0 overflow-hidden"
        data-testid="create-device-modal"
      >
        <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col">
          {/* ── Header ─────────────────────────────────────────────── */}
          <div className="flex items-center justify-between px-7 py-4 border-b border-[#EEF0F2]">
            <div className="flex flex-col gap-0.5">
              <h2
                className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                Agregar Dispositivo
              </h2>
              <p
                className="text-[12px] italic text-[#666666]"
                style={{ fontFamily: 'var(--font-serif)' }}
              >
                Registre un nuevo dispositivo biométrico en la red
              </p>
            </div>
            <DialogClose
              type="button"
              aria-label="Cerrar"
              className="flex items-center justify-center w-8 h-8 rounded bg-[#F3F4F6] hover:bg-[#E5E7EB] transition-colors"
            >
              <X size={18} className="text-[#666666]" />
            </DialogClose>
          </div>

          {/* ── Body ───────────────────────────────────────────────── */}
          <div className="px-7 py-5 flex flex-col gap-4 max-h-[60vh] overflow-y-auto">
            {/* Section 1 — Identificación */}
            <Section icon={<MonitorSmartphone size={16} className="text-[#1E3FB8]" />}
              title="Identificación del Dispositivo">
              <Row>
                <Field label="Nombre" required error={errors.name?.message}>
                  <input
                    {...register('name')}
                    placeholder="Entrada Principal"
                    className={inputCls(!!errors.name)}
                    data-testid="device-name"
                  />
                </Field>
                <Field label="Función" required error={errors.direction?.message}>
                  <select
                    {...register('direction')}
                    className={inputCls(!!errors.direction)}
                    data-testid="device-direction"
                  >
                    <option value="entry">Entrada</option>
                    <option value="exit">Salida</option>
                  </select>
                </Field>
              </Row>
            </Section>

            <Divider />

            {/* Section 2 — Red */}
            <Section icon={<Wifi size={16} className="text-[#1E3FB8]" />}
              title="Configuración de Red">
              <Row>
                <Field label="Dirección IP / Host" required error={errors.ip?.message}>
                  <input
                    {...register('ip')}
                    placeholder="192.168.1.50"
                    className={inputCls(!!errors.ip)}
                    data-testid="device-ip"
                  />
                </Field>
                <Field label="Puerto" required error={errors.port?.message}>
                  <input
                    type="number"
                    {...register('port', { valueAsNumber: true })}
                    placeholder="80"
                    className={inputCls(!!errors.port)}
                    data-testid="device-port"
                  />
                </Field>
              </Row>
              <Row>
                <Field label="Esquema" required error={errors.scheme?.message}>
                  <select
                    {...register('scheme')}
                    className={inputCls(!!errors.scheme)}
                    data-testid="device-scheme"
                  >
                    <option value="http">HTTP</option>
                    <option value="https">HTTPS</option>
                  </select>
                </Field>
                <Field label="Usuario" required error={errors.username?.message}>
                  <input
                    {...register('username')}
                    placeholder="admin"
                    className={inputCls(!!errors.username)}
                    data-testid="device-username"
                  />
                </Field>
              </Row>
              <Row>
                <Field label="Contraseña" required error={errors.password?.message}>
                  <input
                    type="password"
                    {...register('password')}
                    placeholder="••••••••"
                    autoComplete="new-password"
                    className={inputCls(!!errors.password)}
                    data-testid="device-password"
                  />
                </Field>
                <div /> {/* spacer to keep two-column layout */}
              </Row>
            </Section>

            <Divider />

            {/* Section 3 — Opciones */}
            <Section icon={<Settings size={16} className="text-[#1E3FB8]" />}
              title="Opciones">
              <label className="flex items-start gap-3 py-1 cursor-pointer">
                <input
                  type="checkbox"
                  {...register('allow_insecure_tls')}
                  data-testid="device-insecure-tls"
                  className="mt-0.5 h-[18px] w-[18px] rounded border-[#D1D5DB] text-[#1E3FB8] focus:ring-[#1E3FB8] focus:ring-offset-0"
                />
                <div className="flex flex-col gap-0.5">
                  <span className="text-[13px] font-medium text-[#1A1A1A]">
                    Permitir TLS inseguro
                  </span>
                  <span className="text-[11px] text-[#666666]">
                    Acepta certificados auto-firmados. Solo úsalo en redes
                    locales controladas.
                  </span>
                </div>
              </label>
            </Section>
          </div>

          {/* ── Footer ─────────────────────────────────────────────── */}
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
                data-testid="cancel-device-btn"
              >
                Cancelar
              </button>
              <PrimaryButton
                type="submit"
                size="md"
                icon={Save}
                disabled={isSubmitting || mutation.isPending}
                data-testid="save-device-btn"
              >
                {mutation.isPending ? 'Guardando…' : 'Guardar Dispositivo'}
              </PrimaryButton>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ── Layout primitives ──────────────────────────────────────────────────

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        {icon}
        <h3
          className="text-[14px] font-bold text-[#1A1A1A]"
          style={{ fontFamily: 'var(--font-sans)' }}
        >
          {title}
        </h3>
      </div>
      <div className="flex flex-col gap-3">{children}</div>
    </div>
  )
}

function Row({ children }: { children: React.ReactNode }) {
  return <div className="grid grid-cols-2 gap-4">{children}</div>
}

function Field({
  label,
  required,
  error,
  children,
}: {
  label: string
  required?: boolean
  error?: string
  children: React.ReactNode
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[12px] font-medium text-[#1A1A1A]">
        {label}
        {required && <span className="text-[#DC2626] ml-0.5">*</span>}
      </span>
      {children}
      {error && (
        <span className="text-[11px] text-[#DC2626]">{error}</span>
      )}
    </label>
  )
}

function Divider() {
  return <div className="h-px bg-[#EEF0F2] -mx-7" />
}

function inputCls(hasError: boolean) {
  return [
    'w-full px-3 py-2 rounded text-[13px]',
    'border bg-white',
    hasError ? 'border-[#DC2626]' : 'border-[#EEF0F2]',
    'focus:outline-none focus:ring-2 focus:ring-[#1E3FB8] focus:ring-offset-0 focus:border-transparent',
    'disabled:bg-[#F8F9FA] disabled:text-[#666666]',
  ].join(' ')
}
