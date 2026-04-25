'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { DeviceTable } from '@/components/devices/device-table'
import { CommandModal } from '@/components/devices/command-modal'
import type { PaginatedResponse, Device } from '@/types/api'

export default function DevicesPage() {
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null)
  const [commandModalOpen, setCommandModalOpen] = useState(false)

  const { data, isLoading } = useQuery<PaginatedResponse<Device>>({
    queryKey: ['devices'],
    queryFn: () => api.get('/devices').then(r => r.data),
    refetchInterval: 30_000,
  })

  const devices = data?.data ?? []
  const onlineCount = devices.filter(d => d.status === 'online').length

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Dispositivos" />
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <p className="text-sm text-slate-500">
            {isLoading ? 'Cargando…' : `${onlineCount} de ${devices.length} dispositivos en línea`}
          </p>
        </div>

        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          {isLoading ? (
            <div className="p-8 text-center text-slate-400 text-sm">Cargando dispositivos…</div>
          ) : (
            <DeviceTable
              devices={devices}
              onCommandClick={(device) => {
                setSelectedDevice(device)
                setCommandModalOpen(true)
              }}
            />
          )}
        </div>
      </div>

      <CommandModal
        open={commandModalOpen}
        device={selectedDevice}
        onClose={() => { setCommandModalOpen(false); setSelectedDevice(null) }}
      />
    </div>
  )
}
