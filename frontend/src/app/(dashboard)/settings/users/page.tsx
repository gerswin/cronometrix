'use client'

import { useState, useMemo } from 'react'
import { useRouter } from 'next/navigation'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import {
  Plus,
  LogOut,
  Loader2,
  Pencil,
  ShieldCheck,
  ShieldAlert,
  Eye,
  Power,
} from 'lucide-react'

import { api, setAccessToken } from '@/lib/api'
import { useAuth } from '@/hooks/use-auth'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  createUserFormSchema,
  updateUserFormSchema,
  type CreateUserFormValues,
  type UpdateUserFormValues,
} from '@/lib/validations'
import type { PaginatedResponse, User, UserRole } from '@/types/api'

// ── Helpers ─────────────────────────────────────────────────────────────────

const ROLE_LABEL: Record<UserRole, string> = {
  admin: 'Admin',
  supervisor: 'Supervisor',
  viewer: 'Visor',
}

const ROLE_BADGE: Record<UserRole, { bg: string; icon: typeof ShieldCheck }> = {
  admin: { bg: '#7C3AED', icon: ShieldAlert },
  supervisor: { bg: '#1E3FB8', icon: ShieldCheck },
  viewer: { bg: '#64748B', icon: Eye },
}

function initialsFor(name: string): string {
  return name
    .split(' ')
    .filter(Boolean)
    .map((p) => p[0])
    .slice(0, 2)
    .join('')
    .toUpperCase()
}

const AVATAR_PALETTE = [
  '#5588DD', '#A855F7', '#22C55E', '#F59E0B',
  '#EF4444', '#06B6D4', '#84CC16', '#C4D9E8',
]
function avatarColor(seed: string): string {
  let h = 0
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0
  return AVATAR_PALETTE[h % AVATAR_PALETTE.length]
}

// ── Page ────────────────────────────────────────────────────────────────────

