import { z } from 'zod'

export const setupSchema = z
  .object({
    full_name: z.string().min(1, 'This field is required.'),
    username: z.string().min(1, 'This field is required.'),
    password: z.string().min(8, 'Password must be at least 8 characters.'),
    confirm_password: z.string().min(1, 'This field is required.'),
  })
  .refine((data) => data.password === data.confirm_password, {
    message: 'Passwords do not match.',
    path: ['confirm_password'],
  })

export type SetupFormData = z.infer<typeof setupSchema>

export const loginSchema = z.object({
  username: z.string().min(1, 'This field is required.'),
  password: z.string().min(1, 'This field is required.'),
})

export type LoginFormData = z.infer<typeof loginSchema>

export const evidenceFileSchema = z
  .instanceof(File)
  .refine(f => f.size <= 5 * 1024 * 1024, 'Máximo 5MB')
  .refine(
    f => ['application/pdf', 'image/jpeg', 'image/png'].includes(f.type),
    'Solo PDF, JPG o PNG'
  )

export const novedadSchema = z
  .object({
    employee_id: z.string().min(1, 'Requerido'),
    department_id: z.string().min(1, 'Requerido'),
    fecha_inicio: z.string().min(1, 'Requerido'),
    fecha_fin: z.string().min(1, 'Requerido'),
    tipo_novedad: z.enum(['medical', 'vacation', 'unpaid', 'manual']),
    justification: z.string().min(1, 'La justificación es requerida'),
    motivo: z.string().optional(),
    evidence: evidenceFileSchema.optional(),
    impacto_nomina: z.string().optional(),
    notificar_supervisor: z.boolean().optional(),
  })
  // WR-06: ensure fecha_fin is on or after fecha_inicio. Lexicographic
  // comparison is correct because both are ISO `YYYY-MM-DD` strings.
  .refine(
    (data) => !data.fecha_inicio || !data.fecha_fin || data.fecha_fin >= data.fecha_inicio,
    {
      message: 'La fecha fin no puede ser anterior a la fecha inicio',
      path: ['fecha_fin'],
    },
  )

export type NovedadFormData = z.infer<typeof novedadSchema>

// ──────────────────────────────────────────────────────────────────────
// Phase 5 — Tenant info / Datos de Empresa form
// D-30 single-row tenant_info table; PATCH requires version for optimistic
// concurrency. RIF format `^[VJG]-\d+-\d$` (loose match per D-30 minimal scope).
// ──────────────────────────────────────────────────────────────────────

export const tenantInfoSchema = z.object({
  client_name: z.string().max(200, 'Máximo 200 caracteres'),
  client_rif: z
    .string()
    .max(50, 'Máximo 50 caracteres')
    .refine((v) => v === '' || /^[VJG]-\d+-\d$/.test(v), {
      message: 'RIF inválido (formato: J-12345678-9)',
    }),
  address: z.string().max(500, 'Máximo 500 caracteres'),
  version: z.number(),
})

export type TenantInfoFormValues = z.infer<typeof tenantInfoSchema>

// ──────────────────────────────────────────────────────────────────────
// Phase 6 — License activation (UI-SPEC §Form Validation Contract)
// ──────────────────────────────────────────────────────────────────────

export const licenseSchema = z.object({
  license_key: z
    .string()
    .min(1, 'License key is required.')
    .regex(
      /^[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}$/i,
      'License key must be in XXXX-XXXX-XXXX-XXXX format.',
    ),
})

export type LicenseFormData = z.infer<typeof licenseSchema>
