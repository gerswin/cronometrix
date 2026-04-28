import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { KPITile } from '../kpi-tile'

describe('KPITile', () => {
  it('renders title and value', () => {
    render(<KPITile title="Empleados Activos" value={42} />)
    expect(screen.getByText('Empleados Activos')).toBeTruthy()
    expect(screen.getByText('42')).toBeTruthy()
  })

  it('renders string value', () => {
    render(<KPITile title="Hora Actual" value="14:35" />)
    expect(screen.getByText('14:35')).toBeTruthy()
  })

  it('renders sub element below value when provided', () => {
    render(
      <KPITile
        title="Asistencia"
        value="95%"
        sub={<span>↑ 3% vs semana anterior</span>}
      />
    )
    expect(screen.getByText(/3% vs semana anterior/)).toBeTruthy()
  })

  it('omits sub when not provided', () => {
    const { container } = render(<KPITile title="X" value={1} />)
    // Only one .mt-1 wrapper if sub was passed; without sub there should be no <div className="mt-1">
    const mt1 = container.querySelector('.mt-1')
    expect(mt1).toBeNull()
  })

  it('default variant does not apply warning or danger border classes', () => {
    const { container } = render(<KPITile title="OK" value={1} />)
    const root = container.firstChild as HTMLElement
    expect(root.className).not.toContain('border-yellow-300')
    expect(root.className).not.toContain('border-red-300')
  })

  it('warning variant applies the yellow border class', () => {
    const { container } = render(<KPITile title="WARN" value={1} variant="warning" />)
    const root = container.firstChild as HTMLElement
    expect(root.className).toContain('border-yellow-300')
  })

  it('danger variant applies the red border class', () => {
    const { container } = render(<KPITile title="DANGER" value={1} variant="danger" />)
    const root = container.firstChild as HTMLElement
    expect(root.className).toContain('border-red-300')
  })
})
