import { Device } from '@/types/api'

export function DeviceStatusSummary({ devices }: { devices: Device[] }) {
  const active = devices.filter(d => d.status === 'active')
  const inactive = devices.length - active.length
  const disconnected = active.filter(d => d.connection_state !== 'online').length
  const offline = active.filter(d => d.connection_state === 'offline').length

  let connectivity
  if (active.length === 0) {
    connectivity = <span className="text-xs text-slate-500">Sin dispositivos activos</span>
  } else if (disconnected === 0) {
    connectivity = <span className="text-xs text-green-600">{active.length}/{active.length} en línea</span>
  } else if (offline === active.length) {
    connectivity = <span className="text-xs text-red-600 font-medium">{offline} OFFLINE</span>
  } else {
    connectivity = (
      <span className="text-xs text-yellow-600 font-medium">
        {disconnected} desconectado{disconnected > 1 ? 's' : ''}
      </span>
    )
  }

  return (
    <span className="inline-flex items-center gap-2">
      {connectivity}
      {inactive > 0 && (
        <span className="text-xs text-slate-500">
          {inactive} inactivo{inactive > 1 ? 's' : ''}
        </span>
      )}
    </span>
  )
}
