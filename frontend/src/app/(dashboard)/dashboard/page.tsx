'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useQuery } from '@tanstack/react-query'
import { format } from 'date-fns'
import { Bell, Settings, LogOut } from 'lucide-react'
import { toast } from 'sonner'
import { api, setAccessToken } from '@/lib/api'
import { aggregateKPIs } from '@/lib/kpi-utils'
import { useAuth } from '@/hooks/use-auth'
import { KPITile } from '@/components/dashboard/kpi-tile'
import { ActivityFeed } from '@/components/dashboard/activity-feed'
import { DeptChart } from '@/components/dashboard/dept-chart'
import type { PaginatedResponse, DailyRecord, Device, Department, RawAttendanceEvent } from '@/types/api'

const DEVICE_PAGE_LIMIT = 100

// ── Caracas UTC-4 (no DST) epoch helpers ────────────────────────────────────
function getTodayCaracasEpochs(): { from: number; to: number } {
  const today = format(new Date(), 'yyyy-MM-dd') // date-fns uses local TZ (set to America/Caracas)
  const from = Date.parse(`${today}T00:00:00-04:00`) / 1000
  const to   = Date.parse(`${today}T23:59:59-04:00`) / 1000
  return { from, to }
}

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

export default function DashboardPage() {
  const today = format(new Date(), 'yyyy-MM-dd')
  const { from: epochFrom, to: epochTo } = getTodayCaracasEpochs()
  const router = useRouter()
  const { role } = useAuth()
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  // ── Data queries ─────────────────────────────────────────────────────────

  const { data: recordsData } = useQuery<PaginatedResponse<DailyRecord>>({
    queryKey: ['daily-records-today', today],
    queryFn: () =>
      api.get('/daily-records', { params: { from_date: today, to_date: today, limit: 500 } })
        .then(r => r.data),
    staleTime: 60_000,
  })

  const { data: devicesData } = useQuery<PaginatedResponse<Device>>({
    queryKey: ['devices'],
    queryFn: fetchAllDevices,
    refetchInterval: 30_000,
  })

  // KPI 1 denominator: total active employees
  const { data: employeesTotalData } = useQuery<PaginatedResponse<unknown>>({
    queryKey: ['employees-total-active'],
    queryFn: () =>
      api.get('/employees', { params: { status: 'active', limit: 1 } }).then(r => r.data),
    staleTime: 5 * 60_000,
  })

  // Department names for donut legend
  const { data: departmentsData } = useQuery<PaginatedResponse<Department>>({
    queryKey: ['departments'],
    queryFn: () => api.get('/departments', { params: { limit: 1000 } }).then(r => r.data),
    staleTime: 10 * 60_000,
  })

  // KPI 4: unknown events today
  const { data: unknownEventsData } = useQuery<PaginatedResponse<RawAttendanceEvent>>({
    queryKey: ['raw-events', 'unknown-today', epochFrom, epochTo],
    queryFn: () =>
      api.get('/events', {
        params: { from: epochFrom, to: epochTo, include_unknown: true, limit: 500 },
      }).then(r => r.data),
    refetchInterval: 60_000,
  })

  // ── Derived values ───────────────────────────────────────────────────────

  const records = recordsData?.data ?? []
  const devices = devicesData?.data ?? []
  const kpis = aggregateKPIs(records)
  const latePercent = records.length > 0 ? Math.round((kpis.late / records.length) * 100) : 0

  const totalActiveEmployees = employeesTotalData?.total
  const presentSub = totalActiveEmployees != null
    ? `de ${totalActiveEmployees} registrados`
    : 'de — registrados'

  const activeDevices = devices.filter(d => d.status === 'active')
  const inactiveCount = devices.length - activeDevices.length
  const onlineCount = activeDevices.filter(d => d.connection_state === 'online').length
  const activeProblemCount = activeDevices.length - onlineCount
  const deviceValueColor =
    activeDevices.length === 0     ? '#1A1A1A' :
    onlineCount === activeDevices.length ? '#22C55E' :
    onlineCount === 0             ? '#EF4444' :
    '#F59E0B'
  const deviceSub =
    devices.length === 0 ? 'sin dispositivos' :
    activeDevices.length === 0 ? `${inactiveCount} inactivo${inactiveCount > 1 ? 's' : ''}` :
    activeProblemCount === 0 && inactiveCount === 0 ? 'todos operativos' :
    [
      activeProblemCount > 0
        ? `${activeProblemCount} activo${activeProblemCount > 1 ? 's' : ''} con problemas`
        : null,
      inactiveCount > 0
        ? `${inactiveCount} inactivo${inactiveCount > 1 ? 's' : ''}`
        : null,
    ].filter(Boolean).join(' · ')

  const unknownCount = unknownEventsData?.data.filter(e => e.is_unknown).length ?? 0

  // Department name lookup map for donut
  const nameById = new Map<string, string>(
    (departmentsData?.data ?? []).map(d => [d.id, d.name])
  )

  // ── Logout ───────────────────────────────────────────────────────────────

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await api.post('/auth/logout').catch(() => undefined)
    } finally {
      setAccessToken(null)
      router.push('/login')
    }
  }

  function handleSettings() {
    if (role === 'admin') {
      router.push('/settings/tenant-info')
    } else {
      toast.info('Solo administradores')
    }
  }

  function handleBell() {
    toast.info('Notificaciones próximamente')
  }

  // ── Render ───────────────────────────────────────────────────────────────

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
            Inicio / Dashboard
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Centro de Mando
          </h1>
        </div>

        {/* Right: bell + settings + logout */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleBell}
            aria-label="Notificaciones"
            className="text-[#666666] hover:text-[#1A1A1A] transition-colors"
          >
            <Bell size={20} />
          </button>
          <button
            type="button"
            onClick={handleSettings}
            aria-label="Configuración"
            className="text-[#666666] hover:text-[#1A1A1A] transition-colors"
          >
            <Settings size={20} />
          </button>
          <button
            type="button"
            onClick={handleLogout}
            disabled={isLoggingOut}
            aria-label="Cerrar sesión"
            data-testid="logout-button"
            className="inline-flex items-center gap-1.5 text-xs text-[#666666] hover:text-[#1A1A1A] px-2.5 py-1.5 rounded-md border border-[#EEF0F2] hover:bg-slate-50 disabled:opacity-50 transition-colors"
          >
            <LogOut size={14} />
            {isLoggingOut ? 'Saliendo…' : 'Salir'}
          </button>
        </div>
      </header>

      {/* ── Main content ────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto p-6 flex flex-col gap-6">

        {/* Row 1: KPI grid */}
        <div className="grid grid-cols-4 gap-4">
          {/* KPI 1 — Empleados Presentes */}
          <KPITile
            testId="kpi-empleados-presentes"
            title="Empleados Presentes"
            value={kpis.present}
            valueColor="#1A1A1A"
            sub={presentSub}
          />

          {/* KPI 2 — % Retraso Hoy */}
          <KPITile
            testId="kpi-retraso-hoy"
            title="% Retraso Hoy"
            value={`${latePercent}%`}
            valueColor="#F59E0B"
            sub={`${kpis.late} empleados con retraso`}
          />

          {/* KPI 3 — Dispositivos Activos */}
          <KPITile
            testId="kpi-dispositivos-activos"
            title="Dispositivos Activos"
            value={`${onlineCount}/${activeDevices.length}`}
            valueColor={deviceValueColor}
            sub={deviceSub}
          />

          {/* KPI 4 — Alertas Diurnas / Huérfanos
              Keep both testids and the "Alertas Diurnas" label because e2e
              asserts SEL.kpiAlertas = 'kpi-alertas-diurnas' and the text
              "Alertas Diurnas" (dashboard.spec.ts T-01). */}
          <div data-testid="kpi-alertas-huerfanos">
            <KPITile
              testId="kpi-alertas-diurnas"
              title="Alertas Diurnas"
              value={unknownCount}
              valueColor={unknownCount > 0 ? '#EF4444' : '#1A1A1A'}
              sub="marcajes sin empleado"
            />
          </div>
        </div>

        {/* Row 2: Activity + Donut */}
        <div className="flex gap-6 flex-1 min-h-[360px]">
          {/* Activity card (flex-1) */}
          <div
            className="flex-1 bg-white rounded border border-[#EEF0F2] overflow-hidden flex flex-col"
            style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
          >
            <ActivityFeed />
          </div>

          {/* Donut card (fixed 320px) */}
          <div
            className="w-[320px] shrink-0 bg-white rounded border border-[#EEF0F2] flex flex-col"
            style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
          >
            <div className="flex items-center px-4 py-[14px] border-b border-[#EEF0F2]">
              <span className="text-[15px] font-semibold text-[#1A1A1A]">
                Distribución por Depto.
              </span>
            </div>
            <div className="flex-1 flex items-start justify-center p-5 overflow-auto">
              <DeptChart records={records} nameById={nameById} />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
