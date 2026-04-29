'use client'
import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import type { Device } from '@/types/api'

const COMMANDS = [
  { value: 'door_open', label: 'Abrir Puerta' },
  { value: 'reboot', label: 'Reiniciar Dispositivo' },
  { value: 'enrollment_mode', label: 'Modo Enrolamiento' },
] as const

type CommandValue = typeof COMMANDS[number]['value']

interface CommandModalProps {
  open: boolean
  device: Device | null
  onClose: () => void
}

export function CommandModal({ open, device, onClose }: CommandModalProps) {
  const [selectedCommand, setSelectedCommand] = useState<CommandValue>('door_open')

  const mutation = useMutation({
    mutationFn: () =>
      api.post(`/devices/${device!.id}/commands`, { command: selectedCommand }),
    onSuccess: () => {
      toast.success(`Comando "${COMMANDS.find(c => c.value === selectedCommand)?.label}" enviado`)
      onClose()
    },
    onError: (err: unknown) => {
      const message = (err as { response?: { data?: { message?: string } } })?.response?.data?.message ?? 'Error al enviar comando'
      toast.error(message)
    },
  })

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose() }}>
      <DialogContent className="max-w-sm" data-testid="command-modal">
        <DialogHeader>
          <DialogTitle>Enviar Comando ISAPI</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <p className="text-sm text-slate-600">
            Dispositivo: <strong>{device?.name}</strong> ({device?.ip_address})
          </p>
          <div>
            <label className="text-xs text-slate-500 font-medium uppercase tracking-wide">Comando</label>
            <select
              value={selectedCommand}
              onChange={e => setSelectedCommand(e.target.value as CommandValue)}
              className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
              data-testid="command-modal-select"
            >
              {COMMANDS.map(c => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>
          {selectedCommand === 'reboot' && (
            <p className="text-xs text-amber-600 bg-amber-50 px-3 py-2 rounded-md">
              Advertencia: El dispositivo perderá conexión temporalmente.
            </p>
          )}
        </div>
        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button
            onClick={() => mutation.mutate()}
            disabled={mutation.isPending}
            data-testid="command-modal-submit"
          >
            {mutation.isPending ? 'Enviando…' : 'Enviar Comando'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
