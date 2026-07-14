import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { FrameAnalysis } from '../face-detection'

const { getMock, postMock } = vi.hoisted(() => ({
  getMock: vi.fn(),
  postMock: vi.fn(),
}))

vi.mock('@/lib/api', () => ({
  api: {
    get: (...args: unknown[]) => getMock(...args),
    post: (...args: unknown[]) => postMock(...args),
  },
}))

import {
  createEnrollment,
  getDeviceCapture,
  getEnrollment,
  listInProgressEnrollments,
  retryEnrollmentPush,
  startDeviceCapture,
} from '../enrollment-api'

const ACCEPTABLE: FrameAnalysis = {
  faceDetected: true,
  luminanceOk: true,
  sizeOk: true,
  luminance: 120,
  width: 200,
  height: 200,
}

describe('enrollment-api', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getMock.mockResolvedValue({ data: { ok: true } })
    postMock.mockResolvedValue({ data: { ok: true } })
  })

  it('creates a device enrollment with semantic quality JSON and no manual multipart header', async () => {
    postMock.mockResolvedValueOnce({
      data: { enrollment_id: 'enr-1', face_id: 'face-1', device_pushes: [] },
    })
    const photo = new Blob(['jpeg'], { type: 'image/jpeg' })

    await createEnrollment({
      employeeId: 'emp-1',
      capturedVia: 'device',
      sourceDeviceId: 'device-origin',
      photo,
      faceQualityScore: ACCEPTABLE,
    })

    expect(postMock).toHaveBeenCalledTimes(1)
    expect(postMock).toHaveBeenCalledWith('/enrollments', expect.any(FormData))
    const body = postMock.mock.calls[0][1] as FormData
    expect(body.get('employee_id')).toBe('emp-1')
    expect(body.get('captured_via')).toBe('device')
    expect(body.get('source_device_id')).toBe('device-origin')
    const serializedPhoto = body.get('photo')
    expect(serializedPhoto).toBeInstanceOf(Blob)
    expect((serializedPhoto as Blob).size).toBe(photo.size)
    expect((serializedPhoto as Blob).type).toBe(photo.type)
    expect(JSON.parse(String(body.get('face_quality_score')))).toEqual(ACCEPTABLE)
  })

  it('omits source_device_id for webcam and upload enrollments', async () => {
    await createEnrollment({
      employeeId: 'emp-1',
      capturedVia: 'upload',
      sourceDeviceId: null,
      photo: new Blob(['jpeg'], { type: 'image/jpeg' }),
      faceQualityScore: ACCEPTABLE,
    })

    const body = postMock.mock.calls[0][1] as FormData
    expect(body.has('source_device_id')).toBe(false)
  })

  it('rejects a missing quality analysis before making an HTTP request', async () => {
    await expect(createEnrollment({
      employeeId: 'emp-1',
      capturedVia: 'webcam',
      sourceDeviceId: null,
      photo: new Blob(['jpeg']),
      faceQualityScore: null as unknown as FrameAnalysis,
    })).rejects.toThrow(/calidad/i)
    expect(postMock).not.toHaveBeenCalled()
  })

  it('rejects unacceptable quality before making an HTTP request', async () => {
    await expect(createEnrollment({
      employeeId: 'emp-1',
      capturedVia: 'upload',
      sourceDeviceId: null,
      photo: new Blob(['jpeg']),
      faceQualityScore: { ...ACCEPTABLE, faceDetected: false },
    })).rejects.toThrow(/calidad/i)
    expect(postMock).not.toHaveBeenCalled()
  })

  it('rejects a device capture without source provenance before making an HTTP request', async () => {
    await expect(createEnrollment({
      employeeId: 'emp-1',
      capturedVia: 'device',
      sourceDeviceId: null,
      photo: new Blob(['jpeg']),
      faceQualityScore: ACCEPTABLE,
    })).rejects.toThrow(/dispositivo de origen/i)
    expect(postMock).not.toHaveBeenCalled()
  })

  it('gets one enrollment through an encoded canonical path', async () => {
    await getEnrollment('enr/with space')
    expect(getMock).toHaveBeenCalledWith('/enrollments/enr%2Fwith%20space')
  })

  it('lists only in-progress enrollments with explicit pagination', async () => {
    await listInProgressEnrollments({ limit: 100, offset: 20 })
    expect(getMock).toHaveBeenCalledWith('/enrollments', {
      params: { status: 'in_progress', limit: 100, offset: 20 },
    })
  })

  it('starts device capture with the backend snake_case body', async () => {
    await startDeviceCapture({ deviceId: 'dev-1', employeeId: 'emp-1' })
    expect(postMock).toHaveBeenCalledWith('/enrollments/captures', {
      device_id: 'dev-1',
      employee_id: 'emp-1',
    })
  })

  it('gets a device capture through an encoded canonical path', async () => {
    await getDeviceCapture('cap/with space')
    expect(getMock).toHaveBeenCalledWith('/enrollments/captures/cap%2Fwith%20space')
  })

  it('retries one device push through the encoded canonical path', async () => {
    postMock.mockResolvedValueOnce({
      data: { enrollment_id: 'enr/1', device_id: 'dev/2', status: 'pending' },
    })
    const response = await retryEnrollmentPush('enr/1', 'dev/2')
    expect(postMock).toHaveBeenCalledWith('/enrollments/enr%2F1/pushes/dev%2F2/retry')
    expect(response.status).toBe('pending')
  })
})
