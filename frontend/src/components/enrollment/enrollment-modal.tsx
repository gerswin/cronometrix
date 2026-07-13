'use client'
import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { X } from 'lucide-react'
import {
  createEnrollment,
  getEnrollment,
  type CapturedPhoto,
  type CapturedPhotoCandidate,
} from '@/lib/enrollment-api'
import { analyzePhotoBlob, isAcceptableFace } from '@/lib/face-detection'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import { ValidationPanel } from './validation-panel'
import { SyncPanel } from './sync-panel'
import { KioskCaptureTab } from './kiosk-capture-tab'
import { WebcamCaptureTab } from './webcam-capture-tab'
import { UploadCaptureTab } from './upload-capture-tab'
import type { Employee, Enrollment, EnrollmentDevicePush } from '@/types/api'

type CaptureTab = 'hikvision' | 'webcam' | 'upload'

interface EnrollmentModalProps {
  open: boolean
  employee: Employee | null
  initialEnrollmentId?: string | null
  onClose: () => void
}

function shouldPollEnrollment(enrollment: Enrollment): boolean {
  return enrollment.status === 'in_progress'
    || enrollment.device_pushes.some(
      (push) => push.status === 'pending' || push.status === 'in_progress',
    )
}

function showRecoveryToast(enrollmentId: string, pushes: EnrollmentDevicePush[]) {
  const successCount = pushes.filter((push) => push.status === 'success').length
  toast(
    `Enrolamiento en curso — ${successCount}/${pushes.length} dispositivos`,
    { id: `enrollment-${enrollmentId}`, duration: Infinity },
  )
}

