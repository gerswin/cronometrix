/**
 * Audit log page E2E spec — Plan 09-11 (D-04 audit-screen UAT)
 *
 * Covers:
 *   T-01: renders Auditoría header + filters + table
 *   T-02: list shows immutable entries after a mutation elsewhere
 *   T-03: filter by actor — select admin actor_id → only admin-actor rows
 *   T-04: filter by date range narrows results
 *   T-05: Viewer is denied access (AccessRestricted renders; audit-page absent)
 *
 * Design constraints:
 *   - ZERO explicit-wait API violations (RESEARCH §Pitfalls B4 ban — uses expect.poll only)
 *   - Uses expect.poll(...) and locator visibility / count assertions exclusively
 *   - Actor dropdown (W6 OPTION A) uses actor_id as the <option value>, not
 *     display name. selectOption('e2e-admin-id') is the correct form.
 *     See 09-05-SUMMARY.md "W6 actor dropdown — OPTION A" for the full rationale.
 *
 * Data-testid contract (locked by Plan 05 audit-table.tsx / audit-filters.tsx):
 *   audit-page         — root <div> in page.tsx
 *   audit-table        — wrapper <div> in audit-table.tsx
 *   audit-row-${id}    — <tr> per audit entry
 *   audit-empty        — <tr> when data=[]
 *   audit-filter-actor — <select> (value = actor_id string)
 *   audit-filter-from  — <input type="date">
 *   audit-filter-to    — <input type="date">
 *
 * Seed data (seed_e2e.rs):
 *   Departments: dept-prod (Producción), dept-admin (Administración), dept-rrhh
 *   SQL-trigger mutations have actor_id = null; app-code report exports capture
 *   the authenticated e2e_admin user (actor_id = 'e2e-admin-id').
 */

import { test, expect, newRoleContext } from './fixtures/auth'
import { auditWindowStart, resetMutableTables, getAudit, API_BASE } from './fixtures/api'

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

