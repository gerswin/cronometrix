'use client'
import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { AccessRestricted } from '@/components/common/access-restricted'
import { useAuth } from '@/hooks/use-auth'
import { AuditTable } from '@/components/audit/audit-table'
import { AuditFilters, type FilterState } from '@/components/audit/audit-filters'
import type { AuditEntry } from '@/types/audit'
import type { PaginatedResponse } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const PAGE_SIZE = 20

/**
 * W6 actor dropdown — OPTION A implementation:
 * Actor IDs are derived from the distinct actor_id values in the current page data.
 * Reason: Plan 09-04 explicitly deferred GET /audit/actors to this plan or later.
 * The actors dropdown is populated from the currently-visible audit entries so that
 * Plan 11 (audit.spec.ts) can still use selectOption('e2e-admin-id') — the actor_id
 * value appears in the loaded rows.
 *
 * Note: actor_id in audit_log is users.id, NOT employees.id. This is correctly typed
 * in AuditEntry.actor_id: string | null.
 */

/**
 * Known table names that appear in audit_log triggers.
 * These match the trigger definitions from migrations 002, 006, 011, 014, 017.
 */
const TABLE_OPTIONS = [
  'employees',
  'departments',
  'leaves',
  'daily_records',
  'daily_record_overrides',
  'devices',
  'rules',
  'tenant_info',
  'enrollments',
]

export default function AuditPage() {
  const { role } = useAuth()
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: PAGE_SIZE,
  })
  const [filters, setFilters] = useState<FilterState>({})

  const { data, isLoading } = useQuery<PaginatedResponse<AuditEntry>>({
    queryKey: ['audit', pagination.pageIndex, filters],
    queryFn: () =>
      api
        .get('/audit', {
          params: {
            limit: PAGE_SIZE,
            offset: pagination.pageIndex * PAGE_SIZE,
            ...(filters.actor_id && { actor_id: filters.actor_id }),
            ...(filters.table_name && { table_name: filters.table_name }),
            ...(filters.operation && { operation: filters.operation }),
            ...(filters.record_id && { record_id: filters.record_id }),
            ...(filters.from_ts !== undefined && { from_ts: filters.from_ts }),
            ...(filters.to_ts !== undefined && { to_ts: filters.to_ts }),
          },
        })
        .then(r => r.data),
    enabled: role === 'admin' || role === 'supervisor',
  })

  /**
   * OPTION A: Derive distinct actor options from the current page's data.
   * actor_id is users.id (not employees.id — they are different tables).
   * The username is the actor_id itself (we don't have a username join from
   * this endpoint). Plan 11 audit.spec.ts selects by actor_id value, e.g.
   * selectOption('e2e-admin-id') — this works because <option value=actor_id>.
   */
  const actors = useMemo(() => {
    if (!data?.data) return []
    const seen = new Map<string, string>()
    for (const entry of data.data) {
      if (entry.actor_id && !seen.has(entry.actor_id)) {
        seen.set(entry.actor_id, entry.actor_id)
      }
    }
    return Array.from(seen.entries()).map(([id, username]) => ({ id, username }))
  }, [data?.data])

  // T-09-03: Frontend role gate (defense in depth).
  // Backend /api/v1/audit rejects Viewer with 403 — authoritative.
  // useQuery is disabled for non-authorized roles, so no API call is made.
  if (role !== 'admin' && role !== 'supervisor') {
    return <AccessRestricted />
  }

  return (
    <div className="flex flex-col h-full" data-testid="audit-page">
      <TopBar title="Auditoría" />
      <div className="p-6 space-y-4">
        <AuditFilters
          value={filters}
          onChange={next => {
            setFilters(next)
            setPagination(p => ({ ...p, pageIndex: 0 }))
          }}
          actors={actors}
          tables={TABLE_OPTIONS}
        />
        <div className="bg-white rounded-xl border shadow-sm overflow-hidden">
          <AuditTable
            data={data?.data ?? []}
            total={data?.total ?? 0}
            pagination={pagination}
            onPaginationChange={setPagination}
            isLoading={isLoading}
          />
        </div>
      </div>
    </div>
  )
}
