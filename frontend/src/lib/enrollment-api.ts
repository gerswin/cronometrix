import { api } from '@/lib/api'
import { isAcceptableFace, type FrameAnalysis } from '@/lib/face-detection'
import type {
  CaptureFromDeviceState,
  CaptureStartResponse,
  Enrollment,
  EnrollmentSubmitResponse,
  PaginatedResponse,
  RetryEnrollmentPushResponse,
} from '@/types/api'

export type EnrollmentCaptureMethod = 'device' | 'webcam' | 'upload'

export interface CapturedPhotoCandidate {
  blob: Blob
  capturedVia: EnrollmentCaptureMethod
  sourceDeviceId: string | null
}

export interface CapturedPhoto extends CapturedPhotoCandidate {
  analysis: FrameAnalysis
}

export interface CreateEnrollmentInput {
  employeeId: string
  capturedVia: EnrollmentCaptureMethod
  sourceDeviceId: string | null
  photo: Blob
  faceQualityScore: FrameAnalysis
}

function encoded(id: string): string {
  return encodeURIComponent(id)
}

export async function createEnrollment(
  input: CreateEnrollmentInput,
): Promise<EnrollmentSubmitResponse> {
  if (!input.faceQualityScore || !isAcceptableFace(input.faceQualityScore)) {
    throw new Error('Se requiere un análisis válido de calidad facial')
  }
  if (input.capturedVia === 'device' && !input.sourceDeviceId) {
    throw new Error('Se requiere el dispositivo de origen para la captura')
  }

  const body = new FormData()
  body.append('employee_id', input.employeeId)
  body.append('captured_via', input.capturedVia)
  if (input.capturedVia === 'device' && input.sourceDeviceId) {
    body.append('source_device_id', input.sourceDeviceId)
  }
  body.append('photo', input.photo)
  body.append('face_quality_score', JSON.stringify(input.faceQualityScore))

  const response = await api.post<EnrollmentSubmitResponse>('/enrollments', body)
  return response.data
}

export async function getEnrollment(id: string): Promise<Enrollment> {
  const response = await api.get<Enrollment>(`/enrollments/${encoded(id)}`)
  return response.data
}

export async function listInProgressEnrollments(
  params: { limit?: number; offset?: number } = {},
): Promise<PaginatedResponse<Enrollment>> {
  const query: { status: 'in_progress'; limit?: number; offset?: number } = {
    status: 'in_progress',
  }
  if (params.limit !== undefined) query.limit = params.limit
  if (params.offset !== undefined) query.offset = params.offset

  const response = await api.get<PaginatedResponse<Enrollment>>('/enrollments', {
    params: query,
  })
  return response.data
}

export async function startDeviceCapture(input: {
  deviceId: string
  employeeId: string
}): Promise<CaptureStartResponse> {
  const response = await api.post<CaptureStartResponse>('/enrollments/captures', {
    device_id: input.deviceId,
    employee_id: input.employeeId,
  })
  return response.data
}

export async function getDeviceCapture(captureId: string): Promise<CaptureFromDeviceState> {
  const response = await api.get<CaptureFromDeviceState>(
    `/enrollments/captures/${encoded(captureId)}`,
  )
  return response.data
}

export async function retryEnrollmentPush(
  enrollmentId: string,
  deviceId: string,
): Promise<RetryEnrollmentPushResponse> {
  const response = await api.post<RetryEnrollmentPushResponse>(
    `/enrollments/${encoded(enrollmentId)}/pushes/${encoded(deviceId)}/retry`,
  )
  return response.data
}
