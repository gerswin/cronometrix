/**
 * Presence E2E spec — Task 8 (Plan 2026-08-05-presencia-y-deficit-de-horas)
 *
 * Covers: the dashboard's presence table (tabs "Dentro ahora" / "Asistieron
 * hoy") renders real attendance data, and department scope isolation (H-11)
 * holds for a supervisor confined to a single department.
 *
 * NOTE on scope (read before touching this file): the seeded `e2e_supervisor`
 * (backend/src/bin/seed_e2e.rs) is inserted WITHOUT a department_id, which per
 * D1 (backend/src/auth/scope.rs `ActorScope::from_claims`) makes it
 * `ActorScope::Unscoped` — org-wide, not confined to a department. Reusing
 * `e2e_supervisor` for an isolation assertion (as the task-8-brief skeleton
 * sketches) would silently pass without ever exercising the department
 * filter. Instead, the isolation test below creates a *scoped* supervisor via
 * the admin-only `POST /users` endpoint (`department_id: 'dept-prod'`, a
 * department already seeded by seed_e2e.rs) so the assertion is meaningful.
 * Users are not truncated by `__test_reset` (backend/src/test_reset/mod.rs),
 * so creation is written to be idempotent across reruns.
 *
 * Language: Spanish copy per D-19 (dashboard is Spanish locale).
 */

import { test, expect } from './fixtures/auth'
import type { APIRequestContext, Browser, Page } from '@playwright/test'
import * as fs from 'node:fs/promises'
import * as path from 'node:path'
import { API_BASE, resetMutableTables, pushHikvisionEvent } from './fixtures/api'

// Login must hit the same host fixtures/auth.ts uses for its login POST
// (`localhost`, not `127.0.0.1`): the refresh-token cookie is host-scoped,
// and the frontend itself calls the API at `localhost` (see
// NEXT_PUBLIC_API_URL in playwright.config.ts). Logging in against
// `127.0.0.1` sets the cookie on a different origin than the one the
// frontend's refresh call later reads it from, so the session never
// authenticates and the page silently redirects to /login.
const AUTH_API_BASE = 'http://localhost:4001/api/v1'

// ---------------------------------------------------------------------------
// Helpers (mirrors frontend/e2e/dashboard.spec.ts patterns)
// ---------------------------------------------------------------------------

/** Read a canned Hikvision event XML from fixtures. */
async function readEvent(filename: string): Promise<string> {
  return fs.readFile(
    path.resolve(__dirname, 'fixtures/hikvision-events', filename),
    'utf8',
  )
}

/** Today's calendar date in the frozen E2E timezone (D-20: America/Caracas, UTC-04:00, no DST). */
function todayCaracas(): string {
  return new Intl.DateTimeFormat('en-CA', { timeZone: 'America/Caracas' }).format(new Date())
}

/**
 * Build a unique entry event for `employeeCode`/`name` from an existing
 * fixture's XML shape, dated "today" so it lands in /presence/today.
 * `index` keeps concurrent pushes' dateTime unique.
 */
function buildEntryEvent(
  base: string,
  employeeCode: string,
  name: string,
  index: number,
): string {
  const minute = String(8 + (index % 40)).padStart(2, '0')
  const second = String(index % 60).padStart(2, '0')
  return base
    .replace(
      /<dateTime>[^<]+<\/dateTime>/,
      `<dateTime>${todayCaracas()}T08:${minute}:${second}-04:00</dateTime>`,
    )
    .replace(
      /<employeeNoString>[^<]+<\/employeeNoString>/,
      `<employeeNoString>${employeeCode}</employeeNoString>`,
    )
    .replace(/<name>[^<]+<\/name>/, `<name>${name}</name>`)
}

/** Cycle the entry device's port so the mock Hikvision server (re)registers
 * its webhook host — required before any event pushed through it will reach
 * the backend. Copied from dashboard.spec.ts's identical helper. */
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

/**
 * Create a supervisor confined to `departmentId` via the admin-only
 * `POST /users` endpoint. Idempotent: a 409 (username already taken by a
 * prior run — users survive `__test_reset`) is treated as already-created.
 */
async function ensureScopedSupervisor(
  request: APIRequestContext,
  username: string,
  password: string,
  departmentId: string,
): Promise<void> {
  const res = await request.post(`${API_BASE}/users`, {
    data: {
      username,
      full_name: 'E2E Scoped Supervisor',
      role: 'supervisor',
      password,
      department_id: departmentId,
    },
  })
  if (res.status() === 201) return
  expect(res.status(), `unexpected status creating scoped supervisor: ${await res.text()}`).toBe(409)
}

/** Log in as an arbitrary user in a fresh browser context and return an
 * authenticated page, mirroring fixtures/auth.ts's newRoleSession (which is
 * restricted to the fixed ROLE_CREDENTIALS map). */
