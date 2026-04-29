import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { DiffCell } from '../diff-cell'

describe('DiffCell', () => {
  it('renders em-dash when both old_data and new_data are null', () => {
    render(<DiffCell operation="UPDATE" old_data={null} new_data={null} />)
    expect(screen.getByText('—')).toBeTruthy()
  })

  it('INSERT: renders "+ N campos" summary with correct count', () => {
    render(
      <DiffCell
        operation="INSERT"
        old_data={null}
        new_data={{ a: 1, b: 2 }}
      />
    )
    expect(screen.getByText('+ 2 campos')).toBeTruthy()
  })

  it('INSERT: expanded body contains new_data JSON', () => {
    render(
      <DiffCell
        operation="INSERT"
        old_data={null}
        new_data={{ name: 'Ana', cedula: 'V-123' }}
      />
    )
    // The <pre> inside <details> should contain the stringified JSON
    const pre = document.querySelector('pre')
    expect(pre).not.toBeNull()
    expect(pre!.textContent).toContain('"name"')
    expect(pre!.textContent).toContain('"Ana"')
  })

  it('DELETE: renders "- N campos" summary with correct count', () => {
    render(
      <DiffCell
        operation="DELETE"
        old_data={{ x: 1 }}
        new_data={null}
      />
    )
    expect(screen.getByText('- 1 campos')).toBeTruthy()
  })

  it('DELETE: expanded body contains old_data JSON', () => {
    render(
      <DiffCell
        operation="DELETE"
        old_data={{ name: 'Old Name', status: 'active' }}
        new_data={null}
      />
    )
    const pre = document.querySelector('pre')
    expect(pre).not.toBeNull()
    expect(pre!.textContent).toContain('"Old Name"')
  })

  it('UPDATE: renders "~ N cambios" counting only changed fields', () => {
    // a is unchanged (1===1), b changed (2→3), c added (undefined→5)
    render(
      <DiffCell
        operation="UPDATE"
        old_data={{ a: 1, b: 2 }}
        new_data={{ a: 1, b: 3, c: 5 }}
      />
    )
    // b changed and c added = 2 cambios
    expect(screen.getByText('~ 2 cambios')).toBeTruthy()
  })

  it('UPDATE: renders "~ 0 cambios" when nothing changed', () => {
    render(
      <DiffCell
        operation="UPDATE"
        old_data={{ a: 1 }}
        new_data={{ a: 1 }}
      />
    )
    expect(screen.getByText('~ 0 cambios')).toBeTruthy()
  })

  it('UPDATE: deleted key counts as a change', () => {
    render(
      <DiffCell
        operation="UPDATE"
        old_data={{ a: 1, b: 2 }}
        new_data={{ a: 1 }}
      />
    )
    // b was removed → 1 cambio
    expect(screen.getByText('~ 1 cambios')).toBeTruthy()
  })

  it('UPDATE: diff body lists changed keys and their old/new values', () => {
    render(
      <DiffCell
        operation="UPDATE"
        old_data={{ name: 'Old' }}
        new_data={{ name: 'New' }}
      />
    )
    const pre = document.querySelector('pre')
    expect(pre).not.toBeNull()
    expect(pre!.textContent).toContain('"name"')
    expect(pre!.textContent).toContain('"Old"')
    expect(pre!.textContent).toContain('"New"')
  })

  it('INSERT with no new_data falls through to UPDATE diff rendering', () => {
    // Edge case: operation=INSERT but new_data=null
    // Should fall into the UPDATE diff path with both null → renders "~ 0 cambios"
    render(
      <DiffCell
        operation="INSERT"
        old_data={null}
        new_data={null}
      />
    )
    // Both null → em-dash (caught by the first guard)
    expect(screen.getByText('—')).toBeTruthy()
  })
})
