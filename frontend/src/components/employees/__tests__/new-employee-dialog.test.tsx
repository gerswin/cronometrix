import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { NewEmployeeDialog } from '../new-employee-dialog'
import type { Department } from '@/types/api'

function makeDepartment(over: Partial<Department> = {}): Department {
  return {
    id: 'dept-1',
    name: 'Producción',
    base_salary_cents: 100000,
    shift_start_time: '08:00',
    shift_end_time: '17:00',
    lunch_mode: 'fixed',
    lunch_duration_min: 60,
    status: 'active',
    deleted_at: null,
    version: 1,
    created_at: '2023-01-01T00:00:00Z',
    updated_at: '2023-01-01T00:00:00Z',
    ...over,
  }
}

function fillMandatoryFieldsExceptSalaryUnit() {
  fireEvent.change(screen.getByLabelText(/^Nombre completo/), {
    target: { value: 'Ana Pérez' },
  })
  fireEvent.change(screen.getByLabelText(/^Cédula/), {
    target: { value: 'V-12345678' },
  })
  fireEvent.change(screen.getByLabelText(/^Departamento/), {
    target: { value: 'dept-1' },
  })
  fireEvent.change(screen.getByTestId('new-employee-base-salary'), {
    target: { value: '500.00' },
  })
}

function renderDialog(overrides: Partial<React.ComponentProps<typeof NewEmployeeDialog>> = {}) {
  const onSubmit = vi.fn()
  const onClose = vi.fn()
  const onEnrollAfterSaveChange = vi.fn()
  render(
    <NewEmployeeDialog
      open
      departments={[makeDepartment()]}
      enrollAfterSave
      onEnrollAfterSaveChange={onEnrollAfterSaveChange}
      onClose={onClose}
      onSubmit={onSubmit}
      isPending={false}
      {...overrides}
    />
  )
  return { onSubmit, onClose, onEnrollAfterSaveChange }
}

