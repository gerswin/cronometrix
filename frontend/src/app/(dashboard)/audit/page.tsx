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
 * W6 actor dropdown — OPTION B implementation (Plan 10-04):
 * Replaces the OPTION A "raw actor_id from current page data" workaround.
 * Actor names are fetched from GET /audit/actors with username+role join.
 * value={a.id} stays as actor_id so audit.spec.ts T-03 selectOption('e2e-admin-id') is unaffected.
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

  const { data: actorsData } = useQuery<
    Array<{ actor_id: string | null; username: string | null; role: string | null }>
  >({
    queryKey: ['audit-actors'],
    queryFn: () => api.get('/audit/actors').then(r => r.data),
    staleTime: 5 * 60 * 1000,
    enabled: role === 'admin' || role === 'supervisor',
  })

  /**
   * OPTION B (Plan 10-04): Derive actors from /audit/actors endpoint with username+role join.
   * Replaces the OPTION A "raw actor_id from current page data" workaround that 09-05 deferred.
   * value={a.id} stays as actor_id (E2E spec selectOption('e2e-admin-id') unaffected).
   * Display text becomes "{username} ({role})", falling back to actor_id when LEFT JOIN missed
   * (e.g., user was deleted).
   */
  const actors = useMemo(() => {
    if (!actorsData) return []
    return actorsData
      .filter(a => a.actor_id != null)
      .map(a => ({
        id: a.actor_id!,
        username: a.username ? `${a.username} (${a.role})` : a.actor_id!,
      }))
  }, [actorsData])

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
