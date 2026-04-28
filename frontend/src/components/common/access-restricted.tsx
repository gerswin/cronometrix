'use client'
import { ShieldOff } from 'lucide-react'
import Link from 'next/link'

export function AccessRestricted() {
  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] gap-4 text-center">
      <ShieldOff className="text-slate-400" size={48} />
      <h1 className="text-xl font-semibold">Acceso restringido</h1>
      <p className="text-sm text-slate-500 max-w-md">
        Solo los administradores pueden enrolar rostros. Contacta a tu administrador para más información.
      </p>
      <Link
        href="/dashboard"
        className="inline-flex items-center justify-center rounded-lg border border-border bg-background px-4 py-2 text-sm font-medium hover:bg-muted hover:text-foreground transition-colors"
      >
        Volver al panel
      </Link>
    </div>
  )
}
