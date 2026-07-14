import type { APIRequestContext, APIResponse } from '@playwright/test'

export const API_BASE = 'http://127.0.0.1:4001/api/v1'

/** Inclusive epoch-second boundary for isolating audit evidence from prior E2E runs. */
export function auditWindowStart(): number {
  return Math.floor(Date.now() / 1000)
}

/** GET /audit — typed wrapper for assertions in CRUD specs (mutation→audit). */
export async function getAudit(
  req: APIRequestContext,
  params: Partial<{
    actor_id: string
    table_name: string
    record_id: string
    operation: 'INSERT' | 'UPDATE' | 'DELETE'
    from_ts: number
    to_ts: number
    limit: number
    offset: number
  }> = {}
): Promise<APIResponse> {
  return req.get(`${API_BASE}/audit`, {
    params: params as Record<string, string | number>,
  })
}

/** Reset mutable tables between describe blocks (D-12). */
export async function resetMutableTables(req: APIRequestContext): Promise<void> {
  const r = await req.post(`${API_BASE}/__test_reset`)
  if (r.status() !== 200) {
    throw new Error(
      `__test_reset failed: ${r.status()} (is CRONOMETRIX_E2E=true on backend?)`
    )
  }
}

/** Push a Hikvision event into the mock alertStream queue. */
export async function pushHikvisionEvent(
  req: APIRequestContext,
  xml: string
): Promise<APIResponse> {
  return req.post('http://127.0.0.1:4401/admin/push-event', {
    data: { xml },
    headers: { 'Content-Type': 'application/json' },
  })
}
