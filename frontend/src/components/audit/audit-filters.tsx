'use client'

export type FilterState = {
  actor_id?: string
  table_name?: string
  from_ts?: number
  to_ts?: number
  operation?: 'INSERT' | 'UPDATE' | 'DELETE' | ''
  record_id?: string
}

interface AuditFiltersProps {
  value: FilterState
  onChange: (next: FilterState) => void
  actors: Array<{ id: string; username: string }>
  tables: string[]
}

/** Convert an HTML date-input value (YYYY-MM-DD) to epoch seconds (start/end of day). */
function dateToEpoch(dateStr: string, endOfDay = false): number | undefined {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateStr)
  if (!match) return undefined
  const year = Number(match[1])
  const month = Number(match[2]) - 1
  const day = Number(match[3])
  const d = endOfDay
    ? new Date(year, month, day, 23, 59, 59, 999)
    : new Date(year, month, day, 0, 0, 0, 0)
  if (
    d.getFullYear() !== year ||
    d.getMonth() !== month ||
    d.getDate() !== day
  ) return undefined
  return Math.floor(d.getTime() / 1000)
}

/** Convert epoch seconds back to YYYY-MM-DD for the date input value. */
function epochToDate(epoch: number | undefined): string {
  if (epoch === undefined) return ''
  const d = new Date(epoch * 1000)
  if (isNaN(d.getTime())) return ''
  const year = d.getFullYear()
  const month = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

/**
 * AuditFilters — filter form for the audit log list.
 *
 * Renders 6 inputs with data-testid contract:
 *   data-testid="audit-filter-actor"
 *   data-testid="audit-filter-table"
 *   data-testid="audit-filter-from"
 *   data-testid="audit-filter-to"
 *   data-testid="audit-filter-operation"
 *   data-testid="audit-filter-record-id"
 */
export function AuditFilters({ value, onChange, actors, tables }: AuditFiltersProps) {
  return (
    <div className="flex items-center gap-3 flex-wrap">
      {/* Actor dropdown */}
      <select
        data-testid="audit-filter-actor"
        value={value.actor_id ?? ''}
        onChange={e =>
          onChange({ ...value, actor_id: e.target.value || undefined })
        }
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      >
        <option value="">Actor</option>
        {actors.map(a => (
          <option key={a.id} value={a.id}>
            {a.username}
          </option>
        ))}
      </select>

      {/* Table dropdown */}
      <select
        data-testid="audit-filter-table"
        value={value.table_name ?? ''}
        onChange={e =>
          onChange({ ...value, table_name: e.target.value || undefined })
        }
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      >
        <option value="">Tabla</option>
        {tables.map(t => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </select>

      {/* From date */}
      <input
        type="date"
        data-testid="audit-filter-from"
        value={epochToDate(value.from_ts)}
        onChange={e =>
          onChange({ ...value, from_ts: dateToEpoch(e.target.value, false) })
        }
        title="Desde"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />

      {/* To date */}
      <input
        type="date"
        data-testid="audit-filter-to"
        value={epochToDate(value.to_ts)}
        onChange={e =>
          onChange({ ...value, to_ts: dateToEpoch(e.target.value, true) })
        }
        title="Hasta"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      />

      {/* Operation dropdown */}
      <select
        data-testid="audit-filter-operation"
        value={value.operation ?? ''}
        onChange={e =>
          onChange({
            ...value,
            operation: (e.target.value as FilterState['operation']) || undefined,
          })
        }
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
      >
        <option value="">Operación</option>
        <option value="INSERT">INSERT</option>
        <option value="UPDATE">UPDATE</option>
        <option value="DELETE">DELETE</option>
      </select>

      {/* Record ID text input */}
      <input
        type="text"
        data-testid="audit-filter-record-id"
        value={value.record_id ?? ''}
        onChange={e =>
          onChange({ ...value, record_id: e.target.value || undefined })
        }
        placeholder="ID Registro"
        className="rounded-md border border-slate-200 px-3 py-2 text-sm w-48"
      />
    </div>
  )
}
