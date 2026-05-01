export function fmtMin(n: number | null | undefined): string {
  if (n === null || n === undefined || n === 0) return '—'
  const h = Math.floor(n / 60)
  const m = n % 60
  return `${h}:${m.toString().padStart(2, '0')}`
}
