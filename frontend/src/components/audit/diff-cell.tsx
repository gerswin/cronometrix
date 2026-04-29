'use client'
import type { AuditEntry } from '@/types/audit'

type Props = Pick<AuditEntry, 'operation' | 'old_data' | 'new_data'>

/**
 * DiffCell — renders a collapsible <details> summary of changed fields
 * between old_data and new_data JSON objects.
 *
 * Behaviour per operation:
 *   INSERT  → "+ N campos"  (shows new_data keys)
 *   DELETE  → "- N campos"  (shows old_data keys)
 *   UPDATE  → "~ N cambios" (shows only keys whose values changed or were added/removed)
 *   Both null → "—" (no-op log entry)
 */
export function DiffCell({ operation, old_data, new_data }: Props) {
  if (!old_data && !new_data) {
    return <span className="text-slate-400">—</span>
  }

  if (operation === 'INSERT' && new_data) {
    const keys = Object.keys(new_data)
    return (
      <details>
        <summary className="cursor-pointer text-emerald-700">+ {keys.length} campos</summary>
        <pre className="text-xs mt-1 whitespace-pre-wrap">{JSON.stringify(new_data, null, 2)}</pre>
      </details>
    )
  }

  if (operation === 'DELETE' && old_data) {
    const keys = Object.keys(old_data)
    return (
      <details>
        <summary className="cursor-pointer text-rose-700">- {keys.length} campos</summary>
        <pre className="text-xs mt-1 whitespace-pre-wrap">{JSON.stringify(old_data, null, 2)}</pre>
      </details>
    )
  }

  // UPDATE: diff old_data vs new_data, report only changed keys
  const allKeys = new Set([
    ...(old_data ? Object.keys(old_data) : []),
    ...(new_data ? Object.keys(new_data) : []),
  ])
  const changed: Record<string, { old: unknown; new: unknown }> = {}
  for (const k of allKeys) {
    const o = old_data?.[k]
    const n = new_data?.[k]
    if (JSON.stringify(o) !== JSON.stringify(n)) {
      changed[k] = { old: o, new: n }
    }
  }
  const count = Object.keys(changed).length
  return (
    <details>
      <summary className="cursor-pointer text-amber-700">~ {count} cambios</summary>
      <pre className="text-xs mt-1 whitespace-pre-wrap">{JSON.stringify(changed, null, 2)}</pre>
    </details>
  )
}
