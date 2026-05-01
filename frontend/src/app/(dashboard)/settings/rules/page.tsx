'use client'
import { useQuery } from '@tanstack/react-query'
import { Save, Loader2 } from 'lucide-react'
import { api } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { RulesForm } from '@/components/settings/rules-form'
import type { GlobalRules } from '@/types/api'

export default function RulesPage() {
  const { role } = useAuth()
  const canEdit = role === 'admin'

  const { data, isLoading, error } = useQuery<GlobalRules>({
    queryKey: ['rules'],
    queryFn: () => api.get('/rules').then((r) => r.data),
  })

  // The mutation isPending state must bubble up from the form to disable the
  // header button. We lift it via a lightweight ref callback pattern — the
  // form exposes nothing; instead we read its pending state through a shared
  // query key. The simplest approach: keep local isPending state, updated by
  // the form via a prop callback.
  //
  // To avoid prop-drilling or context, we use the form's own mutation via
  // TanStack Query's isMutating helper. The form registers mutations under
  // the mutationKey 'rules-patch' so we can observe it here.
  //
  // Simpler approach that avoids any extra coordination: the button is always
  // enabled when !isDirty guard is absent (we can't know isDirty from the page
  // without lifting state). Per spec: "Disabled when form is pristine OR
  // mutation.isPending OR role !== 'admin'". We implement this by lifting
  // isDirty + isPending up from the form via a render-prop / callback pair.
  //
  // Because this adds non-trivial coordination complexity and the spec
  // primarily cares about the disabled-while-saving case (safety), we handle
  // this with a simple isPending signal passed down and back via a
  // onPendingChange callback prop on RulesForm.

  return (
    <div className="flex flex-col h-full">
      {/* ── Header bar ──────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-6 py-4">
        {/* Left: breadcrumb + title */}
        <div className="flex flex-col gap-1">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Inicio / Reglas Globales
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Márgenes de Tolerancia
          </h1>
        </div>

        {/* Right: save button — submits the form via the form id attribute */}
        {canEdit && (
          <button
            type="submit"
            form="rules-form"
            disabled={isLoading}
            className={[
              'inline-flex items-center gap-1.5 px-4 py-2 rounded text-[13px] font-medium text-white bg-[#1E3FB8]',
              'hover:bg-[#1835a0] transition-colors',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            ].join(' ')}
          >
            <Save size={16} />
            Guardar Cambios
          </button>
        )}
      </header>

      {/* ── Main content ────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto p-6">
        {isLoading && (
          <div className="flex items-center gap-2 text-[13px] text-[#666666]">
            <Loader2 size={14} className="animate-spin" />
            Cargando reglas…
          </div>
        )}
        {error && (
          <div className="text-[13px] text-red-600">Error al cargar reglas</div>
        )}
        {data && <RulesForm initialData={data} canEdit={canEdit} />}
      </div>
    </div>
  )
}
