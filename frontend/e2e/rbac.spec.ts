/**
 * RBAC cross-cut spec — Plan 09-11 (D-01 RBAC cross-cut)
 *
 * Covers HTTP-level and UI-level role gating for all 3 roles + anonymous.
 *
 * Tests:
 *   T-01: viewer: GET /employees → 200 (read allowed per viewer_routes)
 *   T-02: viewer: POST /employees → 403 (supervisor+ only)
 *   T-03: viewer: POST /devices/{id}/commands → 403 (admin only)
 *   T-04: viewer: POST /leaves → 403 (admin only)
 *   T-05: viewer: GET /audit → 403 (supervisor_read_routes; viewer not allowed)
 *   T-06: supervisor: POST /employees → not 403 (supervisor_routes allows create)
 *   T-07: supervisor: DELETE /employees/{id} → 403 (admin_routes; admin only)
 *   T-08: supervisor: POST /devices/{id}/commands → 403 (admin_routes; admin only)
 *   T-09: admin: full access — POST /employees + DELETE /employees/{id}
 *   T-10: unauthenticated: GET /employees → 401
 *   T-11: viewer UI: /employees page hides Nuevo Empleado button (UI gating mirror)
 */

// RBAC source of truth: backend/src/main.rs route_layer assignments.
// Reconciled by executor before commit (W4 requirement).
//
// Route group breakdown (as of Phase 9, commit reconciled against main.rs):
//
//   viewer_routes (require_auth — any authenticated role):
//     GET  /employees, GET  /employees/{id}
//     GET  /departments, GET  /departments/{id}
//     GET  /rules
//     GET  /devices, GET  /devices/{id}
//     GET  /events, GET  /events/{id}, GET  /events/{id}/photo
//     GET  /daily-records, GET  /daily-records/{id}
//     GET  /leaves, GET  /leaves/{id}, GET  /leaves/{id}/evidence
//     GET  /tenant-info
//
//   supervisor_read_routes (require_supervisor_or_above — Admin + Supervisor, NOT Viewer):
//     GET  /anomalies
//     GET  /audit
//
//   supervisor_routes (require_supervisor_or_above — Admin + Supervisor):
//     POST  /employees            ← supervisor CAN create employees
//     PATCH /employees/{id}
//
//   report_routes (require_supervisor_or_above):
//     POST /reports/json, POST /reports/excel
//
//   admin_routes (require_admin — Admin ONLY):
//     DELETE /employees/{id}      ← supervisor CANNOT delete employees
//     POST   /departments, PATCH  /departments/{id}
//     PATCH  /rules
//     POST   /devices             ← only admin can create devices
//     PATCH  /devices/{id}
//     DELETE /devices/{id}
//     POST   /devices/{id}/commands  ← admin ONLY (not supervisor)
//     POST   /leaves              ← admin ONLY (not supervisor)
//     DELETE /leaves/{id}
//     POST   /daily-records/{id}/overrides
//     PATCH  /tenant-info
//
//   enrollment_routes (require_admin):
//     POST /enrollments, GET /enrollments/{id}, POST /enrollments/captures, …

import { test, expect } from '@playwright/test'
import { API_BASE } from './fixtures/api'
import { SEL } from './fixtures/selectors'

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

