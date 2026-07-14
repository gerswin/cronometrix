'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useQuery } from '@tanstack/react-query'
import { Plus, RefreshCw, LogOut } from 'lucide-react'
import { api, logoutCurrentSession } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { DeviceCard } from '@/components/devices/device-card'
import { CommandModal } from '@/components/devices/command-modal'
import { CreateDeviceModal } from '@/components/devices/create-device-modal'
import { PrimaryButton } from '@/components/ui/primary-button'
import type { PaginatedResponse, Device } from '@/types/api'

const DEVICE_PAGE_LIMIT = 100

async function fetchDevicesByStatus(status: Device['status']): Promise<Device[]> {
  const devices: Device[] = []
  let offset = 0
  let total: number | undefined

  while (total === undefined || offset < total) {
    const page = await api
      .get('/devices', { params: { status, limit: DEVICE_PAGE_LIMIT, offset } })
      .then(r => r.data as PaginatedResponse<Device>)

    total = page.total
    if (page.data.length === 0) break

    devices.push(...page.data)
    offset += page.data.length
  }

  return devices
}

async function fetchAllDevices(): Promise<PaginatedResponse<Device>> {
  const [active, inactive] = await Promise.all([
    fetchDevicesByStatus('active'),
    fetchDevicesByStatus('inactive'),
  ])
  const data = [...active, ...inactive]

  return {
    data,
    total: data.length,
    limit: data.length,
    offset: 0,
  }
}

export default function DevicesPage() {
  const router = useRouter()
  const { role } = useAuth()
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null)
  const [commandModalOpen, setCommandModalOpen] = useState(false)
  const [createModalOpen, setCreateModalOpen] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  const { data, isLoading } = useQuery<PaginatedResponse<Device>>({
    queryKey: ['devices'],
    queryFn: fetchAllDevices,
    refetchInterval: 30_000,
  })

  const devices = data?.data ?? []
  const activeCount = devices.filter(d => d.status === 'active').length
  const inactiveCount = devices.length - activeCount
  const onlineCount = devices.filter(
    d => d.status === 'active' && d.connection_state === 'online',
  ).length
  const canEdit = role === 'admin'

  // ── Logout ───────────────────────────────────────────────────────────────

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await logoutCurrentSession()
    } finally {
      router.push('/login')
    }
  }

  // ── Add device ───────────────────────────────────────────────────────────

  function handleAddDevice() {
    setCreateModalOpen(true)
  }

  return (
    <div className="flex flex-col h-full">
      {/* ── Header bar ──────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-6 py-4">
        {/* Left: breadcrumb + title */}
        <div className="flex flex-col gap-1">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Inicio / Dispositivos
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Administración de Dispositivos
          </h1>
        </div>

        {/* Right: sync-all + add device + logout */}
        <div className="flex items-center gap-3">
          {/* Sincronización Total — disabled, coming soon */}
          <button
            type="button"
            disabled
            title="Próximamente"
            data-testid="sync-all-button"
            className="inline-flex items-center gap-1.5 rounded border border-[#EEF0F2] bg-white px-4 py-2 text-[13px] font-medium text-[#1A1A1A] opacity-60 cursor-not-allowed"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <RefreshCw size={16} aria-hidden="true" />
            Sincronización Total
          </button>

          {/* Agregar Dispositivo — admin only */}
          {canEdit && (
            <PrimaryButton
              type="button"
              size="sm"
              icon={Plus}
              onClick={handleAddDevice}
              data-testid="add-device-button"
            >
              Agregar Dispositivo
            </PrimaryButton>
          )}

          {/* Logout */}
          <button
            type="button"
            onClick={handleLogout}
            disabled={isLoggingOut}
            aria-label="Cerrar sesión"
            data-testid="logout-button"
            className="inline-flex items-center gap-1.5 text-xs text-[#666666] hover:text-[#1A1A1A] px-2.5 py-1.5 rounded-md border border-[#EEF0F2] hover:bg-slate-50 disabled:opacity-50 transition-colors"
          >
            <LogOut size={14} aria-hidden="true" />
            {isLoggingOut ? 'Saliendo…' : 'Salir'}
          </button>
        </div>
      </header>

      {/* ── Main content ────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto px-8 py-6 bg-[#F8F9FA]">
        <div className="flex flex-col gap-6">

          {/* Status summary line */}
          <p
            className="text-[13px] text-[#666666]"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            {isLoading
              ? 'Cargando…'
              : `${onlineCount} de ${activeCount} dispositivos activos en línea${
                inactiveCount > 0
                  ? ` · ${inactiveCount} inactivo${inactiveCount > 1 ? 's' : ''}`
                  : ''
              }`}
          </p>

          {/* Device card grid */}
          {isLoading ? (
            <div className="text-[13px] text-[#666666] text-center py-12">
              Cargando dispositivos…
            </div>
          ) : devices.length === 0 ? (
            /* Empty state */
            <div className="flex flex-col items-center gap-4 py-16 text-center">
              <p className="text-[14px] text-[#666666]" style={{ fontFamily: 'var(--font-sans)' }}>
                No hay dispositivos registrados
              </p>
              {canEdit && (
                <PrimaryButton
                  type="button"
                  size="sm"
                  icon={Plus}
                  onClick={handleAddDevice}
                >
                  Agregar Dispositivo
                </PrimaryButton>
              )}
            </div>
          ) : (
            <div className="grid gap-5 md:grid-cols-1 lg:grid-cols-2">
              {devices.map(device => (
                <DeviceCard
                  key={device.id}
                  device={device}
                  canEdit={canEdit}
                  onCommandClick={(d) => {
                    setSelectedDevice(d)
                    setCommandModalOpen(true)
                  }}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {/* CommandModal — keep wiring intact */}
      <CommandModal
        open={commandModalOpen}
        device={selectedDevice}
        onClose={() => {
          setCommandModalOpen(false)
          setSelectedDevice(null)
        }}
      />

      {/* CreateDeviceModal — wired to /devices POST */}
      <CreateDeviceModal
        open={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
      />
    </div>
  )
}
