'use client'
import { SyncRow } from './sync-row'
import type { EnrollmentDevicePush } from '@/types/api'

interface SyncPanelProps {
  device_pushes: EnrollmentDevicePush[]
  enrollmentId: string
}

export function SyncPanel({ device_pushes, enrollmentId }: SyncPanelProps) {
  return (
    <div className="space-y-1">
      <p className="text-sm font-semibold text-slate-700">Sincronización a Dispositivos</p>
      <p className="text-xs text-slate-500">Estado por dispositivo</p>

      {device_pushes.length === 0 ? (
        <p className="text-xs text-slate-400 py-2">
          No hay dispositivos activos. Registra al menos uno antes de enrolar.
        </p>
      ) : (
        <div className="mt-2">
          {device_pushes.map(push => (
            <SyncRow key={push.id} push={push} enrollmentId={enrollmentId} />
          ))}
        </div>
      )}
    </div>
  )
}
