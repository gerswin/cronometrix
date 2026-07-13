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
  username: z.string().min(1, 'Este campo es obligatorio.'),
  password: z.string().min(1, 'Este campo es obligatorio.'),
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
// Departments — POST/PATCH /api/v1/departments
// Backend canonical fields: base_salary_cents (i64), HH:MM time strings,
// lunch_mode = "fixed" | "punch", lunch_duration_min required when mode=fixed.
// Form takes salary in major-unit currency (Bs / VES) and converts × 100 on submit.
// ──────────────────────────────────────────────────────────────────────

const TIME_HHMM = /^([01]\d|2[0-3]):[0-5]\d$/

export const departmentFormSchema = z
  .object({
    name: z.string().min(1, 'Nombre es requerido').max(200, 'Máximo 200 caracteres'),
    base_salary: z
      .number({ error: 'Salario base debe ser un número' })
      .nonnegative('Salario base no puede ser negativo'),
    shift_start_time: z.string().regex(TIME_HHMM, 'Hora inválida (HH:MM, 24 h)'),
    shift_end_time: z.string().regex(TIME_HHMM, 'Hora inválida (HH:MM, 24 h)'),
    lunch_mode: z.enum(['fixed', 'punch'], { error: 'Modalidad inválida' }),
    lunch_duration_min: z
      .number({ error: 'Duración debe ser un número' })
      .int('Debe ser entero')
      .min(0, 'No puede ser negativo')
      .nullable()
      .optional(),
  })
  .refine(
    (v) => v.lunch_mode !== 'fixed' || (v.lunch_duration_min !== null && v.lunch_duration_min !== undefined && v.lunch_duration_min > 0),
    {
      message: 'Duración del almuerzo es requerida cuando la modalidad es fija',
      path: ['lunch_duration_min'],
    },
  )

export type DepartmentFormValues = z.infer<typeof departmentFormSchema>

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

// ──────────────────────────────────────────────────────────────────────
// Global Rules — PATCH /api/v1/rules
// Tolerances bounded 0–60 min by backend (validator crate).
// ──────────────────────────────────────────────────────────────────────

export const rulesFormSchema = z.object({
  late_arrival_tolerance_min: z
    .number({ error: 'Debe ser un número' })
    .int('Debe ser entero')
    .min(0, 'Mínimo 0')
    .max(60, 'Máximo 60'),
  early_departure_tolerance_min: z
    .number({ error: 'Debe ser un número' })
    .int('Debe ser entero')
    .min(0, 'Mínimo 0')
    .max(60, 'Máximo 60'),
  bonus_minutes: z
    .number({ error: 'Debe ser un número' })
    .int('Debe ser entero')
    .min(0, 'Mínimo 0')
    .max(60, 'Máximo 60'),
})

export type RulesFormValues = z.infer<typeof rulesFormSchema>

// ──────────────────────────────────────────────────────────────────────────
// Phase 7 — Facial Enrollment (07-02)
// enrollmentSubmitSchema per UI-SPEC § Form Validation Contract
// ──────────────────────────────────────────────────────────────────────────

export const enrollmentSubmitSchema = z.object({
  employee_id: z.string().uuid("Debes seleccionar un empleado válido."),
  captured_via: z.enum(['device', 'webcam', 'upload']),
  source_device_id: z.string().uuid().nullable(),
  photo: z.instanceof(Blob, { message: "Falta la foto a enrolar." }),
}).refine(
  (data) => data.captured_via !== 'device' || data.source_device_id !== null,
  { message: "Selecciona el dispositivo Hikvision usado para capturar.", path: ['source_device_id'] }
)

export type EnrollmentSubmitData = z.infer<typeof enrollmentSubmitSchema>

// ──────────────────────────────────────────────────────────────────────
// User CRUD (Plan: opcion 3 — admin /settings/users)
// ──────────────────────────────────────────────────────────────────────

export const createUserFormSchema = z.object({
  username: z
    .string()
    .min(1, 'Usuario es requerido')
    .max(100, 'Máximo 100 caracteres')
    .regex(/^[a-zA-Z0-9._-]+$/, 'Solo letras, números y . _ -'),
  full_name: z.string().min(1, 'Nombre es requerido').max(200, 'Máximo 200 caracteres'),
  role: z.enum(['admin', 'supervisor', 'viewer'], { error: 'Rol inválido' }),
  password: z.string().min(8, 'Mínimo 8 caracteres'),
})
export type CreateUserFormValues = z.infer<typeof createUserFormSchema>

export const updateUserFormSchema = z.object({
  full_name: z.string().min(1, 'Nombre es requerido').max(200, 'Máximo 200 caracteres'),
  role: z.enum(['admin', 'supervisor', 'viewer'], { error: 'Rol inválido' }),
  password: z
    .string()
    .min(8, 'Mínimo 8 caracteres')
    .or(z.literal(''))
    .optional(),
})
export type UpdateUserFormValues = z.infer<typeof updateUserFormSchema>

// ──────────────────────────────────────────────────────────────────────
// Devices — POST /api/v1/devices
// Backend validates: name (1-100), ip (1-100), port (1-65535), scheme (1-10),
// username (1-100), password (1-200), direction (1-10), allow_insecure_tls (bool).
// ──────────────────────────────────────────────────────────────────────

const IPV4_OR_HOST = /^([a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*)$/

export const createDeviceFormSchema = z.object({
  name: z.string().min(1, 'Nombre es requerido').max(100, 'Máximo 100 caracteres'),
  ip: z
    .string()
    .min(1, 'IP / hostname es requerido')
    .max(100, 'Máximo 100 caracteres')
    .regex(IPV4_OR_HOST, 'IP o hostname inválido'),
  port: z
    .number({ error: 'Puerto debe ser un número' })
    .int('Debe ser entero')
    .min(1, 'Mínimo 1')
    .max(65535, 'Máximo 65535'),
  scheme: z.enum(['http', 'https'], { error: 'Esquema inválido' }),
  username: z.string().min(1, 'Usuario es requerido').max(100, 'Máximo 100 caracteres'),
  password: z.string().min(1, 'Contraseña es requerida').max(200, 'Máximo 200 caracteres'),
  direction: z.enum(['entry', 'exit', 'both'], { error: 'Función inválida' }),
  allow_insecure_tls: z.boolean(),
})
export type CreateDeviceFormValues = z.infer<typeof createDeviceFormSchema>
