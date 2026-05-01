'use client'
import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import { X } from 'lucide-react'
import {
  Dialog, DialogContent,
} from '@/components/ui/dialog'
import { PrimaryButton } from '@/components/ui/primary-button'
import { ValidationPanel } from './validation-panel'
import { SyncPanel } from './sync-panel'
import { KioskCaptureTab } from './kiosk-capture-tab'
import { WebcamCaptureTab } from './webcam-capture-tab'
import { UploadCaptureTab } from './upload-capture-tab'
import type { Employee, Enrollment } from '@/types/api'

type CaptureTab = 'hikvision' | 'webcam' | 'upload'

interface EnrollmentModalProps {
  open: boolean
  employee: Employee | null
  onClose: () => void
}

function isTerminal(enrollment: Enrollment): boolean {
  return enrollment.device_pushes.every(
    p => p.status === 'success' || p.status === 'failed'
  )
}

export function EnrollmentModal({ open, employee, onClose }: EnrollmentModalProps) {
  const [tab, setTab] = useState<CaptureTab>('hikvision')
  const [photoBlob, setPhotoBlob] = useState<Blob | null>(null)
  const [enrollmentId, setEnrollmentId] = useState<string | null>(null)
  const [allValidationGreen, setAllValidationGreen] = useState(false)
  const [selectedKioskDevice, setSelectedKioskDevice] = useState('')
  const videoRef = useRef<HTMLVideoElement | null>(null)

  // Reset state when modal closes/opens for a new employee
  useEffect(() => {
    if (!open) {
      setTab('hikvision')
      setPhotoBlob(null)
      setAllValidationGreen(false)
    }
  }, [open])

  // Submit enrollment mutation
  const submitMutation = useMutation({
    mutationFn: () => {
      if (!employee || !photoBlob) throw new Error('Missing employee or photo')
      const fd = new FormData()
      fd.append('employee_id', employee.id)
      fd.append(
        'captured_via',
        tab === 'hikvision' ? 'device' : tab === 'webcam' ? 'webcam' : 'upload'
      )
      if (tab === 'hikvision' && selectedKioskDevice) {
        fd.append('source_device_id', selectedKioskDevice)
      }
      fd.append('photo', photoBlob)
      return api
        .post('/enrollments', fd, { headers: { 'Content-Type': 'multipart/form-data' } })
        .then(r => r.data as { enrollment_id: string; device_pushes: Enrollment['device_pushes'] })
    },
    onSuccess: (data) => {
      setEnrollmentId(data.enrollment_id)
    },
    onError: (err: unknown) => {
      const msg =
        (err as { response?: { data?: { message?: string } } })?.response?.data?.message ??
        'No se pudo registrar el enrolamiento.'
      toast.error(msg)
    },
  })

  // Poll enrollment status
  const { data: enrollmentStatus } = useQuery<Enrollment>({
    queryKey: ['enrollment', enrollmentId],
    queryFn: () => api.get(`/enrollments/${enrollmentId}`).then(r => r.data),
    enabled: !!enrollmentId,
    refetchInterval: (q) => {
      const d = q.state.data as Enrollment | undefined
      if (!d) return 1500
      return isTerminal(d) ? false : 1500
    },
  })

  // Terminal toast — fires once when status flips to all-terminal
  useEffect(() => {
    if (!enrollmentStatus || !enrollmentId || !employee) return
    if (!isTerminal(enrollmentStatus)) return

    const succ = enrollmentStatus.device_pushes.filter(p => p.status === 'success').length
    const tot = enrollmentStatus.device_pushes.length

    if (succ === tot) {
      toast.success(`Enrolamiento completado para ${employee.name}.`, {
        id: `enrollment-${enrollmentId}`,
      })
    } else if (succ > 0) {
      toast.warning(
        `Enrolamiento parcial: ${succ}/${tot}. Revisa los dispositivos fallidos.`,
        { id: `enrollment-${enrollmentId}` }
      )
    } else {
      toast.error('Enrolamiento falló en todos los dispositivos. Reintenta desde el panel.', {
        id: `enrollment-${enrollmentId}`,
      })
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    enrollmentStatus?.status,
    enrollmentStatus?.device_pushes.map(p => p.status).join(','),
  ])

  // Modal close behavior (D-09): keep polling alive, fire sticky toast
  function handleClose() {
    if (
      enrollmentId &&
      enrollmentStatus &&
      !isTerminal(enrollmentStatus)
    ) {
      const successCount = enrollmentStatus.device_pushes.filter(
        p => p.status === 'success'
      ).length
      toast(
        `Enrolamiento en curso — ${successCount}/${enrollmentStatus.device_pushes.length} dispositivos`,
        { id: `enrollment-${enrollmentId}`, duration: Infinity }
      )
    }
    onClose()
  }

  function handleOpenChange(isOpen: boolean) {
    if (!isOpen) handleClose()
  }

  const isSyncing = !!enrollmentId
  const canSubmit = !!photoBlob && allValidationGreen && !submitMutation.isPending && !isSyncing

  const TABS: Array<{ key: CaptureTab; label: string }> = [
    { key: 'hikvision', label: 'Lector Hikvision' },
    { key: 'webcam', label: 'Webcam' },
    { key: 'upload', label: 'Subir JPG' },
  ]

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        className="max-w-[700px] p-0 overflow-hidden"
        data-testid="enrollment-modal"
      >
        <div className="flex flex-col max-h-[88vh]">
          {/* ── Header ──────────────────────────────────────────── */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-[#EEF0F2]">
            <div className="flex flex-col gap-0.5">
              <h2
                className="text-[20px] font-bold text-[#1A1A1A] leading-tight"
                style={{ fontFamily: 'var(--font-sans)' }}
              >
                Enrolamiento Facial
              </h2>
              {employee && (
                <p
                  className="text-[13px] italic text-[#666666]"
                  style={{ fontFamily: 'var(--font-serif)' }}
                >
                  {employee.name}
                  {employee.employee_code ? ` — ${employee.employee_code}` : ''}
                </p>
              )}
            </div>
            <button
              type="button"
              aria-label="Cerrar"
              onClick={handleClose}
              className="flex items-center justify-center w-8 h-8 rounded hover:bg-[#F3F4F6] transition-colors"
            >
              <X size={20} className="text-[#666666]" />
            </button>
          </div>

          {/* ── Tabs ────────────────────────────────────────────── */}
          {!isSyncing && (
            <div className="flex items-center px-6 border-b border-[#EEF0F2]">
              {TABS.map((t) => {
                const active = tab === t.key
                return (
                  <button
                    key={t.key}
                    type="button"
                    onClick={() => {
                      setTab(t.key)
                      setPhotoBlob(null)
                      setAllValidationGreen(false)
                    }}
                    data-testid={`enroll-tab-${t.key}`}
                    className={`px-4 py-3 text-[13px] transition-colors border-b-2 ${
                      active
                        ? 'text-[#1E3FB8] font-semibold border-[#1E3FB8]'
                        : 'text-[#666666] border-transparent hover:text-[#1A1A1A]'
                    }`}
                  >
                    {t.label}
                  </button>
                )
              })}
            </div>
          )}

          {/* ── Body (2 columns) ────────────────────────────────── */}
          <div className="flex-1 px-6 py-5 flex gap-6 overflow-y-auto">
            {/* Left column: capture content */}
            <div className="flex-1 min-w-0">
              {!isSyncing ? (
                <>
                  {tab === 'hikvision' && employee && (
                    <KioskCaptureTab
                      employeeId={employee.id}
                      onCaptured={(blob) => setPhotoBlob(blob)}
                    />
                  )}
                  {tab === 'webcam' && (
                    <WebcamCaptureTab
                      onCaptured={(blob) => setPhotoBlob(blob)}
                      onValidationChange={setAllValidationGreen}
                    />
                  )}
                  {tab === 'upload' && (
                    <UploadCaptureTab
                      onCaptured={(file) => {
                        setPhotoBlob(file)
                        setAllValidationGreen(true)
                      }}
                    />
                  )}
                </>
              ) : (
                <p className="text-[13px] text-[#666666]">
                  Enrolamiento enviado. Monitoreando sincronización por dispositivo…
                </p>
              )}
            </div>

            {/* Right column: validation + sync (280px per design) */}
            <div className="w-[280px] shrink-0 flex flex-col gap-5">
              {!isSyncing && tab === 'webcam' && (
                <ValidationPanel
                  videoRef={videoRef}
                  onValidationChange={setAllValidationGreen}
                  active={tab === 'webcam' && !photoBlob}
                />
              )}

              {enrollmentId && enrollmentStatus && (
                <SyncPanel
                  device_pushes={enrollmentStatus.device_pushes}
                  enrollmentId={enrollmentId}
                />
              )}
            </div>
          </div>

          {/* ── Footer ──────────────────────────────────────────── */}
          <div className="flex items-center justify-between px-6 py-3 border-t border-[#EEF0F2] bg-[#FAFBFC]">
            <p className="text-[11px] text-[#666666] flex-1 pr-4">
              {!photoBlob
                ? 'Captura una foto para continuar.'
                : !allValidationGreen && tab !== 'upload'
                ? 'Espera que las validaciones de IA sean verdes.'
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
              {!isSyncing && (
                <PrimaryButton
                  type="button"
                  size="md"
                  disabled={!canSubmit}
                  aria-disabled={!canSubmit}
                  onClick={() => submitMutation.mutate()}
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
