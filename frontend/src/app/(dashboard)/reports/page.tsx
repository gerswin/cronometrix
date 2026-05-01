'use client'
import { useState, useMemo } from 'react'
import { useRouter } from 'next/navigation'
import { useQuery, useMutation } from '@tanstack/react-query'
import {
  format,
  startOfMonth,
  endOfMonth,
  startOfWeek,
  endOfWeek,
} from 'date-fns'
import { es } from 'date-fns/locale'
import { toast } from 'sonner'
import { FileSpreadsheet, FileText, FileDown, LogOut, Loader2 } from 'lucide-react'
import { api, setAccessToken } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { renderReportPdf } from '@/lib/reports/pdf'
import { renderReportCsv } from '@/lib/reports/csv'
import { fmtMoney } from '@/lib/format/currency'
import { fmtMin } from '@/lib/format/duration'
import type {
  ReportPayload,
  ReportFilters,
  PeriodType,
} from '@/types/api'

// ── Helpers ─────────────────────────────────────────────────────────────────

type TabId = 'biweekly' | 'weekly' | 'monthly'

interface PeriodRange {
  from_date: string
  to_date: string
  period_type: PeriodType
}

function computePeriod(tab: TabId, ref: Date = new Date()): PeriodRange {
  if (tab === 'monthly') {
    return {
      period_type: 'monthly',
      from_date: format(startOfMonth(ref), 'yyyy-MM-dd'),
      to_date: format(endOfMonth(ref), 'yyyy-MM-dd'),
    }
  }
  if (tab === 'weekly') {
    return {
      period_type: 'weekly',
      from_date: format(startOfWeek(ref, { weekStartsOn: 1 }), 'yyyy-MM-dd'),
      to_date: format(endOfWeek(ref, { weekStartsOn: 1 }), 'yyyy-MM-dd'),
    }
  }
  // biweekly — first half (1–15) or second half (16–end)
  const day = ref.getDate()
  if (day <= 15) {
    return {
      period_type: 'biweekly_first',
      from_date: format(startOfMonth(ref), 'yyyy-MM-dd'),
      to_date: format(new Date(ref.getFullYear(), ref.getMonth(), 15), 'yyyy-MM-dd'),
    }
  }
  return {
    period_type: 'biweekly_second',
    from_date: format(new Date(ref.getFullYear(), ref.getMonth(), 16), 'yyyy-MM-dd'),
    to_date: format(endOfMonth(ref), 'yyyy-MM-dd'),
  }
}

function formatPeriodLabel(from: string, to: string): string {
  // "1 — 15 Abril 2026" style — derive from ISO dates.
  try {
    const fromD = new Date(from + 'T00:00:00')
    const toD = new Date(to + 'T00:00:00')
    const sameMonth =
      fromD.getMonth() === toD.getMonth() && fromD.getFullYear() === toD.getFullYear()
    if (sameMonth) {
      const m = format(fromD, 'MMMM yyyy', { locale: es })
      return `Período: ${fromD.getDate()} — ${toD.getDate()} ${m.charAt(0).toUpperCase() + m.slice(1)}`
    }
    return `Período: ${format(fromD, 'd MMM yyyy', { locale: es })} — ${format(toD, 'd MMM yyyy', { locale: es })}`
  } catch {
    return `Período: ${from} — ${to}`
  }
}

function initialsFor(name: string): string {
  return name
    .split(' ')
    .filter(Boolean)
    .map((p) => p[0])
    .slice(0, 2)
    .join('')
    .toUpperCase()
}

// Deterministic palette so the same employee always gets the same avatar tint.
const AVATAR_PALETTE = [
  '#5588DD',
  '#A855F7',
  '#22C55E',
  '#F59E0B',
  '#EF4444',
  '#06B6D4',
  '#84CC16',
  '#C4D9E8',
]
function avatarColor(seed: string): string {
  let h = 0
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0
  return AVATAR_PALETTE[h % AVATAR_PALETTE.length]
}

