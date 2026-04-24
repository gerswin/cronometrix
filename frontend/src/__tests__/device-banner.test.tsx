import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { DeviceStatusSummary } from '../components/dashboard/device-banner'
import { Device } from '../types/api'

const base: Partial<Device> = { name: 'dev', ip_address: '10.0.0.1', direction: 'entry', last_seen_at: null, created_at: '', updated_at: '' }

describe('DeviceStatusSummary', () => {
  it('shows all online when no device is offline', () => {
    const devices = [{ ...base, id: '1', status: 'online' as const }]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText('1/1 en línea')).toBeTruthy()
  })
  it('shows yellow warning when some devices offline', () => {
    const devices = [
      { ...base, id: '1', status: 'online' as const },
      { ...base, id: '2', status: 'offline' as const },
    ]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText(/desconectado/)).toBeTruthy()
  })
  it('shows red alert when all devices offline', () => {
    const devices = [{ ...base, id: '1', status: 'offline' as const }]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText(/OFFLINE/)).toBeTruthy()
  })
})
