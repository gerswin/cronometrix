/**
 * Extra branch coverage for src/lib/validations.ts. Existing
 * src/__tests__/novedad-modal.test.tsx covers a subset of novedadSchema;
 * this fills the cross-schema branches.
 */
import { describe, it, expect } from 'vitest'
import {
  setupSchema,
  loginSchema,
  evidenceFileSchema,
  novedadSchema,
  tenantInfoSchema,
  licenseSchema,
  enrollmentSubmitSchema,
} from '../validations'

describe('validations', () => {
  describe('setupSchema', () => {
    it('rejects mismatched password and confirm_password', () => {
      const result = setupSchema.safeParse({
        full_name: 'A', username: 'u',
        password: 'longenough1', confirm_password: 'somethingelse',
      })
      expect(result.success).toBe(false)
    })
    it('accepts matching password and confirm_password', () => {
      const result = setupSchema.safeParse({
        full_name: 'A', username: 'u',
        password: 'longenough1', confirm_password: 'longenough1',
      })
      expect(result.success).toBe(true)
    })
    it('rejects too-short password', () => {
      const result = setupSchema.safeParse({
        full_name: 'A', username: 'u',
        password: 'short', confirm_password: 'short',
      })
      expect(result.success).toBe(false)
    })
  })

  describe('loginSchema', () => {
    it('rejects empty username', () => {
      expect(loginSchema.safeParse({ username: '', password: 'x' }).success).toBe(false)
    })
    it('accepts non-empty values', () => {
      expect(loginSchema.safeParse({ username: 'u', password: 'p' }).success).toBe(true)
    })
  })

  describe('evidenceFileSchema', () => {
    it('accepts a small PDF', () => {
      const file = new File([new Uint8Array(100)], 'a.pdf', { type: 'application/pdf' })
      expect(evidenceFileSchema.safeParse(file).success).toBe(true)
    })
    it('accepts JPEG and PNG', () => {
      expect(evidenceFileSchema.safeParse(new File([new Uint8Array(100)], 'a.jpg', { type: 'image/jpeg' })).success).toBe(true)
      expect(evidenceFileSchema.safeParse(new File([new Uint8Array(100)], 'a.png', { type: 'image/png' })).success).toBe(true)
    })
    it('rejects non-allowed mime types', () => {
      const file = new File([new Uint8Array(100)], 'a.txt', { type: 'text/plain' })
      expect(evidenceFileSchema.safeParse(file).success).toBe(false)
    })
    it('rejects files > 5MB', () => {
      const big = new File([new Uint8Array(6 * 1024 * 1024)], 'a.pdf', { type: 'application/pdf' })
      expect(evidenceFileSchema.safeParse(big).success).toBe(false)
    })
  })

  describe('novedadSchema cross-field', () => {
    it('rejects fecha_fin before fecha_inicio', () => {
      const result = novedadSchema.safeParse({
        employee_id: 'e', department_id: 'd',
        fecha_inicio: '2026-05-10', fecha_fin: '2026-05-01',
        tipo_novedad: 'manual', justification: 'X',
      })
      expect(result.success).toBe(false)
      if (!result.success) {
        const fields = result.error.issues.map((i) => String(i.path[0]))
        expect(fields).toContain('fecha_fin')
      }
    })
    it('accepts fecha_fin equal to fecha_inicio', () => {
      const result = novedadSchema.safeParse({
        employee_id: 'e', department_id: 'd',
        fecha_inicio: '2026-05-10', fecha_fin: '2026-05-10',
        tipo_novedad: 'manual', justification: 'X',
      })
      expect(result.success).toBe(true)
    })
  })

  describe('tenantInfoSchema RIF', () => {
    it('accepts empty RIF (allowed)', () => {
      expect(tenantInfoSchema.safeParse({ client_name: '', client_rif: '', address: '', version: 1 }).success).toBe(true)
    })
    it('accepts a valid RIF', () => {
      expect(tenantInfoSchema.safeParse({ client_name: 'Acme', client_rif: 'J-12345678-9', address: '', version: 1 }).success).toBe(true)
    })
    it('rejects malformed RIF', () => {
      expect(tenantInfoSchema.safeParse({ client_name: 'Acme', client_rif: 'XYZ-1', address: '', version: 1 }).success).toBe(false)
    })
    it('rejects an over-long client_name', () => {
      const long = 'a'.repeat(201)
      expect(tenantInfoSchema.safeParse({ client_name: long, client_rif: '', address: '', version: 1 }).success).toBe(false)
    })
    it('rejects over-long address', () => {
      const long = 'a'.repeat(501)
      expect(tenantInfoSchema.safeParse({ client_name: '', client_rif: '', address: long, version: 1 }).success).toBe(false)
    })
  })

  describe('licenseSchema', () => {
    it('accepts a valid 4-4-4-4 license key', () => {
      expect(licenseSchema.safeParse({ license_key: 'AB12-CD34-EF56-GH78' }).success).toBe(true)
    })
    it('accepts lowercase via case-insensitive regex', () => {
      expect(licenseSchema.safeParse({ license_key: 'ab12-cd34-ef56-gh78' }).success).toBe(true)
    })
    it('rejects malformed key', () => {
      expect(licenseSchema.safeParse({ license_key: 'short-key' }).success).toBe(false)
    })
    it('rejects empty key', () => {
      expect(licenseSchema.safeParse({ license_key: '' }).success).toBe(false)
    })
  })

  describe('enrollmentSubmitSchema cross-field', () => {
    it('accepts captured_via=upload without source_device_id', () => {
      const photo = new Blob(['x'], { type: 'image/jpeg' })
      const result = enrollmentSubmitSchema.safeParse({
        employee_id: '11111111-1111-4111-8111-111111111111',
        captured_via: 'upload', source_device_id: null, photo,
      })
      expect(result.success).toBe(true)
    })
    it('rejects captured_via=device with null source_device_id', () => {
      const photo = new Blob(['x'], { type: 'image/jpeg' })
      const result = enrollmentSubmitSchema.safeParse({
        employee_id: '11111111-1111-4111-8111-111111111111',
        captured_via: 'device', source_device_id: null, photo,
      })
      expect(result.success).toBe(false)
    })
    it('accepts captured_via=device with a UUID source_device_id', () => {
      const photo = new Blob(['x'], { type: 'image/jpeg' })
      const result = enrollmentSubmitSchema.safeParse({
        employee_id: '11111111-1111-4111-8111-111111111111',
        captured_via: 'device', source_device_id: '22222222-2222-4222-8222-222222222222', photo,
      })
      expect(result.success).toBe(true)
    })
    it('rejects non-UUID employee_id', () => {
      const photo = new Blob(['x'], { type: 'image/jpeg' })
      const result = enrollmentSubmitSchema.safeParse({
        employee_id: 'not-uuid', captured_via: 'upload', source_device_id: null, photo,
      })
      expect(result.success).toBe(false)
    })
  })
})
