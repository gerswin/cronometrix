'use client'
import type { ReportFilters, DeptSummary } from '@/types/api'

export interface EmployeeOption {
  id: string
  label: string
}

interface Props {
  value: ReportFilters
  onChange: (next: ReportFilters) => void
  departments: DeptSummary[]
  employees?: EmployeeOption[]
}

export function FiltersBar({ value, onChange, departments, employees }: Props) {
  const selectedDepts = value.department_ids ?? []

  const toggleDept = (id: string) => {
    const next = selectedDepts.includes(id)
      ? selectedDepts.filter((x) => x !== id)
      : [...selectedDepts, id]
    onChange({ ...value, department_ids: next.length ? next : undefined })
  }

  return (
    <div className="flex items-end gap-3 flex-wrap">
      {/* Department multi-select */}
      <div className="flex flex-col">
        <label className="text-xs text-slate-500 mb-1">Departamentos</label>
        <details className="rounded-md border border-slate-200 px-3 py-2 text-sm w-52 cursor-pointer">
          <summary className="list-none">
            {selectedDepts.length === 0
              ? 'Todos'
              : `${selectedDepts.length} seleccionado(s)`}
          </summary>
          <div className="mt-2 max-h-48 overflow-y-auto space-y-1">
            {departments.map((d) => (
              <label
                key={d.id}
                className="flex items-center gap-2 text-xs cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={selectedDepts.includes(d.id)}
                  onChange={() => toggleDept(d.id)}
                  aria-label={`Departamento ${d.name}`}
                />
                {d.name}
              </label>
            ))}
          </div>
        </details>
      </div>

      {/* Include inactive toggle */}
      <label className="flex items-center gap-2 text-sm pb-2">
        <input
          type="checkbox"
          checked={value.include_inactive ?? false}
          onChange={(e) =>
            onChange({ ...value, include_inactive: e.target.checked })
          }
          aria-label="Incluir empleados inactivos"
        />
        <span className="text-slate-700">Incluir inactivos</span>
      </label>

      {/* Employee picker (optional, for personal pay-slip) */}
      {employees && employees.length > 0 && (
        <div className="flex flex-col">
          <label className="text-xs text-slate-500 mb-1">Empleado</label>
          <select
            value={value.employee_id ?? ''}
            onChange={(e) =>
              onChange({
                ...value,
                employee_id: e.target.value === '' ? undefined : e.target.value,
              })
            }
            aria-label="Filtrar por empleado"
            className="rounded-md border border-slate-200 px-3 py-2 text-sm"
          >
            <option value="">Todos</option>
            {employees.map((e) => (
              <option key={e.id} value={e.id}>
                {e.label}
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Shift type select */}
      <div className="flex flex-col">
        <label className="text-xs text-slate-500 mb-1">Turno</label>
        <select
          value={value.shift_type ?? ''}
          onChange={(e) =>
            onChange({
              ...value,
              shift_type:
                e.target.value === ''
                  ? undefined
                  : (e.target.value as 'day' | 'night' | 'mixed'),
            })
          }
          aria-label="Tipo de turno"
          className="rounded-md border border-slate-200 px-3 py-2 text-sm"
        >
          <option value="">Todos</option>
          <option value="day">Día</option>
          <option value="night">Noche</option>
          <option value="mixed">Mixto</option>
        </select>
      </div>
    </div>
  )
}
