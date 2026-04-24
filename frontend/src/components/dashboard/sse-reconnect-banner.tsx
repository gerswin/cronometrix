export function SSEReconnectBanner({ reconnecting }: { reconnecting: boolean }) {
  if (!reconnecting) return null
  return (
    <div className="bg-orange-500 text-white text-sm px-4 py-2 rounded-md mb-4 flex items-center gap-2">
      <span className="animate-pulse h-2 w-2 rounded-full bg-white inline-block" />
      Conexión perdida — reconectando…
    </div>
  )
}
