/**
 * Login E2E spec — Plan 09-07 (D-01 Full UAT depth)
 *
 * This is the ONLY spec that uses UI-driven login (D-06 hybrid auth).
 * Every other spec uses fresh API-authenticated contexts from fixtures/auth.ts.
 *
 * Language: Spanish copy per the 2026-07-13 Phase 12 supersession of D-19.
 * Strings asserted: "Iniciar Sesión", "Usuario", "Contraseña", "Mostrar contraseña",
 *                   "Ocultar contraseña", "Usuario o contraseña inválidos.",
 *                   "Ocurrió un error. Inténtelo de nuevo.", "Este campo es obligatorio."
 *
 * NOTE: No test.use({ storageState }) — spec runs in fresh browser contexts.
 * NOTE: No page.waitForTimeout() calls — all waits are explicit Playwright auto-waits.
 */

import { test, expect, type Page } from '@playwright/test'
import { SEL } from './fixtures/selectors'

// ─────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────

/** Fill login form and click submit. */
async function fillAndSubmit(page: Page, username: string, password: string) {
  await page.getByLabel(SEL.loginUsername.name, { exact: true }).fill(username)
  await page.getByLabel(SEL.loginPassword.name, { exact: true }).fill(password)
  await page.getByRole(SEL.loginSubmit.role, {
    name: SEL.loginSubmit.name,
    exact: true,
  }).click()
}

/** Select the login error banner without matching Next.js' route announcer. */
function loginAlert(page: Page, message: string) {
  return page.getByRole('alert').filter({ hasText: message })
}

// ─────────────────────────────────────────────────────────────────
// Test suite
// ─────────────────────────────────────────────────────────────────

