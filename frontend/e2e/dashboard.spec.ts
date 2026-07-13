/**
 * Dashboard E2E spec — Plan 09-08 (D-02 UAT depth)
 *
 * Covers: KPI tiles (4), donut chart, ring buffer 20-event cap, photo fallback,
 * SSE disconnect banner DOM attachment, empty state.
 *
 * Authenticated tests use a fresh admin context per test.
 * test.beforeEach resets mutable tables for determinism (D-12).
 *
 * SSE banner: per RESEARCH §Pitfall 5, we assert DOM attachment only.
 * Full disconnect simulation requires a backend test-mode endpoint; deferred.
 *
 * Language: Spanish copy per D-19 (dashboard is Spanish locale).
 */

import { test, expect } from './fixtures/auth'
import type { APIRequestContext, Page } from '@playwright/test'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { API_BASE, resetMutableTables, pushHikvisionEvent } from './fixtures/api'
import { SEL } from './fixtures/selectors'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Read a canned Hikvision event XML from fixtures. */
async function readEvent(filename: string): Promise<string> {
  return fs.readFile(
    path.resolve(__dirname, 'fixtures/hikvision-events', filename),
    'utf8',
  )
}

/**
 * Mutate the dateTime field in an event XML to make it unique.
 * Keeps event in valid ISO-8601 range while varying seconds + minutes.
 */
function mutateDateTime(xml: string, index: number): string {
  const minutes = String(index % 60).padStart(2, '0')
  const seconds = String(index % 60).padStart(2, '0')
  // Replace the dateTime content with a unique timestamp
  return xml.replace(
    /<dateTime>[^<]+<\/dateTime>/,
    `<dateTime>2026-04-15T08:${minutes}:${seconds}-04:00</dateTime>`,
  )
}

async function openDashboardWithSSE(page: Page): Promise<void> {
  const streamReady = page.waitForResponse((response) =>
    response.url().includes('/api/v1/events/stream?token=') &&
    response.status() === 200,
  )
  await page.goto('/dashboard')
  await streamReady
}

