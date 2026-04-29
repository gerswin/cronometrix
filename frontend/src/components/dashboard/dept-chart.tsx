'use client'
import { PieChart, Pie, Cell, Tooltip, Legend, ResponsiveContainer } from 'recharts'
import { DailyRecord } from '@/types/api'

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#8b5cf6', '#ef4444', '#06b6d4', '#84cc16']

interface DeptChartProps { records: DailyRecord[] }

export function DeptChart({ records }: DeptChartProps) {
  // Group records by department_id, count present employees
  const counts: Record<string, number> = {}
  records.forEach(r => {
    if (r.work_minutes > 0) {
      counts[r.department_id] = (counts[r.department_id] ?? 0) + 1
    }
  })
  const data = Object.entries(counts).map(([name, value]) => ({ name, value }))

  if (data.length === 0) {
    return (
      <div data-testid="donut-by-dept" className="flex items-center justify-center h-full text-slate-400 text-sm">
        Sin datos para hoy
      </div>
    )
  }

  return (
    <div data-testid="donut-by-dept">
      <ResponsiveContainer width="100%" height={220}>
        <PieChart>
          <Pie
            data={data}
            cx="50%"
            cy="50%"
            innerRadius={60}
            outerRadius={90}
            paddingAngle={2}
            dataKey="value"
          >
            {data.map((_, i) => (
              <Cell key={i} fill={COLORS[i % COLORS.length]} />
            ))}
          </Pie>
          <Tooltip formatter={(val) => [`${typeof val === 'number' ? val : 0} presentes`, '']} />
          <Legend />
        </PieChart>
      </ResponsiveContainer>
    </div>
  )
}
