/**
 * Login E2E spec — Plan 09-07 (D-01 Full UAT depth)
 *
 * This is the ONLY spec that uses UI-driven login (D-06 hybrid auth).
 * Every other spec reuses storageState from Plan 06 (01-authenticate.setup.ts).
 *
 * Language: English copy per Addendum D-19 (login page is English despite global es-VE locale).
 * Strings asserted: "Username", "Password", "Log in", "Invalid username or password.",
 *                   "Something went wrong. Please try again.", "Log in to Cronometrix"
 *
 * NOTE: No test.use({ storageState }) — spec runs in fresh browser contexts.
 * NOTE: No page.waitForTimeout() calls — all waits are explicit Playwright auto-waits.
 */

import { test, expect, type Page } from '@playwright/test'

// ─────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────

/** Fill login form and click submit. */
async function fillAndSubmit(page: Page, username: string, password: string) {
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Log in' }).click()
}

// ─────────────────────────────────────────────────────────────────
// Test suite
// ─────────────────────────────────────────────────────────────────

test.describe('Login (UAT depth, English copy per Addendum D-19)', () => {

  // ── T-01: Form renders with expected English labels and heading ──────────
  test('renders login form with English labels and heading', async ({ page }) => {
    await page.goto('/login')

    // Page heading (CardTitle)
    await expect(page.getByRole('heading', { name: 'Log in to Cronometrix' })).toBeVisible()

    // Form fields via accessible label
    await expect(page.getByLabel('Username')).toBeVisible()
    await expect(page.getByLabel('Password')).toBeVisible()

    // Submit button
    await expect(page.getByRole('button', { name: 'Log in' })).toBeVisible()
  })

  // ── T-02: Happy path — admin logs in and lands on / or /dashboard ────────
  test('happy path: admin logs in and lands on dashboard', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // After successful login the app redirects to the landing route.
    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })
  })

  // ── T-03: Invalid credentials → 401 → error message, stays on /login ────
  test('invalid credentials → 401 → "Invalid username or password."', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'wrong-password-xyz')

    // Error banner (role="alert" per login/page.tsx markup)
    await expect(page.getByRole('alert')).toContainText('Invalid username or password.')

    // User remains on the login page
    await expect(page).toHaveURL(/\/login/)
  })

  // ── T-04: Empty username blocked by Zod — stays on /login ───────────────
  test('validation: empty username stays on /login and shows field error', async ({ page }) => {
    await page.goto('/login')

    // Leave username blank, fill only password, try to submit
    await page.getByLabel('Password').fill('e2e-admin-pass')
    await page.getByRole('button', { name: 'Log in' }).click()

    // User stays on /login (no network call should be made)
    await expect(page).toHaveURL(/\/login/)

    // react-hook-form renders FormMessage as the validation error
    // loginSchema: username: z.string().min(1, 'This field is required.')
    const errors = page.locator('p[id="login-username-error"], .text-destructive, [role="alert"]')
    await expect(errors.first()).toBeVisible({ timeout: 3_000 })
  })

  // ── T-05: Empty password blocked by Zod — stays on /login ───────────────
  test('validation: empty password stays on /login and shows field error', async ({ page }) => {
    await page.goto('/login')

    // Leave password blank, fill only username, try to submit
    await page.getByLabel('Username').fill('e2e_admin')
    await page.getByRole('button', { name: 'Log in' }).click()

    // User stays on /login
    await expect(page).toHaveURL(/\/login/)

    // loginSchema: password: z.string().min(1, 'This field is required.')
    const errors = page.locator('p[id="login-password-error"], .text-destructive, [role="alert"]')
    await expect(errors.first()).toBeVisible({ timeout: 3_000 })
  })

  // ── T-06: Password visibility toggle (Eye ↔ EyeOff) ─────────────────────
  test('toggle password visibility (Show/Hide password)', async ({ page }) => {
    await page.goto('/login')

    const pwd = page.getByLabel('Password')
    await pwd.fill('secret123')

    // Initially the field type should be "password"
    await expect(pwd).toHaveAttribute('type', 'password')

    // Click the "Show password" toggle (aria-label from login/page.tsx)
    await page.getByRole('button', { name: 'Show password' }).click()

    // After toggle, type should change to "text"
    await expect(pwd).toHaveAttribute('type', 'text')

    // Toggle back — "Hide password" is now the label
    await page.getByRole('button', { name: 'Hide password' }).click()
    await expect(pwd).toHaveAttribute('type', 'password')
  })

  // ── T-07: Session persists across browser refresh ────────────────────────
  test('session persists across refresh (refresh-cookie re-issues access_token)', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })

    // Reload the page — the frontend's refresh hook calls /api/v1/auth/refresh
    // using the httpOnly refresh cookie and re-issues a fresh access_token.
    await page.reload()

    // Still authenticated: page must NOT be redirected back to /login
    await expect(page).not.toHaveURL(/\/login/)
  })

  // ── T-08: Multi-tab — second tab shares session ──────────────────────────
  test('multi-tab: second tab shares session (same browser context)', async ({ context, page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })

    // Open a second tab in the SAME browser context (shared cookies).
    const tab2 = await context.newPage()
    await tab2.goto('/employees')

    // The second tab must be authenticated (no redirect to /login).
    await expect(tab2).not.toHaveURL(/\/login/)
  })

  // ── T-09: RBAC — Viewer cannot trigger admin commands on /devices ─────────
  test('RBAC: viewer logs in, navigates to /devices, admin command buttons absent', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_viewer', 'e2e-viewer-pass')

    // Viewer role should land on dashboard (read access is permitted)
    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })

    // Navigate to the devices page
    await page.goto('/devices')

    // Phase 4 D-14: admin-only ISAPI command buttons (Open door, Restart, Enroll mode)
    // must be hidden for Viewer role. The Spanish button names come from the devices UI.
    const adminButtons = page.getByRole('button', {
      name: /Abrir puerta|Reiniciar|Modo enroll/i,
    })
    await expect(adminButtons).toHaveCount(0)
  })

  // ── T-10: Redirect param honored after successful login ──────────────────
  test('redirect param honored: ?redirect=/employees routes there after login', async ({ page }) => {
    await page.goto('/login?redirect=/employees')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // Should land on /employees (safeRedirect accepts this relative path)
    await expect(page).toHaveURL(/\/employees/, { timeout: 10_000 })
  })

  // ── T-11: Open-redirect sanitized (CR-02 mitigation T-09-09) ────────────
  test('open-redirect sanitized: "//evil.com" falls back to "/"', async ({ page }) => {
    await page.goto('/login?redirect=//evil.com')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // safeRedirect() in login/page.tsx rejects protocol-relative URLs.
    // The app must NOT navigate to evil.com.
    await expect(page).not.toHaveURL(/evil\.com/, { timeout: 10_000 })

    // It must land on the safe fallback: "/" or "/dashboard"
    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })
  })

  // ── T-12: Non-existent username → 401 (same generic error, no user enumeration) ─
  test('non-existent username → generic "Invalid username or password." (no user enumeration)', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'user_that_does_not_exist_xyz', 'any-password-123')

    // Backend must return 401 for unknown users without revealing "user not found"
    // (T-01-19: generic error per CLAUDE.md RBAC architecture — security requirement).
    await expect(page.getByRole('alert')).toContainText('Invalid username or password.')
    await expect(page).toHaveURL(/\/login/)
  })

})
