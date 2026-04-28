import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SSEReconnectBanner } from '../sse-reconnect-banner'

describe('SSEReconnectBanner', () => {
  it('renders nothing when reconnecting=false (connected state)', () => {
    const { container } = render(<SSEReconnectBanner reconnecting={false} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders Spanish reconnect copy when reconnecting=true', () => {
    render(<SSEReconnectBanner reconnecting={true} />)
    expect(screen.getByText(/Conexión perdida — reconectando…/)).toBeTruthy()
  })

  it('banner uses the orange-500 background utility class when shown', () => {
    const { container } = render(<SSEReconnectBanner reconnecting={true} />)
    const root = container.firstChild as HTMLElement
    expect(root.className).toContain('bg-orange-500')
  })
})
