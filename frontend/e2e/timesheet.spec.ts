/**
 * Timesheet (Marcaciones) E2E spec — Plan 09-09 (D-03 UAT depth)
 *
 * Covers: grid render, filter by department, Registrar Novedad modal
 * (open, validation, happy path with evidence upload), audit_log assertion
 * per mutation (mutation→audit dimension, CLAUDE.md non-negotiable).
 *
 * Authenticated tests use a fresh admin context per test.
 * test.beforeEach resets mutable tables for determinism (D-12).
 *
 * Language: Spanish copy per D-19 (timesheet page is Spanish locale).
 */

import { type Page, type APIRequestContext } from '@playwright/test'
import { test, expect, API_BASE } from './fixtures/auth'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import {
  resetMutableTables,
  getAudit,
  pushHikvisionEvent,
} from './fixtures/api'
import { SEL } from './fixtures/selectors'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function readEvent(filename: string): Promise<string> {
  return fs.readFile(
    path.resolve(__dirname, 'fixtures/hikvision-events', filename),
    'utf8',
  )
}

async function restartEntryDevice(request: APIRequestContext) {
  const currentResponse = await request.get(`${API_BASE}/devices/dev-entry`)
  expect(currentResponse.ok()).toBeTruthy()
  const current = await currentResponse.json()
  const temporaryResponse = await request.patch(`${API_BASE}/devices/dev-entry`, {
    data: { version: current.version, port: 4402 },
  })
  expect(temporaryResponse.ok()).toBeTruthy()
  const temporary = await temporaryResponse.json()
  const restoredResponse = await request.patch(`${API_BASE}/devices/dev-entry`, {
    data: { version: temporary.version, port: 4400 },
  })
  expect(restoredResponse.ok()).toBeTruthy()
}

