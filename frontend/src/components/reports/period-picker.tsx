'use client'
// I-8: deriveDates mirrors backend/src/reports/periods.rs::resolve_period (D-32).
// If you change the boundary math here, update the backend AND its tests,
// otherwise the picker preview will silently disagree with the report it
// requests. Canonical rules:
//   weekly         → ISO Mon..Sun (Monday = start of week, weekStartsOn: 1)
//   biweekly_first → day 1..15 of ref month
//   biweekly_second→ day 16..lastDayOfMonth of ref month
//   monthly        → day 1..lastDayOfMonth of ref month
//   custom         → passthrough (UI provides from/to inputs)
import {
  startOfWeek,
  endOfWeek,
  startOfMonth,
  endOfMonth,
  format,
  setDate,
} from 'date-fns'
import type { ReportFilters, PeriodType } from '@/types/api'

interface Props {
  value: ReportFilters
  onChange: (next: ReportFilters) => void
  refDate?: Date
}

/** Pure helper for tests. See I-8 reference comment above. */
export function deriveDates(
  periodType: PeriodType,
  ref: Date,
  half: '1' | '2' = '1',
): { from: string; to: string } {
  if (periodType === 'weekly') {
    const from = startOfWeek(ref, { weekStartsOn: 1 })
    const to = endOfWeek(ref, { weekStartsOn: 1 })
    return { from: format(from, 'yyyy-MM-dd'), to: format(to, 'yyyy-MM-dd') }
  }
  if (periodType === 'biweekly_first') {
    const from = setDate(startOfMonth(ref), 1)
    const to = setDate(startOfMonth(ref), 15)
    return { from: format(from, 'yyyy-MM-dd'), to: format(to, 'yyyy-MM-dd') }
  }
  // half is reserved for future range-style biweekly variants; ignored
  // for the canonical biweekly_first/biweekly_second values which encode
  // the half directly into the period_type.
  void half
  if (periodType === 'biweekly_second') {
    const from = setDate(startOfMonth(ref), 16)
    const to = endOfMonth(ref)
    return { from: format(from, 'yyyy-MM-dd'), to: format(to, 'yyyy-MM-dd') }
  }
  if (periodType === 'monthly') {
    const from = startOfMonth(ref)
    const to = endOfMonth(ref)
    return { from: format(from, 'yyyy-MM-dd'), to: format(to, 'yyyy-MM-dd') }
  }
  // custom: caller drives from/to via inputs; this helper returns the
  // current month as a sensible default.
  const from = startOfMonth(ref)
  const to = endOfMonth(ref)
  return { from: format(from, 'yyyy-MM-dd'), to: format(to, 'yyyy-MM-dd') }
}

export function PeriodPicker({ value, onChange, refDate }: Props) {
  const ref = refDate ?? new Date()

  const handlePeriodTypeChange = (next: PeriodType) => {
    if (next === 'custom') {
      onChange({ ...value, period_type: next })
      return
    }
    const { from, to } = deriveDates(next, ref)
    onChange({ ...value, period_type: next, from_date: from, to_date: to })
  }

  const handleHalfChange = (half: '1' | '2') => {
    const pt: PeriodType = half === '1' ? 'biweekly_first' : 'biweekly_second'
    const { from, to } = deriveDates(pt, ref)
    onChange({ ...value, period_type: pt, from_date: from, to_date: to })
  }

  const isBiweekly =
    value.period_type === 'biweekly_first' ||
    value.period_type === 'biweekly_second'

  const halfValue: '1' | '2' =
    value.period_type === 'biweekly_second' ? '2' : '1'

  return (
    <div className="flex items-end gap-2">
      <div className="flex flex-col">
        <label className="text-xs text-slate-500 mb-1">Período</label>
        <select
          value={isBiweekly ? 'biweekly' : value.period_type}
          onChange={(e) => {
            const v = e.target.value
            if (v === 'biweekly') {
              handleHalfChange('1')
              return
            }
            handlePeriodTypeChange(v as PeriodType)
          }}
          aria-label="Tipo de período"
          className="rounded-md border border-slate-200 px-3 py-2 text-sm"
        >
          <option value="weekly">Semanal</option>
          <option value="biweekly">Quincenal</option>
          <option value="monthly">Mensual</option>
          <option value="custom">Personalizado</option>
        </select>
      </div>

      {isBiweekly && (
        <div className="flex flex-col">
          <label className="text-xs text-slate-500 mb-1">Quincena</label>
          <select
            value={halfValue}
            onChange={(e) => handleHalfChange(e.target.value as '1' | '2')}
            aria-label="Quincena"
            className="rounded-md border border-slate-200 px-3 py-2 text-sm"
          >
            <option value="1">1ra quincena</option>
            <option value="2">2da quincena</option>
          </select>
        </div>
      )}

      {value.period_type === 'custom' && (
        <>
          <div className="flex flex-col">
            <label className="text-xs text-slate-500 mb-1">Desde</label>
            <input
              type="date"
              value={value.from_date}
              onChange={(e) =>
                onChange({ ...value, from_date: e.target.value })
              }
              aria-label="Fecha desde"
              className="rounded-md border border-slate-200 px-3 py-2 text-sm"
            />
          </div>
          <div className="flex flex-col">
            <label className="text-xs text-slate-500 mb-1">Hasta</label>
            <input
              type="date"
              value={value.to_date}
              onChange={(e) =>
                onChange({ ...value, to_date: e.target.value })
              }
              aria-label="Fecha hasta"
              className="rounded-md border border-slate-200 px-3 py-2 text-sm"
            />
          </div>
        </>
      )}

      {value.period_type !== 'custom' && (
        <span className="text-xs text-slate-500 self-center">
          {value.from_date} – {value.to_date}
        </span>
      )}
    </div>
  )
}
