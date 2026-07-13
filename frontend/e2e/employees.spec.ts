/**
 * Employees (Empleados) E2E spec — Plan 09-09 (D-03 UAT depth)
 *
 * Covers: list, search filter, dept filter, happy create + audit assertion,
 * validation errors, edit + audit assertion, deactivate (soft delete) + audit
 * assertion, RBAC (viewer cannot see Nuevo Empleado button).
 *
 * Seed data (Plan 03 / seed_e2e.rs):
 *   emp-ana   / Ana Pérez    / dept-prod (Producción)
 *   emp-luis  / Luis García  / dept-prod (Producción)
 *   emp-maria / María López  / dept-admin (Administración)
 *   emp-pedro / Pedro Ramírez/ dept-admin (Administración)
 *   emp-carmen/ Carmen Silva / dept-rrhh (Recursos Humanos)
 *   emp-jose  / José Hernández/ dept-rrhh (Recursos Humanos)
 *
 * Authenticated tests use a fresh admin context except the explicit RBAC test.
 * test.beforeEach resets mutable tables for determinism (D-12).
 */

import { test, expect, newRoleContext } from './fixtures/auth'
import { resetMutableTables, getAudit } from './fixtures/api'
import { SEL } from './fixtures/selectors'

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

test.describe('Employees (Empleados) — D-03 CRUD UAT', () => {
  test.beforeEach(async ({ request }) => {
    await resetMutableTables(request)
  })

  // ── T-01: List renders with Spanish heading and seeded employees ──────────
  test('lists seeded employees with Spanish heading', async ({ page }) => {
    await page.goto('/employees')
    await expect(page.getByText('Empleados')).toBeVisible()
    await expect(page.getByText('Ana Pérez')).toBeVisible()
    await expect(page.getByText('Luis García')).toBeVisible()
  })

  // ── T-02: Search by name filters the list ────────────────────────────────
  test('search by name filters employee list', async ({ page }) => {
    await page.goto('/employees')
    await page.getByPlaceholder(/Buscar empleado/i).fill('Ana')
    // After typing, query re-fires with name=Ana; Ana Pérez remains visible
    await expect(page.getByText('Ana Pérez')).toBeVisible({ timeout: 5_000 })
    // Luis García should not be visible once filter applies
    await expect(page.getByText('Luis García')).not.toBeVisible({ timeout: 5_000 })
  })

  // ── T-03: Dept filter shows only employees from that department ───────────
  test('department filter shows only Producción employees', async ({ page }) => {
    await page.goto('/employees')
    // The dept select has no testid; select by role / first select element
    const selects = page.getByRole('combobox')
    await selects.first().selectOption({ label: 'Producción' })
    // After filter: Ana and Luis (both dept-prod) visible
    await expect(page.getByText('Ana Pérez')).toBeVisible({ timeout: 5_000 })
    await expect(page.getByText('Luis García')).toBeVisible({ timeout: 5_000 })
    // María López (dept-admin) should not be visible
    await expect(page.getByText('María López')).not.toBeVisible({ timeout: 5_000 })
  })

  // ── T-04: Nuevo Empleado button opens the create dialog ──────────────────
  test('Nuevo Empleado button opens create dialog', async ({ page }) => {
    await page.goto('/employees')
    await page.getByTestId(SEL.newEmpButton).click()
    await expect(page.getByTestId(SEL.newEmpForm)).toBeVisible({ timeout: 5_000 })
    // Form title
    await expect(page.getByText('Nuevo Empleado')).toBeVisible()
  })

  // ── T-05: Validation — missing name shows form error ─────────────────────
  test('validation: missing name shows form error and keeps dialog open', async ({ page }) => {
    await page.goto('/employees')
    await page.getByTestId(SEL.newEmpButton).click()
    await expect(page.getByTestId(SEL.newEmpForm)).toBeVisible({ timeout: 5_000 })
    // Click submit without filling any field
    await page.getByTestId(SEL.newEmpSubmit).click()
    // Dialog remains open (validation prevented submit)
    await expect(page.getByTestId(SEL.newEmpForm)).toBeVisible()
    // A role="alert" validation message should appear
    const alertMsg = page.locator('[role="alert"]').first()
    await expect(alertMsg).toBeVisible({ timeout: 3_000 })
  })

  // ── T-06: Happy create — Nuevo Empleado → list update + audit entry ───────
  test('happy create: fill form → save → employee appears in list + audit INSERT', async ({
    page,
    request,
  }) => {
    await page.goto('/employees')
    await page.getByTestId(SEL.newEmpButton).click()
    await expect(page.getByTestId(SEL.newEmpForm)).toBeVisible({ timeout: 5_000 })

    // Fill mandatory fields matching the dialog's label ids
    await page.locator('#new-emp-name').fill('Test Empleado Plan09')
    await page.locator('#new-emp-code').fill('EMP_TEST_09')
    // Department — select Producción (first non-empty option in the seeded list)
    await page.locator('#new-emp-dept').selectOption({ label: 'Producción' })

    await page.getByTestId(SEL.newEmpSubmit).click()

    // Dialog closes and new employee appears in the refreshed list
    await expect(page.getByTestId(SEL.newEmpForm)).toBeHidden({ timeout: 10_000 })
    await expect(page.getByText('Test Empleado Plan09')).toBeVisible({ timeout: 10_000 })

    // Audit assertion — employees INSERT
    await expect.poll(
      async () => {
        const r = await getAudit(request, {
          table_name: 'employees',
          operation: 'INSERT',
          limit: 5,
        })
        if (r.status() !== 200) return null
        const body = await r.json()
        return body.total ?? body.data?.length ?? 0
      },
      { timeout: 15_000, message: 'Expected audit_log entry for employees INSERT' },
    ).toBeGreaterThanOrEqual(1)
  })

  // ── T-07: Edit employee → list update + audit UPDATE entry ───────────────
  test('edit employee: change name → list updates + audit UPDATE entry', async ({
    page,
    request,
  }) => {
    await page.goto('/employees')
    // Click the edit button for emp-ana
    await expect(page.getByTestId(SEL.empActionEdit('emp-ana'))).toBeVisible({ timeout: 10_000 })
    await page.getByTestId(SEL.empActionEdit('emp-ana')).click()

    // Edit dialog opens with current name
    const editForm = page.getByTestId('edit-employee-form')
    await expect(editForm).toBeVisible({ timeout: 5_000 })

    // Change the name
    await page.locator('#edit-emp-name').fill('Ana Pérez Editada')
    await page.getByRole('button', { name: /Guardar/i }).click()

    // Dialog closes and updated name appears
    await expect(editForm).toBeHidden({ timeout: 10_000 })
    await expect(page.getByText('Ana Pérez Editada')).toBeVisible({ timeout: 10_000 })

    // Audit assertion — employees UPDATE
    await expect.poll(
      async () => {
        const r = await getAudit(request, {
          record_id: 'emp-ana',
          operation: 'UPDATE',
          limit: 5,
        })
        if (r.status() !== 200) return null
        const body = await r.json()
        return body.total ?? body.data?.length ?? 0
      },
      { timeout: 15_000, message: 'Expected audit_log entry for employees UPDATE on emp-ana' },
    ).toBeGreaterThanOrEqual(1)
  })

  // ── T-08: Deactivate employee → audit DELETE (soft delete) entry ──────────
  test('deactivate employee → audit DELETE (soft delete) entry', async ({ page, request }) => {
    await page.goto('/employees')
    // Click the deactivate button for emp-luis
    await expect(page.getByTestId(SEL.empActionDeactivate('emp-luis'))).toBeVisible({ timeout: 10_000 })
    await page.getByTestId(SEL.empActionDeactivate('emp-luis')).click()

    // Confirm dialog appears
    const confirmBtn = page.getByRole('button', { name: /Desactivar/i }).last()
    await expect(confirmBtn).toBeVisible({ timeout: 5_000 })
    await confirmBtn.click()

    // Audit assertion — employees DELETE (soft delete via DELETE /employees/:id)
    await expect.poll(
      async () => {
        const r = await getAudit(request, {
          record_id: 'emp-luis',
          operation: 'DELETE',
          limit: 5,
        })
        if (r.status() !== 200) return null
        const body = await r.json()
        return body.total ?? body.data?.length ?? 0
      },
      { timeout: 15_000, message: 'Expected audit_log entry for employees DELETE on emp-luis' },
    ).toBeGreaterThanOrEqual(1)
  })

  // ── T-09: RBAC — Viewer cannot see Nuevo Empleado button ─────────────────
  test('Viewer cannot see Nuevo Empleado button', async ({ browser }) => {
    const ctx = await newRoleContext(browser, 'viewer')
    const page = await ctx.newPage()
    await page.goto('/employees')
    // Viewer role: new-employee-button must not be present in DOM
    await expect(page.getByTestId(SEL.newEmpButton)).toHaveCount(0)
    await ctx.close()
  })
})