test.describe('Audit log page (D-04 UAT)', () => {
  test.beforeEach(async ({ request }) => {
    await resetMutableTables(request)
  })

  // ── T-01: Page structure renders correctly after reset ─────────────────────
  test('renders Auditoría header + filters + table container', async ({ page }) => {
    await page.goto('/audit')
    await expect(
      page.getByRole('heading', { name: 'Auditoría', exact: true })
    ).toBeVisible()
    await expect(page.getByTestId('audit-page')).toBeVisible()
    await expect(page.getByTestId('audit-filter-actor')).toBeVisible()
    await expect(page.getByTestId('audit-filter-from')).toBeVisible()
    await expect(page.getByTestId('audit-filter-to')).toBeVisible()
    // audit-table wrapper always renders (empty-state row inside it)
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })
  })

  // ── T-02: Mutation elsewhere seeds an audit_log entry ─────────────────────
  test('list shows immutable entries after a mutation elsewhere', async ({ page, request }) => {
    const fromTs = auditWindowStart()
    // POST /employees triggers the employees INSERT audit trigger
    const r = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Audit Test User',
        employee_code: `AUD${Date.now()}`,
        department_id: 'dept-prod',
      },
    })

    expect(r.ok()).toBe(true)
    const employee = await r.json()
    let auditId = ''
    await expect.poll(async () => {
      const evidence = await getAudit(request, {
        table_name: 'employees',
        record_id: employee.id,
        operation: 'INSERT',
        from_ts: fromTs,
        limit: 5,
      })
      if (!evidence.ok()) return false
      const body = await evidence.json()
      auditId = body.data?.[0]?.id ?? ''
      return Boolean(auditId)
    }).toBe(true)
    await page.goto('/audit')
    await expect(page.getByTestId(`audit-row-${auditId}`)).toBeVisible({ timeout: 10_000 })
  })

  // ── T-03: Actor filter — select admin actor_id → only admin-actor rows ─────
  test('filter by actor: select admin actor_id → only admin-actor rows', async ({
    page,
    request,
  }) => {
    const fromTs = auditWindowStart()
    // Keep a trigger-created row with actor_id = null. It must be excluded by
    // the actor filter below.
    const triggerMutation = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Admin Made',
        employee_code: `ADM${Date.now()}`,
        department_id: 'dept-prod',
      },
    })
    expect(triggerMutation.ok()).toBe(true)
    const triggerEmployee = await triggerMutation.json()

    const reportAuditBefore = await getAudit(request, {
      table_name: 'reports',
      operation: 'REPORT_EXPORT',
      actor_id: 'e2e-admin-id',
      limit: 200,
    })
    expect(reportAuditBefore.ok()).toBe(true)
    const reportAuditBeforeBody = await reportAuditBefore.json()
    const reportAuditBaseline = new Set<string>(
      (reportAuditBeforeBody.data ?? []).map((entry: { id: string }) => entry.id)
    )

    // Report exports are audited in app code, so the actor comes from the
    // authenticated admin JWT instead of being lost in a SQL trigger.
    const today = new Date()
    const year = today.getFullYear()
    const month = String(today.getMonth() + 1).padStart(2, '0')
    const day = String(today.getDate()).padStart(2, '0')
    const reportExport = await request.post(`${API_BASE}/reports/json`, {
      data: {
        period_type: 'monthly',
        from_date: `${year}-${month}-01`,
        to_date: `${year}-${month}-${day}`,
        include_inactive: false,
      },
    })
    expect(reportExport.ok()).toBe(true)

    let nullActorRowId = ''
    let reportAuditRowId = ''
    await expect.poll(
      async () => {
        const employeeEvidence = await getAudit(request, {
          table_name: 'employees',
          record_id: triggerEmployee.id,
          operation: 'INSERT',
          from_ts: fromTs,
          limit: 5,
        })
        const reportEvidence = await getAudit(request, {
          table_name: 'reports',
          operation: 'REPORT_EXPORT',
          actor_id: 'e2e-admin-id',
          limit: 200,
        })
        if (!employeeEvidence.ok() || !reportEvidence.ok()) return false
        const employeeBody = await employeeEvidence.json()
        const reportBody = await reportEvidence.json()
        const employeeEntries: Array<{
          id: string
          record_id: string
          actor_id: string | null
        }> = employeeBody.data ?? []
        const nullActorRow = employeeEntries.find(
          (entry) =>
            entry.record_id === triggerEmployee.id &&
            entry.actor_id === null
        )
        const newReportRow = (reportBody.data ?? []).find(
          (entry: { id: string }) => !reportAuditBaseline.has(entry.id)
        )
        nullActorRowId = nullActorRow?.id ?? ''
        reportAuditRowId = newReportRow?.id ?? ''
        return Boolean(nullActorRowId && reportAuditRowId)
      },
      {
        message: 'Expected both a null-actor trigger row and an admin report export row',
      }
    ).toBe(true)

    await page.goto('/audit')

    // Wait for audit-table wrapper to be visible before interacting with filters.
    // Explicit visibility assertion — uses toBeVisible timeout, not sleep.
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })

    const actorFilter = page.getByTestId('audit-filter-actor')
    await expect(
      actorFilter.locator('option[value="e2e-admin-id"]')
    ).toHaveCount(1)

    // Select by actor_id (W6 OPTION A: value = actor_id string "e2e-admin-id").
    // See 09-05-SUMMARY.md: actor_id is users.id; the option value IS the actor_id.
    await actorFilter.selectOption('e2e-admin-id')

    // Explicit-wait for table to settle after filter change. Poll until table is
    // visible AND either rows exist OR empty-state is shown (expect.poll pattern).
    await expect.poll(
      async () => {
        const visible = await page.getByTestId('audit-table').isVisible()
        const rowCount = await page.locator('[data-testid^="audit-row-"]').count()
        const emptyVisible = await page
          .getByTestId('audit-empty')
          .isVisible()
          .catch(() => false)
        return visible && (rowCount > 0 || emptyVisible)
      },
      { timeout: 5_000, message: 'audit table did not settle after actor filter change' }
    ).toBe(true)

    // The authenticated report export remains, while the null-actor trigger row
    // is excluded. Every rendered actor cell must match the selected actor.
    const rows = page.locator('[data-testid^="audit-row-"]')
    await expect(rows.first()).toBeVisible()
    await expect(page.getByTestId(`audit-row-${reportAuditRowId}`)).toBeVisible()
    await expect(page.getByTestId(`audit-row-${nullActorRowId}`)).toHaveCount(0)
    const rowCount = await rows.count()
    expect(rowCount).toBeGreaterThan(0)
    for (let index = 0; index < rowCount; index += 1) {
      await expect(rows.nth(index).locator('td').nth(1)).toHaveText('e2e-admin-id')
    }
  })

  // ── T-04: Date range filter narrows results ────────────────────────────────
  test('filter by date range narrows results', async ({ page, request }) => {
    const fromTs = auditWindowStart()
    // Pre-insert a deterministic audit entry
    const mutation = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Date Filter Test',
        employee_code: `DTE${Date.now()}`,
        department_id: 'dept-prod',
      },
    })
    expect(mutation.ok()).toBe(true)
    const employee = await mutation.json()
    let auditId = ''
    let auditCreatedAt = 0
    await expect.poll(async () => {
      const evidence = await getAudit(request, {
        table_name: 'employees',
        record_id: employee.id,
        operation: 'INSERT',
        from_ts: fromTs,
      })
      if (!evidence.ok()) return false
      const body = await evidence.json()
      auditId = body.data?.[0]?.id ?? ''
      auditCreatedAt = body.data?.[0]?.created_at ?? 0
      return Boolean(auditId && auditCreatedAt)
    }).toBe(true)

    await page.goto('/audit')

    // Wait for audit-table to render before applying date filter.
    // Explicit-wait on visibility via toBeVisible timeout.
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })

    // Select the business-local day that contains the exact audit row.
    const dateParts = new Intl.DateTimeFormat('en-US', {
      timeZone: 'America/Caracas',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    }).formatToParts(new Date(auditCreatedAt * 1000))
    const part = (type: Intl.DateTimeFormatPartTypes) =>
      dateParts.find(value => value.type === type)?.value ?? ''
    const today = `${part('year')}-${part('month')}-${part('day')}`
    await page.getByTestId('audit-filter-from').fill(today)
    await page.getByTestId('audit-filter-to').fill(today)

    await expect(page.getByTestId(`audit-row-${auditId}`)).toBeVisible({ timeout: 5_000 })
  })

  // ── T-05: Viewer is denied access (RBAC gate) ─────────────────────────────
  test('Viewer is denied access (AccessRestricted renders; audit-page absent)', async ({
    browser,
  }) => {
    const ctx = await newRoleContext(browser, 'viewer')
    const page = await ctx.newPage()
    await page.goto('/audit')

    // Plan 05 RBAC gate: role !== 'admin' && role !== 'supervisor' → <AccessRestricted />
    // The AccessRestricted component renders text matching "no tiene permiso" or similar.
    await expect(
      page.getByText(/no.*permis|acceso.*restring|access.*restrict/i)
    ).toBeVisible({ timeout: 10_000 })

    // The audit-page data-testid must NOT be present for Viewer (guard fires before render)
    await expect(page.getByTestId('audit-page')).toHaveCount(0)

    await ctx.close()
  })
})