test.describe('Inicio de sesión (UAT completo, contrato en español de Phase 12)', () => {

  // ── T-01: Form renders with expected Spanish labels, heading, and locale ─
  test('renderiza el formulario con copy en español y lang es-VE', async ({ page }) => {
    await page.goto('/login')

    await expect(page.locator('html')).toHaveAttribute('lang', 'es-VE')

    // Page heading
    await expect(page.getByRole(SEL.loginHeading.role, {
      name: SEL.loginHeading.name,
      exact: true,
    })).toBeVisible()

    // Form fields via accessible label
    await expect(page.getByLabel(SEL.loginUsername.name, { exact: true })).toBeVisible()
    await expect(page.getByLabel(SEL.loginPassword.name, { exact: true })).toBeVisible()

    // Submit button
    await expect(page.getByRole(SEL.loginSubmit.role, {
      name: SEL.loginSubmit.name,
      exact: true,
    })).toBeVisible()
  })

  // ── T-02: Happy path — admin logs in and lands on / or /dashboard ────────
  test('flujo exitoso: admin inicia sesión y llega al dashboard', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // After successful login the app redirects to the landing route.
    await expect(page).toHaveURL(/\/$|\/dashboard/, { timeout: 10_000 })
  })

  // ── T-03: Invalid credentials → 401 → error message, stays on /login ────
  test('credenciales inválidas → 401 → error genérico en español', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'wrong-password-xyz')

    // Error banner (role="alert" per login/page.tsx markup)
    await expect(loginAlert(page, 'Usuario o contraseña inválidos.')).toHaveText(
      'Usuario o contraseña inválidos.',
    )

    // User remains on the login page
    await expect(page).toHaveURL(/\/login/)
  })

  // ── T-04: Empty username blocked by Zod — stays on /login ───────────────
  test('validación: usuario vacío permanece en /login y muestra el error exacto', async ({ page }) => {
    await page.goto('/login')

    // Leave username blank, fill only password, try to submit
    await page.getByLabel(SEL.loginPassword.name, { exact: true }).fill('e2e-admin-pass')
    await page.getByRole(SEL.loginSubmit.role, {
      name: SEL.loginSubmit.name,
      exact: true,
    }).click()

    // User stays on /login (no network call should be made)
    await expect(page).toHaveURL(/\/login/)

    // react-hook-form renders FormMessage as the validation error
    // loginSchema: username uses the authoritative Spanish required-field copy.
    await expect(page.locator('#login-username-error')).toHaveText(
      'Este campo es obligatorio.',
      { timeout: 3_000 },
    )
  })

  // ── T-05: Empty password blocked by Zod — stays on /login ───────────────
  test('validación: contraseña vacía permanece en /login y muestra el error exacto', async ({ page }) => {
    await page.goto('/login')

    // Leave password blank, fill only username, try to submit
    await page.getByLabel(SEL.loginUsername.name, { exact: true }).fill('e2e_admin')
    await page.getByRole(SEL.loginSubmit.role, {
      name: SEL.loginSubmit.name,
      exact: true,
    }).click()

    // User stays on /login
    await expect(page).toHaveURL(/\/login/)

    // loginSchema: password uses the authoritative Spanish required-field copy.
    await expect(page.locator('#login-password-error')).toHaveText(
      'Este campo es obligatorio.',
      { timeout: 3_000 },
    )
  })

  // ── T-06: Password visibility toggle (Eye ↔ EyeOff) ─────────────────────
  test('alterna la visibilidad de la contraseña con nombres accesibles en español', async ({ page }) => {
    await page.goto('/login')

    const pwd = page.getByLabel(SEL.loginPassword.name, { exact: true })
    await pwd.fill('secret123')

    // Initially the field type should be "password"
    await expect(pwd).toHaveAttribute('type', 'password')

    // Click the Spanish "Mostrar contraseña" toggle (aria-label from login/page.tsx)
    await page.getByRole(SEL.loginShowPassword.role, {
      name: SEL.loginShowPassword.name,
      exact: true,
    }).click()

    // After toggle, type should change to "text"
    await expect(pwd).toHaveAttribute('type', 'text')

    // Toggle back — "Ocultar contraseña" is now the label
    await page.getByRole(SEL.loginHidePassword.role, {
      name: SEL.loginHidePassword.name,
      exact: true,
    }).click()
    await expect(pwd).toHaveAttribute('type', 'password')
  })

  // ── T-07: Session persists across browser refresh ────────────────────────
  test('la sesión persiste tras recargar (refresh cookie reemite access_token)', async ({ page }) => {
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
  test('múltiples pestañas: la segunda comparte la sesión del mismo contexto', async ({ context, page }) => {
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
  test('RBAC: viewer inicia sesión y no ve comandos admin en /devices', async ({ page }) => {
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
  test('respeta redirect: ?redirect=/employees navega allí tras iniciar sesión', async ({ page }) => {
    await page.goto('/login?redirect=/employees')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // Should land on /employees (safeRedirect accepts this relative path)
    await expect(page).toHaveURL(
      (url) => url.pathname === '/employees',
      { timeout: 10_000 },
    )
  })

  // ── T-11: Open-redirect sanitized (CR-02 mitigation T-09-09) ────────────
  test('sanea open redirect: "//evil.com" usa el destino seguro', async ({ page }) => {
    await page.goto('/login?redirect=//evil.com')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    // safeRedirect() in login/page.tsx rejects protocol-relative URLs.
    // The app must stay on the expected origin and land on the safe fallback.
    await expect(page).toHaveURL(
      (url) =>
        url.origin === 'http://localhost:3001'
        && (url.pathname === '/' || url.pathname === '/dashboard'),
      { timeout: 10_000 },
    )
  })

  // ── T-12: Non-existent username → 401 (same generic error, no user enumeration) ─
  test('usuario inexistente → mismo error genérico (sin enumeración)', async ({ page }) => {
    await page.goto('/login')
    await fillAndSubmit(page, 'user_that_does_not_exist_xyz', 'any-password-123')

    // Backend must return 401 for unknown users without revealing "user not found"
    // (T-01-19: generic error per CLAUDE.md RBAC architecture — security requirement).
    await expect(loginAlert(page, 'Usuario o contraseña inválidos.')).toHaveText(
      'Usuario o contraseña inválidos.',
    )
    await expect(page).toHaveURL(/\/login/)
  })

  // ── T-13: Non-401 failure → generic Spanish fallback ────────────────────
  test('error no-401 → fallback genérico en español', async ({ page }) => {
    await page.route('**/api/v1/auth/login', async (route) => {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({ error: { code: 'SERVICE_UNAVAILABLE' } }),
      })
    })
    await page.goto('/login')
    await fillAndSubmit(page, 'e2e_admin', 'e2e-admin-pass')

    await expect(loginAlert(page, 'Ocurrió un error. Inténtelo de nuevo.')).toHaveText(
      'Ocurrió un error. Inténtelo de nuevo.',
    )
    await expect(page).toHaveURL(/\/login/)
  })

})
