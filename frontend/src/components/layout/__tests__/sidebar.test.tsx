import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Sidebar } from '../sidebar'

const { useAuthMock, usePathnameMock } = vi.hoisted(() => ({
  useAuthMock: vi.fn(),
  usePathnameMock: vi.fn(),
}))
vi.mock('@/hooks/use-auth', () => ({ useAuth: () => useAuthMock() }))
vi.mock('next/navigation', () => ({ usePathname: () => usePathnameMock() }))

// Stub next/link to a plain anchor (we just need the rendered href + text)
vi.mock('next/link', () => ({
  default: ({ href, children, className }: { href: string; children: React.ReactNode; className?: string }) => (
    <a href={href} className={className}>{children}</a>
  ),
}))

describe('Sidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    usePathnameMock.mockReturnValue('/dashboard')
  })

  it('renders the brand label "Cronometrix"', () => {
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    render(<Sidebar />)
    expect(screen.getByText('Cronometrix')).toBeTruthy()
  })

  it('admin sees ALL nav items including Configuración', () => {
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    render(<Sidebar />)
    for (const label of [
      'Dashboard',
      'Marcaciones',
      'Empleados',
      'Dispositivos',
      'Enrolamiento',
      'Reportes',
      'Auditoría',
      'Configuración',
    ]) {
      expect(screen.getByText(label)).toBeTruthy()
    }
  })

  it('supervisor sees the un-gated items but NOT Configuración (admin-only)', () => {
    useAuthMock.mockReturnValue({ role: 'supervisor', sub: 'u1', claims: null })
    render(<Sidebar />)
    for (const label of ['Dashboard', 'Marcaciones', 'Empleados', 'Dispositivos', 'Enrolamiento', 'Reportes', 'Auditoría']) {
      expect(screen.getByText(label)).toBeTruthy()
    }
    expect(screen.queryByText('Configuración')).toBeNull()
  })

  it('viewer also does not see Configuración', () => {
    useAuthMock.mockReturnValue({ role: 'viewer', sub: 'u1', claims: null })
    render(<Sidebar />)
    expect(screen.getByText('Dashboard')).toBeTruthy()
    expect(screen.queryByText('Configuración')).toBeNull()
  })

  it('null role hides Configuración (display-hint guard for unauthenticated render race)', () => {
    useAuthMock.mockReturnValue({ role: null, sub: null, claims: null })
    render(<Sidebar />)
    // Items without a `roles` attribute remain visible
    expect(screen.getByText('Dashboard')).toBeTruthy()
    // Configuración is gated to admin only; null role must not see it
    expect(screen.queryByText('Configuración')).toBeNull()
  })

  it('current pathname applies the active class on the matching item', () => {
    usePathnameMock.mockReturnValue('/timesheet')
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    render(<Sidebar />)
    const link = screen.getByText('Marcaciones').closest('a')
    expect(link?.className).toContain('bg-slate-700')
    // Inactive item should NOT have the active background
    const inactive = screen.getByText('Empleados').closest('a')
    expect(inactive?.className).not.toContain('bg-slate-700')
  })

  it('sub-route activates the parent nav (e.g. /timesheet/edit/123 → Marcaciones is active)', () => {
    usePathnameMock.mockReturnValue('/timesheet/edit/123')
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    render(<Sidebar />)
    const link = screen.getByText('Marcaciones').closest('a')
    expect(link?.className).toContain('bg-slate-700')
  })

  it('a sibling-prefix path does NOT light up the parent (WR-07: /reports-archive does not match /reports)', () => {
    usePathnameMock.mockReturnValue('/reports-archive')
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
    render(<Sidebar />)
    const reportsLink = screen.getByText('Reportes').closest('a')
    expect(reportsLink?.className).not.toContain('bg-slate-700')
  })
})
