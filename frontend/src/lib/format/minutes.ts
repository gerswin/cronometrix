/** Formatea minutos como "4h 30m", "1h", "45m". Nunca negativo. */
export function fmtMinutes(min: number): string {
  const total = Math.max(0, Math.round(min))
  const h = Math.floor(total / 60)
  const m = total % 60
  if (h === 0) return `${m}m`
  if (m === 0) return `${h}h`
  return `${h}h ${m}m`
}