// Storage state files used by this helper:
//   e2e/.auth/admin.json
//   e2e/.auth/supervisor.json
//   e2e/.auth/viewer.json
async function withRole(
  browser: import('@playwright/test').Browser,
  role: 'admin' | 'supervisor' | 'viewer'
) {
  const ctx = await browser.newContext({ storageState: `e2e/.auth/${role}.json` })
  return { ctx, request: ctx.request }
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

test.describe('RBAC cross-cut (D-01 + cross-stack)', () => {
  // ── T-01: Viewer read — GET /employees is in viewer_routes (all auth allowed) ──
  test('viewer: GET /employees → 200 (read allowed per viewer_routes)', async ({ browser }) => {
    const { ctx, request } = await withRole(browser, 'viewer')
    const r = await request.get(`${API_BASE}/employees`)
    // viewer_routes includes GET /employees via require_auth (any authenticated role)
    expect(r.status()).toBe(200)
    await ctx.close()
  })

  // ── T-02: Viewer negative — POST /employees is in supervisor_routes (supervisor+) ──
  test('viewer: POST /employees → 403 (supervisor_routes, supervisor+ only)', async ({
    browser,
  }) => {
    // main.rs: POST /employees is in supervisor_routes behind require_supervisor_or_above.
    // Viewer token is valid but role === Viewer → rbac.rs returns AppError::Forbidden → 403.
    const { ctx, request } = await withRole(browser, 'viewer')
    const r = await request.post(`${API_BASE}/employees`, {
      data: { name: 'should not work', employee_code: 'X1', department_id: 'dept-prod' },
    })
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-03: Viewer negative — POST /devices/{id}/commands is in admin_routes ────
  test('viewer: POST /devices/{id}/commands → 403 (admin_routes, admin only)', async ({
    browser,
  }) => {
    // main.rs: POST /devices/{id}/commands is in admin_routes behind require_admin.
    // Viewer is not Admin → 403. (Supervisor is also 403 — see T-08.)
    const { ctx, request } = await withRole(browser, 'viewer')
    const r = await request.post(`${API_BASE}/devices/dev-entry/commands`, {
      data: { command: 'door_open' },
    })
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-04: Viewer negative — POST /leaves is in admin_routes (admin only) ──────
  test('viewer: POST /leaves → 403 (admin_routes, admin only)', async ({ browser }) => {
    // main.rs: POST /leaves is in admin_routes behind require_admin.
    // Viewer is not Admin → 403. (Supervisor is also 403 — see T-08 pattern.)
    const { ctx, request } = await withRole(browser, 'viewer')
    const r = await request.post(`${API_BASE}/leaves`, {
      data: {
        employee_id: 'emp-ana',
        leave_type: 'medical',
        from_date: '2026-04-15',
        to_date: '2026-04-16',
        justification: 'x',
      },
    })
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-05: Viewer negative — GET /audit is in supervisor_read_routes ───────────
  test('viewer: GET /audit → 403 (supervisor_read_routes, supervisor+ only)', async ({
    browser,
  }) => {
    // main.rs: GET /audit is in supervisor_read_routes behind require_supervisor_or_above.
    // Viewer role → rbac.rs returns AppError::Forbidden → 403.
    const { ctx, request } = await withRole(browser, 'viewer')
    const r = await request.get(`${API_BASE}/audit`)
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-06: Supervisor positive — POST /employees (supervisor_routes) ──────────
  test('supervisor: POST /employees → not 403 (supervisor_routes allows create)', async ({
    browser,
  }) => {
    // main.rs: POST /employees is in supervisor_routes (require_supervisor_or_above).
    // Supervisor role passes the check → 200/201 (created) or 422 (validation) — NOT 403.
    const { ctx, request } = await withRole(browser, 'supervisor')
    const r = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Supervisor Created',
        employee_code: 'SUP001',
        department_id: 'dept-prod',
      },
    })
    // Any non-403 response is acceptable: 200/201 means created; 422 means validation error.
    // The important contract is that role enforcement did NOT fire (no 403).
    expect(r.status()).not.toBe(403)
    await ctx.close()
  })

  // ── T-07: Supervisor negative — DELETE /employees/{id} is in admin_routes ────
  test('supervisor: DELETE /employees/{id} → 403 (admin_routes, admin only)', async ({
    browser,
  }) => {
    // main.rs: DELETE /employees/{id} is in admin_routes behind require_admin.
    // Supervisor role is not Admin → rbac.rs returns AppError::Forbidden → 403.
    const { ctx, request } = await withRole(browser, 'supervisor')
    const r = await request.delete(`${API_BASE}/employees/emp-ana`)
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-08: Supervisor negative — POST /devices/{id}/commands is in admin_routes ─
  test('supervisor: POST /devices/{id}/commands → 403 (admin_routes, admin only)', async ({
    browser,
  }) => {
    // main.rs: POST /devices/{id}/commands is in admin_routes (require_admin).
    // Supervisor cannot dispatch device commands — admin only per D-14.
    const { ctx, request } = await withRole(browser, 'supervisor')
    const r = await request.post(`${API_BASE}/devices/dev-entry/commands`, {
      data: { command: 'door_open' },
    })
    expect(r.status()).toBe(403)
    await ctx.close()
  })

  // ── T-09: Admin positive — POST + DELETE /employees (full access) ────────────
  test('admin: full access — POST /employees + DELETE /employees/{id}', async ({
    browser,
  }) => {
    const { ctx, request } = await withRole(browser, 'admin')

    // Create an employee as admin
    const create = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Admin Created Then Deleted',
        employee_code: 'ADC001',
        department_id: 'dept-prod',
      },
    })
    expect([200, 201]).toContain(create.status())

    // Delete the same employee — admin_routes includes DELETE /employees/{id}
    const body = await create.json().catch(() => ({}))
    if (body?.id) {
      const del = await request.delete(`${API_BASE}/employees/${body.id}`)
      expect([200, 204]).toContain(del.status())
    }

    await ctx.close()
  })

  // ── T-10: Unauthenticated — no token → 401 (require_auth fires) ──────────────
  test('unauthenticated: GET /employees → 401', async ({ browser }) => {
    // No storageState → no Authorization header → require_auth returns AppError::Unauthorized → 401
    const ctx = await browser.newContext()
    const request = ctx.request
    const r = await request.get(`${API_BASE}/employees`)
    expect(r.status()).toBe(401)
    await ctx.close()
  })

  // ── T-11: Viewer UI — Nuevo Empleado button hidden (UI gating mirror) ─────────
  test('viewer UI: /employees page hides Nuevo Empleado button (UI gating mirror)', async ({
    browser,
  }) => {
    // Frontend conditionally renders the new-employee-button only for admin/supervisor.
    // This is defense-in-depth — backend enforcement (T-02) is authoritative.
    const ctx = await browser.newContext({ storageState: 'e2e/.auth/viewer.json' })
    const page = await ctx.newPage()
    await page.goto('/employees')
    // new-employee-button must not be present in DOM for Viewer (not just hidden — absent)
    await expect(page.getByTestId(SEL.newEmpButton)).toHaveCount(0)
    await ctx.close()
  })
})
