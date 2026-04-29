/**
 * Caracas-anchored Date helpers for deterministic E2E time-calc assertions.
 *
 * All test fixtures anchor to fixed dates so attendance calculations are
 * deterministic across all CI runs. America/Caracas = UTC-4, no DST since
 * 2016, so the UTC offset is constant.
 */

/** The canonical Monday used as anchor for all fixture events. */
export const SEED_DATE_ISO = '2026-04-15' // Wednesday; deterministic anchor for fixtures

/** IANA timezone string for Venezuela (no DST since May 2016). */
export const CARACAS_TZ = 'America/Caracas'

/** UTC offset for Caracas = -4 hours (constant, no DST). */
const CARACAS_UTC_OFFSET_HOURS = 4

/**
 * Convert ISO date + HH:mm (Caracas local time) to UTC epoch seconds.
 *
 * Caracas = UTC-4 fixed; e.g. "2026-04-15" + "08:00" → "2026-04-15T12:00:00Z"
 *
 * @param isoDate - Date in YYYY-MM-DD format
 * @param hhmm    - Time in HH:MM format (24h, Caracas local)
 * @returns UTC epoch seconds
 */
export function caracasEpoch(isoDate: string, hhmm: string): number {
  const [h, m] = hhmm.split(':').map(Number)
  const utcHour = h + CARACAS_UTC_OFFSET_HOURS
  // Handle day overflow (e.g. 23:00 Caracas = 03:00+1day UTC)
  const utcHourStr = String(utcHour % 24).padStart(2, '0')
  const dayOverflow = utcHour >= 24
  const dateParts = isoDate.split('-').map(Number) as [number, number, number]
  let [year, month, day] = dateParts
  if (dayOverflow) {
    const d = new Date(Date.UTC(year, month - 1, day + 1))
    year = d.getUTCFullYear()
    month = d.getUTCMonth() + 1
    day = d.getUTCDate()
  }
  const utcStr = `${year}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}T${utcHourStr}:${String(m).padStart(2, '0')}:00.000Z`
  return Math.floor(new Date(utcStr).getTime() / 1000)
}

/**
 * Format a UTC epoch (seconds) as a Caracas local time string HH:MM.
 * Useful in assertions that display wall-clock times in the UI.
 */
export function epochToCaracasHHMM(epochSeconds: number): string {
  const ms = epochSeconds * 1000
  const utcDate = new Date(ms)
  const caracasHour = (utcDate.getUTCHours() - CARACAS_UTC_OFFSET_HOURS + 24) % 24
  const m = utcDate.getUTCMinutes()
  return `${String(caracasHour).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}
