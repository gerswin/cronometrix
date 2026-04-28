import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { DeviceTable } from '../device-table'
import type { Device } from '@/types/api'

const { useAuthMock } = vi.hoisted(() => ({ useAuthMock: vi.fn() }))
vi.mock('@/hooks/use-auth', () => ({
  useAuth: () => useAuthMock(),
}))

function makeDevice(over: Partial<Device> = {}): Device {
  return {
    id: 'dev-1',
    name: 'Entrada Principal',
    ip_address: '10.0.0.10',
    direction: 'entry',
    status: 'online',
    last_seen_at: '2026-04-28T12:00:00Z',
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...over,
  }
}

describe('DeviceTable', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthMock.mockReturnValue({ role: 'admin', sub: 'u1', claims: null })
  })

  it('renders all column headers', () => {
    render(<DeviceTable devices={[]} onCommandClick={() => {}} />)
    for (const h of ['Nombre', 'IP', 'Dirección', 'Estado', 'Última conexión', 'Acciones']) {
      expect(screen.getByText(h)).toBeTruthy()
    }
  })

  it('shows the empty state row when no devices', () => {
    render(<DeviceTable devices={[]} onCommandClick={() => {}} />)
    expect(screen.getByText('Sin dispositivos registrados')).toBeTruthy()
  })

  it('renders a row per device with name, IP, and direction translation', () => {
    render(
      <DeviceTable
        devices={[
          makeDevice({ id: 'a', name: 'Lector A', direction: 'entry', status: 'online' }),
          makeDevice({ id: 'b', name: 'Lector B', direction: 'exit', status: 'offline', ip_address: '10.0.0.11' }),
          makeDevice({ id: 'c', name: 'Recepción', direction: 'both', status: 'unknown', ip_address: '10.0.0.12' }),
        ]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByText('Lector A')).toBeTruthy()
    expect(screen.getByText('Lector B')).toBeTruthy()
    // Direction translations rendered in their own cells
    expect(screen.getByText('Entrada')).toBeTruthy()
    expect(screen.getByText('Salida')).toBeTruthy()
    expect(screen.getByText('Ambos')).toBeTruthy()
    expect(screen.getByText('10.0.0.10')).toBeTruthy()
    expect(screen.getByText('10.0.0.11')).toBeTruthy()
  })

  it('renders status badges with the Spanish labels for each status', () => {
    render(
      <DeviceTable
        devices={[
          makeDevice({ id: 'a', name: 'A', status: 'online' }),
          makeDevice({ id: 'b', name: 'B', status: 'offline' }),
          makeDevice({ id: 'c', name: 'C', status: 'unknown' }),
        ]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByText('En línea')).toBeTruthy()
    expect(screen.getByText('Offline')).toBeTruthy()
    expect(screen.getByText('Desconocido')).toBeTruthy()
  })

  it('shows em-dash for last_seen_at when null', () => {
    render(
      <DeviceTable
        devices={[makeDevice({ id: 'a', name: 'A', last_seen_at: null })]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByText('—')).toBeTruthy()
  })

  it('admin sees a "Comando" button per device that fires onCommandClick', () => {
    const onCommandClick = vi.fn()
    const dev = makeDevice({ id: 'a', name: 'Entrada' })
    render(<DeviceTable devices={[dev]} onCommandClick={onCommandClick} />)
    const btn = screen.getByRole('button', { name: /Enviar comando a Entrada/i })
    fireEvent.click(btn)
    expect(onCommandClick).toHaveBeenCalledWith(dev)
  })

  it('non-admin (supervisor) does NOT see Comando buttons (D-14 RBAC)', () => {
    useAuthMock.mockReturnValue({ role: 'supervisor', sub: 'u1', claims: null })
    render(<DeviceTable devices={[makeDevice()]} onCommandClick={() => {}} />)
    expect(screen.queryByRole('button', { name: /Enviar comando/i })).toBeNull()
  })

  it('viewer also does not see Comando buttons', () => {
    useAuthMock.mockReturnValue({ role: 'viewer', sub: 'u1', claims: null })
    render(<DeviceTable devices={[makeDevice()]} onCommandClick={() => {}} />)
    expect(screen.queryByRole('button', { name: /Enviar comando/i })).toBeNull()
  })
})
