'use client'
import { useState } from 'react'
import { useAuth } from '@/hooks/use-auth'
import { AccessRestricted } from '@/components/common/access-restricted'
import { EnrollmentModal } from '@/components/enrollment/enrollment-modal'
import { EmployeeEnrollmentPicker } from '@/components/enrollment/employee-enrollment-picker'
import { InProgressList } from '@/components/enrollment/in-progress-list'
import { TopBar } from '@/components/layout/top-bar'
import type { Employee } from '@/types/api'

export default function EnrollmentPage() {
  const { role } = useAuth()
  const [selectedEmployee, setSelectedEmployee] = useState<Employee | null>(null)
  const [resumeEnrollmentId, setResumeEnrollmentId] = useState<string | null>(null)
  const [modalOpen, setModalOpen] = useState(false)

  if (role !== 'admin') return <AccessRestricted />

  function handleSelect(emp: Employee) {
    setSelectedEmployee(emp)
    setResumeEnrollmentId(null)
    setModalOpen(true)
  }

  function handleReopen(enrollmentId: string) {
    setSelectedEmployee(null)
    setResumeEnrollmentId(enrollmentId)
    setModalOpen(true)
  }

  return (
    <div className="flex flex-col h-full" data-testid="enrollment-page">
      <TopBar title="Enrolamiento Facial" />
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-semibold">Enrolamiento Facial</h1>
          <EmployeeEnrollmentPicker onSelect={handleSelect} />
        </div>

        <InProgressList onReopen={handleReopen} />
      </div>

      <EnrollmentModal
        open={modalOpen}
        employee={selectedEmployee}
        initialEnrollmentId={resumeEnrollmentId}
        onClose={() => setModalOpen(false)}
      />
    </div>
  )
}
