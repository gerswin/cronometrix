'use client'
import { useForm, Controller } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { novedadSchema, type NovedadFormData } from '@/lib/validations'
import type { DailyRecord } from '@/types/api'

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'

interface NovedadModalProps {
  open: boolean
  record: DailyRecord | null
  onClose: () => void
}

export function NovedadModal({ open, record, onClose }: NovedadModalProps) {
  const queryClient = useQueryClient()

  const {
    register,
    handleSubmit,
    control,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<NovedadFormData>({
    resolver: zodResolver(novedadSchema),
    defaultValues: { tipo_novedad: 'manual', notificar_supervisor: false },
  })

  const mutation = useMutation({
    mutationFn: async (values: NovedadFormData) => {
      const fd = new FormData()
      fd.append('employee_id', values.employee_id)
      fd.append('department_id', values.department_id)
      fd.append('from_date', values.fecha_inicio)
      fd.append('to_date', values.fecha_fin)
      fd.append('leave_type', values.tipo_novedad)
      fd.append('justification', values.justification)
      if (values.motivo) fd.append('motivo', values.motivo)
      if (values.evidence) fd.append('evidence', values.evidence)

      // POST to overrides endpoint if record has id, else to /leaves
      if (record?.id) {
        await api.post(`/daily-records/${record.id}/overrides`, fd, {
          headers: { 'Content-Type': 'multipart/form-data' },
        })
      } else {
        await api.post('/leaves', fd, {
          headers: { 'Content-Type': 'multipart/form-data' },
        })
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['daily-records'] })
      queryClient.invalidateQueries({ queryKey: ['leaves'] })
      reset()
      onClose()
    },
  })

  const handleClose = () => {
    reset()
    onClose()
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) handleClose()
      }}
    >
      <DialogContent className="max-w-lg" data-testid="novedad-modal">
        <DialogHeader>
          <DialogTitle>Registrar Novedad</DialogTitle>
        </DialogHeader>

        <form
          onSubmit={handleSubmit((v) => mutation.mutate(v))}
          className="space-y-4"
        >
          {/* Estado Inicial — D-1: read-only decorative "Aprobado" (no workflow) */}
          <div className="flex items-center gap-2">
            <Label className="text-xs text-slate-500">Estado Inicial:</Label>
            <span className="text-xs font-medium px-2 py-0.5 rounded-full bg-green-100 text-green-700">
              Aprobado
            </span>
          </div>

          {/* Required fields — D-9 */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label htmlFor="employee_id">Empleado *</Label>
              <Input
                id="employee_id"
                {...register('employee_id')}
                defaultValue={record?.employee_id ?? ''}
              />
              {errors.employee_id && (
                <p className="text-xs text-red-500 mt-1">
                  {errors.employee_id.message}
                </p>
              )}
            </div>
            <div>
              <Label htmlFor="department_id">Departamento *</Label>
              <Input
                id="department_id"
                {...register('department_id')}
                defaultValue={record?.department_id ?? ''}
              />
              {errors.department_id && (
                <p className="text-xs text-red-500 mt-1">
                  {errors.department_id.message}
                </p>
              )}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label htmlFor="fecha_inicio">Fecha Inicio *</Label>
              <Input
                id="fecha_inicio"
                type="date"
                {...register('fecha_inicio')}
                defaultValue={record?.anchor_date ?? ''}
              />
              {errors.fecha_inicio && (
                <p className="text-xs text-red-500 mt-1">
                  {errors.fecha_inicio.message}
                </p>
              )}
            </div>
            <div>
              <Label htmlFor="fecha_fin">Fecha Fin *</Label>
              <Input
                id="fecha_fin"
                type="date"
                {...register('fecha_fin')}
                defaultValue={record?.anchor_date ?? ''}
              />
              {errors.fecha_fin && (
                <p className="text-xs text-red-500 mt-1">
                  {errors.fecha_fin.message}
                </p>
              )}
            </div>
          </div>

          <div>
            <Label htmlFor="tipo_novedad">Tipo de Novedad *</Label>
            <select
              id="tipo_novedad"
              {...register('tipo_novedad')}
              className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm"
            >
              <option value="medical">Médica</option>
              <option value="vacation">Vacaciones</option>
              <option value="unpaid">Sin Goce</option>
              <option value="manual">Manual</option>
            </select>
          </div>

          <div>
            <Label htmlFor="justification">
              Descripción / Justificación *
            </Label>
            <textarea
              id="justification"
              data-testid="novedad-justification"
              {...register('justification')}
              rows={3}
              className="mt-1 w-full rounded-md border border-slate-200 px-3 py-2 text-sm resize-none"
              placeholder="Describa la razón de la novedad…"
            />
            {errors.justification && (
              <p className="text-xs text-red-500 mt-1">
                {errors.justification.message}
              </p>
            )}
          </div>

          {/* Optional fields */}
          <div>
            <Label htmlFor="motivo">Motivo (opcional)</Label>
            <Input id="motivo" {...register('motivo')} />
          </div>

          {/* File upload — Pitfall 4: must use Controller, not register */}
          <div>
            <Label>Adjuntar soporte (PDF / JPG / PNG, máx. 5MB)</Label>
            <Controller
              name="evidence"
              control={control}
              render={({ field }) => (
                <input
                  type="file"
                  accept=".pdf,.jpg,.jpeg,.png"
                  data-testid="novedad-evidence"
                  onChange={(e) =>
                    field.onChange(e.target.files?.[0] ?? undefined)
                  }
                  className="mt-1 block w-full text-sm text-slate-500 file:mr-3 file:py-1 file:px-3 file:rounded file:border-0 file:text-xs file:bg-slate-100 file:text-slate-700"
                />
              )}
            />
            {errors.evidence && (
              <p className="text-xs text-red-500 mt-1">
                {errors.evidence.message as string}
              </p>
            )}
          </div>

          <DialogFooter className="gap-2">
            <Button type="button" variant="outline" onClick={handleClose}>
              Cancelar
            </Button>
            <Button
              type="submit"
              data-testid="novedad-submit"
              disabled={isSubmitting || mutation.isPending}
            >
              {mutation.isPending ? 'Registrando…' : 'Registrar Novedad'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
