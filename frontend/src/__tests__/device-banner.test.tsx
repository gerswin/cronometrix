import { beforeEach, describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { DeviceStatusSummary } from '../components/dashboard/device-banner'
import type { Device, PaginatedResponse } from '../types/api'
import deviceFixture from '../components/devices/__tests__/fixtures/device.json'

const { apiGetMock, useQueryMock } = vi.hoisted(() => ({
  apiGetMock: vi.fn(),
  useQueryMock: vi.fn(),
}))
vi.mock('@/lib/api', () => ({
  api: { get: (...args: unknown[]) => apiGetMock(...args) },
  setAccessToken: vi.fn(),
}))
vi.mock('@tanstack/react-query', () => ({
  useQuery: (options: unknown) => useQueryMock(options),
}))
vi.mock('next/navigation', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))
vi.mock('@/hooks/use-auth', () => ({
  useAuth: () => ({ role: 'admin' }),
}))
vi.mock('@/components/devices/device-card', () => ({ DeviceCard: () => null }))
vi.mock('@/components/devices/command-modal', () => ({ CommandModal: () => null }))
vi.mock('@/components/devices/create-device-modal', () => ({ CreateDeviceModal: () => null }))
vi.mock('@/components/dashboard/kpi-tile', () => ({ KPITile: () => null }))
vi.mock('@/components/dashboard/activity-feed', () => ({ ActivityFeed: () => null }))
vi.mock('@/components/dashboard/dept-chart', () => ({ DeptChart: () => null }))

type FetchAllDevices = () => Promise<PaginatedResponse<Device>>
type DeviceQueryOptions = {
  queryKey: string[]
  queryFn: FetchAllDevices
}
let capturedDevicesFetcher: FetchAllDevices | undefined
const DEVICE_PAGE_LIMIT = 100

function makeDevice(overrides: Partial<Device> = {}): Device {
  return { ...(deviceFixture as Device), ...overrides }
}

async function expectBothLifecycleStates(fetchAllDevices: FetchAllDevices | undefined) {
  expect(fetchAllDevices).toBeTypeOf('function')
  const devicesByStatus: Record<Device['status'], Device[]> = {
    active: [
      makeDevice({ id: 'device-active-1', status: 'active' }),
      makeDevice({ id: 'device-active-2', status: 'active' }),
    ],
    inactive: [
      makeDevice({ id: 'device-inactive-1', status: 'inactive' }),
      makeDevice({ id: 'device-inactive-2', status: 'inactive' }),
    ],
  }
  apiGetMock.mockImplementation(
    (
      _url: string,
      config?: {
        params?: { status?: Device['status']; limit?: number; offset?: number }
      },
    ) => {
      const status = config?.params?.status ?? 'active'
      const offset = config?.params?.offset ?? 0
      return Promise.resolve({
        data: {
          data: devicesByStatus[status].slice(offset, offset + 1),
          total: devicesByStatus[status].length,
          limit: 1,
          offset,
        },
      })
    },
  )

  const result = await fetchAllDevices!()

  expect(apiGetMock).toHaveBeenCalledTimes(4)
  for (const status of ['active', 'inactive'] as const) {
    expect(apiGetMock).toHaveBeenCalledWith('/devices', {
      params: { status, limit: DEVICE_PAGE_LIMIT, offset: 0 },
    })
    expect(apiGetMock).toHaveBeenCalledWith('/devices', {
      params: { status, limit: DEVICE_PAGE_LIMIT, offset: 1 },
    })
  }
  expect(result.data.map(device => device.id)).toEqual([
    'device-active-1',
    'device-active-2',
    'device-inactive-1',
    'device-inactive-2',
  ])
  expect(result.total).toBe(4)
}

async function expectEmptyPagesStopPagination(fetchAllDevices: FetchAllDevices | undefined) {
  apiGetMock.mockClear()
  apiGetMock.mockResolvedValue({
    data: { data: [], total: 2, limit: DEVICE_PAGE_LIMIT, offset: 0 },
  })

  const result = await fetchAllDevices!()

  expect(apiGetMock).toHaveBeenCalledTimes(2)
  expect(result.data).toEqual([])
  expect(result.total).toBe(0)
}

function captureDevicesQuery(options: DeviceQueryOptions): { data: undefined; isLoading: false } {
  if (options.queryKey.length === 1 && options.queryKey[0] === 'devices') {
    capturedDevicesFetcher = options.queryFn
  }
  return { data: undefined, isLoading: false }
}

describe('DeviceStatusSummary', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedDevicesFetcher = undefined
    useQueryMock.mockImplementation(captureDevicesQuery)
  })

  it('shows all active devices online when none is disconnected', () => {
    const devices = [makeDevice({ id: '1', status: 'active', connection_state: 'online' })]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText('1/1 en línea')).toBeTruthy()
  })
  it('shows yellow warning when some active devices are disconnected', () => {
    const devices = [
      makeDevice({ id: '1', connection_state: 'online' }),
      makeDevice({ id: '2', connection_state: 'offline' }),
    ]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText(/desconectado/)).toBeTruthy()
  })
  it('shows red alert when all active devices are offline', () => {
    const devices = [makeDevice({ id: '1', connection_state: 'offline' })]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText(/OFFLINE/)).toBeTruthy()
  })
  it('excludes inactive devices from connectivity totals and shows them separately', () => {
    const devices = [
      makeDevice({ id: '1', status: 'active', connection_state: 'online' }),
      makeDevice({ id: '2', status: 'inactive', connection_state: 'online' }),
    ]
    render(<DeviceStatusSummary devices={devices} />)
    expect(screen.getByText('1/1 en línea')).toBeTruthy()
    expect(screen.getByText('1 inactivo')).toBeTruthy()
    expect(screen.queryByText('2/2 en línea')).toBeNull()
  })

  it('devices page fetches and merges active and inactive lifecycle pages', async () => {
    const { default: DevicesPage } = await import('../app/(dashboard)/devices/page')
    render(<DevicesPage />)
    await expectBothLifecycleStates(capturedDevicesFetcher)
    await expectEmptyPagesStopPagination(capturedDevicesFetcher)
  })

  it('dashboard page fetches and merges active and inactive lifecycle pages', async () => {
    const { default: DashboardPage } = await import('../app/(dashboard)/dashboard/page')
    render(<DashboardPage />)
    await expectBothLifecycleStates(capturedDevicesFetcher)
    await expectEmptyPagesStopPagination(capturedDevicesFetcher)
  })
})
