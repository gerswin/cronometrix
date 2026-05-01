'use client'
import type { Department } from '@/types/api'

interface Props {
  lateMin: number
  earlyMin: number
  bonusMin: number
  department: Department | null
}

/** Parse "HH:MM" → minutes since midnight */
function parseHHMM(hhmm: string): number {
  const [h, m] = hhmm.split(':').map(Number)
  return h * 60 + m
}

/** Format minutes since midnight → "HH:MM" zero-padded */
function fmtHHMM(totalMin: number): string {
  const clamped = Math.max(0, Math.round(totalMin))
  const h = Math.floor(clamped / 60)
  const m = clamped % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

/** Format total minutes as "Xh Ym" */
function fmtDuration(totalMin: number): string {
  const clamped = Math.max(0, Math.round(totalMin))
  const h = Math.floor(clamped / 60)
  const m = clamped % 60
  if (h === 0) return `${m}m`
  if (m === 0) return `${h}h`
  return `${h}h ${m}m`
}

interface TimelineRow {
  time: string
  label: string
  color: string
}

interface Props {
  lateMin: number
  earlyMin: number
  bonusMin: number
  department: Department | null
}

export function ToleranceSimulator({ lateMin, earlyMin, bonusMin, department }: Props) {
  if (!department) {
    return (
      <div className="flex items-center justify-center h-24 text-[13px] text-[#666666]">
        Configure al menos un departamento para ver el simulador.
      </div>
    )
  }

  const shiftStartMin = parseHHMM(department.shift_start_time)
  const shiftEndMin = parseHHMM(department.shift_end_time)
  const lunchMin = department.lunch_duration_min ?? 0
  const shiftDuration = shiftEndMin - shiftStartMin

  // Place lunch midway through the shift
  const lunchStartMin = shiftStartMin + Math.floor((shiftDuration - lunchMin) / 2)
  const lunchEndMin = lunchStartMin + lunchMin

  const idealEffectiveMin = shiftDuration - lunchMin

  // Applied tolerances after burning through the bonus bag
  const lateApplied = Math.max(0, lateMin - bonusMin)
  const earlyApplied = Math.max(0, earlyMin - bonusMin)
  const tolEffectiveMin = idealEffectiveMin - lateApplied - earlyApplied

  // Ideal timeline rows
  const idealRows: TimelineRow[] = [
    { time: fmtHHMM(shiftStartMin), label: 'Entrada', color: '#22C55E' },
    { time: fmtHHMM(lunchStartMin), label: 'Almuerzo', color: '#F59E0B' },
    { time: fmtHHMM(lunchEndMin), label: 'Regreso', color: '#F59E0B' },
    { time: fmtHHMM(shiftEndMin), label: 'Salida', color: '#1E3FB8' },
  ]

  // Tolerance timeline — actual clock times are adjusted by the applied tolerances
  const tolRows: TimelineRow[] = [
    { time: fmtHHMM(shiftStartMin + lateApplied), label: 'Entrada', color: '#22C55E' },
    { time: fmtHHMM(lunchStartMin + lateApplied), label: 'Almuerzo', color: '#F59E0B' },
    { time: fmtHHMM(lunchEndMin + lateApplied), label: 'Regreso', color: '#F59E0B' },
    { time: fmtHHMM(shiftEndMin - earlyApplied), label: 'Salida', color: '#1E3FB8' },
  ]

  // Map label → tolerance minutes offset (for the red marker)
  const tolApplied: Record<string, number> = {
    Entrada: lateApplied,
    Salida: earlyApplied,
  }

  return (
    <div className="flex gap-8">
      {/* ── Left: Ideal day ─────────────────────────────────────────────── */}
      <div className="flex-1 flex flex-col gap-3">
        <span
          className="text-[13px] text-[#666666] tracking-[0.5px]"
          style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
        >
          Jornada Ideal
        </span>

        <div className="flex flex-col gap-2 flex-1">
          {idealRows.map((row) => (
            <div key={row.label + '-ideal'} className="flex items-center gap-3">
              <span
                className="text-[13px] text-[#1A1A1A] shrink-0"
                style={{ fontFamily: 'var(--font-mono)', width: 52 }}
              >
                {row.time}
              </span>
              <div
                className="flex-1 flex items-center px-3 rounded"
                style={{
                  backgroundColor: row.color,
                  height: 24,
                }}
              >
                <span className="text-white text-[12px] font-medium leading-none">
                  {row.label}
                </span>
              </div>
            </div>
          ))}
        </div>

        <span className="text-[13px] font-semibold text-[#1A1A1A]">
          Total: {fmtDuration(idealEffectiveMin)} efectivos
        </span>
      </div>

      {/* ── Right: Day with tolerances ───────────────────────────────────── */}
      <div className="flex-1 flex flex-col gap-3">
        <span
          className="text-[13px] text-[#666666] tracking-[0.5px]"
          style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
        >
          Jornada con Tolerancias
        </span>

        <div className="flex flex-col gap-2 flex-1">
          {tolRows.map((row) => {
            const appliedMin = tolApplied[row.label] ?? 0
            return (
              <div key={row.label + '-tol'} className="flex items-center gap-3">
                <span
                  className="text-[13px] text-[#1A1A1A] shrink-0"
                  style={{ fontFamily: 'var(--font-mono)', width: 52 }}
                >
                  {row.time}
                </span>
                <div className="flex-1 relative flex items-center" style={{ height: 24 }}>
                  {/* Main bar */}
                  <div
                    className="absolute inset-0 flex items-center px-3 rounded"
                    style={{ backgroundColor: row.color }}
                  >
                    <span className="text-white text-[12px] font-medium leading-none">
                      {row.label}
                    </span>
                  </div>
                  {/* Tolerance marker — red leading edge overlay */}
                  {appliedMin > 0 && (
                    <>
                      <div
                        className="absolute left-0 top-0 bottom-0 rounded-l flex items-center justify-center"
                        style={{
                          backgroundColor: '#EF4444',
                          width: Math.min(48, appliedMin * 2),
                        }}
                      />
                      <span
                        className="absolute text-[10px] font-semibold text-[#EF4444] leading-none"
                        style={{ left: -(appliedMin * 2 + 2), whiteSpace: 'nowrap' }}
                      >
                        +{appliedMin}m
                      </span>
                    </>
                  )}
                </div>
              </div>
            )
          })}
        </div>

        <span className="text-[13px] font-semibold" style={{ color: '#EF4444' }}>
          Total: {fmtDuration(Math.max(0, tolEffectiveMin))} efectivos
        </span>
      </div>
    </div>
  )
}