// ── Page ────────────────────────────────────────────────────────────────────

export default function ReportsPage() {
  const router = useRouter()
  const { role } = useAuth()
  const canRead = role === 'admin' || role === 'supervisor'
  const [isLoggingOut, setIsLoggingOut] = useState(false)
  const [tab, setTab] = useState<TabId>('biweekly')

  const period = useMemo(() => computePeriod(tab), [tab])
  const filters: ReportFilters = useMemo(
    () => ({
      ...period,
      include_inactive: false,
    }),
    [period],
  )

  // Auto-fetch when period changes (per design — no manual button).
  const reportQ = useQuery<ReportPayload>({
    queryKey: ['reports', filters],
    queryFn: () =>
      api.post<ReportPayload>('/reports/json', filters).then((r) => r.data),
    enabled: canRead,
    staleTime: 60_000,
  })

  // ── Logout ────────────────────────────────────────────────────────────────

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

  // ── Exports ───────────────────────────────────────────────────────────────

  const exportExcel = useMutation({
    mutationFn: async () => {
      const resp = await api.post('/reports/excel', filters, {
        responseType: 'blob',
      })
      const blob = new Blob([resp.data as BlobPart], {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `prenomina_${filters.from_date}_${filters.to_date}.xlsx`
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
    },
    onSuccess: () => toast.success('Excel descargado'),
    onError: () => toast.error('Error al generar Excel'),
  })

  const exportPdf = useMutation({
    mutationFn: async () => {
      const resp = await api.post<ReportPayload>('/reports/json', filters)
      renderReportPdf(resp.data)
    },
    onSuccess: () => toast.success('PDF generado'),
    onError: () => toast.error('Error al generar PDF'),
  })

  const exportCsv = useMutation({
    mutationFn: async () => {
      // CSV se renderiza cliente desde el payload ya cargado para evitar
      // un round-trip extra. Si no hay payload, dispara una carga primero.
      const payload =
        reportQ.data ??
        (await api.post<ReportPayload>('/reports/json', filters)).data
      renderReportCsv(payload)
    },
    onSuccess: () => toast.success('CSV descargado'),
    onError: () => toast.error('Error al generar CSV'),
  })

  // ── Derived (KPIs + table data) ───────────────────────────────────────────

  const payload = reportQ.data
  const totalEmpleados = payload?.rows.length ?? 0
  const horasExtra = payload ? payload.grand_total.ot_min / 60 : 0
  const recargosCents = payload?.grand_total.rest_day_surcharge_cents ?? 0
  const descuentosCents = payload?.grand_total.late_deduction_cents ?? 0
  const totalAPagarCents = payload?.grand_total.total_a_pagar_cents ?? 0

  const periodLabel = formatPeriodLabel(filters.from_date, filters.to_date)

  if (!canRead) {
    return (
      <div className="p-8 text-[14px] text-[#666666]">
        Acceso restringido.
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full bg-[#F8F9FA]">
      {/* ── Header ─────────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-8 py-3">
        <div className="flex flex-col gap-0.5">
          <span
            className="text-[11px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Cronometrix / Reportes
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Reportes y Pre-Nómina
          </h1>
        </div>
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
      </header>

      {/* ── Body ───────────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto px-8 py-6 flex flex-col gap-5">
        {/* Top bar: tabs + date label + export buttons */}
        <div className="flex items-center justify-between gap-4 flex-wrap">
          {/* Period tabs */}
          <div
            className="inline-flex border border-[#EEF0F2] rounded overflow-hidden"
            data-testid="period-tabs"
            role="tablist"
          >
            {(
              [
                { id: 'biweekly', label: 'Quincenal' },
                { id: 'weekly', label: 'Semanal' },
                { id: 'monthly', label: 'Mensual' },
              ] as const
            ).map((t) => (
              <button
                key={t.id}
                type="button"
                role="tab"
                aria-selected={tab === t.id}
                onClick={() => setTab(t.id)}
                data-testid={`period-tab-${t.id}`}
                className={[
                  'px-4 py-2 text-[13px] font-medium transition-colors',
                  tab === t.id
                    ? 'bg-[#1E3FB8] text-white'
                    : 'bg-white text-[#666666] hover:bg-slate-50',
                ].join(' ')}
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                {t.label}
              </button>
            ))}
          </div>

          {/* Date label */}
          <span
            className="text-[13px] text-[#1A1A1A]"
            style={{ fontFamily: 'var(--font-mono)' }}
            data-testid="period-label"
          >
            {periodLabel}
          </span>

          {/* Export buttons */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => exportExcel.mutate()}
              disabled={exportExcel.isPending || !payload}
              data-testid="export-excel"
              className="inline-flex items-center gap-1.5 rounded px-3.5 py-1.5 text-[12px] font-medium text-white bg-[#16A34A] hover:bg-[#15803D] transition-colors disabled:opacity-50"
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              <FileSpreadsheet size={15} aria-hidden="true" />
              {exportExcel.isPending ? 'Generando…' : 'Excel'}
            </button>
            <button
              type="button"
              onClick={() => exportPdf.mutate()}
              disabled={exportPdf.isPending || !payload}
              data-testid="export-pdf"
              className="inline-flex items-center gap-1.5 rounded px-3.5 py-1.5 text-[12px] font-medium text-white bg-[#DC2626] hover:bg-[#B91C1C] transition-colors disabled:opacity-50"
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              <FileText size={15} aria-hidden="true" />
              {exportPdf.isPending ? 'Generando…' : 'PDF'}
            </button>
            <button
              type="button"
              onClick={() => exportCsv.mutate()}
              disabled={exportCsv.isPending || !payload}
              data-testid="export-csv"
              className="inline-flex items-center gap-1.5 rounded px-3.5 py-1.5 text-[12px] font-medium text-white bg-[#0D1A5C] hover:bg-[#0A1346] transition-colors disabled:opacity-50"
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              <FileDown size={15} aria-hidden="true" />
              {exportCsv.isPending ? 'Generando…' : 'CSV'}
            </button>
          </div>
        </div>

        {/* Hero KPI: total a pagar — la pregunta de "cuánto debo pagar" */}
        <div
          className="rounded border border-[#16A34A]/20 bg-gradient-to-r from-[#F0FDF4] to-white px-6 py-5 flex items-center justify-between gap-6"
          style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
          data-testid="hero-total-a-pagar"
        >
          <div className="flex flex-col gap-1">
            <span
              className="text-[12px] text-[#15803D] tracking-widest font-semibold"
              style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
            >
              TOTAL A PAGAR — {periodLabel.replace(/^Período:\s*/, '')}
            </span>
            <span
              className="text-[40px] font-bold leading-none text-[#15803D]"
              style={{ fontFamily: 'var(--font-mono)' }}
            >
              {reportQ.isLoading ? '—' : fmtMoney(totalAPagarCents)}
            </span>
            <span className="text-[12px] text-[#666666]">
              {totalEmpleados} empleados · sueldo + recargos − descuentos
            </span>
          </div>
          <div className="hidden md:flex flex-col items-end text-[11px] text-[#666666] gap-0.5">
            <span>Exporta el detalle en Excel / PDF / CSV</span>
            <span>arriba ↑</span>
          </div>
        </div>

        {/* KPI row — componentes del cálculo */}
        <div className="grid grid-cols-4 gap-4">
          <KpiCard
            label="TOTAL EMPLEADOS"
            value={String(totalEmpleados)}
            color="#1A1A1A"
            sub="en período activo"
            isLoading={reportQ.isLoading}
          />
          <KpiCard
            label="HORAS EXTRA"
            value={horasExtra.toFixed(1)}
            color="#1E3FB8"
            sub="horas acumuladas"
            isLoading={reportQ.isLoading}
          />
          <KpiCard
            label="RECARGOS FESTIVOS"
            value={fmtMoney(recargosCents)}
            color="#F59E0B"
            sub="(+) suma al neto"
            isLoading={reportQ.isLoading}
          />
          <KpiCard
            label="DESCUENTOS"
            value={fmtMoney(descuentosCents)}
            color="#EF4444"
            sub="(−) resta al neto"
            isLoading={reportQ.isLoading}
          />
        </div>

        {/* Settlement table */}
        <section
          className="bg-white rounded border border-[#EEF0F2] overflow-hidden flex flex-col"
          style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
          data-testid="settlement-table"
        >
          {/* Table header (title + period) */}
          <div className="flex items-center justify-between px-5 py-3 border-b border-[#EEF0F2]">
            <span className="text-[16px] font-bold text-[#1A1A1A]">
              Resumen de Liquidación
            </span>
            <span
              className="text-[11px] text-[#666666]"
              style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
            >
              {periodLabel}
            </span>
          </div>

          {/* Column headers */}
          <div
            className="flex items-center bg-[#F3F4F6] border-b border-[#EEF0F2] px-5 py-2"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <div className="flex-1 text-[10px] font-semibold text-[#666666] tracking-wider">
              EMPLEADO
            </div>
            <div className="w-[80px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              HORAS
            </div>
            <div className="w-[80px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              HRS EXTRA
            </div>
            <div className="w-[100px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              REC. FESTIVO
            </div>
            <div className="w-[95px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              DESCUENTOS
            </div>
            <div className="w-[100px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              NETO
            </div>
          </div>

          {/* Rows */}
          <div className="flex-1 overflow-auto">
            {reportQ.isLoading && (
              <div className="flex items-center gap-2 px-5 py-12 text-[13px] text-[#666666]">
                <Loader2 size={14} className="animate-spin" />
                Generando reporte…
              </div>
            )}
            {!reportQ.isLoading && reportQ.error && (
              <div className="px-5 py-12 text-[13px] text-red-600">
                Error al generar el reporte. Intenta de nuevo.
              </div>
            )}
            {!reportQ.isLoading && payload && payload.rows.length === 0 && (
              <div className="px-5 py-12 text-center text-[13px] text-[#666666]">
                Sin datos para el período seleccionado.
              </div>
            )}
            {!reportQ.isLoading &&
              payload?.rows.map((row, i) => (
                <div
                  key={row.employee_id}
                  className="flex items-center px-5 py-2.5 border-b border-[#EEF0F2]"
                  style={{
                    backgroundColor: i % 2 === 1 ? '#FAFBFC' : '#FFFFFF',
                  }}
                  data-testid={`settlement-row-${row.employee_id}`}
                >
                  {/* Employee */}
                  <div className="flex-1 flex items-center gap-2.5 min-w-0">
                    <span
                      className="w-[26px] h-[26px] rounded-full text-white text-[10px] font-semibold flex items-center justify-center shrink-0"
                      style={{ backgroundColor: avatarColor(row.employee_id) }}
                      aria-hidden="true"
                    >
                      {initialsFor(row.nombre)}
                    </span>
                    <div className="flex flex-col min-w-0">
                      <span className="text-[13px] font-medium text-[#1A1A1A] truncate">
                        {row.nombre}
                      </span>
                      <span className="text-[11px] text-[#666666] truncate">
                        {row.departamento || '—'}
                      </span>
                    </div>
                  </div>
                  {/* Horas trabajadas (HH:MM) */}
                  <div
                    className="w-[80px] text-right text-[12px] text-[#1A1A1A]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                    title={`${row.work_min.toLocaleString('en-US')} min`}
                  >
                    {fmtMin(row.work_min)}
                  </div>
                  {/* Hrs extra */}
                  <div
                    className="w-[80px] text-right text-[12px]"
                    style={{
                      fontFamily: 'var(--font-mono)',
                      color: row.ot_min > 0 ? '#1E3FB8' : '#666666',
                    }}
                  >
                    {(row.ot_min / 60).toFixed(1)}
                  </div>
                  {/* Rec festivo */}
                  <div
                    className="w-[100px] text-right text-[12px]"
                    style={{
                      fontFamily: 'var(--font-mono)',
                      color:
                        row.rest_day_surcharge_cents > 0 ? '#F59E0B' : '#666666',
                    }}
                  >
                    {row.rest_day_surcharge_cents > 0
                      ? fmtMoney(row.rest_day_surcharge_cents)
                      : '$0'}
                  </div>
                  {/* Descuentos */}
                  <div
                    className="w-[95px] text-right text-[12px]"
                    style={{
                      fontFamily: 'var(--font-mono)',
                      color:
                        row.late_deduction_cents > 0 ? '#EF4444' : '#666666',
                    }}
                  >
                    {row.late_deduction_cents > 0
                      ? fmtMoney(row.late_deduction_cents)
                      : '$0'}
                  </div>
                  {/* Neto */}
                  <div
                    className="w-[100px] text-right text-[12px] font-semibold text-[#1A1A1A]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {fmtMoney(row.total_a_pagar_cents)}
                  </div>
                </div>
              ))}
          </div>

          {/* Footer totals */}
          {payload && payload.rows.length > 0 && (
            <div
              className="flex items-center px-5 py-3 bg-[#0D1A5C]"
              data-testid="settlement-totals"
            >
              <div className="flex-1 text-[12px] font-bold text-white">
                TOTALES ({payload.rows.length} empleados)
              </div>
              <div
                className="w-[80px] text-right text-[12px] font-bold text-white"
                style={{ fontFamily: 'var(--font-mono)' }}
                title={`${payload.grand_total.work_min.toLocaleString('en-US')} min`}
              >
                {fmtMin(payload.grand_total.work_min)}
              </div>
              <div
                className="w-[80px] text-right text-[12px] font-bold"
                style={{ fontFamily: 'var(--font-mono)', color: '#5588DD' }}
              >
                {(payload.grand_total.ot_min / 60).toFixed(1)}
              </div>
              <div
                className="w-[100px] text-right text-[12px] font-bold"
                style={{ fontFamily: 'var(--font-mono)', color: '#F59E0B' }}
              >
                {fmtMoney(payload.grand_total.rest_day_surcharge_cents)}
              </div>
              <div
                className="w-[95px] text-right text-[12px] font-bold"
                style={{ fontFamily: 'var(--font-mono)', color: '#EF4444' }}
              >
                {fmtMoney(payload.grand_total.late_deduction_cents)}
              </div>
              <div
                className="w-[100px] text-right text-[12px] font-bold text-white"
                style={{ fontFamily: 'var(--font-mono)' }}
              >
                {fmtMoney(payload.grand_total.total_a_pagar_cents)}
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  )
}

// ── KPI card ────────────────────────────────────────────────────────────────

interface KpiCardProps {
  label: string
  value: string
  color: string
  sub: string
  isLoading?: boolean
}

function KpiCard({ label, value, color, sub, isLoading }: KpiCardProps) {
  return (
    <div
      className="bg-white rounded border border-[#EEF0F2] px-5 py-4 flex flex-col gap-1"
      style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
    >
      <span
        className="text-[11px] text-[#666666] tracking-widest"
        style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
      >
        {label}
      </span>
      <span
        className="text-[28px] font-bold leading-none"
        style={{ fontFamily: 'var(--font-mono)', color }}
      >
        {isLoading ? '—' : value}
      </span>
      <span
        className="text-[11px] text-[#666666]"
        style={{ fontFamily: 'var(--font-sans)' }}
      >
        {sub}
      </span>
    </div>
  )
}
