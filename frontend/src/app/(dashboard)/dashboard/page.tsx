'use client'
import { useQuery } from '@tanstack/react-query'
import { format } from 'date-fns'
import { api } from '@/lib/api'
import { aggregateKPIs } from '@/lib/kpi-utils'
import { KPITile } from '@/components/dashboard/kpi-tile'
import { ActivityFeed } from '@/components/dashboard/activity-feed'
import { DeptChart } from '@/components/dashboard/dept-chart'
import { DeviceStatusSummary } from '@/components/dashboard/device-banner'
import { TopBar } from '@/components/layout/top-bar'
import type { PaginatedResponse, DailyRecord, Device } from '@/types/api'

export default function DashboardPage() {
  const today = format(new Date(), 'yyyy-MM-dd')

  const { data: recordsData } = useQuery<PaginatedResponse<DailyRecord>>({
    queryKey: ['daily-records-today', today],
    queryFn: () =>
      api.get('/daily-records', { params: { from_date: today, to_date: today, limit: 500 } })
        .then(r => r.data),
    staleTime: 60_000,
  })

  const { data: devicesData } = useQuery<PaginatedResponse<Device>>({
    queryKey: ['devices'],
    queryFn: () => api.get('/devices').then(r => r.data),
    refetchInterval: 30_000,
  })

  const records = recordsData?.items ?? []
  const devices = devicesData?.items ?? []
  const kpis = aggregateKPIs(records)
  const offlineCount = devices.filter(d => d.status === 'offline').length
  const deviceVariant = offlineCount === 0 ? 'default' : offlineCount === devices.length ? 'danger' : 'warning'

  // Anomaly count (Alertas Diurnas): records with non-empty anomalies
  const alertCount = records.filter(r => r.anomalies.length > 0).length
  const latePercent = records.length > 0 ? Math.round((kpis.late / records.length) * 100) : 0

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Dashboard" />
      <div className="p-6 space-y-6">
        {/* KPI row — D-5 */}
        <div className="grid grid-cols-4 gap-4">
          <KPITile title="Empleados Presentes" value={kpis.present} />
          <KPITile title="% Retraso Hoy" value={`${latePercent}%`} />
          <KPITile
            title="Dispositivos Activos"
            value={`${devices.length - offlineCount}/${devices.length}`}
            sub={<DeviceStatusSummary devices={devices} />}
            variant={deviceVariant}
          />
          <KPITile title="Alertas Diurnas" value={alertCount} variant={alertCount > 0 ? 'warning' : 'default'} />
        </div>

        {/* Bottom panels — D-5: left 60% activity feed, right 40% donut */}
        <div className="grid grid-cols-5 gap-4">
          <div className="col-span-3 bg-white rounded-xl border p-4 shadow-sm min-h-[320px]">
            <ActivityFeed />
          </div>
          <div className="col-span-2 bg-white rounded-xl border p-4 shadow-sm">
            <h2 className="text-sm font-semibold text-slate-700 mb-3">Distribución por Depto.</h2>
            <DeptChart records={records} />
          </div>
        </div>
      </div>
    </div>
  )
}