describe('NewEmployeeDialog', () => {
  it('renders with no salary unit preselected', () => {
    renderDialog()
    const select = screen.getByTestId('new-employee-salary-kind') as HTMLSelectElement
    expect(select.value).toBe('')
  })

  it('shows the three Spanish unit options: por hora / diario / mensual', () => {
    renderDialog()
    const select = screen.getByTestId('new-employee-salary-kind')
    expect(screen.getByRole('option', { name: 'Por hora' })).toBeTruthy()
    expect(screen.getByRole('option', { name: 'Diario' })).toBeTruthy()
    expect(screen.getByRole('option', { name: 'Mensual' })).toBeTruthy()
    // The default option is the unselected placeholder, not any unit.
    expect((select as HTMLSelectElement).value).toBe('')
  })

  it('does NOT submit when the salary unit is left unselected', async () => {
    const { onSubmit } = renderDialog()
    fillMandatoryFieldsExceptSalaryUnit()
    // salary_kind intentionally left as the unselected placeholder ('')

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => {
      expect(screen.getByText('Selecciona la unidad del sueldo')).toBeTruthy()
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('does NOT submit when the salary amount is blank, even with a unit chosen', async () => {
    const { onSubmit } = renderDialog()
    fireEvent.change(screen.getByLabelText(/^Nombre completo/), {
      target: { value: 'Ana Pérez' },
    })
    fireEvent.change(screen.getByLabelText(/^Cédula/), {
      target: { value: 'V-12345678' },
    })
    fireEvent.change(screen.getByLabelText(/^Departamento/), {
      target: { value: 'dept-1' },
    })
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'monthly' },
    })
    // base_salary left blank

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => {
      expect(screen.getByText('Sueldo es requerido')).toBeTruthy()
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('submits with salary_kind: "monthly" and the amount converted to cents once a unit is chosen', async () => {
    const { onSubmit } = renderDialog()
    fillMandatoryFieldsExceptSalaryUnit()
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'monthly' },
    })

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1))
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        employee_code: 'V-12345678',
        name: 'Ana Pérez',
        department_id: 'dept-1',
        base_salary_cents: 50000,
        salary_kind: 'monthly',
      })
    )
  })

  it('submits with salary_kind: "hourly" when that unit is chosen', async () => {
    const { onSubmit } = renderDialog()
    fillMandatoryFieldsExceptSalaryUnit()
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'hourly' },
    })

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1))
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ salary_kind: 'hourly' })
    )
  })

  it('relabels the amount field to name the chosen unit instead of the old ambiguous label', () => {
    renderDialog()
    // Before a unit is chosen, the generic legacy-shaped label is shown.
    expect(screen.getByText('Sueldo Base ($)')).toBeTruthy()

    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'monthly' },
    })
    expect(screen.getByText('Monto por mes ($)')).toBeTruthy()
    expect(screen.queryByText('Sueldo Base ($)')).toBeNull()

    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'daily' },
    })
    expect(screen.getByText('Monto por día ($)')).toBeTruthy()

    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'hourly' },
    })
    expect(screen.getByText('Monto por hora ($)')).toBeTruthy()
  })

  it('does not submit when required name/cédula/departamento fields are missing', async () => {
    const { onSubmit } = renderDialog()
    fireEvent.click(screen.getByTestId('new-employee-submit'))
    await waitFor(() => {
      expect(screen.getByText('Nombre es requerido')).toBeTruthy()
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('rejects a non-numeric salary amount', async () => {
    const { onSubmit } = renderDialog()
    fireEvent.change(screen.getByLabelText(/^Nombre completo/), {
      target: { value: 'Ana Pérez' },
    })
    fireEvent.change(screen.getByLabelText(/^Cédula/), {
      target: { value: 'V-12345678' },
    })
    fireEvent.change(screen.getByLabelText(/^Departamento/), {
      target: { value: 'dept-1' },
    })
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'monthly' },
    })
    fireEvent.change(screen.getByTestId('new-employee-base-salary'), {
      target: { value: 'abc' },
    })

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => {
      expect(
        screen.getByText('Sueldo debe ser un número válido (solo dígitos, máx. 2 decimales)')
      ).toBeTruthy()
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('populates department options from the departments prop', () => {
    renderDialog({
      departments: [
        makeDepartment({ id: 'd1', name: 'Producción' }),
        makeDepartment({ id: 'd2', name: 'Administración' }),
      ],
    })
    expect(screen.getByRole('option', { name: 'Producción' })).toBeTruthy()
    expect(screen.getByRole('option', { name: 'Administración' })).toBeTruthy()
  })

  it('calls onClose when Cancelar is clicked', () => {
    const { onClose } = renderDialog()
    fireEvent.click(screen.getByText('Cancelar'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('calls onClose when the X button is clicked', () => {
    const { onClose } = renderDialog()
    fireEvent.click(screen.getByLabelText('Cerrar'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('toggles enrollAfterSave via the checkbox', () => {
    const { onEnrollAfterSaveChange } = renderDialog({ enrollAfterSave: true })
    fireEvent.click(screen.getByTestId('enroll-after-save'))
    expect(onEnrollAfterSaveChange).toHaveBeenCalledWith(false)
  })

  it('disables the submit button and shows "Guardando…" while isPending', () => {
    renderDialog({ isPending: true })
    const btn = screen.getByTestId('new-employee-submit') as HTMLButtonElement
    expect(btn.disabled).toBe(true)
    expect(screen.getByText('Guardando…')).toBeTruthy()
  })

  it('renders nothing in the DOM when closed', () => {
    renderDialog({ open: false })
    expect(screen.queryByTestId('new-employee-form')).toBeNull()
  })

  it('resets its fields after being closed and reopened', () => {
    const { rerender } = render(
      <NewEmployeeDialog
        open
        departments={[makeDepartment()]}
        enrollAfterSave
        onEnrollAfterSaveChange={() => {}}
        onClose={() => {}}
        onSubmit={() => {}}
        isPending={false}
      />
    )
    fireEvent.change(screen.getByLabelText(/^Nombre completo/), {
      target: { value: 'Valor Temporal' },
    })
    expect((screen.getByLabelText(/^Nombre completo/) as HTMLInputElement).value).toBe(
      'Valor Temporal'
    )

    rerender(
      <NewEmployeeDialog
        open={false}
        departments={[makeDepartment()]}
        enrollAfterSave
        onEnrollAfterSaveChange={() => {}}
        onClose={() => {}}
        onSubmit={() => {}}
        isPending={false}
      />
    )
    rerender(
      <NewEmployeeDialog
        open
        departments={[makeDepartment()]}
        enrollAfterSave
        onEnrollAfterSaveChange={() => {}}
        onClose={() => {}}
        onSubmit={() => {}}
        isPending={false}
      />
    )

    expect((screen.getByLabelText(/^Nombre completo/) as HTMLInputElement).value).toBe('')
  })

  it('omits optional position/hire_date from the payload when left blank', async () => {
    const { onSubmit } = renderDialog()
    fillMandatoryFieldsExceptSalaryUnit()
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'daily' },
    })

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1))
    const payload = onSubmit.mock.calls[0][0]
    expect(payload.position).toBeUndefined()
    expect(payload.hire_date).toBeUndefined()
  })

  it('includes position and hire_date in the payload when provided', async () => {
    const { onSubmit } = renderDialog()
    fillMandatoryFieldsExceptSalaryUnit()
    fireEvent.change(screen.getByTestId('new-employee-salary-kind'), {
      target: { value: 'daily' },
    })
    fireEvent.change(screen.getByPlaceholderText('Ej: Operario'), {
      target: { value: 'Supervisor' },
    })
    fireEvent.change(screen.getByLabelText('Fecha de Ingreso'), {
      target: { value: '2024-01-15' },
    })

    fireEvent.click(screen.getByTestId('new-employee-submit'))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1))
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ position: 'Supervisor', hire_date: '2024-01-15' })
    )
  })
})