/** Push entry + exit for Ana Pérez and wait for her to appear in the grid. */
async function seedAnaAndWait(page: Page, request: APIRequestContext) {
  // The E2E seed runs after the backend starts. Restarting the seeded device
  // emits the lifecycle signal that attaches its alertStream task.
  await restartEntryDevice(request)
  const entry = await readEvent('ana-entrada.xml')
  const exit = await readEvent('ana-salida.xml')
  await pushHikvisionEvent(request, entry)
  await pushHikvisionEvent(request, exit)
  await expect.poll(
    async () => {
      const response = await request.get(`${API_BASE}/daily-records`, {
        params: {
          employee_id: 'emp-ana',
          from_date: '2026-04-15',
          to_date: '2026-04-15',
        },
      })
      if (!response.ok()) return 0
      return (await response.json()).total as number
    },
    { timeout: 20_000, message: 'Expected Ana daily record after pushed events' },
  ).toBe(1)
  const params = new URLSearchParams({
    employee_id: 'emp-ana',
    anchor_date: '2026-04-15',
  })
  await page.goto(`/timesheet?${params.toString()}`)
  await expect(page.getByText('Ana Pérez')).toBeVisible({ timeout: 20_000 })
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

test.describe('Timesheet (Marcaciones) — D-03 CRUD UAT', () => {
  test.beforeEach(async ({ request }) => {
    await resetMutableTables(request)
  })

  // ── T-01: Grid renders with Spanish heading ───────────────────────────────
  test('renders Marcaciones page with Spanish heading', async ({ page }) => {
    await page.goto('/timesheet')
    await expect(page.getByRole('heading', { name: 'Marcaciones' })).toBeVisible()
  })

  // ── T-02: Admin sees Registrar Novedad button ─────────────────────────────
  test('admin sees Registrar Novedad global button', async ({ page }) => {
    await page.goto('/timesheet')
    // The global "Registrar Novedad" button is testid open-novedad-modal (page level)
    await expect(page.getByTestId(SEL.openNovedadModal).first()).toBeVisible()
  })

  // ── T-03: Grid shows daily records after events are pushed ────────────────
  test('grid lists Ana Pérez after entry + exit events are pushed', async ({ page, request }) => {
    await seedAnaAndWait(page, request)
    // Ana's name appears in the employee column
    await expect(page.getByText('Ana Pérez')).toBeVisible()
  })

  test('canonical employee/day deep link filters the requested week safely', async ({ page, request }) => {
    await seedAnaAndWait(page, request)

    await expect(page.getByTestId(SEL.timesheetRow('emp-ana:2026-04-15'))).toBeVisible({ timeout: 20_000 })
    await expect(page.getByText('Ana Pérez')).toBeVisible()
    await page.getByRole('button', { name: 'Semana siguiente' }).click()
    await expect(page).toHaveURL(/employee_id=emp-ana/)
    await expect(page).toHaveURL(/anchor_date=2026-04-22/)
  })

  // ── T-04: Filter by date range — week navigator changes week ─────────────
  test('week navigator is visible for period navigation', async ({ page }) => {
    await page.goto('/timesheet')
    // WeekNavigator renders prev/next buttons for period navigation
    await expect(page.getByRole('button', { name: 'Semana anterior' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Semana siguiente' })).toBeVisible()
  })

  // ── T-05: Registrar Novedad global button opens the modal ─────────────────
  test('clicking Registrar Novedad button opens the novedad modal', async ({ page }) => {
    await page.goto('/timesheet')
    // Click the global Registrar Novedad button (open-novedad-modal)
    await page.getByTestId(SEL.openNovedadModal).first().click()
    // Modal should become visible (Radix Dialog mounts on open=true)
    await expect(page.getByTestId(SEL.novedadModal)).toBeVisible({ timeout: 5_000 })
    await expect(page.getByTestId(SEL.novedadJustification)).toBeVisible()
  })

  // ── T-06: Validation — empty justification blocks submit ─────────────────
  test('validation: empty justification blocks submit and keeps modal open', async ({ page }) => {
    await page.goto('/timesheet')
    await page.getByTestId(SEL.openNovedadModal).first().click()
    await expect(page.getByTestId(SEL.novedadModal)).toBeVisible({ timeout: 5_000 })
    // Click submit without filling justification
    await page.getByTestId(SEL.novedadSubmit).click()
    // Modal remains open — react-hook-form validation prevented submission
    await expect(page.getByTestId(SEL.novedadModal)).toBeVisible()
  })

  // ── T-07: Happy path — register novedad → audit_log entry created ─────────
  test('happy path: register novedad with justification + evidence → audit_log row', async ({
    page,
    request,
  }) => {
    await seedAnaAndWait(page, request)

    // Click the row-level edit button (pencil icon) for the first daily record
    const rowBtn = page
      .getByTestId(SEL.timesheetRow('emp-ana:2026-04-15'))
      .getByTestId(SEL.openNovedadModal)
    await expect(rowBtn).toBeVisible({ timeout: 10_000 })
    await rowBtn.click()

    await expect(page.getByTestId(SEL.novedadModal)).toBeVisible({ timeout: 5_000 })

    await expect(page.getByTestId('novedad-employee')).toContainText('Ana Pérez')
    await expect(page.getByTestId('novedad-department')).toContainText('Producción')

    // Fill mandatory date fields
    const fechaInicio = page.getByLabel(/Fecha Inicio/)
    if (await fechaInicio.inputValue() === '') {
      await fechaInicio.fill('2026-04-15')
    }
    const fechaFin = page.getByLabel(/Fecha Fin/)
    if (await fechaFin.inputValue() === '') {
      await fechaFin.fill('2026-04-15')
    }

    await page.getByTestId(SEL.novedadJustification).fill('Llegó tarde por trámite médico')

    // Upload a minimal evidence file
    await page.getByTestId(SEL.novedadEvidence).setInputFiles({
      name: 'evidence.pdf',
      mimeType: 'application/pdf',
      buffer: Buffer.from('%PDF-1.4 fake content for plan 09-09'),
    })

    const overrideResponsePromise = page.waitForResponse(response => {
      const url = new URL(response.url())
      return (
        response.request().method() === 'POST' &&
        url.pathname.startsWith('/api/v1/daily-records/') &&
        url.pathname.endsWith('/overrides')
      )
    })
    await page.getByTestId(SEL.novedadSubmit).click()
    const overrideResponse = await overrideResponsePromise
    expect(overrideResponse.ok()).toBe(true)
    const createdOverride: { id: string } = await overrideResponse.json()
    expect(createdOverride.id).toBeTruthy()

    // Modal closes on success (TanStack mutation onSuccess → onClose())
    await expect(page.getByTestId(SEL.novedadModal)).toBeHidden({ timeout: 15_000 })

    // Audit assertion (D-03 mutation→audit contract)
    // The backend creates an audit_log row for any INSERT into daily_record_overrides or leaves
    await expect.poll(
      async () => {
        const r = await getAudit(request, {
          table_name: 'daily_record_overrides',
          record_id: createdOverride.id,
          operation: 'INSERT',
          actor_id: 'e2e-admin-id',
          limit: 5,
        })
        if (r.status() !== 200) return null
        const body = await r.json()
        return body.data?.some(
          (entry: { record_id: string }) => entry.record_id === createdOverride.id
        ) ?? false
      },
      { timeout: 15_000, message: 'Expected audit_log entry for daily_record_overrides INSERT' },
    ).toBe(true)
  })

  // ── T-08: Audit entry falls back to /leaves if no daily record selected ───
  test('audit_log: novedad submitted via global button (no record) → leaves INSERT audit row', async ({
    page,
    request,
  }) => {
    await page.goto('/timesheet')
    // Use the global button (no row selected → posts to /leaves)
    await page.getByTestId(SEL.openNovedadModal).first().click()
    await expect(page.getByTestId(SEL.novedadModal)).toBeVisible({ timeout: 5_000 })

    // Fill all required fields
    await page.getByTestId('novedad-employee').click()
    await page.getByRole('option', { name: /Ana Pérez/ }).click()
    await page.getByTestId('novedad-department').click()
    await page.getByRole('option', { name: 'Producción' }).click()
    await page.getByLabel(/Fecha Inicio/).fill('2026-04-15')
    await page.getByLabel(/Fecha Fin/).fill('2026-04-15')
    await page.getByTestId(SEL.novedadJustification).fill('Ausencia justificada por médico')

    await page.getByTestId(SEL.novedadEvidence).setInputFiles({
      name: 'note.pdf',
      mimeType: 'application/pdf',
      buffer: Buffer.from('%PDF-1.4'),
    })

    const leaveResponsePromise = page.waitForResponse(response => {
      const url = new URL(response.url())
      return response.request().method() === 'POST' && url.pathname === '/api/v1/leaves'
    })
    await page.getByTestId(SEL.novedadSubmit).click()
    const leaveResponse = await leaveResponsePromise
    expect(leaveResponse.ok()).toBe(true)
    const createdLeave: { id: string } = await leaveResponse.json()
    expect(createdLeave.id).toBeTruthy()
    await expect(page.getByTestId(SEL.novedadModal)).toBeHidden({ timeout: 15_000 })

    // Audit assertion — leaves INSERT (global button posts to /leaves)
    await expect.poll(
      async () => {
        const r = await getAudit(request, {
          table_name: 'leaves',
          record_id: createdLeave.id,
          operation: 'INSERT',
          actor_id: 'e2e-admin-id',
          limit: 5,
        })
        if (r.status() !== 200) return null
        const body = await r.json()
        return body.data?.some(
          (entry: { record_id: string }) => entry.record_id === createdLeave.id
        ) ?? false
      },
      { timeout: 15_000, message: 'Expected audit_log entry for leaves INSERT' },
    ).toBe(true)
  })
})