async function restartEntryDevice(request: APIRequestContext): Promise<void> {
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

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

test.describe('Dashboard (D-02 UAT depth)', () => {
  test.beforeEach(async ({ request }) => {
    await resetMutableTables(request)
  })

  // ── T-01: 4 KPI tiles render with Spanish labels ─────────────────────────
  test('renders 4 KPI tiles with Spanish labels', async ({ page }) => {
    await page.goto('/dashboard')

    // All 4 tiles visible
    await expect(page.getByTestId(SEL.kpiPresentes)).toBeVisible()
    await expect(page.getByTestId(SEL.kpiRetraso)).toBeVisible()
    await expect(page.getByTestId(SEL.kpiDispositivos)).toBeVisible()
    await expect(page.getByTestId(SEL.kpiAlertas)).toBeVisible()

    // Spanish copy (load-bearing per D-19)
    await expect(page.getByText('Empleados Presentes')).toBeVisible()
    await expect(page.getByText('% Retraso Hoy')).toBeVisible()
    await expect(page.getByText('Dispositivos Activos')).toBeVisible()
    await expect(page.getByText('Alertas Diurnas')).toBeVisible()
  })

  // ── T-02: KPI "Empleados Presentes" increments after entry events ─────────
  test('Empleados Presentes shows non-zero count after entry events are pushed', async ({
    page,
    request,
  }) => {
    // Push entry events for two employees seeded by Plan 03
    const anaXml = await readEvent('ana-entrada.xml')
    const luisXml = await readEvent('luis-entrada.xml')
    await restartEntryDevice(request)
    await pushHikvisionEvent(request, anaXml)
    await pushHikvisionEvent(request, luisXml)

    await page.goto('/dashboard')

    // The dashboard queries /daily-records (staleTime 60s) and aggregates.
    // Use a generous timeout because the backend alertStream consumer may take
    // a few seconds to process the events and update daily records.
    await expect(page.getByTestId(SEL.kpiPresentes)).toContainText(/[1-9]/, {
      timeout: 15_000,
    })
  })

  // ── T-03: Donut chart renders with Spanish section heading ────────────────
  test('renders Distribución por Depto donut chart', async ({ page }) => {
    await page.goto('/dashboard')

    // Container is always rendered (empty state shows "Sin datos para hoy")
    await expect(page.getByTestId(SEL.donutDept)).toBeVisible()

    // Section heading (Spanish per D-19)
    await expect(page.getByText(/Distribución por Depto/i)).toBeVisible()
  })

  // ── T-04: Ring buffer caps at 20 most-recent events ──────────────────────
  test('ring buffer shows at most 20 rows after 25 events are pushed', async ({
    page,
    request,
  }) => {
    const anaXml = await readEvent('ana-entrada.xml')

    // The broadcast channel has no replay, so establish the real SSE response
    // before injecting the events that this page must render.
    await restartEntryDevice(request)
    await openDashboardWithSSE(page)

    // Push 25 events with unique timestamps to avoid deduplication
    for (let i = 0; i < 25; i++) {
      const mutated = mutateDateTime(anaXml, i)
      await pushHikvisionEvent(request, mutated)
    }

    // Ring buffer container must be visible
    const ringBuffer = page.getByTestId(SEL.ringBuffer)
    await expect(ringBuffer).toBeVisible()

    // SSE delivers events in real-time; after page load the ring may still be
    // filling. Wait for it to stabilise at the 20-event cap.
    const rows = ringBuffer.locator('[data-testid^="ring-row-"]')
    await expect(rows).toHaveCount(20, { timeout: 20_000 })
  })

  // ── T-05: Photo fallback shows initials avatar when no photo available ────
  test('photo fallback renders initials avatar when event has no photo', async ({
    page,
    request,
  }) => {
    // Push one event (current XML fixtures include no JPEG part → has_photo=false)
    const xml = await readEvent('ana-entrada.xml')
    await restartEntryDevice(request)
    await openDashboardWithSSE(page)
    await pushHikvisionEvent(request, xml)

    // Wait for the SSE event to appear in the ring buffer
    const ringBuffer = page.getByTestId(SEL.ringBuffer)
    await expect(ringBuffer.locator('[data-testid^="ring-row-"]').first()).toBeVisible({
      timeout: 15_000,
    })

    // Without a photo, EventAvatar renders the initials fallback
    await expect(page.getByTestId('photo-fallback').first()).toBeVisible()
  })

  // ── T-06: SSE disconnect banner is in DOM (eventual-consistency assertion) ─
  test('SSE disconnect banner is attached to DOM (hidden when connected)', async ({
    page,
  }) => {
    await page.goto('/dashboard')

    // Initially the banner is hidden — SSE is connected on fresh page load.
    const banner = page.getByTestId(SEL.sseBanner)

    // Per RESEARCH §Pitfall 5: without a backend test-mode endpoint to kill
    // the SSE stream, the strongest non-flaky assertion is "element exists in
    // DOM." The HTML `hidden` attribute controls visibility; the element is
    // always rendered so Playwright can always locate it.
    await expect(banner).toBeAttached()

    // Confirm the banner is hidden (not visible) while the connection is live.
    // Using toBeHidden() is correct — the element has hidden attr but IS attached.
    await expect(banner).toBeHidden()
  })

  // ── T-07: Empty state — ring buffer shows no rows before any events ───────
  test('empty state: ring buffer has no rows when no events exist', async ({ page }) => {
    // beforeEach already called resetMutableTables; no events are in the DB.
    await page.goto('/dashboard')

    const ringBuffer = page.getByTestId(SEL.ringBuffer)
    await expect(ringBuffer).toBeVisible()

    // No ring-row-* items should exist
    const rows = ringBuffer.locator('[data-testid^="ring-row-"]')
    await expect(rows).toHaveCount(0)
  })
})