async function loginAsPage(
  browser: Browser,
  username: string,
  password: string,
): Promise<Page> {
  const context = await browser.newContext()
  const resp = await context.request.post(`${AUTH_API_BASE}/auth/login`, {
    data: { username, password },
  })
  expect(resp.ok(), `login failed for ${username}: ${resp.status()}`).toBeTruthy()
  return context.newPage()
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

const SCOPED_SUPERVISOR_USER = 'e2e-presence-scoped-supervisor'
const SCOPED_SUPERVISOR_PASS = 'e2e-scoped-supervisor-pass'

test.describe('Presencia del dashboard y aislamiento por departamento', () => {
  test.beforeEach(async ({ request }) => {
    await resetMutableTables(request)
  })

  test('el dashboard muestra pestañas de presencia y la tabla de asistencia', async ({
    page,
    request,
  }) => {
    const anaXml = await readEvent('ana-entrada.xml')
    await restartEntryDevice(request)
    const pushResp = await pushHikvisionEvent(
      request,
      buildEntryEvent(anaXml, 'EMP001', 'Ana Pérez', 0),
    )
    expect(pushResp.ok()).toBeTruthy()

    // The dashboard's presence query fetches once on mount (refetchInterval
    // is 60s — too slow to rely on for this assertion). Wait for the backend
    // to finish the async recompute (attendance_events -> daily_records)
    // BEFORE navigating, so the first fetch already has the data instead of
    // racing a recompute that may still be in flight.
    await expect(async () => {
      const res = await request.get(`${API_BASE}/presence/today`)
      expect(res.ok()).toBeTruthy()
      const body = await res.json()
      expect(body.data.length).toBeGreaterThanOrEqual(1)
    }).toPass({ timeout: 15_000 })

    await page.goto('/dashboard')

    await expect(page.getByTestId('presence-tab-inside')).toBeVisible()
    await expect(page.getByTestId('presence-tab-attended')).toBeVisible()

    // "Dentro ahora" is the default tab; wait for the pushed event to land
    // (backend alertStream consumer processes it asynchronously).
    const table = page.getByRole('table')
    await expect(table).toBeVisible({ timeout: 15_000 })
    await expect(table.locator('thead th')).toHaveText(['Empleado', 'Entrada', 'Departamento'])
    await expect(table.locator('tbody')).toContainText('Ana Pérez')

    await page.getByTestId('presence-tab-attended').click()
    await expect(page.getByRole('table')).toBeVisible()
    await expect(page.getByRole('table').locator('tbody')).toContainText('Ana Pérez')
  })

  test('un supervisor confinado a un departamento no ve empleados de otro', async ({
    page: _adminPage,
    request,
    browser,
  }) => {
    // Seed cross-department attendance: Ana (dept-prod) and María (dept-admin).
    const anaXml = await readEvent('ana-entrada.xml')
    await restartEntryDevice(request)
    const anaPush = await pushHikvisionEvent(
      request,
      buildEntryEvent(anaXml, 'EMP001', 'Ana Pérez', 1),
    )
    expect(anaPush.ok()).toBeTruthy()
    const mariaPush = await pushHikvisionEvent(
      request,
      buildEntryEvent(anaXml, 'EMP003', 'María López', 2),
    )
    expect(mariaPush.ok()).toBeTruthy()

    // Wait for both to land in /presence/today (admin sees everyone) before
    // scoping the assertion to a confined supervisor.
    await expect(async () => {
      const res = await request.get(`${API_BASE}/presence/today`)
      expect(res.ok()).toBeTruthy()
      const body = await res.json()
      expect(body.data.length).toBeGreaterThanOrEqual(2)
    }).toPass({ timeout: 15_000 })

    await ensureScopedSupervisor(
      request,
      SCOPED_SUPERVISOR_USER,
      SCOPED_SUPERVISOR_PASS,
      'dept-prod',
    )

    const scopedPage = await loginAsPage(browser, SCOPED_SUPERVISOR_USER, SCOPED_SUPERVISOR_PASS)
    try {
      await scopedPage.goto('/dashboard')
      const table = scopedPage.getByRole('table')
      await expect(table).toBeVisible({ timeout: 15_000 })

      // Every visible row must belong to dept-prod ("Producción") — none from
      // dept-admin ("Administración"), where María López was seeded.
      // La aserción es incondicional a propósito: con `if (unique.size === 1)`
      // un tablero de cero filas pasaba el test sin probar nada.
      await expect(table.getByText('Ana Pérez')).toHaveCount(1)
      const departments = await table.locator('tbody tr td:nth-child(3)').allTextContents()
      const unique = new Set(departments.map(d => d.trim()).filter(Boolean))
      expect([...unique]).toEqual(['Producción'])
      await expect(table.getByText('María López')).toHaveCount(0)
    } finally {
      await scopedPage.context().close()
    }
  })
})
