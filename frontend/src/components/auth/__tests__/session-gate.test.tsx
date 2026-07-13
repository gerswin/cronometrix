import { render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const navigation = vi.hoisted(() => ({
  pathname: vi.fn(),
  replace: vi.fn(),
  router: { replace: vi.fn() },
  search: vi.fn(),
  useAuth: vi.fn(),
}))

vi.mock('@/contexts/auth-context', () => ({
  useAuth: () => navigation.useAuth(),
}))

vi.mock('next/navigation', () => ({
  usePathname: () => navigation.pathname(),
  useRouter: () => navigation.router,
  useSearchParams: () => new URLSearchParams(navigation.search()),
}))

import { SessionGate } from '../session-gate'

beforeEach(() => {
  vi.clearAllMocks()
  navigation.pathname.mockReturnValue('/dashboard')
  navigation.search.mockReturnValue('')
  navigation.router.replace = navigation.replace
})

describe('SessionGate', () => {
  it('hides protected content behind the initializing marker', () => {
    navigation.useAuth.mockReturnValue({
      claims: null,
      role: null,
      status: 'initializing',
      sub: null,
    })

    render(
      <SessionGate>
        <div>protected dashboard</div>
      </SessionGate>
    )

    expect(screen.getByTestId('session-initializing')).toBeInTheDocument()
    expect(screen.queryByText('protected dashboard')).not.toBeInTheDocument()
    expect(navigation.replace).not.toHaveBeenCalled()
  })

  it('redirects an anonymous session once while preserving pathname and query', async () => {
    navigation.useAuth.mockReturnValue({
      claims: null,
      role: null,
      status: 'anonymous',
      sub: null,
    })
    navigation.pathname.mockReturnValue('/employees')
    navigation.search.mockReturnValue('status=active&sort=name')

    const { rerender } = render(
      <SessionGate>
        <div>protected employees</div>
      </SessionGate>
    )

    const expectedTarget = `/login?redirect=${encodeURIComponent(
      '/employees?status=active&sort=name'
    )}`
    await waitFor(() => expect(navigation.replace).toHaveBeenCalledWith(expectedTarget))
    expect(navigation.replace).toHaveBeenCalledTimes(1)
    expect(screen.queryByText('protected employees')).not.toBeInTheDocument()

    rerender(
      <SessionGate>
        <div>protected employees</div>
      </SessionGate>
    )
    expect(navigation.replace).toHaveBeenCalledTimes(1)
  })

  it('renders children for an authenticated session', () => {
    navigation.useAuth.mockReturnValue({
      claims: null,
      role: 'admin',
      status: 'authenticated',
      sub: 'admin-1',
    })

    render(
      <SessionGate>
        <div>protected dashboard</div>
      </SessionGate>
    )

    expect(screen.getByText('protected dashboard')).toBeInTheDocument()
    expect(screen.queryByTestId('session-initializing')).not.toBeInTheDocument()
    expect(navigation.replace).not.toHaveBeenCalled()
  })
})
