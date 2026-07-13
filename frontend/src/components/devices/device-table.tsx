'use client'
import { format } from 'date-fns'
import { Terminal } from 'lucide-react'
import { useAuth } from '@/hooks/use-auth'
import type { Device } from '@/types/api'

function StatusBadge({
  state,
  deviceId,
}: {
  state: Device['connection_state']
  deviceId: string
}) {
  const map = {
    online: 'bg-green-100 text-green-700',
    offline: 'bg-red-100 text-red-700',
    unknown: 'bg-slate-100 text-slate-600',
  }
  const labels = { online: 'En línea', offline: 'Offline', unknown: 'Desconocido' }
  return (
    <span
      className={`px-2 py-0.5 rounded-full text-xs font-medium ${map[state]}`}
      data-testid={`dev-status-${deviceId}`}
    >
      {labels[state]}
    </span>
  )
}

function LifecycleBadge({ status, deviceId }: { status: Device['status']; deviceId: string }) {
  return (
    <span
      className={
        status === 'active'
          ? 'px-2 py-0.5 rounded-full text-xs font-medium bg-blue-50 text-blue-700'
          : 'px-2 py-0.5 rounded-full text-xs font-medium bg-slate-100 text-slate-600'
      }
      data-testid={`dev-lifecycle-${deviceId}`}
    >
      {status === 'active' ? 'Activo' : 'Inactivo'}
    </span>
  )
}

interface DeviceTableProps {
  devices: Device[]
  onCommandClick: (device: Device) => void
}

export function DeviceTable({ devices, onCommandClick }: DeviceTableProps) {
  const { role } = useAuth()

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-slate-200">
          {['Nombre', 'IP', 'Dirección', 'Estado', 'Última conexión', 'Acciones'].map(h => (
            <th key={h} className="px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {devices.map(device => (
          <tr
            key={device.id}
            className="border-b border-slate-100 hover:bg-slate-50"
            data-testid={`dev-row-${device.id}`}
          >
            <td className="px-3 py-3 font-medium text-slate-800">{device.name}</td>
            <td className="px-3 py-3 text-slate-600 font-mono text-xs">{device.ip}</td>
            <td className="px-3 py-3 text-slate-600 capitalize">
              {device.direction === 'entry' ? 'Entrada' : 'Salida'}
            </td>
            <td className="px-3 py-3">
              <div className="flex items-center gap-2">
                <StatusBadge state={device.connection_state} deviceId={device.id} />
                <LifecycleBadge status={device.status} deviceId={device.id} />
              </div>
            </td>
            <td className="px-3 py-3 text-xs text-slate-500">
              {device.last_seen_at
                ? format(new Date(device.last_seen_at), 'dd/MM/yyyy HH:mm')
                : '—'}
            </td>
            <td className="px-3 py-3">
              {/* D-14: ISAPI command buttons — Admin only, hidden for Supervisor/Viewer */}
              {role === 'admin' && device.status === 'active' && (
                <button
                  onClick={() => onCommandClick(device)}
                  className="flex items-center gap-1.5 px-3 py-1 text-xs rounded border border-slate-200 hover:bg-slate-50 text-slate-600"
                  aria-label={`Enviar comando a ${device.name}`}
                  data-testid={`dev-actions-${device.id}`}
                >
                  <Terminal size={12} />
                  Comando
                </button>
              )}
            </td>
          </tr>
        ))}
        {devices.length === 0 && (
          <tr>
            <td colSpan={6} className="px-3 py-8 text-center text-slate-400 text-xs">
              Sin dispositivos registrados
            </td>
          </tr>
        )}
      </tbody>
    </table>
  )
}
