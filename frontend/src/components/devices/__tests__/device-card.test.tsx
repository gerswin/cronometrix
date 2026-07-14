import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Device } from '@/types/api'
import deviceFixture from './fixtures/device.json'
import { DeviceCard } from '../device-card'

function makeDevice(overrides: Partial<Device> = {}): Device {
  return {
    ...(deviceFixture as Device),
    ...overrides,
  }
}

function renderCard(
  overrides: Partial<Device> = {},
  canEdit = true,
  onCommandClick = vi.fn(),
) {
  const device = makeDevice(overrides)
  render(
    <DeviceCard
      device={device}
      canEdit={canEdit}
      onCommandClick={onCommandClick}
    />,
  )
  return { device, onCommandClick }
}

describe('DeviceCard', () => {
  it.each([
    ['online', 'En línea'],
    ['offline', 'Offline'],
    ['unknown', 'Desconocido'],
  ] as const)('renders the %s connection state', (connectionState, label) => {
    renderCard({ id: connectionState, connection_state: connectionState })
    expect(screen.getByTestId(`dev-status-${connectionState}`)).toHaveTextContent(label)
  })

  it.each([
    ['active', 'Activo'],
    ['inactive', 'Inactivo'],
  ] as const)('renders the %s lifecycle independently', (status, label) => {
    renderCard({ id: status, status })
    expect(screen.getByTestId(`dev-lifecycle-${status}`)).toHaveTextContent(label)
  })

  it.each([
    ['entry', 'Entrada'],
    ['exit', 'Salida'],
  ] as const)('translates %s direction to %s', (direction, label) => {
    renderCard({ direction })
    expect(screen.getByText(label)).toBeInTheDocument()
  })

  it('shows an em dash when the device has never been seen', () => {
    renderCard({ last_seen_at: null })
    expect(screen.getByText('—')).toBeInTheDocument()
  })

  it('formats a non-null heartbeat in the project timezone', () => {
    renderCard({ last_seen_at: '2026-04-15T12:00:00Z' })
    expect(screen.getByText('15/04/2026, 08:00')).toBeInTheDocument()
  })

  it('lets an admin command an active device with the exact device payload', () => {
    const onCommandClick = vi.fn()
    const { device } = renderCard(
      { id: 'dev-command', name: 'Lector Norte', status: 'active' },
      true,
      onCommandClick,
    )

    fireEvent.click(
      screen.getByRole('button', { name: 'Enviar comando a Lector Norte' }),
    )

    expect(onCommandClick).toHaveBeenCalledTimes(1)
    expect(onCommandClick).toHaveBeenCalledWith(device)
  })

  it.each([
    ['editing is forbidden', false, 'active'],
    ['the device is inactive', true, 'inactive'],
  ] as const)('hides commands when %s', (_case, canEdit, status) => {
    renderCard({ status }, canEdit)
    expect(screen.queryByRole('button', { name: /Enviar comando/i })).not.toBeInTheDocument()
  })
})
