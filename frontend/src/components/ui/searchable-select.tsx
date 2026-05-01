'use client'

import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { Check, ChevronDown, X } from 'lucide-react'
import { cn } from '@/lib/utils'

export interface SearchableOption {
  id: string
  label: string
  sublabel?: string
}

interface SearchableSelectProps {
  value: string | null
  onChange: (id: string) => void
  options: SearchableOption[]
  placeholder?: string
  emptyText?: string
  loading?: boolean
  disabled?: boolean
  id?: string
  'data-testid'?: string
}

export function SearchableSelect({
  value,
  onChange,
  options,
  placeholder = 'Buscar…',
  emptyText = 'Sin resultados',
  loading = false,
  disabled = false,
  id,
  ...rest
}: SearchableSelectProps) {
  const generatedId = useId()
  const inputId = id ?? generatedId
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const wrapperRef = useRef<HTMLDivElement>(null)

  const selected = useMemo(
    () => options.find((o) => o.id === value) ?? null,
    [options, value],
  )

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return options
    return options.filter((o) => {
      const haystack = (o.label + ' ' + (o.sublabel ?? '')).toLowerCase()
      return haystack.includes(q)
    })
  }, [options, query])

  // Close on outside click
  useEffect(() => {
    if (!open) return
    function onClick(e: MouseEvent) {
      if (!wrapperRef.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', onClick)
    return () => document.removeEventListener('mousedown', onClick)
  }, [open])

  const handleSelect = (optId: string) => {
    onChange(optId)
    setOpen(false)
    setQuery('')
  }

  const handleClear = () => {
    onChange('')
    setQuery('')
  }

  return (
    <div ref={wrapperRef} className="relative">
      <button
        id={inputId}
        type="button"
        disabled={disabled}
        onClick={() => !disabled && setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        data-testid={rest['data-testid']}
        className={cn(
          'mt-1 w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm text-left flex items-center justify-between gap-2',
          'focus:outline-none focus:ring-2 focus:ring-slate-300',
          disabled && 'opacity-50 cursor-not-allowed',
        )}
      >
        <span
          className={cn(
            'truncate',
            !selected && 'text-slate-400',
          )}
        >
          {selected ? selected.label : placeholder}
        </span>
        <span className="flex items-center gap-1 shrink-0">
          {selected && !disabled && (
            <X
              size={14}
              className="text-slate-400 hover:text-slate-700"
              role="button"
              aria-label="Limpiar selección"
              onClick={(e) => {
                e.stopPropagation()
                handleClear()
              }}
            />
          )}
          <ChevronDown size={16} className="text-slate-400" />
        </span>
      </button>

      {open && (
        <div
          role="listbox"
          className="absolute z-50 mt-1 w-full rounded-md border border-slate-200 bg-white shadow-lg max-h-64 overflow-hidden flex flex-col"
        >
          <input
            type="text"
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={placeholder}
            className="px-3 py-2 text-sm border-b border-slate-100 focus:outline-none"
          />
          <ul className="overflow-auto flex-1">
            {loading && (
              <li className="px-3 py-2 text-sm text-slate-500">Cargando…</li>
            )}
            {!loading && filtered.length === 0 && (
              <li className="px-3 py-2 text-sm text-slate-500">{emptyText}</li>
            )}
            {!loading &&
              filtered.map((o) => {
                const isSelected = o.id === value
                return (
                  <li
                    key={o.id}
                    role="option"
                    aria-selected={isSelected}
                    onClick={() => handleSelect(o.id)}
                    className={cn(
                      'px-3 py-2 text-sm cursor-pointer flex items-center justify-between gap-2',
                      isSelected ? 'bg-slate-100' : 'hover:bg-slate-50',
                    )}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-slate-900">{o.label}</div>
                      {o.sublabel && (
                        <div className="truncate text-xs text-slate-500">{o.sublabel}</div>
                      )}
                    </div>
                    {isSelected && <Check size={14} className="text-slate-700 shrink-0" />}
                  </li>
                )
              })}
          </ul>
        </div>
      )}
    </div>
  )
}
