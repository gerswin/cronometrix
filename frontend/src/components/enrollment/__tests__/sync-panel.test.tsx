import { describe, it } from 'vitest'

describe('SyncPanel (Wave 0 stubs)', () => {
  it.todo('renders one SyncRow per device_pushes entry')
  it.todo('status pill: pending/in_progress -> slate-100, success -> green-100, failed -> red-100')
  it.todo('Reintentar button rendered only on failed rows')
  it.todo('Reintentar fires POST /enrollments/:id/devices/:device_id/retry mutation')
  it.todo('empty state: "No hay dispositivos activos." when device_pushes is empty')
})
