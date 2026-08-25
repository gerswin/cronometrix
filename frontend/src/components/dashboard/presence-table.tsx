'use client'
import { useState } from 'react'
import { fmtTime } from '@/lib/format/datetime'
import type { PresenceRow } from '@/types/api'

interface Props {
  rows: PresenceRow[]
}

export function PresenceTable({ rows }: Props) {
  const [tab, setTab] = useState<'inside' | 'attended'>('inside')
  const visible = tab === 'inside' ? rows.filter(r => r.status === 'inside') : rows

  const tabClass = (active: boolean) =>
    `px-3 py-1.5 text-[13px] rounded-md ${
      active ? 'bg-[#1E3FB8] text-white' : 'text-[#666666] hover:bg-[#F5F6F8]'
    }`

  return (
    <div className="bg-white rounded-lg border border-[#EEF0F2]">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-[#EEF0F2]">
        <button
          data-testid="presence-tab-inside"
          className={tabClass(tab === 'inside')}
          onClick={() => setTab('inside')}
        >
          Dentro ahora
        </button>
        <button
          data-testid="presence-tab-attended"
          className={tabClass(tab === 'attended')}
          onClick={() => setTab('attended')}
        >
          Asistieron hoy
        </button>
      </div>

      {visible.length === 0 ? (
        <p data-testid="presence-empty" className="px-4 py-8 text-center text-[13px] text-[#666666]">
          Sin registros
        </p>
      ) : (
        <table className="w-full text-[13px]">
          <thead>
            <tr className="text-left text-[11px] uppercase text-[#666666]">
              <th className="px-4 py-2 font-medium">Empleado</th>
              <th className="px-4 py-2 font-medium">Entrada</th>
              <th className="px-4 py-2 font-medium">Departamento</th>
            </tr>
          </thead>
          <tbody>
            {visible.map(r => (
              <tr key={r.employee_id} className="border-t border-[#EEF0F2]">
                <td className="px-4 py-2 text-[#1A1A1A]">{r.employee_name}</td>
                <td className="px-4 py-2 text-[#666666]">
                  {r.entry_at ? fmtTime(r.entry_at) : '—'}
                </td>
                <td className="px-4 py-2 text-[#666666]">{r.department_name}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
