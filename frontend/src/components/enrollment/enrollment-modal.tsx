'use client'
import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
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

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-5xl w-full max-h-[88vh] overflow-y-auto p-0">
        <DialogHeader className="px-6 pt-6 pb-0">
          <DialogTitle>
            Enrolamiento Facial
            {employee ? ` — ${employee.name}` : ''}
          </DialogTitle>
        </DialogHeader>

        <div className="px-6 py-4 flex gap-6">
          {/* Left column: capture tabs */}
          <div className="flex-1 min-w-0">
            {!isSyncing ? (
              <Tabs
                value={tab}
                onValueChange={(v) => {
                  setTab(v as CaptureTab)
                  setPhotoBlob(null)
                  setAllValidationGreen(false)
                }}
              >
                <TabsList>
                  <TabsTrigger value="hikvision">Lector Hikvision</TabsTrigger>
                  <TabsTrigger value="webcam">Webcam</TabsTrigger>
                  <TabsTrigger value="upload">Subir JPG</TabsTrigger>
                </TabsList>

                <div className="mt-4">
                  <TabsContent value="hikvision">
                    {employee && (
                      <KioskCaptureTab
                        employeeId={employee.id}
                        onCaptured={(blob) => setPhotoBlob(blob)}
                      />
                    )}
                  </TabsContent>

                  <TabsContent value="webcam">
                    <WebcamCaptureTab
                      onCaptured={(blob) => setPhotoBlob(blob)}
                      onValidationChange={setAllValidationGreen}
                    />
                  </TabsContent>

                  <TabsContent value="upload">
                    <UploadCaptureTab
                      onCaptured={(file) => {
                        setPhotoBlob(file)
                        // Upload tab: skip face-api validation — server validates
                        setAllValidationGreen(true)
                      }}
                    />
                  </TabsContent>
                </div>
              </Tabs>
            ) : (
              <p className="text-sm text-slate-500">
                Enrolamiento enviado. Monitoreando sincronización por dispositivo…
              </p>
            )}
          </div>

          {/* Right column: validation + sync */}
          <div className="w-64 shrink-0 space-y-6">
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

        <DialogFooter className="px-6 pb-6 gap-2">
          <p className="text-xs text-slate-400 flex-1">
            {!photoBlob
              ? 'Captura una foto para continuar.'
              : !allValidationGreen && tab !== 'upload'
              ? 'Espera que las validaciones de IA sean verdes.'
              : 'Listo para enrolar.'}
          </p>
          <Button variant="outline" onClick={handleClose} type="button">
            Cerrar
          </Button>
          {!isSyncing && (
            <Button
              type="button"
              disabled={!canSubmit}
              aria-disabled={!canSubmit}
              onClick={() => submitMutation.mutate()}
            >
              {submitMutation.isPending ? 'Enviando…' : 'Enrolar'}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