export function EnrollmentModal({
  open,
  employee,
  initialEnrollmentId = null,
  onClose,
}: EnrollmentModalProps) {
  const queryClient = useQueryClient()
  const [tab, setTab] = useState<CaptureTab>('hikvision')
  const [capturedPhoto, setCapturedPhoto] = useState<CapturedPhoto | null>(null)
  const [analyzing, setAnalyzing] = useState(false)
  const [enrollmentId, setEnrollmentId] = useState<string | null>(initialEnrollmentId)
  const generationRef = useRef(0)
  const enrollmentIdRef = useRef<string | null>(initialEnrollmentId)
  const immediatePushesRef = useRef<EnrollmentDevicePush[]>([])
  const terminalHandledRef = useRef<string | null>(null)
  const closedDuringSubmitRef = useRef(false)
  const openRef = useRef(open)
  openRef.current = open
  const sessionIdentity = initialEnrollmentId
    ? `enrollment:${initialEnrollmentId}`
    : employee
      ? `employee:${employee.id}`
      : 'none'
  const sessionIdentityRef = useRef(sessionIdentity)

  const submitMutation = useMutation({
    mutationFn: async (request: {
      generation: number
      employeeId: string
      photo: CapturedPhoto
    }) => ({
      generation: request.generation,
      response: await createEnrollment({
        employeeId: request.employeeId,
        capturedVia: request.photo.capturedVia,
        sourceDeviceId: request.photo.sourceDeviceId,
        photo: request.photo.blob,
        faceQualityScore: request.photo.analysis,
      }),
    }),
    onSuccess: ({ generation, response }) => {
      void queryClient.invalidateQueries({
        queryKey: ['enrollment', response.enrollment_id],
      })
      void queryClient.invalidateQueries({ queryKey: ['enrollments', 'in_progress'] })
      if (generation !== generationRef.current) return
      immediatePushesRef.current = response.device_pushes
      terminalHandledRef.current = null
      enrollmentIdRef.current = response.enrollment_id
      setEnrollmentId(response.enrollment_id)
      if (!openRef.current || closedDuringSubmitRef.current) {
        showRecoveryToast(response.enrollment_id, response.device_pushes)
      }
      closedDuringSubmitRef.current = false
    },
    onError: (error: unknown, request) => {
      if (request.generation !== generationRef.current) return
      const message = (error as { response?: { data?: { message?: string } } })?.response?.data?.message
        ?? (error instanceof Error ? error.message : null)
        ?? 'No se pudo registrar el enrolamiento.'
      toast.error(message)
    },
  })

  useEffect(() => {
    if (sessionIdentityRef.current === sessionIdentity) return
    sessionIdentityRef.current = sessionIdentity
    generationRef.current += 1
    const previousEnrollmentId = enrollmentIdRef.current
    if (previousEnrollmentId) {
      void queryClient.cancelQueries({ queryKey: ['enrollment', previousEnrollmentId] })
      toast.dismiss(`enrollment-${previousEnrollmentId}`)
    }
    enrollmentIdRef.current = initialEnrollmentId
    immediatePushesRef.current = []
    terminalHandledRef.current = null
    closedDuringSubmitRef.current = false
    setEnrollmentId(initialEnrollmentId)
    setTab('hikvision')
    setCapturedPhoto(null)
    setAnalyzing(false)
    submitMutation.reset()
  // Session identity is the only reset boundary; closing the same session must keep polling.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionIdentity])

  const { data: enrollmentStatus } = useQuery<Enrollment>({
    queryKey: ['enrollment', enrollmentId],
    queryFn: () => getEnrollment(enrollmentId as string),
    enabled: enrollmentId !== null,
    refetchInterval: (query) => {
      const enrollment = query.state.data as Enrollment | undefined
      if (!enrollment) return 1500
      return shouldPollEnrollment(enrollment) ? 1500 : false
    },
  })

  useEffect(() => {
    if (
      !enrollmentStatus
      || !enrollmentId
      || enrollmentIdRef.current !== enrollmentId
      || enrollmentStatus.id !== enrollmentId
    ) return

    if (shouldPollEnrollment(enrollmentStatus)) {
      terminalHandledRef.current = null
      return
    }
    if (terminalHandledRef.current === enrollmentId) return
    terminalHandledRef.current = enrollmentId
    void queryClient.invalidateQueries({ queryKey: ['enrollments', 'in_progress'] })

    const successCount = enrollmentStatus.device_pushes.filter(
      (push) => push.status === 'success',
    ).length
    const totalCount = enrollmentStatus.device_pushes.length
    const employeeName = employee?.name ?? enrollmentStatus.employee_name

    if (successCount === totalCount) {
      toast.success(`Enrolamiento completado para ${employeeName}.`, {
        id: `enrollment-${enrollmentId}`,
      })
    } else if (successCount > 0) {
      toast.warning(
        `Enrolamiento parcial: ${successCount}/${totalCount}. Revisa los dispositivos fallidos.`,
        { id: `enrollment-${enrollmentId}` },
      )
    } else {
      toast.error('Enrolamiento falló en todos los dispositivos. Reintenta desde el panel.', {
        id: `enrollment-${enrollmentId}`,
      })
    }
  }, [employee?.name, enrollmentId, enrollmentStatus, queryClient])

  function clearCapturedPhoto() {
    generationRef.current += 1
    setCapturedPhoto(null)
    setAnalyzing(false)
  }

  async function handleCandidate(candidate: CapturedPhotoCandidate) {
    const generation = ++generationRef.current
    setCapturedPhoto(null)
    setAnalyzing(true)
    try {
      const analysis = await analyzePhotoBlob(candidate.blob)
      if (generation !== generationRef.current) return
      setCapturedPhoto({ ...candidate, analysis })
    } catch {
      if (generation !== generationRef.current) return
      setCapturedPhoto(null)
      toast.error('No se pudo analizar la calidad de la foto.')
    } finally {
      if (generation === generationRef.current) setAnalyzing(false)
    }
  }

  function handleTabChange(nextTab: CaptureTab) {
    if (nextTab === tab) return
    clearCapturedPhoto()
    setTab(nextTab)
  }

  function handleSubmit() {
    if (!employee || !capturedPhoto || !isAcceptableFace(capturedPhoto.analysis)) return
    closedDuringSubmitRef.current = false
    submitMutation.mutate({
      generation: generationRef.current,
      employeeId: employee.id,
      photo: capturedPhoto,
    })
  }

  function handleClose() {
    if (submitMutation.isPending && !enrollmentId) {
      closedDuringSubmitRef.current = true
      onClose()
      return
    }

    if (enrollmentId) {
      const currentStatus = enrollmentStatus?.id === enrollmentId ? enrollmentStatus : null
      if (!currentStatus || shouldPollEnrollment(currentStatus)) {
        showRecoveryToast(
          enrollmentId,
          currentStatus?.device_pushes ?? immediatePushesRef.current,
        )
        onClose()
        return
      }

      generationRef.current += 1
      void queryClient.cancelQueries({ queryKey: ['enrollment', enrollmentId] })
      enrollmentIdRef.current = null
      immediatePushesRef.current = []
      terminalHandledRef.current = null
      closedDuringSubmitRef.current = false
      setEnrollmentId(null)
      clearCapturedPhoto()
      setTab('hikvision')
      submitMutation.reset()
      onClose()
      return
    }

    clearCapturedPhoto()
    closedDuringSubmitRef.current = false
    setTab('hikvision')
    submitMutation.reset()
    onClose()
  }

  const isSyncing = enrollmentId !== null
  const canSubmit = employee !== null
    && capturedPhoto !== null
    && isAcceptableFace(capturedPhoto.analysis)
    && !analyzing
    && !submitMutation.isPending
    && !isSyncing
  const displayName = employee?.name ?? enrollmentStatus?.employee_name
  const displayCode = employee?.employee_code ?? enrollmentStatus?.employee_code
  const tabs: Array<{ key: CaptureTab; label: string }> = [
    { key: 'hikvision', label: 'Lector Hikvision' },
    { key: 'webcam', label: 'Webcam' },
    { key: 'upload', label: 'Subir JPG' },
  ]

  return (
    <Dialog open={open} onOpenChange={(isOpen) => { if (!isOpen) handleClose() }}>
      <DialogContent
        className="max-w-[700px] p-0 overflow-hidden"
        data-testid="enrollment-modal"
      >
        <div className="flex flex-col max-h-[88vh]">
          <div className="flex items-center justify-between px-6 py-4 border-b border-[#EEF0F2]">
            <div className="flex flex-col gap-0.5">
              <h2
                className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                Enrolamiento Facial
              </h2>
              {displayName && (
                <p
                  className="text-[13px] italic text-[#666666]"
                  style={{ fontFamily: 'var(--font-serif)' }}
                >
                  {displayName}
                  {displayCode ? ` — ${displayCode}` : ''}
                </p>
              )}
            </div>
            <button
              type="button"
              aria-label="Cerrar enrolamiento"
              onClick={handleClose}
              className="flex items-center justify-center w-8 h-8 rounded hover:bg-[#F3F4F6] transition-colors"
            >
              <X size={20} className="text-[#666666]" />
            </button>
          </div>

          {!isSyncing && employee && (
            <div className="flex items-center px-6 border-b border-[#EEF0F2]">
              {tabs.map((nextTab) => {
                const active = tab === nextTab.key
                return (
                  <button
                    key={nextTab.key}
                    type="button"
                    onClick={() => handleTabChange(nextTab.key)}
                    data-testid={`enroll-tab-${nextTab.key}`}
                    className={`px-4 py-3 text-[13px] transition-colors border-b-2 ${
                      active
                        ? 'text-[#1E3FB8] font-semibold border-[#1E3FB8]'
                        : 'text-[#666666] border-transparent hover:text-[#1A1A1A]'
                    }`}
                  >
                    {nextTab.label}
                  </button>
                )
              })}
            </div>
          )}

          <div className="flex-1 px-6 py-5 flex gap-6 overflow-y-auto">
            <div className="flex-1 min-w-0">
              {!isSyncing && employee ? (
                <>
                  {tab === 'hikvision' && (
                    <KioskCaptureTab
                      employeeId={employee.id}
                      onCaptured={handleCandidate}
                      onCleared={clearCapturedPhoto}
                    />
                  )}
                  {tab === 'webcam' && (
                    <WebcamCaptureTab
                      onCaptured={handleCandidate}
                      onCleared={clearCapturedPhoto}
                    />
                  )}
                  {tab === 'upload' && (
                    <UploadCaptureTab
                      onCaptured={handleCandidate}
                      onCleared={clearCapturedPhoto}
                    />
                  )}
                </>
              ) : (
                <p className="text-[13px] text-[#666666]">
                  Enrolamiento enviado. Monitoreando sincronización por dispositivo…
                </p>
              )}
            </div>

            <div className="w-[280px] shrink-0 flex flex-col gap-5">
              {!isSyncing && (
                <ValidationPanel analysis={capturedPhoto?.analysis ?? null} analyzing={analyzing} />
              )}

              {enrollmentId && enrollmentStatus?.id === enrollmentId && (
                <SyncPanel
                  device_pushes={enrollmentStatus.device_pushes}
                  enrollmentId={enrollmentId}
                />
              )}
            </div>
          </div>

          <div className="flex items-center justify-between px-6 py-3 border-t border-[#EEF0F2] bg-[#FAFBFC]">
            <p className="text-[11px] text-[#666666] flex-1 pr-4">
              {isSyncing
                ? 'La sincronización continúa aunque cierres esta ventana.'
                : analyzing
                  ? 'Analizando la calidad de la foto.'
                  : !capturedPhoto
                    ? 'Captura una foto para continuar.'
                    : !isAcceptableFace(capturedPhoto.analysis)
                      ? 'La foto no cumple las validaciones de IA.'
                      : 'Listo para enrolar.'}
            </p>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={handleClose}
                className="px-6 py-2.5 rounded text-[13px] font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50 transition-colors"
              >
                Cerrar
              </button>
              {!isSyncing && employee && (
                <PrimaryButton
                  type="button"
                  size="md"
                  disabled={!canSubmit}
                  aria-disabled={!canSubmit}
                  onClick={handleSubmit}
                >
                  {submitMutation.isPending ? 'Enviando…' : 'Enrolar'}
                </PrimaryButton>
              )}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
