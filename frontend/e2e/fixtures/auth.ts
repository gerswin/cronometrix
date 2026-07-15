import {
  test as base,
  expect,
  type APIRequestContext,
  type Browser,
  type BrowserContext,
} from '@playwright/test'
import { API_BASE } from './api'

const AUTH_API_BASE = 'http://localhost:4001/api/v1'

export type E2ERole = 'admin' | 'supervisor' | 'viewer'

export const ROLE_CREDENTIALS: Record<
  E2ERole,
  { username: string; password: string }
> = {
  admin: { username: 'e2e_admin', password: 'e2e-admin-pass' },
  supervisor: { username: 'e2e_supervisor', password: 'e2e-supervisor-pass' },
  viewer: { username: 'e2e_viewer', password: 'e2e-viewer-pass' },
}

export type E2ERoleSession = {
  context: BrowserContext
  accessToken: string
}

export type E2ERequestFactory = {
  request: {
    newContext: (options?: {
      extraHTTPHeaders?: Record<string, string>
    }) => Promise<APIRequestContext>
  }
}

export async function newRoleSession(
  browser: Browser,
  role: E2ERole,
): Promise<E2ERoleSession> {
  const context = await browser.newContext()
  const credentials = ROLE_CREDENTIALS[role]
  const resp = await context.request.post(`${AUTH_API_BASE}/auth/login`, {
    data: credentials,
  })

  expect(resp.ok(), `Login failed for ${credentials.username}: ${resp.status()}`).toBeTruthy()
  const body = await resp.json()
  expect(
    body.access_token,
    `No access_token in login response for ${credentials.username}`,
  ).toBeTruthy()

  return { context, accessToken: body.access_token as string }
}

export async function newRoleContext(
  browser: Browser,
  role: E2ERole,
): Promise<BrowserContext> {
  const session = await newRoleSession(browser, role)
  return session.context
}

export async function newAuthenticatedRequest(
  playwright: E2ERequestFactory,
  accessToken: string,
): Promise<APIRequestContext> {
  return playwright.request.newContext({
    extraHTTPHeaders: {
      Authorization: `Bearer ${accessToken}`,
    },
  })
}

type E2EFixtures = {
  role: E2ERole
  roleSession: E2ERoleSession
}

export const test = base.extend<E2EFixtures>({
  role: ['admin', { option: true }],

  roleSession: async ({ browser, role }, provide) => {
    const session = await newRoleSession(browser, role)
    try {
      await provide(session)
    } finally {
      await session.context.close()
    }
  },

  context: async ({ roleSession }, provide) => {
    await provide(roleSession.context)
  },

  request: async ({ playwright, roleSession }, provide) => {
    const request = await newAuthenticatedRequest(playwright, roleSession.accessToken)
    try {
      await provide(request)
    } finally {
      await request.dispose()
    }
  },
})

export { expect, API_BASE }
