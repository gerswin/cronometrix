import { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface KPITileProps {
  title: string
  value: string | number
  sub?: ReactNode
  /** Explicit hex/CSS color for the big number. Overrides `variant` coloring. */
  valueColor?: string
  variant?: 'default' | 'warning' | 'danger'
  testId?: string
}

export function KPITile({ title, value, sub, valueColor, variant = 'default', testId }: KPITileProps) {
  const numberColor = valueColor ?? (
    variant === 'warning' ? '#F59E0B' :
    variant === 'danger'  ? '#EF4444' :
    '#1A1A1A'
  )

  return (
    <div
      data-testid={testId}
      className={cn(
        'rounded bg-white p-5 flex flex-col gap-2',
        'border border-[#EEF0F2]',
        '[box-shadow:0_2px_4px_#00000008,0_6px_16px_#0000000d]',
      )}
    >
      <p
        className="text-[12px] tracking-[0.5px] text-[#666666]"
        style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
      >
        {title}
      </p>
      <p
        className="text-[36px] font-bold leading-none"
        style={{ fontFamily: 'var(--font-mono)', color: numberColor }}
      >
        {value}
      </p>
      {sub && (
        <div className="text-[12px] text-[#666666]">{sub}</div>
      )}
    </div>
  )
}
