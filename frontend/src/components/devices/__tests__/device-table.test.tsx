import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { DeviceTable } from '../device-table'
import { DeviceCard } from '../device-card'
import type { Device } from '@/types/api'
import { createDeviceFormSchema } from '@/lib/validations'
import deviceFixture from './fixtures/device.json'

const { useAuthMock } = vi.hoisted(() => ({ useAuthMock: vi.fn() }))
vi.mock('@/hooks/use-auth', () => ({
  useAuth: () => useAuthMock(),
}))

function makeDevice(over: Partial<Device> = {}): Device {
  return {
    ...(deviceFixture as Device),
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
          makeDevice({ id: 'a', name: 'Lector A', direction: 'entry' }),
          makeDevice({ id: 'b', name: 'Lector B', direction: 'exit', ip: '10.0.0.11' }),
        ]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByText('Lector A')).toBeTruthy()
    expect(screen.getByText('Lector B')).toBeTruthy()
    // Direction translations rendered in their own cells
    expect(screen.getByText('Entrada')).toBeTruthy()
    expect(screen.getByText('Salida')).toBeTruthy()
    expect(screen.queryByText('Ambos')).toBeNull()
    expect(screen.getByText('127.0.0.1')).toBeTruthy()
    expect(screen.getByText('10.0.0.11')).toBeTruthy()
  })

  it('renders connectivity badges from connection_state with stable device testids', () => {
    render(
      <DeviceTable
        devices={[
          makeDevice({ id: 'a', name: 'A', connection_state: 'online' }),
          makeDevice({ id: 'b', name: 'B', connection_state: 'offline' }),
          makeDevice({ id: 'c', name: 'C', connection_state: 'unknown' }),
        ]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByTestId('dev-status-a').textContent).toBe('En línea')
    expect(screen.getByTestId('dev-status-b').textContent).toBe('Offline')
    expect(screen.getByTestId('dev-status-c').textContent).toBe('Desconocido')
  })

  it('shows inactive lifecycle separately from online connectivity', () => {
    render(
      <DeviceTable
        devices={[makeDevice({ id: 'inactive', status: 'inactive', connection_state: 'online' })]}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByTestId('dev-status-inactive').textContent).toBe('En línea')
    expect(screen.getByText('Inactivo')).toBeTruthy()
    expect(screen.queryByRole('button', { name: /Enviar comando/i })).toBeNull()
  })

  it('does not expose card commands for inactive devices', () => {
    render(
      <DeviceCard
        device={makeDevice({ id: 'inactive-card', status: 'inactive' })}
        canEdit={true}
        onCommandClick={() => {}}
      />
    )
    expect(screen.getByTestId('dev-lifecycle-inactive-card').textContent).toBe('Inactivo')
    expect(screen.queryByRole('button', { name: /Enviar comando/i })).toBeNull()
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

  it('accepts only entry or exit when creating a device', () => {
    const request = {
      name: 'Entrada Principal',
      ip: '127.0.0.1',
      port: 4400,
      scheme: 'http' as const,
      username: 'admin',
      password: 'secret',
      allow_insecure_tls: false,
    }

    expect(createDeviceFormSchema.safeParse({ ...request, direction: 'entry' }).success).toBe(true)
    expect(createDeviceFormSchema.safeParse({ ...request, direction: 'exit' }).success).toBe(true)
    expect(createDeviceFormSchema.safeParse({ ...request, direction: 'both' }).success).toBe(false)
  })
})
