'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { ScanFace } from 'lucide-react'
import { PrimaryButton } from '@/components/ui/primary-button'
import type { Employee, PaginatedResponse } from '@/types/api'

interface EmployeeEnrollmentPickerProps {
  onSelect: (employee: Employee) => void
}

export function EmployeeEnrollmentPicker({ onSelect }: EmployeeEnrollmentPickerProps) {
  const [selectedId, setSelectedId] = useState('')

  const { data } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees-active-picker'],
    queryFn: () => api.get('/employees?status=active&limit=100').then(r => r.data),
  })
  const employees = data?.data ?? []

  function handleStart() {
    const emp = employees.find(e => e.id === selectedId)
    if (emp) onSelect(emp)
  }

  return (
    <div className="flex items-center gap-2">
      <select
        value={selectedId}
        onChange={e => setSelectedId(e.target.value)}
        className="rounded-md border border-slate-200 px-3 py-2 text-sm"
        aria-label="Selecciona un empleado"
      >
        <option value="">Selecciona un empleado…</option>
        {employees.map(emp => (
          <option key={emp.id} value={emp.id}>
            {emp.name} — {emp.employee_code}
          </option>
        ))}
      </select>
      <PrimaryButton size="sm" icon={ScanFace} disabled={!selectedId} onClick={handleStart} type="button">
        Iniciar Enrolamiento
      </PrimaryButton>
    </div>
  )
}
