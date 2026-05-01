'use client'
/**
 * Universal action button — Pencil design `zlfPL` (primary variant).
 *
 * Tokens (primary):
 *   bg #1E3FB8 / hover #1835A0
 *   text white, font-semibold, gap 8px to icon
 *   rounded 4px (radius 4)
 *
 * Variants:
 *   primary  — accent fill, white text  (default)
 *   outline  — white fill, slate text, slate-200 border
 *   ghost    — transparent, slate text, slate-50 hover
 *   danger   — red fill, white text (destructive actions)
 *
 * Sizes:
 *   sm  h-9  px-4 text-[13px]   (toolbar)
 *   md  h-10 px-5 text-[14px]   (modal submit, default)
 *   lg  h-12 px-6 text-[15px]   (login / hero)
 *
 * Usage:
 *   <PrimaryButton size="md" icon={Save} onClick={...}>Guardar</PrimaryButton>
 *   <PrimaryButton variant="outline" onClick={...}>Cancelar</PrimaryButton>
 *   <PrimaryButton variant="danger" icon={Trash2}>Eliminar</PrimaryButton>
 */
import { forwardRef, type ButtonHTMLAttributes, type ComponentType } from 'react'
import { type LucideProps } from 'lucide-react'

type Size = 'sm' | 'md' | 'lg'
type Variant = 'primary' | 'outline' | 'ghost' | 'danger'

const SIZE_CLS: Record<Size, string> = {
  sm: 'h-9 px-4 text-[13px]',
  md: 'h-10 px-5 text-[14px]',
  lg: 'h-12 px-6 text-[15px]',
}
const ICON_SIZE: Record<Size, number> = {
  sm: 14,
  md: 16,
  lg: 18,
}

const VARIANT_CLS: Record<Variant, string> = {
  primary:
    'font-semibold text-white bg-[#1E3FB8] hover:bg-[#1835A0]',
  outline:
    'font-medium text-[#1A1A1A] bg-white border border-[#EEF0F2] hover:bg-slate-50',
  ghost:
    'font-medium text-[#666666] bg-transparent hover:bg-[#F3F4F6] hover:text-[#1A1A1A]',
  danger:
    'font-semibold text-white bg-[#DC2626] hover:bg-[#B91C1C]',
}

interface PrimaryButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  size?: Size
  variant?: Variant
  icon?: ComponentType<LucideProps>
  loading?: boolean
}

export const PrimaryButton = forwardRef<HTMLButtonElement, PrimaryButtonProps>(
  function PrimaryButton(
    {
      size = 'md',
      variant = 'primary',
      icon: Icon,
      loading,
      disabled,
      className = '',
      children,
      ...rest
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={[
          'inline-flex items-center justify-center gap-2 rounded',
          'transition-colors',
          'disabled:opacity-60 disabled:cursor-not-allowed',
          VARIANT_CLS[variant],
          SIZE_CLS[size],
          className,
        ].join(' ')}
        {...rest}
      >
        {Icon && <Icon size={ICON_SIZE[size]} aria-hidden />}
        {children}
      </button>
    )
  },
)
