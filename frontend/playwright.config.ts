import { defineConfig, devices } from '@playwright/test'

const RUN_ID = process.env.GITHUB_RUN_ID ?? `local-${process.pid}`
const PATHS_ROOT = `/tmp/cronometrix-e2e-${RUN_ID}`
const DB_PATH = `${PATHS_ROOT}.db`

export default defineConfig({
  testDir: './e2e',
  globalTeardown: require.resolve('./e2e/global-teardown'),
  fullyParallel: false,        // D-12: shared DB requires serial execution
  workers: 1,                  // D-12 determinism
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI
    ? [['html', { outputFolder: 'playwright-report', open: 'never' }], ['github']]
    : [['html', { outputFolder: 'playwright-report', open: 'never' }], ['list']],
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: 'http://localhost:3001',
    timezoneId: 'America/Caracas',   // D-20 browser context TZ
    locale: 'es-VE',                 // D-19 dashboard locale
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    { name: 'setup', testMatch: /.*\/setup\/.*\.setup\.ts/ },
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } },
      dependencies: ['setup'],
    },
  ],
  webServer: [
    {
      // Pre-built binary (CI builds in a separate step; local dev runs `cargo build` first).
      command: process.env.CI
        ? '../backend/target/release/cronometrix'
        : '../backend/target/debug/cronometrix',
      url: 'http://127.0.0.1:4001/api/v1/health',
      reuseExistingServer: !process.env.CI,
      timeout: 60_000,
      stdout: 'pipe',
      stderr: 'pipe',
      env: {
        SERVER_HOST: '127.0.0.1',
        SERVER_PORT: '4001',
        TURSO_DATABASE_URL: `file:${DB_PATH}`,
        TZ: 'America/Caracas',                  // D-20 backend TZ
        CRONOMETRIX_E2E: 'true',
        CRONOMETRIX_LICENSE_BYPASS: 'true',
        // Filesystem-root injection (CLAUDE.md convention):
        CRONOMETRIX_LEAVES_ROOT: `${PATHS_ROOT}/leaves`,
        CRONOMETRIX_EVENTS_ROOT: `${PATHS_ROOT}/events`,
        ENROLLMENTS_DIR: `${PATHS_ROOT}/enrollments`,
        CRONOMETRIX_CAPTURES_TMP: `${PATHS_ROOT}/captures-tmp`,
        DATA_DIR: PATHS_ROOT,
        JWT_SECRET: 'e2e-test-secret-must-be-32-bytes-long-1234',
        DEVICE_CREDS_KEY: 'e2e-test-device-creds-key-32bytes',
        LICENSE_JWT_PATH: `${PATHS_ROOT}/license.jwt`,
        // Direct device.* outbound to mock_hikvision:
        MOCK_HIKVISION_BASE_URL: 'http://127.0.0.1:4400',
      },
    },
    {
      command: process.env.CI
        ? '../backend/target/release/mock_hikvision'
        : '../backend/target/debug/mock_hikvision',
      url: 'http://127.0.0.1:4400/ISAPI/System/status',
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
      stdout: 'pipe',
      stderr: 'pipe',
      env: {
        MOCK_HIKVISION_PORT: '4400',
        MOCK_HIKVISION_ADMIN_PORT: '4401',
        MOCK_HIKVISION_FIXTURES_DIR: './e2e/fixtures/hikvision-events',
        TZ: 'America/Caracas',
      },
    },
    {
      command: process.env.CI ? 'next start --port 3001' : 'next dev --port 3001',
      url: 'http://localhost:3001/login',
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
      env: {
        NEXT_PUBLIC_API_URL: 'http://localhost:4001',
        TZ: 'America/Caracas',                  // D-20 Next.js TZ
      },
    },
  ],
})
