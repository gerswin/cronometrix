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
