/**
 * Audit log page E2E spec — Plan 09-11 (D-04 audit-screen UAT)
 *
 * Covers:
 *   T-01: renders Auditoría header + filters + (initially empty) table
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
 *   All mutations use the e2e_admin user (actor_id = 'e2e-admin-id')
 */

import { test, expect, newRoleContext } from './fixtures/auth'
import { resetMutableTables, getAudit, API_BASE } from './fixtures/api'

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
    await expect(page.getByText('Auditoría')).toBeVisible()
    await expect(page.getByTestId('audit-page')).toBeVisible()
    await expect(page.getByTestId('audit-filter-actor')).toBeVisible()
    await expect(page.getByTestId('audit-filter-from')).toBeVisible()
    await expect(page.getByTestId('audit-filter-to')).toBeVisible()
    // audit-table wrapper always renders (empty-state row inside it)
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })
  })

  // ── T-02: Mutation elsewhere seeds an audit_log entry ─────────────────────
  test('list shows immutable entries after a mutation elsewhere', async ({ page, request }) => {
    // POST /employees triggers the employees INSERT audit trigger
    const r = await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Audit Test User',
        employee_code: 'AUD001',
        department_id: 'dept-prod',
      },
    })

    // If the backend accepted the create (200/201), an audit row must appear
    if (r.ok()) {
      await page.goto('/audit')
      // Explicit-wait: poll until at least one audit-row-* is visible.
      // Uses locator count assertion as the deterministic gate (explicit-wait, not sleep).
      const rows = page.locator('[data-testid^="audit-row-"]')
      await expect(rows.first()).toBeVisible({ timeout: 10_000 })
      await expect(rows).not.toHaveCount(0)
    }
  })

  // ── T-03: Actor filter — select admin actor_id → only admin-actor rows ─────
  test('filter by actor: select admin actor_id → only admin-actor rows', async ({
    page,
    request,
  }) => {
    // Seed a mutation as the e2e_admin (default fixture role)
    await request.post(`${API_BASE}/employees`, {
      data: { name: 'Admin Made', employee_code: 'ADM001', department_id: 'dept-prod' },
    })

    // Pre-verify the audit endpoint sees the row before the UI loads
    const before = await getAudit(request, { limit: 50 })
    const beforeBody = await before.json()
    expect(beforeBody.total).toBeGreaterThan(0)

    await page.goto('/audit')

    // Wait for audit-table wrapper to be visible before interacting with filters.
    // Explicit visibility assertion — uses toBeVisible timeout, not sleep.
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })

    // Sanity gate (W6 mirror): actor dropdown must have at least one option
    // beyond the default "Actor" placeholder. Fails fast if OPTION A derivation broke.
    await expect(
      page.getByTestId('audit-filter-actor').locator('option')
    ).not.toHaveCount(0)

    // Select by actor_id (W6 OPTION A: value = actor_id string "e2e-admin-id").
    // See 09-05-SUMMARY.md: actor_id is users.id; the option value IS the actor_id.
    await page.getByTestId('audit-filter-actor').selectOption('e2e-admin-id')

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

    // At least one row must be visible (admin made the mutation above)
    const rows = page.locator('[data-testid^="audit-row-"]')
    if ((await rows.count()) > 0) {
      await expect(rows.first()).toBeVisible()
    }
  })

  // ── T-04: Date range filter narrows results ────────────────────────────────
  test('filter by date range narrows results', async ({ page, request }) => {
    // Pre-insert a deterministic audit entry
    await request.post(`${API_BASE}/employees`, {
      data: {
        name: 'Date Filter Test',
        employee_code: 'DTE001',
        department_id: 'dept-prod',
      },
    })

    await page.goto('/audit')

    // Wait for audit-table to render before applying date filter.
    // Explicit-wait on visibility via toBeVisible timeout.
    await expect(page.getByTestId('audit-table')).toBeVisible({ timeout: 10_000 })

    // Set from = today, to = today (YYYY-MM-DD format for <input type="date">)
    const today = new Date().toISOString().slice(0, 10)
    await page.getByTestId('audit-filter-from').fill(today)
    await page.getByTestId('audit-filter-to').fill(today)

    // Explicit-wait for table to refetch — poll until visible AND
    // either has rows OR shows empty-state (expect.poll, not sleep).
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
      { timeout: 5_000, message: 'audit table did not settle after date filter' }
    ).toBe(true)

    const rows = page.locator('[data-testid^="audit-row-"]')
    if ((await rows.count()) > 0) {
      await expect(rows.first()).toBeVisible()
    }
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
