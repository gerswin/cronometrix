'use client'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from '@/lib/api'
import {
  tenantInfoSchema,
  type TenantInfoFormValues,
} from '@/lib/validations'
import type { TenantInfo } from '@/types/api'

interface Props {
  initialData: TenantInfo
  canEdit: boolean
}

export function TenantInfoForm({ initialData, canEdit }: Props) {
  const queryClient = useQueryClient()
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<TenantInfoFormValues>({
    resolver: zodResolver(tenantInfoSchema),
    defaultValues: { ...initialData },
  })

  // Re-prime the form whenever the upstream tenant_info changes (e.g.
  // after a successful PATCH or a 409 invalidation refetch).
  useEffect(() => {
    reset(initialData)
  }, [initialData, reset])

  const mutation = useMutation({
    mutationFn: async (values: TenantInfoFormValues) => {
      const resp = await api.patch('/tenant-info', {
        ...values,
        version: initialData.version,
      })
      return resp.data
    },
    onSuccess: () => {
      toast.success('Datos actualizados')
      queryClient.invalidateQueries({ queryKey: ['tenant-info'] })
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response
        ?.status
      if (status === 409) {
        toast.error(
          'Esta información fue modificada por otro usuario; recargando…',
        )
        queryClient.invalidateQueries({ queryKey: ['tenant-info'] })
      } else {
        toast.error('Error al guardar')
      }
    },
  })

  return (
    <form
      onSubmit={handleSubmit((v) => mutation.mutate(v))}
      className="space-y-4 max-w-xl"
      aria-label="Datos de Empresa"
    >
      <label className="block">
        <span className="text-sm text-slate-700">Nombre del Cliente</span>
        <input
          {...register('client_name')}
          disabled={!canEdit}
          aria-label="Nombre del Cliente"
          className="mt-1 block w-full rounded-md border border-slate-200 px-3 py-2 text-sm disabled:bg-slate-50 disabled:text-slate-500"
        />
        {errors.client_name && (
          <span className="text-xs text-red-600">
            {errors.client_name.message}
          </span>
        )}
      </label>

      <label className="block">
        <span className="text-sm text-slate-700">RIF</span>
        <input
          {...register('client_rif')}
          disabled={!canEdit}
          placeholder="J-12345678-9"
          aria-label="RIF"
          className="mt-1 block w-full rounded-md border border-slate-200 px-3 py-2 text-sm disabled:bg-slate-50 disabled:text-slate-500"
        />
        {errors.client_rif && (
          <span className="text-xs text-red-600">
            {errors.client_rif.message}
          </span>
        )}
      </label>

      <label className="block">
        <span className="text-sm text-slate-700">Dirección</span>
        <textarea
          {...register('address')}
          disabled={!canEdit}
          rows={3}
          aria-label="Dirección"
          className="mt-1 block w-full rounded-md border border-slate-200 px-3 py-2 text-sm disabled:bg-slate-50 disabled:text-slate-500"
        />
        {errors.address && (
          <span className="text-xs text-red-600">
            {errors.address.message}
          </span>
        )}
      </label>

      {canEdit && (
        <button
          type="submit"
          disabled={isSubmitting || mutation.isPending}
          className="px-4 py-2 bg-slate-900 text-white text-sm rounded-md hover:bg-slate-800 disabled:opacity-50"
        >
          {mutation.isPending ? 'Guardando…' : 'Guardar Cambios'}
        </button>
      )}
    </form>
  )
}
