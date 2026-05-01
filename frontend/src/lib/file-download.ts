// Open a Blob in a new browser tab. Auto-revokes the object URL after a tick.
// Used for evidence/photo downloads where the response is gated by Bearer auth
// (so a naive `<a href>` won't work — must come through axios with credentials).

export function openBlobInNewTab(blob: Blob): void {
  const url = URL.createObjectURL(blob)
  const w = window.open(url, '_blank', 'noopener,noreferrer')
  // Some browsers block window.open during async — fall back to anchor click.
  if (!w) {
    const a = document.createElement('a')
    a.href = url
    a.target = '_blank'
    a.rel = 'noopener noreferrer'
    a.click()
  }
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}

// Trigger a forced file download (used for exports where we want a save dialog,
// not an inline view). Filename inferred from server's Content-Disposition when
// available; otherwise caller supplies one.
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}
