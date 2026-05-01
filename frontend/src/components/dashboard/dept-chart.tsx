'use client'
import { PieChart, Pie, Cell } from 'recharts'
import { DailyRecord } from '@/types/api'

const COLORS = ['#3B82F6', '#A855F7', '#22C55E', '#F59E0B', '#EF4444', '#06B6D4', '#84CC16']

interface DeptChartProps {
  records: DailyRecord[]
  nameById?: Map<string, string>
}

export function DeptChart({ records, nameById }: DeptChartProps) {
  // Group present employees by department
  const counts: Record<string, number> = {}
  records.forEach(r => {
    if (r.work_minutes > 0) {
      counts[r.department_id] = (counts[r.department_id] ?? 0) + 1
    }
  })

  const data = Object.entries(counts).map(([deptId, value]) => ({
    deptId,
    name: nameById?.get(deptId) ?? 'Sin dpto.',
    value,
  }))

  const total = data.reduce((sum, d) => sum + d.value, 0)

  if (data.length === 0) {
    return (
      <div
        data-testid="donut-by-dept"
        className="flex items-center justify-center h-full text-[13px] text-[#666666]"
      >
        Sin datos para hoy
      </div>
    )
  }

  return (
    <div data-testid="donut-by-dept" className="flex flex-col gap-5 items-center w-full">
      {/* Fixed 140×140 donut with center total */}
      <div className="shrink-0">
        <PieChart width={140} height={140}>
          <Pie
            data={data}
            cx={70}
            cy={70}
            innerRadius={50}
            outerRadius={70}
            paddingAngle={2}
            dataKey="value"
            startAngle={90}
            endAngle={-270}
          >
            {data.map((_, i) => (
              <Cell key={i} fill={COLORS[i % COLORS.length]} stroke="none" />
            ))}
          </Pie>
          {/* Center total */}
          <text
            x={70}
            y={66}
            textAnchor="middle"
            dominantBaseline="central"
            style={{ fontFamily: 'var(--font-mono)', fontSize: 24, fontWeight: 700, fill: '#1A1A1A' }}
          >
            {total}
          </text>
          <text
            x={70}
            y={85}
            textAnchor="middle"
            dominantBaseline="central"
            style={{ fontFamily: 'var(--font-sans)', fontSize: 11, fill: '#666666' }}
          >
            Total
          </text>
        </PieChart>
      </div>

      {/* Custom vertical legend with counts. min-w-0 + truncate keeps long
          department names from colliding with the right-aligned count. */}
      <div className="w-full flex flex-col gap-1.5">
        {data.map((entry, i) => (
          <div key={entry.deptId} className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2 min-w-0 flex-1">
              <span
                className="inline-block w-2 h-2 rounded-sm shrink-0"
                style={{ backgroundColor: COLORS[i % COLORS.length] }}
              />
              <span
                className="text-[13px] text-[#1A1A1A] truncate"
                title={entry.name}
              >
                {entry.name}
              </span>
            </div>
            <span className="text-[13px] font-semibold text-[#1A1A1A] shrink-0 tabular-nums">
              {entry.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
