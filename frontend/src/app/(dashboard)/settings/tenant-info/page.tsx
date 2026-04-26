'use client'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import { TopBar } from '@/components/layout/top-bar'
import { TenantInfoForm } from '@/components/settings/tenant-info-form'
import type { TenantInfo } from '@/types/api'

export default function TenantInfoPage() {
  const { role } = useAuth()
  const { data, isLoading, error } = useQuery<TenantInfo>({
    queryKey: ['tenant-info'],
    queryFn: () => api.get('/tenant-info').then((r) => r.data),
  })

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Datos de Empresa" />
      <div className="p-6">
        {isLoading && (
          <div className="text-sm text-slate-500">Cargando…</div>
        )}
        {error && (
          <div className="text-sm text-red-600">Error al cargar datos</div>
        )}
        {data && (
          <TenantInfoForm
            initialData={data}
            canEdit={role === 'admin'}
          />
        )}
      </div>
    </div>
  )
}
