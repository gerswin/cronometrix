'use client'
import { SearchableSelect, type SearchableOption } from '@/components/ui/searchable-select'

export type EventsFilterState = {
  employee_id?: string
  device_id?: string
  from?: number  // epoch seconds
  to?: number    // epoch seconds
  include_unknown?: boolean
}

function dateTimeToEpoch(s: string): number | undefined {
  if (!s) return undefined
  const d = new Date(s)
  if (isNaN(d.getTime())) return undefined
  return Math.floor(d.getTime() / 1000)
}

function epochToDateTime(epoch: number | undefined): string {
  if (epoch === undefined) return ''
  const d = new Date(epoch * 1000)
  if (isNaN(d.getTime())) return ''
  // YYYY-MM-DDTHH:MM in local time (datetime-local input format)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

interface Props {
  value: EventsFilterState
  onChange: (next: EventsFilterState) => void
  employees: SearchableOption[]
  devices: SearchableOption[]
}

export function EventsFilters({ value, onChange, employees, devices }: Props) {
  return (
    <div className="flex items-center gap-3 flex-wrap">
      <div className="w-72">
        <SearchableSelect
          data-testid="events-filter-employee"
          value={value.employee_id ?? null}
          onChange={(id) =>
            onChange({ ...value, employee_id: id || undefined })
          }
          options={employees}
          placeholder="Empleado"
        />
      </div>

      <div className="w-60">
        <SearchableSelect
          data-testid="events-filter-device"
          value={value.device_id ?? null}
          onChange={(id) =>
            onChange({ ...value, device_id: id || undefined })
          }
          options={devices}
          placeholder="Dispositivo"
        />
      </div>

      <input
        type="datetime-local"
        data-testid="events-filter-from"
        value={epochToDateTime(value.from)}
        onChange={(e) =>
          onChange({ ...value, from: dateTimeToEpoch(e.target.value) })
        }
        title="Desde"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />
      <input
        type="datetime-local"
        data-testid="events-filter-to"
        value={epochToDateTime(value.to)}
        onChange={(e) =>
          onChange({ ...value, to: dateTimeToEpoch(e.target.value) })
        }
        title="Hasta"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />

      <label className="inline-flex items-center gap-2 text-sm text-slate-700">
        <input
          type="checkbox"
          data-testid="events-filter-unknown"
          checked={value.include_unknown ?? false}
          onChange={(e) =>
            onChange({
              ...value,
              include_unknown: e.target.checked || undefined,
            })
          }
        />
        Incluir desconocidos
      </label>

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
