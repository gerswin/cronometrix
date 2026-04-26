// D-33: Currency display = en-US dot decimal $X,XXX.XX (parity with Excel
// $#,##0.00 cell format). Backend returns USD cents as i64; frontend divides
// by 100 and formats. This is the canonical money formatter for Phase 5+.

const usdFormatter = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
})

/**
 * Format USD cents as `$X,XXX.XX` using en-US locale (D-33).
 * Returns `'—'` when cents is null/undefined so empty cells render as a
 * dash rather than `$NaN`.
 */
export function fmtMoney(cents: number | null | undefined): string {
  if (cents === null || cents === undefined) return '—'
  return usdFormatter.format(cents / 100)
}

/**
 * Format USD cents as a negative amount (`-$31.25`). Used for the
 * `Descuento por Retraso` column (D-05) which is always shown as a
 * negative line item even though backend stores it as a positive
 * magnitude.
 */
export function fmtMoneyNegative(cents: number | null | undefined): string {
  if (cents === null || cents === undefined) return '—'
  return '-' + fmtMoney(cents)
}
