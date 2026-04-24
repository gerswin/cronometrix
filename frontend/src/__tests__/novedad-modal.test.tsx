import { describe, it, expect } from 'vitest'
import { novedadSchema } from '../lib/validations'

describe('novedadSchema', () => {
  it('rejects empty justification', () => {
    const result = novedadSchema.safeParse({
      employee_id: 'e1',
      department_id: 'd1',
      fecha_inicio: '2026-01-01',
      fecha_fin: '2026-01-01',
      tipo_novedad: 'manual',
      justification: '',
    })
    expect(result.success).toBe(false)
    if (!result.success) {
      const fields = result.error.issues.map(i => i.path[0])
      expect(fields).toContain('justification')
    }
  })

  it('rejects missing justification', () => {
    const result = novedadSchema.safeParse({
      employee_id: 'e1',
      department_id: 'd1',
      fecha_inicio: '2026-01-01',
      fecha_fin: '2026-01-01',
      tipo_novedad: 'manual',
    })
    expect(result.success).toBe(false)
  })

  it('accepts valid novedad with justification', () => {
    const result = novedadSchema.safeParse({
      employee_id: 'e1',
      department_id: 'd1',
      fecha_inicio: '2026-01-01',
      fecha_fin: '2026-01-01',
      tipo_novedad: 'medical',
      justification: 'Reposo médico',
    })
    expect(result.success).toBe(true)
  })

  it('rejects invalid tipo_novedad', () => {
    const result = novedadSchema.safeParse({
      employee_id: 'e1',
      department_id: 'd1',
      fecha_inicio: '2026-01-01',
      fecha_fin: '2026-01-01',
      tipo_novedad: 'invalid_type',
      justification: 'algo',
    })
    expect(result.success).toBe(false)
  })

  it('accepts all valid tipo_novedad values', () => {
    const types = ['medical', 'vacation', 'unpaid', 'manual'] as const
    for (const tipo of types) {
      const result = novedadSchema.safeParse({
        employee_id: 'e1',
        department_id: 'd1',
        fecha_inicio: '2026-01-01',
        fecha_fin: '2026-01-01',
        tipo_novedad: tipo,
        justification: 'Motivo',
      })
      expect(result.success).toBe(true)
    }
  })
})
