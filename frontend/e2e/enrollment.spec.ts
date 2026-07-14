/**
 * Facial enrollment E2E contract against mock_hikvision.
 *
 * This is deterministic application-contract evidence only. It does not claim
 * validation against a physical Hikvision reader, digest auth, or real firmware.
 */
import { chromium, expect, test, type Request } from '@playwright/test'
import { SEL } from './fixtures/selectors'

const API_BASE = 'http://localhost:4001/api/v1'
const MOCK_ADMIN_BASE = 'http://127.0.0.1:4401/admin'

const SEEDED_ENROLLMENT_ID = 'e2e-seed-enrollment'
const ENTRY_DEVICE_ID = 'dev-entry'
const EXIT_DEVICE_ID = 'dev-exit'
const NEW_EMPLOYEE_ID = 'emp-jose'

// Linux CI has no hardware GPU, so Chromium must opt into ANGLE's software
// renderer. On macOS, ANGLE auto-selects the native backend; forcing
// SwiftShader there can stall TinyFaceDetector inference.
const CHROMIUM_GRAPHICS_ARGS = process.platform === 'linux'
  ? [
      '--use-gl=angle',
      '--use-angle=swiftshader',
      '--enable-webgl',
      '--ignore-gpu-blocklist',
      '--enable-unsafe-swiftshader',
    ]
  : [
      '--use-gl=angle',
      '--enable-webgl',
      '--ignore-gpu-blocklist',
    ]

test.describe.configure({ mode: 'serial' })

