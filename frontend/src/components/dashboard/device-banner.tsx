import { Device } from '@/types/api'

export function DeviceStatusSummary({ devices }: { devices: Device[] }) {
  const offline = devices.filter(d => d.status === 'offline').length
  const total = devices.length
  if (offline === 0) return <span className="text-xs text-green-600">{total}/{total} en línea</span>
  if (offline === total) return <span className="text-xs text-red-600 font-medium">{offline} OFFLINE</span>
  return <span className="text-xs text-yellow-600 font-medium">{offline} desconectado{offline > 1 ? 's' : ''}</span>
}
