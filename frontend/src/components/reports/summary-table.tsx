'use client'
// Summary table — 20 columns per D-14 with synthetic per-dept subtotal
// rows (RESEARCH Pattern 7) and a final grand-total row. Anomaly rows
// (anomaly_count > 0) get amber-50 tint; subtotal/grand-total rows are
// bold and not clickable.
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
} from '@tanstack/react-table'
import { useMemo } from 'react'
import type {
  ReportPayload,
  EmployeeReportRow,
  Aggregates,
} from '@/types/api'
import { fmtMoney, fmtMoneyNegative } from '@/lib/format/currency'

type RowKind = 'data' | 'subtotal' | 'grandtotal'

export interface TableRow extends Aggregates {
  _kind: RowKind
  _key: string
  cedula: string
  nombre: string
  departamento: string
  cargo: string
  employee_id?: string
  anomaly_codes: string[]
  anomaly_count: number
}

export function buildTableRows(payload: ReportPayload): TableRow[] {
  const rows: TableRow[] = []
  for (const dept of payload.departments_in_order) {
    const deptRows = payload.rows.filter(
      (r: EmployeeReportRow) => r.dept_id === dept.id,
    )
    for (const r of deptRows) {
      rows.push({
        ...r,
        _kind: 'data',
        _key: `${dept.id}:${r.employee_id}`,
      })
    }
    const sub = payload.dept_subtotals.find((s) => s.dept_id === dept.id)
    if (sub) {
      rows.push({
        ...sub.aggregates,
        _kind: 'subtotal',
        _key: `${dept.id}:subtotal`,
        cedula: '',
        nombre: `Total ${dept.name}`,
        departamento: '',
        cargo: '',
        anomaly_codes: [],
        anomaly_count: 0,
      })
    }
  }
  rows.push({
    ...payload.grand_total,
    _kind: 'grandtotal',
    _key: 'grandtotal',
    cedula: '',
    nombre: 'Total General',
    departamento: '',
    cargo: '',
    anomaly_codes: [],
    anomaly_count: 0,
  })
  return rows
}

interface Props {
  payload?: ReportPayload
  isLoading: boolean
  onDrillDown: (employee_id: string) => void
}

export function SummaryTable({ payload, isLoading, onDrillDown }: Props) {
  const data = useMemo(
    () => (payload ? buildTableRows(payload) : []),
    [payload],
  )

  const columns: ColumnDef<TableRow>[] = useMemo(
    () => [
      { accessorKey: 'cedula', header: 'Cédula' },
      { accessorKey: 'nombre', header: 'Nombre' },
      { accessorKey: 'departamento', header: 'Departamento' },
      { accessorKey: 'cargo', header: 'Cargo' },
      {
        accessorKey: 'work_min',
        header: 'Min Trab',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'ot_min',
        header: 'Min Extra',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'late_min',
        header: 'Min Retraso',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'days_worked',
        header: 'Días Trab',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'days_absent',
        header: 'Días Aus',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'work_pay_cents',
        header: 'Pago Base',
        cell: ({ getValue }) => fmtMoney(getValue() as number),
      },
      {
        accessorKey: 'ot_pay_cents',
        header: 'Pago Extra',
        cell: ({ getValue }) => fmtMoney(getValue() as number),
      },
      {
        accessorKey: 'night_premium_cents',
        header: 'Prima Nocturna',
        cell: ({ getValue }) => fmtMoney(getValue() as number),
      },
      {
        accessorKey: 'rest_day_surcharge_cents',
        header: 'Recargo Domingo',
        cell: ({ getValue }) => fmtMoney(getValue() as number),
      },
      {
        accessorKey: 'late_deduction_cents',
        header: 'Descuento Retraso',
        cell: ({ getValue }) => fmtMoneyNegative(getValue() as number),
      },
      {
        accessorKey: 'total_a_pagar_cents',
        header: 'Total a Pagar',
        cell: ({ getValue }) => fmtMoney(getValue() as number),
      },
      {
        accessorKey: 'days_ivss',
        header: 'Días IVSS',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'days_vacation',
        header: 'Días Vacación',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'days_permission',
        header: 'Días Permiso',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        accessorKey: 'days_unpaid',
        header: 'Días No Remunerado',
        cell: ({ getValue }) => String(getValue() ?? 0),
      },
      {
        id: 'anomalies',
        header: 'Anomalías',
        cell: ({ row }) => {
          const r = row.original
          if (r._kind !== 'data') return ''
          if (r.anomaly_count === 0) return ''
          return `${r.anomaly_count} (${r.anomaly_codes.join(', ')})`
        },
      },
    ],
    [],
  )

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

  const rowClass = (kind: RowKind, anomalous: boolean) => {
    if (kind === 'grandtotal') return 'font-bold border-t-2 bg-blue-50'
    if (kind === 'subtotal') return 'font-semibold border-t bg-slate-50'
    if (anomalous) return 'bg-amber-50 hover:bg-amber-100 cursor-pointer'
    return 'hover:bg-slate-50 cursor-pointer'
  }

  return (
    <div className="overflow-auto bg-white rounded-xl border shadow-sm max-h-[calc(100vh-220px)]">
      <table className="w-full text-sm">
        <thead>
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b border-slate-200">
              {hg.headers.map((h) => (
                <th
                  key={h.id}
                  className="sticky top-0 z-10 bg-white px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide whitespace-nowrap shadow-[inset_0_-1px_0_0_rgb(226_232_240)]"
                >
                  {flexRender(h.column.columnDef.header, h.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {isLoading && (
            <tr>
              <td
                colSpan={columns.length}
                className="px-3 py-8 text-center text-slate-400 text-xs"
              >
                Generando reporte…
              </td>
            </tr>
          )}
          {!isLoading && data.length === 0 && (
            <tr>
              <td
                colSpan={columns.length}
                className="px-3 py-8 text-center text-slate-400 text-xs"
              >
                Sin datos. Configure los filtros y haga clic en &quot;Emitir Reporte&quot;.
              </td>
            </tr>
          )}
          {!isLoading &&
            table.getRowModel().rows.map((row) => {
              const r = row.original
              const clickable = r._kind === 'data'
              return (
                <tr
                  key={r._key}
                  className={`border-b border-slate-100 ${rowClass(
                    r._kind,
                    r.anomaly_count > 0,
                  )}`}
                  onClick={() => {
                    if (clickable && r.employee_id) {
                      onDrillDown(r.employee_id)
                    }
                  }}
                  data-row-kind={r._kind}
                >
                  {row.getVisibleCells().map((cell) => (
                    <td
                      key={cell.id}
                      className="px-3 py-2 text-slate-700 whitespace-nowrap"
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </td>
                  ))}
                </tr>
              )
            })}
        </tbody>
      </table>
    </div>
  )
}
