import { test as setup, expect } from '@playwright/test'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { API_BASE } from '../fixtures/api'

// Ensure the .auth directory exists before writing storageState files.
const AUTH_DIR = path.resolve(__dirname, '../.auth')
if (!fs.existsSync(AUTH_DIR)) {
  fs.mkdirSync(AUTH_DIR, { recursive: true })
}

const ROLES = [
  { username: 'e2e_admin',      password: 'e2e-admin-pass',      file: 'admin' },
  { username: 'e2e_supervisor', password: 'e2e-supervisor-pass', file: 'supervisor' },
  { username: 'e2e_viewer',     password: 'e2e-viewer-pass',     file: 'viewer' },
] as const

for (const r of ROLES) {
  setup(`authenticate ${r.username}`, async ({ request }) => {
    const resp = await request.post(`${API_BASE}/auth/login`, {
      data: { username: r.username, password: r.password },
    })
    expect(resp.ok(), `Login failed for ${r.username}: ${resp.status()}`).toBeTruthy()
    const body = await resp.json()
    expect(body.access_token, `No access_token in login response for ${r.username}`).toBeTruthy()

    // request.storageState writes cookies (refresh) AND any localStorage writes from
    // subsequent fetches. Persist to e2e/.auth/{role}.json (gitignored).
    //
    // NOTE: Frontend stores access_token in memory (not in a cookie or localStorage).
    // The refresh cookie alone is sufficient for storageState — when the spec navigates
    // with this storageState, the frontend's refresh hook calls /auth/refresh and obtains
    // a fresh access_token automatically. This mirrors the production auth lifecycle.
    await request.storageState({ path: path.join(AUTH_DIR, `${r.file}.json`) })
  })
}
