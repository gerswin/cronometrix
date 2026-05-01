'use client'
import { SearchableSelect, type SearchableOption } from '@/components/ui/searchable-select'

export type AnomaliesFilterState = {
  code?: string
  employee_id?: string
  from_date?: string
  to_date?: string
}

const ANOMALY_CODES = [
  'LATE',
  'EARLY_DEP',
  'NO_ENTRY',
  'NO_EXIT',
  'OVERTIME',
  'REST_DAY_WORKED',
]

interface Props {
  value: AnomaliesFilterState
  onChange: (next: AnomaliesFilterState) => void
  employees: SearchableOption[]
}

export function AnomaliesFilters({ value, onChange, employees }: Props) {
  return (
    <div className="flex items-center gap-3 flex-wrap">
      <select
        data-testid="anomalies-filter-code"
        value={value.code ?? ''}
        onChange={(e) =>
          onChange({ ...value, code: e.target.value || undefined })
        }
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      >
        <option value="">Código</option>
        {ANOMALY_CODES.map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
      </select>

      <div className="w-72">
        <SearchableSelect
          data-testid="anomalies-filter-employee"
          value={value.employee_id ?? null}
          onChange={(id) =>
            onChange({ ...value, employee_id: id || undefined })
          }
          options={employees}
          placeholder="Empleado"
        />
      </div>

      <input
        type="date"
        data-testid="anomalies-filter-from"
        value={value.from_date ?? ''}
        onChange={(e) =>
          onChange({ ...value, from_date: e.target.value || undefined })
        }
        title="Desde"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />
      <input
        type="date"
        data-testid="anomalies-filter-to"
        value={value.to_date ?? ''}
        onChange={(e) =>
          onChange({ ...value, to_date: e.target.value || undefined })
        }
        title="Hasta"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />

      <button
        type="button"
        onClick={() => onChange({})}
        className="text-xs px-3 py-2 rounded border border-slate-200 hover:bg-slate-50"
      >
        Limpiar
      </button>
    </div>
  )
}
