'use client'
import { HardDrive, Send } from 'lucide-react'
import { fmtDateTime } from '@/lib/format/datetime'
import type { Device } from '@/types/api'

// ── Connectivity + lifecycle pills ─────────────────────────────────────────

function ConnectionPill({
  state,
  deviceId,
}: {
  state: Device['connection_state']
  deviceId: string
}) {
  const STATUS_CONFIG = {
    online: {
      bg: '#DCFCE7',
      dot: '#22C55E',
      text: '#22C55E',
      label: 'En línea',
    },
    offline: {
      bg: '#FEE2E2',
      dot: '#EF4444',
      text: '#EF4444',
      label: 'Offline',
    },
    unknown: {
      bg: '#F1F5F9',
      dot: '#94A3B8',
      text: '#475569',
      label: 'Desconocido',
    },
  } as const
  const config = STATUS_CONFIG[state]

  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1"
      style={{ backgroundColor: config.bg }}
      data-testid={`dev-status-${deviceId}`}
    >
      <span
        className="block rounded-full w-2 h-2 shrink-0"
        style={{ backgroundColor: config.dot }}
      />
      <span
        className="text-[11px] font-medium leading-none"
        style={{ fontFamily: 'var(--font-sans)', color: config.text }}
      >
        {config.label}
      </span>
    </span>
  )
}

function LifecyclePill({ status, deviceId }: { status: Device['status']; deviceId: string }) {
  const active = status === 'active'
  return (
    <span
      className="rounded-full px-2.5 py-1 text-[11px] font-medium leading-none"
      style={{
        fontFamily: 'var(--font-sans)',
        backgroundColor: active ? '#EFF6FF' : '#F3F4F6',
        color: active ? '#1E3FB8' : '#666666',
      }}
      data-testid={`dev-lifecycle-${deviceId}`}
    >
      {active ? 'Activo' : 'Inactivo'}
    </span>
  )
}

// ── Direction badge ──────────────────────────────────────────────────────────

function DirectionBadge({ direction }: { direction: Device['direction'] }) {
  const DIRECTION_CONFIG = {
    entry: { bg: '#EBF5FB', text: '#1E3FB8', label: 'Entrada' },
    exit:  { bg: '#EBF5FB', text: '#1E3FB8', label: 'Salida' },
  } as const
  const config = DIRECTION_CONFIG[direction]

  return (
    <span
      className="rounded px-3 py-1 text-[11px] font-semibold"
      style={{
        fontFamily: 'var(--font-sans)',
        backgroundColor: config.bg,
        color: config.text,
      }}
    >
      {config.label}
    </span>
  )
}

// ── DeviceCard ───────────────────────────────────────────────────────────────

interface DeviceCardProps {
  device: Device
  onCommandClick: (device: Device) => void
  canEdit: boolean
}

export function DeviceCard({ device, onCommandClick, canEdit }: DeviceCardProps) {
  const iconColor =
    device.status === 'inactive' ? '#999999' :
    device.connection_state === 'online'  ? '#1E3FB8' :
    device.connection_state === 'offline' ? '#666666' :
    '#999999'

  const heartbeatColor =
    device.connection_state === 'offline' ? '#EF4444' : '#1A1A1A'

  return (
    <article
      className="flex flex-col gap-4 bg-white rounded border border-[#EEF0F2] p-5 transition-shadow hover:shadow-lg"
      style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
      data-testid={`dev-card-${device.id}`}
    >
      {/* Card header: name + status */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <HardDrive size={20} style={{ color: iconColor }} aria-hidden="true" />
          <span
            className="text-[15px] font-semibold text-[#1A1A1A]"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            {device.name}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <ConnectionPill state={device.connection_state} deviceId={device.id} />
          <LifecyclePill status={device.status} deviceId={device.id} />
        </div>
      </div>

      {/* Card body: IP + Heartbeat */}
      <div className="flex flex-col gap-2 w-full">
        {/* Row 1: IP Address */}
        <div className="flex items-center justify-between">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            IP Address
          </span>
          <span
            className="text-[12px] text-[#1A1A1A]"
            style={{ fontFamily: 'var(--font-mono)' }}
          >
            {device.ip}
          </span>
        </div>

        {/* Row 2: Heartbeat */}
        <div className="flex items-center justify-between">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Heartbeat
          </span>
          <span
            className="text-[12px]"
            style={{
              fontFamily: 'var(--font-mono)',
              color: device.last_seen_at ? heartbeatColor : '#999999',
            }}
          >
            {fmtDateTime(device.last_seen_at)}
          </span>
        </div>
      </div>

      {/* Card footer: direction badge + commands button */}
      <div className="flex items-center justify-between">
        <DirectionBadge direction={device.direction} />
        {canEdit && device.status === 'active' && (
          <button
            type="button"
            onClick={() => onCommandClick(device)}
            aria-label={`Enviar comando a ${device.name}`}
            data-testid={`open-command-modal-${device.id}`}
            className="inline-flex items-center gap-1.5 rounded border border-[#EEF0F2] px-3 py-1.5 text-[12px] font-medium text-[#1A1A1A] transition-colors hover:bg-slate-50"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <Send size={14} aria-hidden="true" />
            Comandos
          </button>
        )}
      </div>
    </article>
  )
}