export default function UsersPage() {
  const router = useRouter()
  const { role, sub } = useAuth()
  const queryClient = useQueryClient()
  const isAdmin = role === 'admin'

  const [createOpen, setCreateOpen] = useState(false)
  const [editing, setEditing] = useState<User | null>(null)
  const [confirmDeactivate, setConfirmDeactivate] = useState<User | null>(null)
  const [statusFilter, setStatusFilter] = useState<'active' | 'inactive'>('active')
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  const { data, isLoading, error } = useQuery<PaginatedResponse<User>>({
    queryKey: ['users', statusFilter],
    queryFn: () =>
      api
        .get('/users', { params: { status: statusFilter, limit: 200 } })
        .then((r) => r.data),
    enabled: isAdmin,
  })

  const users = useMemo(() => data?.data ?? [], [data])

  async function handleLogout() {
    if (isLoggingOut) return
    setIsLoggingOut(true)
    try {
      await api.post('/auth/logout').catch(() => undefined)
    } finally {
      setAccessToken(null)
      router.push('/login')
    }
  }

  const deactivateMutation = useMutation({
    mutationFn: async (user: User) => {
      await api.delete(`/users/${user.id}`, { params: { version: user.version } })
    },
    onSuccess: () => {
      toast.success('Usuario desactivado')
      queryClient.invalidateQueries({ queryKey: ['users'] })
      setConfirmDeactivate(null)
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response?.status
      const msg =
        (err as { response?: { data?: { error?: { message?: string } } } })?.response?.data?.error?.message ?? 'Error al desactivar'
      if (status === 409) {
        toast.error('Usuario cambió; recargando…')
        queryClient.invalidateQueries({ queryKey: ['users'] })
      } else {
        toast.error(msg)
      }
    },
  })

  if (!isAdmin) {
    return (
      <div className="p-8 text-[14px] text-[#666666]">
        Acceso restringido. Solo administradores pueden gestionar usuarios.
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full bg-[#F8F9FA]">
      {/* Header */}
      <header className="flex items-center justify-between bg-white border-b border-[#EEF0F2] px-6 py-4">
        <div className="flex flex-col gap-1">
          <span
            className="text-[12px] text-[#666666]"
            style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
          >
            Inicio / Configuración / Usuarios
          </span>
          <h1
            className="text-[22px] font-bold text-[#1A1A1A] leading-tight"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            Usuarios del Sistema
          </h1>
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            data-testid="new-user-trigger"
            onClick={() => setCreateOpen(true)}
            className="inline-flex items-center gap-1.5 rounded px-4 py-2 text-[13px] font-medium text-white bg-[#1E3FB8] hover:bg-[#1835A0] transition-colors"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <Plus size={16} aria-hidden="true" />
            Nuevo Usuario
          </button>
          <button
            type="button"
            onClick={handleLogout}
            disabled={isLoggingOut}
            aria-label="Cerrar sesión"
            data-testid="logout-button"
            className="inline-flex items-center gap-1.5 text-xs text-[#666666] hover:text-[#1A1A1A] px-2.5 py-1.5 rounded-md border border-[#EEF0F2] hover:bg-slate-50 disabled:opacity-50 transition-colors"
          >
            <LogOut size={14} aria-hidden="true" />
            {isLoggingOut ? 'Saliendo…' : 'Salir'}
          </button>
        </div>
      </header>

      {/* Body */}
      <div className="flex-1 overflow-hidden flex flex-col gap-4 p-6">
        {/* Status tabs */}
        <div
          className="inline-flex border border-[#EEF0F2] rounded overflow-hidden self-start"
          role="tablist"
        >
          {(['active', 'inactive'] as const).map((s) => (
            <button
              key={s}
              type="button"
              role="tab"
              aria-selected={statusFilter === s}
              onClick={() => setStatusFilter(s)}
              data-testid={`users-tab-${s}`}
              className={[
                'px-4 py-2 text-[13px] font-medium transition-colors',
                statusFilter === s
                  ? 'bg-[#1E3FB8] text-white'
                  : 'bg-white text-[#666666] hover:bg-slate-50',
              ].join(' ')}
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              {s === 'active' ? 'Activos' : 'Inactivos'}
            </button>
          ))}
        </div>

        {/* Table */}
        <section
          className="flex-1 bg-white rounded border border-[#EEF0F2] overflow-hidden flex flex-col"
          style={{ boxShadow: '0 2px 4px #00000008, 0 6px 16px #0000000d' }}
        >
          <div
            className="flex items-center px-4 py-3 bg-[#F3F4F6] border-b border-[#EEF0F2]"
            style={{ fontFamily: 'var(--font-sans)' }}
          >
            <div className="flex-1 text-[10px] font-semibold text-[#666666] tracking-wider">
              USUARIO
            </div>
            <div className="w-[120px] text-[10px] font-semibold text-[#666666] tracking-wider">
              ROL
            </div>
            <div className="w-[160px] text-[10px] font-semibold text-[#666666] tracking-wider">
              CREADO
            </div>
            <div className="w-[120px] text-right text-[10px] font-semibold text-[#666666] tracking-wider">
              ACCIONES
            </div>
          </div>

          <div className="flex-1 overflow-auto">
            {isLoading && (
              <div className="flex items-center gap-2 px-4 py-12 text-[13px] text-[#666666]">
                <Loader2 size={14} className="animate-spin" />
                Cargando…
              </div>
            )}
            {error && (
              <div className="px-4 py-12 text-[13px] text-red-600">
                Error al cargar usuarios.
              </div>
            )}
            {!isLoading && users.length === 0 && (
              <div className="px-4 py-12 text-center text-[13px] text-[#666666]">
                {statusFilter === 'active'
                  ? 'No hay usuarios activos.'
                  : 'No hay usuarios desactivados.'}
              </div>
            )}
            {users.map((u, i) => {
              const roleCfg = ROLE_BADGE[u.role]
              const RoleIcon = roleCfg.icon
              const isSelf = u.id === sub
              return (
                <div
                  key={u.id}
                  className="flex items-center px-4 py-3 border-b border-[#EEF0F2]"
                  style={{ backgroundColor: i % 2 === 1 ? '#FAFBFC' : '#FFFFFF' }}
                  data-testid={`user-row-${u.id}`}
                >
                  <div className="flex-1 flex items-center gap-2.5 min-w-0">
                    <span
                      className="w-[28px] h-[28px] rounded-full text-white text-[11px] font-semibold flex items-center justify-center shrink-0"
                      style={{ backgroundColor: avatarColor(u.id) }}
                      aria-hidden="true"
                    >
                      {initialsFor(u.full_name)}
                    </span>
                    <div className="flex flex-col min-w-0">
                      <span className="text-[13px] font-medium text-[#1A1A1A] truncate">
                        {u.full_name}
                        {isSelf && (
                          <span className="ml-1.5 text-[10px] text-[#1E3FB8] font-normal">
                            (tú)
                          </span>
                        )}
                      </span>
                      <span
                        className="text-[11px] text-[#666666] truncate"
                        style={{ fontFamily: 'var(--font-mono)' }}
                      >
                        @{u.username}
                      </span>
                    </div>
                  </div>
                  <div className="w-[120px]">
                    <span
                      className="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium text-white"
                      style={{ backgroundColor: roleCfg.bg }}
                    >
                      <RoleIcon size={11} />
                      {ROLE_LABEL[u.role]}
                    </span>
                  </div>
                  <div
                    className="w-[160px] text-[12px] text-[#666666]"
                    style={{ fontFamily: 'var(--font-mono)' }}
                  >
                    {new Date(u.created_at).toLocaleDateString('es-VE', {
                      year: 'numeric',
                      month: 'short',
                      day: '2-digit',
                    })}
                  </div>
                  <div className="w-[120px] flex items-center justify-end gap-1.5">
                    {statusFilter === 'active' && (
                      <>
                        <button
                          type="button"
                          onClick={() => setEditing(u)}
                          aria-label={`Editar ${u.username}`}
                          data-testid={`user-edit-${u.id}`}
                          className="text-[#1E3FB8] hover:bg-[#1E3FB8]/10 p-1.5 rounded transition-colors"
                          title="Editar"
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          type="button"
                          onClick={() => setConfirmDeactivate(u)}
                          disabled={isSelf}
                          aria-label={`Desactivar ${u.username}`}
                          data-testid={`user-deactivate-${u.id}`}
                          className="text-[#EF4444] hover:bg-[#EF4444]/10 p-1.5 rounded transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                          title={isSelf ? 'No puedes desactivarte' : 'Desactivar'}
                        >
                          <Power size={14} />
                        </button>
                      </>
                    )}
                    {statusFilter === 'inactive' && (
                      <span className="text-[11px] text-[#666666] italic">
                        desactivado
                      </span>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </section>
      </div>

      {/* Create dialog */}
      <CreateUserDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreated={() => {
          queryClient.invalidateQueries({ queryKey: ['users'] })
          setCreateOpen(false)
        }}
      />

      {/* Edit dialog */}
      {editing && (
        <EditUserDialog
          user={editing}
          isSelf={editing.id === sub}
          onClose={() => setEditing(null)}
          onSaved={() => {
            queryClient.invalidateQueries({ queryKey: ['users'] })
            setEditing(null)
          }}
        />
      )}

      {/* Deactivate confirm */}
      <Dialog
        open={!!confirmDeactivate}
        onOpenChange={(o) => !o && setConfirmDeactivate(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>¿Desactivar usuario?</DialogTitle>
          </DialogHeader>
          <p className="text-[13px] text-[#666666] py-2">
            <strong className="text-[#1A1A1A]">
              {confirmDeactivate?.full_name}
            </strong>{' '}
            (@{confirmDeactivate?.username}) ya no podrá iniciar sesión. Esta
            acción se registra en auditoría y se puede revertir desde la pestaña
            Inactivos (próximamente).
          </p>
          <DialogFooter>
            <button
              type="button"
              onClick={() => setConfirmDeactivate(null)}
              className="px-4 py-2 text-[13px] rounded border border-[#EEF0F2] text-[#666666] hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="button"
              onClick={() =>
                confirmDeactivate &&
                deactivateMutation.mutate(confirmDeactivate)
              }
              disabled={deactivateMutation.isPending}
              className="px-4 py-2 text-[13px] rounded bg-[#EF4444] hover:bg-[#DC2626] text-white disabled:opacity-50"
            >
              {deactivateMutation.isPending ? 'Desactivando…' : 'Desactivar'}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

// ── Create dialog ───────────────────────────────────────────────────────────

function CreateUserDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onCreated: () => void
}) {
  const form = useForm<CreateUserFormValues>({
    resolver: zodResolver(createUserFormSchema),
    defaultValues: { username: '', full_name: '', role: 'viewer', password: '' },
  })

  const mutation = useMutation({
    mutationFn: async (values: CreateUserFormValues) => {
      const r = await api.post<User>('/users', values)
      return r.data
    },
    onSuccess: (u) => {
      toast.success(`Usuario ${u.username} creado`)
      form.reset()
      onCreated()
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response?.status
      const msg =
        (err as { response?: { data?: { error?: { message?: string } } } })?.response?.data?.error?.message ?? 'Error al crear usuario'
      if (status === 409) {
        form.setError('username', { message: 'Usuario ya existe' })
      } else {
        toast.error(msg)
      }
    },
  })

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) form.reset()
        onOpenChange(o)
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nuevo Usuario</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={form.handleSubmit((v) => mutation.mutate(v))}
          className="flex flex-col gap-4 py-2"
        >
          <Field
            label="Usuario"
            error={form.formState.errors.username?.message}
            hint="solo letras, números y . _ -"
          >
            <input
              {...form.register('username')}
              autoComplete="off"
              data-testid="user-form-username"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px]"
              style={{ fontFamily: 'var(--font-mono)' }}
            />
          </Field>
          <Field label="Nombre completo" error={form.formState.errors.full_name?.message}>
            <input
              {...form.register('full_name')}
              data-testid="user-form-full-name"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px]"
            />
          </Field>
          <Field label="Rol" error={form.formState.errors.role?.message}>
            <select
              {...form.register('role')}
              data-testid="user-form-role"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px] bg-white"
            >
              <option value="viewer">Visor — solo lectura</option>
              <option value="supervisor">Supervisor — edita marcaciones</option>
              <option value="admin">Admin — control total</option>
            </select>
          </Field>
          <Field
            label="Contraseña"
            error={form.formState.errors.password?.message}
            hint="mínimo 8 caracteres"
          >
            <input
              {...form.register('password')}
              type="password"
              autoComplete="new-password"
              data-testid="user-form-password"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px]"
              style={{ fontFamily: 'var(--font-mono)' }}
            />
          </Field>
          <DialogFooter>
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-4 py-2 text-[13px] rounded border border-[#EEF0F2] text-[#666666] hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              data-testid="user-form-submit"
              className="px-4 py-2 text-[13px] rounded bg-[#1E3FB8] hover:bg-[#1835A0] text-white disabled:opacity-50"
            >
              {mutation.isPending ? 'Creando…' : 'Crear usuario'}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ── Edit dialog ─────────────────────────────────────────────────────────────

function EditUserDialog({
  user,
  isSelf,
  onClose,
  onSaved,
}: {
  user: User
  isSelf: boolean
  onClose: () => void
  onSaved: () => void
}) {
  const form = useForm<UpdateUserFormValues>({
    resolver: zodResolver(updateUserFormSchema),
    defaultValues: {
      full_name: user.full_name,
      role: user.role,
      password: '',
    },
  })

  const mutation = useMutation({
    mutationFn: async (values: UpdateUserFormValues) => {
      const body: Record<string, unknown> = {
        full_name: values.full_name,
        version: user.version,
      }
      if (!isSelf && values.role !== user.role) body.role = values.role
      if (values.password && values.password.length > 0) body.password = values.password
      await api.patch(`/users/${user.id}`, body)
    },
    onSuccess: () => {
      toast.success('Usuario actualizado')
      onSaved()
    },
    onError: (err: unknown) => {
      const status = (err as { response?: { status?: number } })?.response?.status
      const msg =
        (err as { response?: { data?: { error?: { message?: string } } } })?.response?.data?.error?.message ?? 'Error al actualizar'
      if (status === 409) {
        toast.error('Usuario cambió; recarga la lista')
      } else {
        toast.error(msg)
      }
    },
  })

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Editar @{user.username}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={form.handleSubmit((v) => mutation.mutate(v))}
          className="flex flex-col gap-4 py-2"
        >
          <Field
            label="Nombre completo"
            error={form.formState.errors.full_name?.message}
          >
            <input
              {...form.register('full_name')}
              data-testid="user-edit-full-name"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px]"
            />
          </Field>
          <Field
            label="Rol"
            error={form.formState.errors.role?.message}
            hint={isSelf ? 'No puedes cambiar tu propio rol' : undefined}
          >
            <select
              {...form.register('role')}
              disabled={isSelf}
              data-testid="user-edit-role"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px] bg-white disabled:bg-slate-50 disabled:text-slate-500"
            >
              <option value="viewer">Visor</option>
              <option value="supervisor">Supervisor</option>
              <option value="admin">Admin</option>
            </select>
          </Field>
          <Field
            label="Nueva contraseña (opcional)"
            error={form.formState.errors.password?.message}
            hint="déjalo vacío para no cambiarla"
          >
            <input
              {...form.register('password')}
              type="password"
              autoComplete="new-password"
              data-testid="user-edit-password"
              className="rounded border border-[#EEF0F2] px-3 py-2 text-[14px]"
              style={{ fontFamily: 'var(--font-mono)' }}
            />
          </Field>
          <DialogFooter>
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-[13px] rounded border border-[#EEF0F2] text-[#666666] hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              data-testid="user-edit-submit"
              className="px-4 py-2 text-[13px] rounded bg-[#1E3FB8] hover:bg-[#1835A0] text-white disabled:opacity-50"
            >
              {mutation.isPending ? 'Guardando…' : 'Guardar'}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ── Field wrapper ───────────────────────────────────────────────────────────

function Field({
  label,
  error,
  hint,
  children,
}: {
  label: string
  error?: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <span
        className="text-[12px] text-[#666666] tracking-wide"
        style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}
      >
        {label}
      </span>
      {children}
      {error && <span className="text-[11px] text-red-600">{error}</span>}
      {!error && hint && (
        <span className="text-[11px] text-[#999999]">{hint}</span>
      )}
    </div>
  )
}