test('mock_hikvision: resumes, captures, partially syncs, and retries enrollment', async () => {
  test.setTimeout(60_000)

  // Keep the production TinyFaceDetector unchanged while selecting a graphics
  // backend that is usable on both developer Macs and GPU-less Linux runners.
  const browser = await chromium.launch({
    headless: true,
    args: CHROMIUM_GRAPHICS_ARGS,
  })
  const context = await browser.newContext({
    baseURL: 'http://localhost:3001',
    timezoneId: 'America/Caracas',
    locale: 'es-VE',
    viewport: { width: 1440, height: 900 },
  })
  // Chromium does not expose postDataBuffer() for a FormData containing a
  // Blob. Observe the real FormData.append calls while forwarding the exact
  // same arguments. addInitScript keeps the observer across the reload that
  // exercises refresh-token rotation.
  await context.addInitScript(() => {
    type EnrollmentMultipart = Record<
      string,
      string | { kind: 'blob'; type: string; size: number }
    >
    const observedByForm = new WeakMap<FormData, EnrollmentMultipart>()
    const originalAppend = FormData.prototype.append
    FormData.prototype.append = function patchedAppend(
      name: string,
      value: string | Blob,
      fileName?: string,
    ) {
      const observed = observedByForm.get(this) ?? {}
      observedByForm.set(this, observed)
      observed[name] = typeof value === 'string'
        ? value
        : { kind: 'blob', type: value.type, size: value.size }
      ;(window as typeof window & { __enrollmentMultipart?: EnrollmentMultipart })
        .__enrollmentMultipart = observed
      return fileName === undefined
        ? Reflect.apply(originalAppend, this, [name, value])
        : Reflect.apply(originalAppend, this, [name, value, fileName])
    }
  })
  try {
    const scriptReset = await context.request.post(
      `${MOCK_ADMIN_BASE}/reset-enrollment-script`,
    )
    expect(scriptReset.ok()).toBeTruthy()
    const logReset = await context.request.post(`${MOCK_ADMIN_BASE}/clear-recv-log`)
    expect(logReset.ok()).toBeTruthy()

    const login = await context.request.post(`${API_BASE}/auth/login`, {
      data: {
        username: 'e2e_enrollment_admin',
        password: 'e2e-enrollment-pass',
      },
    })
    expect(login.status()).toBe(200)
    const loginBody = await login.json()
    expect(loginBody.access_token).toBeTruthy()
    const authHeaders = { Authorization: `Bearer ${loginBody.access_token}` }

    // The affected cross-spec run executes devices.spec first, which leaves its
    // created device active because __test_reset intentionally does not wipe
    // inventory. Deactivate only non-seeded devices so fan-out stays the stable
    // entry/exit pair and port 4402 remains available to later stream tests.
    const activeDevicesResponse = await context.request.get(
      `${API_BASE}/devices?status=active&limit=100`,
      { headers: authHeaders },
    )
    expect(activeDevicesResponse.ok()).toBeTruthy()
    const activeDevices: Array<{ id: string; version: number }> =
      (await activeDevicesResponse.json()).data
    for (const device of activeDevices) {
      if (device.id === ENTRY_DEVICE_ID || device.id === EXIT_DEVICE_ID) continue
      const deactivate = await context.request.patch(`${API_BASE}/devices/${device.id}`, {
        headers: authHeaders,
        data: { version: device.version, status: 'inactive' },
      })
      expect(deactivate.ok(), `failed to isolate extra E2E device ${device.id}`).toBeTruthy()
    }

    const page = await context.newPage()
    const graphics = await page.evaluate(() => {
      const canvas = document.createElement('canvas')
      return {
        webgl: Boolean(canvas.getContext('webgl')),
        webgl2: Boolean(canvas.getContext('webgl2')),
      }
    })
    expect(
      graphics.webgl || graphics.webgl2,
      `TinyFaceDetector requires WebGL in headless Chromium: ${JSON.stringify(graphics)}`,
    ).toBeTruthy()
    await page.goto('/enrollment')
    await expect(page.getByTestId(SEL.enrollmentPage)).toBeVisible()

    const seededRow = page.getByTestId(SEL.enrollmentRow(SEEDED_ENROLLMENT_ID))
    await expect(seededRow).toContainText('Carmen Silva')
    await expect(seededRow).toContainText('EMP005')
    await expect(seededRow).toContainText('0/2 dispositivos')

    await page.reload()
    await expect(page.getByTestId(SEL.enrollmentRow(SEEDED_ENROLLMENT_ID))).toContainText(
      'Carmen Silva',
    )

    let captureStarts = 0
    const countCaptureStarts = (request: Request) => {
      if (
        request.method() === 'POST'
        && new URL(request.url()).pathname === '/api/v1/enrollments/captures'
      ) captureStarts += 1
    }
    page.on('request', countCaptureStarts)

    const resumePoll = page.waitForResponse((response) =>
      response.request().method() === 'GET'
      && new URL(response.url()).pathname === `/api/v1/enrollments/${SEEDED_ENROLLMENT_ID}`,
    )
    await page.getByTestId(SEL.enrollmentReopen(SEEDED_ENROLLMENT_ID)).click()
    expect((await resumePoll).status()).toBe(200)
    await expect(page.getByTestId(SEL.enrollmentModal)).toBeVisible()
    await expect(page.getByText('Monitoreando sincronización por dispositivo…')).toBeVisible()
    await expect(page.getByTestId(SEL.enrollmentDeviceTab)).toHaveCount(0)
    expect(captureStarts).toBe(0)
    await page.getByRole('button', { name: 'Cerrar enrolamiento' }).click()
    await expect(page.getByTestId(SEL.enrollmentModal)).not.toBeVisible()

    await page.getByLabel('Selecciona un empleado').selectOption(NEW_EMPLOYEE_ID)
    await page.getByRole('button', { name: 'Iniciar Enrolamiento' }).click()
    await expect(page.getByTestId(SEL.enrollmentModal)).toContainText('José Hernández')
    await page.getByLabel('Seleccionar dispositivo').selectOption(ENTRY_DEVICE_ID)

    const startRequestPromise = page.waitForRequest((request) =>
      request.method() === 'POST'
      && new URL(request.url()).pathname === '/api/v1/enrollments/captures',
    )
    const startResponsePromise = page.waitForResponse((response) =>
      response.request().method() === 'POST'
      && new URL(response.url()).pathname === '/api/v1/enrollments/captures',
    )
    const capturedResponsePromise = page.waitForResponse(async (response) => {
      if (
        response.request().method() !== 'GET'
        || !new URL(response.url()).pathname.startsWith('/api/v1/enrollments/captures/')
        || response.status() !== 200
      ) return false
      return (await response.json()).status === 'captured'
    })

    await page.getByRole('button', { name: 'Iniciar Captura' }).click()
    const startRequest = await startRequestPromise
    expect(startRequest.postDataJSON()).toEqual({
      device_id: ENTRY_DEVICE_ID,
      employee_id: NEW_EMPLOYEE_ID,
    })
    const startResponse = await startResponsePromise
    expect(startResponse.status()).toBe(202)
    const started = await startResponse.json()
    expect(started).toMatchObject({
      status: 'capturing',
      source_device_id: ENTRY_DEVICE_ID,
    })
    expect(started.capture_id).toBeTruthy()

    const capturedResponse = await capturedResponsePromise
    const captured = await capturedResponse.json()
    expect(captured.capture_id).toBe(started.capture_id)
    expect(captured.source_device_id).toBe(ENTRY_DEVICE_ID)
    const capturedBytes = Buffer.from(captured.photo_b64, 'base64')
    expect([...capturedBytes.subarray(0, 3)]).toEqual([0xff, 0xd8, 0xff])

    const preview = page.getByRole('img', { name: 'Captura del dispositivo' })
    await expect(preview).toBeVisible()
    expect(await preview.evaluate((image: HTMLImageElement) => ({
      width: image.naturalWidth,
      height: image.naturalHeight,
    }))).toEqual({ width: 640, height: 480 })

    await page.getByRole('button', { name: 'Aceptar' }).click()
    await expect(page.getByText('Rostro Detectado').locator('..')).toContainText('OK', {
      timeout: 30_000,
    })
    await expect(page.getByText('Buena Iluminación').locator('..')).toContainText('OK')
    await expect(page.getByText('Resolución Óptima').locator('..')).toContainText('OK')
    await expect(page.getByRole('button', { name: 'Enrolar' })).toBeEnabled()

    const createRequestPromise = page.waitForRequest((request) =>
      request.method() === 'POST'
      && new URL(request.url()).pathname === '/api/v1/enrollments',
    )
    const createResponsePromise = page.waitForResponse((response) =>
      response.request().method() === 'POST'
      && new URL(response.url()).pathname === '/api/v1/enrollments',
    )
    const partialResponsePromise = page.waitForResponse(async (response) => {
      const path = new URL(response.url()).pathname
      if (
        response.request().method() !== 'GET'
        || !path.startsWith('/api/v1/enrollments/')
        || path.includes('/captures/')
        || response.status() !== 200
      ) return false
      return (await response.json()).status === 'partial'
    })

    await page.getByRole('button', { name: 'Enrolar' }).click()
    const [createRequest, createResponse, partial] = await Promise.all([
      createRequestPromise,
      createResponsePromise,
      partialResponsePromise,
    ])
    expect(createRequest.headers()['content-type']).toMatch(
      /^multipart\/form-data;\s*boundary=.+/i,
    )
    const observedMultipart = await page.evaluate(() =>
      (window as typeof window & {
        __enrollmentMultipart?: Record<
          string,
          string | { kind: 'blob'; type: string; size: number }
        >
      }).__enrollmentMultipart,
    )
    expect(observedMultipart?.employee_id).toBe(NEW_EMPLOYEE_ID)
    expect(observedMultipart?.captured_via).toBe('device')
    expect(observedMultipart?.source_device_id).toBe(ENTRY_DEVICE_ID)
    expect(JSON.parse(String(observedMultipart?.face_quality_score))).toMatchObject({
      faceDetected: true,
      luminanceOk: true,
      sizeOk: true,
    })
    expect(observedMultipart?.photo).toEqual({
      kind: 'blob',
      type: 'image/jpeg',
      size: capturedBytes.length,
    })

    expect(createResponse.status()).toBe(202)
    const created = await createResponse.json()
    expect(created.enrollment_id).toBeTruthy()
    expect(created.device_pushes).toHaveLength(2)

    const partialBody = await partial.json()
    expect(partialBody.id).toBe(created.enrollment_id)
    expect(partialBody.status).toBe('partial')
    await expect(page.getByTestId(SEL.enrollmentPushStatus(ENTRY_DEVICE_ID))).toHaveText(
      'Sincronizado',
    )
    await expect(page.getByTestId(SEL.enrollmentPushStatus(EXIT_DEVICE_ID))).toHaveText('Falló')

    const retryResponsePromise = page.waitForResponse((response) =>
      response.request().method() === 'POST'
      && new URL(response.url()).pathname
        === `/api/v1/enrollments/${created.enrollment_id}/pushes/${EXIT_DEVICE_ID}/retry`,
    )
    await page.getByTestId(SEL.enrollmentRetry(EXIT_DEVICE_ID)).click()
    const retryResponse = await retryResponsePromise
    expect(retryResponse.status()).toBe(202)
    expect(await retryResponse.json()).toEqual({
      enrollment_id: created.enrollment_id,
      device_id: EXIT_DEVICE_ID,
      status: 'pending',
    })
    // The retry push status and the aggregate enrollment status are serialized
    // as separate queued writes. Poll the API directly until the aggregate
    // settles instead of relying on the UI to emit another GET after `partial`.
    await expect.poll(async () => {
      const response = await context.request.get(
        `${API_BASE}/enrollments/${created.enrollment_id}`,
        { headers: authHeaders },
      )
      expect(response.status()).toBe(200)
      return (await response.json()).status
    }, { timeout: 10_000 }).toBe('success')
    await expect(page.getByTestId(SEL.enrollmentPushStatus(EXIT_DEVICE_ID))).toHaveText(
      'Sincronizado',
    )

    const recvLog = await context.request.get(`${MOCK_ADMIN_BASE}/recv-log`)
    expect(recvLog.ok()).toBeTruthy()
    const commands: Array<{ method: string; path: string }> = (await recvLog.json()).commands
    expect(commands).toEqual(expect.arrayContaining([
      expect.objectContaining({
        method: 'POST',
        path: '/ISAPI/AccessControl/UserInfo/Record',
      }),
      expect.objectContaining({
        method: 'POST',
        path: '/ISAPI/Intelligent/FDLib/FaceDataRecord',
      }),
    ]))

    page.off('request', countCaptureStarts)
  } finally {
    await context.close()
    await browser.close()
  }
})
