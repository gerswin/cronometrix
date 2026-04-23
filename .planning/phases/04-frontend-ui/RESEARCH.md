# Phase 4: Frontend UI - Research

**Researched:** 2026-04-23
**Domain:** Next.js 16 App Router / React 19 / shadcn/ui / TanStack Query v5 / TanStack Table v8
**Confidence:** HIGH (verified from installed node_modules and backend source)

---

## Summary

Phase 4 builds the operator-facing web UI on top of an already-complete Rust backend. The frontend scaffold exists (Next.js 16.2.3, shadcn/ui, TanStack Query v5, Tailwind 4, react-hook-form v7, Zod v4) but only has login and setup wizard screens. Every new screen — dashboard, timesheet, employee directory, device manager — is net-new. The backend exposes all required REST endpoints; the **only missing backend capability is an SSE endpoint** for the live activity feed (DASH-02/DASH-03), which must be added in Phase 4.

The most important discovery: **the installed Next.js is 16.2.3, not 15**. The project renamed `middleware.ts` to `proxy.ts` and renames the default export to `proxy` (already done in the scaffold). Zod installed is **v4.3.6** (not v3), which has a different import path for some utilities. TanStack Table v8 is **not yet installed**. Sonner is not installed. The `@tanstack/react-query` v5 **is** installed.

**Primary recommendation:** Install the 4 missing packages (TanStack Table v8, recharts, sonner, date-fns), add the SSE endpoint to the Rust backend in 04-02, then build screens in order: scaffold/layout → dashboard → timesheet → employees+devices.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-1 — Novedad state field is decorative**
"Registrar Novedad" modal shows "Estado Inicial" as read-only "Aprobado". No approval workflow.

**D-2 — Activity feed uses real face-capture photos**
Dashboard shows mini-thumbnail from `GET /api/v1/events/{id}/photo`. Fallback to initials if no photo.

**D-3 — Desktop-only, minimum 1280px**
No responsive breakpoints. No Tailwind responsive variants for layout.

**D-4 — SSE disconnection UX**
Discrete orange banner on drop. Exponential backoff 1s→2s→4s→8s→30s cap. Auto-hide on reconnect.

**D-5 — Dashboard layout (mockup 6I0fB)**
4 KPI tiles + left "Actividad en Vivo" list + right donut chart "Distribución por Depto."

**D-6 — Live feed ring buffer**
Keep 20 most recent events. "Ver todo" routes to Timesheet filtered to today.

**D-7 — Default view: current week (Mon–Sun)**
Timesheet loads current calendar week. Prev/next arrows. Date range picker for free-jump.

**D-8 — Timesheet table columns**
Empleado, Entrada, Min. Inicio, Min. Fin, Salida, Total Min, Estado + edit icon.
Estado: Normal (green), Ausente (red), Justificado (yellow), Ausente Justificado (amber).

**D-9 — "Registrar Novedad" modal fields**
Required: Empleado, Departamento, Fecha Inicio, Fecha Fin, Tipo de Novedad, Descripción/Justificación.
Optional: Motivo, Adjuntar soporte (PDF/JPG/PNG, max 5MB frontend), Impacto en Nómina, Notificar supervisor.
Estado Inicial: read-only "Aprobado".

**D-10 — Timesheet "Zona de Justificación"**
File upload CTA is inside the modal, not an inline zone.

**D-11 — Employee table columns**
Nombre, Cédula, Departamento, Cargo, Fecha Ingreso, Estatus, Acciones.
Server-side pagination, 10 rows/page.

**D-12 — Enrollment entry point: sidebar placeholder**
"Enrolamiento" sidebar nav item shows placeholder screen (not hidden, not functional).

**D-13 — Session expiry UX**
Global TanStack Query `onError` → toast "Tu sesión ha expirado" (3s) → redirect `/login?redirect=<path>`.

**D-14 — RBAC UI gating**
Admin-only: "Registrar Novedad", edit icons, "Nuevo Empleado", ISAPI command buttons.
Admin+Supervisor: "Emitir Reporte".
All roles: read views.
Simple `role === 'admin'` conditional rendering via `useAuth()` context. No CASL.

### Claude's Discretion

None specified — all decisions locked.

### Deferred Ideas (OUT OF SCOPE)

- Mobile/responsive design
- Leave approval workflow
- Facial enrollment UI (placeholder screen only)
- Payroll export UI (nav item visible, not built)
- Audit log screen (placeholder or read-only table, not full feature)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DASH-01 | Dashboard displays real-time KPIs (present count, late count, absentees) | `GET /api/v1/daily-records` with `from_date=today&to_date=today` provides all needed data; aggregate on frontend |
| DASH-02 | Dashboard shows connection status of all registered devices | `GET /api/v1/devices` returns status field per device; poll every 30s or SSE |
| DASH-03 | Dashboard displays live photo feed from device recognition events | SSE endpoint needed in backend; photos via `GET /api/v1/events/{id}/photo` |
| TS-01 | Supervisor can view daily attendance grid per employee | `GET /api/v1/daily-records?from_date=&to_date=&department_id=` + TanStack Table v8 |
| TS-02 | Supervisor can edit entry/exit time for a specific day | POST to `daily_record_overrides` table via new endpoint (not yet in backend) |
| TS-03 | Every timesheet edit requires text justification | react-hook-form Zod schema with `min(1)` on justification field |
| TS-04 | Every timesheet edit requires evidence file upload | multipart/form-data upload with 5MB frontend cap; same pattern as leaves |
| TS-05 | System generates immutable audit log for every edit | Backend audit triggers on `daily_record_overrides` already exist (migration 011) |
</phase_requirements>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| KPI calculation (present/late/absent) | Frontend | — | Backend provides `daily_records` rows; frontend aggregates for display. No dedicated KPI endpoint exists. |
| Live activity feed (SSE) | API (Rust) | Browser (EventSource) | SSE stream must originate from the Axum backend; browser uses native `EventSource` to consume |
| Device status polling | API (Rust) | Frontend (30s poll) | `GET /api/v1/devices` returns `status` per device; frontend polls since no SSE for device events |
| Timesheet data fetch | API (Rust) | Frontend (TanStack Table) | Backend owns `daily_records`; frontend renders via server-side paginated TanStack Table |
| Timesheet edit (overrides) | API (Rust) | Frontend (form) | Writes to `daily_record_overrides`; audit trigger fires in SQLite automatically |
| Evidence file upload | API (Rust) | Frontend (react-hook-form) | Same multipart pattern as leaves; frontend enforces 5MB cap before sending |
| Auth redirect / session guard | Frontend Server (proxy.ts) | — | Next.js 16 proxy.ts reads JWT cookie, redirects unauthenticated requests |
| RBAC element visibility | Browser | — | Decoded from JWT claims in React context; backend is authoritative for data access |
| Photo thumbnail display | Browser | API (Rust) | `GET /api/v1/events/{id}/photo` serves JPEG bytes; browser renders as `<img>` |
| Department donut chart | Browser | — | Recharts; data from `GET /api/v1/daily-records` grouped by department_id |

---

## Installed Package Inventory (VERIFIED)

**What is already installed in `frontend/package.json`** [VERIFIED: package.json read]:

| Package | Installed Version | Status |
|---------|------------------|--------|
| next | 16.2.3 | Installed |
| react | 19.2.4 | Installed |
| react-dom | 19.2.4 | Installed |
| typescript | ^5 | Installed |
| @tanstack/react-query | ^5.99.0 | Installed |
| react-hook-form | ^7.72.1 | Installed |
| zod | ^4.3.6 | **v4, not v3** |
| axios | ^1.15.0 | Installed |
| tailwindcss | ^4 | Installed |
| shadcn | ^4.2.0 | Installed |
| lucide-react | ^1.8.0 | Installed |
| @hookform/resolvers | ^5.2.2 | Installed |
| clsx + tailwind-merge | Installed | Installed |

**NOT yet installed — must be added in Wave 0:**

| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/react-table` | 8.21.3 (current) | Timesheet + employee tables |
| `recharts` | 3.8.1 (current) | Department donut chart |
| `sonner` | 2.0.7 (current) | Toast notifications (session expiry UX) |
| `date-fns` | 3.x (current) | Week calculations, date formatting |

**Already installed shadcn/ui components** [VERIFIED: ls components/ui]:
- button, card, form, input, label

**Must be added via `npx shadcn add` in Wave 0:**
- dialog (modal for "Registrar Novedad")
- select, checkbox, textarea (modal fields)
- badge (estado chips)
- dropdown-menu (department filter, actions)
- popover + calendar (date range picker for week navigation)
- avatar (employee thumbnails in activity feed)
- separator, skeleton (loading states)
- sonner (toast — from sonner package, separate from shadcn)

---

## Critical Discovery: Next.js 16.2.3 (Not 15)

[VERIFIED: node_modules/next/package.json]

The installed version is **Next.js 16.2.3**. Key differences from CLAUDE.md's "Next.js 15" reference:

1. **`proxy.ts` not `middleware.ts`**: Already done in scaffold. The default export is named `proxy` (not `middleware`). Import from `next/server` unchanged. The project has `src/proxy.ts` correctly set up.

2. **Proxy API for auth guards** [CITED: local docs `authentication.md`]:
   ```typescript
   // proxy.ts — correct pattern for Next.js 16
   export async function proxy(req: NextRequest) { ... }
   export const config = { matcher: [...] }
   ```

3. **Cookies API**: Uses `await cookies()` (async) in server components and route handlers. Already reflected in local docs.

4. **`useRouter` import**: `from 'next/navigation'` (not `next/router`). App Router pattern unchanged.

5. **Metadata**: Must be in `layout.tsx` or `page.tsx` without `'use client'` — already correct in scaffold.

**Impact for Phase 4:** No other breaking changes found. The proxy.ts pattern for JWT cookie auth guard is already the correct pattern (the scaffold uses it for setup redirect). Phase 4 must extend `proxy.ts` to also guard dashboard/timesheet/employees/devices routes.

---

## Standard Stack

### Core (already installed)
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| next | 16.2.3 | Framework | proxy.ts not middleware.ts |
| @tanstack/react-query | 5.99.0 | Server state | v5 API |
| react-hook-form | 7.72.1 | Forms | Use with @hookform/resolvers |
| zod | 4.3.6 | Validation | **v4 — see pitfalls** |
| axios | 1.15.0 | HTTP | Already configured in `src/lib/api.ts` |

### Must Install
| Library | Version | Purpose |
|---------|---------|---------|
| @tanstack/react-table | 8.21.3 | Timesheet + employee grids |
| recharts | 3.8.1 | Department donut chart |
| sonner | 2.0.7 | Toast notifications |
| date-fns | 3.x | Week navigation, date formatting |

**Installation command:**
```bash
cd frontend && npm install @tanstack/react-table@8 recharts sonner date-fns
```

---

## Backend API Surface Map

[VERIFIED: backend/src/main.rs full read]

All routes are under `/api/v1`. Auth: `Authorization: Bearer <token>` header (access token stored in JS memory, set via `setAccessToken()` from `src/lib/api.ts`).

### Auth Routes (public)
| Method | Path | Notes |
|--------|------|-------|
| POST | `/auth/login` | Returns `{ access_token, refresh_token }` |
| POST | `/auth/refresh` | Cookie-auth, returns new access_token |
| POST | `/auth/logout` | Cookie-auth |

### Daily Records (viewer+)
| Method | Path | Query Params | Response |
|--------|------|-------------|---------|
| GET | `/daily-records` | `employee_id`, `department_id`, `from_date`, `to_date`, `limit`, `offset` | `PaginatedResponse<DailyRecordResponse>` |
| GET | `/daily-records/{id}` | — | `DailyRecordResponse` |

**DailyRecordResponse fields** [VERIFIED: backend/src/daily_records/models.rs]:
```
id, employee_id, department_id, anchor_date (YYYY-MM-DD),
shift_type, work_minutes, overtime_minutes, late_minutes,
early_departure_minutes, is_rest_day_worked, entry_at (ISO 8601 opt),
exit_at (ISO 8601 opt), leave_id (opt), computed_at, created_at,
updated_at, anomalies: Vec<String>
```

### Leaves (viewer read / admin write)
| Method | Path | Auth | Notes |
|--------|------|------|-------|
| POST | `/leaves` | Admin | multipart/form-data: employee_id, from_date, to_date, leave_type, justification, evidence (file) |
| GET | `/leaves` | Viewer+ | `employee_id`, `from_date`, `to_date`, `limit`, `offset` |
| GET | `/leaves/{id}` | Viewer+ | Single leave |
| GET | `/leaves/{id}/evidence` | Viewer+ | Streams file bytes with Content-Type |
| DELETE | `/leaves/{id}?version=N` | Admin | Soft-delete with optimistic concurrency |

**Error codes to handle in UI:**
- `409 LEAVE_OVERLAP` — show remediation: "Cancel the existing leave first"
- `422 VALIDATION_ERROR` — field-level error display

### Events (viewer+)
| Method | Path | Notes |
|--------|------|-------|
| GET | `/events` | List attendance events |
| GET | `/events/{id}` | Single event |
| GET | `/events/{id}/photo` | Returns JPEG bytes (image/jpeg) |

### Devices (viewer read / admin write)
| Method | Path | Auth |
|--------|------|------|
| GET | `/devices` | Viewer+ |
| GET | `/devices/{id}` | Viewer+ |
| POST | `/devices` | Admin |
| PATCH | `/devices/{id}` | Admin |
| DELETE | `/devices/{id}` | Admin |
| POST | `/devices/{id}/commands` | Admin — door open, reboot, enrollment mode |

### Employees (viewer read / supervisor+ write)
| Method | Path | Auth |
|--------|------|------|
| GET | `/employees` | Viewer+ |
| GET | `/employees/{id}` | Viewer+ |
| POST | `/employees` | Supervisor+ |
| PATCH | `/employees/{id}` | Supervisor+ |
| DELETE | `/employees/{id}` | Admin |

### Anomalies
| Method | Path | Auth |
|--------|------|------|
| GET | `/anomalies` | Supervisor+ |

### MISSING — Must Be Added in Phase 4

**1. SSE endpoint for live activity feed (DASH-02, DASH-03)**

No SSE endpoint exists in main.rs. Must add in plan 04-02:
```
GET /api/v1/events/stream   — SSE stream, viewer+
```
Axum SSE: use `axum::response::sse::{Event, Sse}` with a `tokio::sync::broadcast` channel. The `events::service` writes to the broadcast when a new event is persisted. The stream sends `AttendanceEventSSEPayload` (id, employee_id, employee_name, department, captured_at, status, has_photo).

**2. Daily record overrides endpoint (TS-02, TS-03, TS-04, TS-05)**

The `daily_record_overrides` table exists (migration 009) with audit triggers (migration 011). No HTTP handlers exist yet. Must add in plan 04-03:
```
POST /api/v1/daily-records/{id}/overrides   — Admin only, multipart
```
Fields: `override_entry_at` (opt), `override_exit_at` (opt), `justification` (required), `evidence` (file, required per TS-04), `overridden_by` = from JWT claims.

---

## Architecture Patterns

### System Architecture Diagram

```
Browser (React 19 + Next.js 16 App Router)
         │
         │  HTTP/SSE (Authorization: Bearer)
         ▼
Axum REST API (localhost:3001)
  ├── GET /daily-records?week=...  ──► SQLite daily_records table
  ├── GET /events/stream (SSE)     ──► broadcast::Sender (pushed by alertStream ingestion)
  ├── GET /events/{id}/photo       ──► ./data/events/ filesystem
  ├── GET /devices                 ──► SQLite devices table
  ├── POST /leaves (multipart)     ──► SQLite leaves + ./data/leaves/ filesystem
  └── POST /daily-records/{id}/overrides ──► SQLite daily_record_overrides (audit trigger fires)
```

**Data flow — Dashboard live feed:**
```
Hikvision device → alertStream → isapi::stream::ingest_pair()
  → persist_attendance_event() → broadcast::Sender::send()
  → GET /events/stream (SSE) → EventSource in browser
  → react state ring buffer (20 items max)
  → "Actividad en Vivo" list re-renders
```

**Data flow — Timesheet edit:**
```
Operator clicks edit icon → "Registrar Novedad" modal opens
  → react-hook-form validates (Zod: justification min(1), file max 5MB)
  → FormData.append() → POST /daily-records/{id}/overrides (multipart)
  → Backend: validates, writes daily_record_overrides row
  → audit_daily_record_overrides_insert trigger fires → audit_log row created
  → recompute worker re-runs the day → daily_records updated
  → TanStack Query invalidate(['daily-records']) → grid refreshes
```

### Recommended Project Structure

```
frontend/src/
├── app/
│   ├── layout.tsx              # Root layout (existing)
│   ├── proxy.ts                # Auth guard (existing — extend for new routes)
│   ├── (dashboard)/            # Route group — authenticated shell
│   │   ├── layout.tsx          # Sidebar nav + AuthProvider
│   │   ├── dashboard/page.tsx
│   │   ├── timesheet/page.tsx
│   │   ├── employees/page.tsx
│   │   ├── devices/page.tsx
│   │   └── enrollment/page.tsx # Placeholder screen (D-12)
│   ├── login/page.tsx          # (existing)
│   └── setup/                  # (existing)
├── components/
│   ├── ui/                     # shadcn/ui primitives (existing: button, card, form, input, label)
│   ├── layout/
│   │   ├── sidebar.tsx         # Nav sidebar (all screens)
│   │   └── top-bar.tsx         # Page header
│   ├── dashboard/
│   │   ├── kpi-tile.tsx
│   │   ├── activity-feed.tsx   # SSE consumer + ring buffer
│   │   ├── dept-chart.tsx      # Recharts donut
│   │   └── device-banner.tsx   # Offline device badge
│   ├── timesheet/
│   │   ├── timesheet-table.tsx # TanStack Table
│   │   ├── novedad-modal.tsx   # react-hook-form + file upload
│   │   └── week-navigator.tsx  # prev/next + date picker
│   ├── employees/
│   │   └── employee-table.tsx
│   └── devices/
│       └── device-table.tsx
├── hooks/
│   ├── use-auth.ts             # JWT decode → role/claims context
│   ├── use-sse.ts              # EventSource wrapper with backoff
│   └── use-daily-records.ts    # TanStack Query hooks
├── lib/
│   ├── api.ts                  # (existing — axios + interceptors)
│   ├── utils.ts                # (existing)
│   └── validations.ts          # (existing — extend with novedad schema)
└── types/
    └── api.ts                  # TypeScript interfaces matching Rust response structs
```

---

## Pattern 1: SSE with useSSE Hook + Exponential Backoff (D-4)

[ASSUMED — based on EventSource API + React patterns; not verified against a specific docs page]

```typescript
// hooks/use-sse.ts
'use client'
import { useEffect, useRef, useCallback } from 'react'

const BACKOFF_DELAYS = [1000, 2000, 4000, 8000, 30000]

export function useSSE(url: string, onMessage: (data: unknown) => void) {
  const esRef = useRef<EventSource | null>(null)
  const attemptRef = useRef(0)
  const [connected, setConnected] = useState(false)
  const [reconnecting, setReconnecting] = useState(false)

  const connect = useCallback(() => {
    const es = new EventSource(url, { withCredentials: true })
    esRef.current = es

    es.onopen = () => {
      attemptRef.current = 0
      setConnected(true)
      setReconnecting(false)
    }

    es.onmessage = (e) => {
      onMessage(JSON.parse(e.data))
    }

    es.onerror = () => {
      es.close()
      setConnected(false)
      setReconnecting(true)
      const delay = BACKOFF_DELAYS[Math.min(attemptRef.current, BACKOFF_DELAYS.length - 1)]
      attemptRef.current++
      setTimeout(connect, delay)
    }
  }, [url, onMessage])

  useEffect(() => {
    connect()
    return () => esRef.current?.close()
  }, [connect])

  return { connected, reconnecting }
}
```

**Important:** `EventSource` does not support custom headers, so Bearer token cannot be attached. The Axum SSE endpoint must accept the token via query param `?token=<jwt>` or via cookie. Since the project uses `withCredentials: true` on axios and the backend has `CorsLayer::permissive()`, the simplest approach is to accept the access_token as a query parameter for the SSE endpoint only: `GET /api/v1/events/stream?token=<jwt>`.

### Pattern 2: TanStack Table v8 Server-Side Pagination

[CITED: @tanstack/react-table v8 — npm version 8.21.3 confirmed]

```typescript
// Server-side pattern for timesheet grid
const table = useReactTable({
  data,
  columns,
  pageCount: Math.ceil(total / PAGE_SIZE),
  state: { pagination, columnFilters },
  onPaginationChange: setPagination,
  onColumnFiltersChange: setColumnFilters,
  getCoreRowModel: getCoreRowModel(),
  manualPagination: true,   // <-- tells table not to paginate locally
  manualFiltering: true,    // <-- tells table not to filter locally
})

// TanStack Query driving the server call
const { data } = useQuery({
  queryKey: ['daily-records', pagination, filters],
  queryFn: () => api.get('/daily-records', {
    params: {
      from_date: weekStart,
      to_date: weekEnd,
      department_id: deptFilter,
      limit: PAGE_SIZE,
      offset: pagination.pageIndex * PAGE_SIZE,
    }
  }).then(r => r.data),
})
```

**Column definitions for timesheet (D-8):**
```typescript
const columns: ColumnDef<DailyRecord>[] = [
  { accessorKey: 'employee_name', header: 'Empleado' },
  { accessorKey: 'entry_at', header: 'Entrada',
    cell: ({ getValue }) => formatTime(getValue()) },
  { accessorKey: 'late_minutes', header: 'Min. Inicio' },
  { accessorKey: 'early_departure_minutes', header: 'Min. Fin' },
  { accessorKey: 'exit_at', header: 'Salida',
    cell: ({ getValue }) => formatTime(getValue()) },
  { accessorKey: 'work_minutes', header: 'Total Min' },
  { id: 'status', header: 'Estado',
    cell: ({ row }) => <StatusBadge record={row.original} /> },
  { id: 'actions', cell: ({ row }) => <EditButton record={row.original} /> },
]
```

**Status badge logic:**
- No leave_id AND anomalies includes `MISSING_EXIT` or entry_at is null → Ausente (red)
- leave_id present → Justificado (yellow) or Ausente Justificado (amber)
- work_minutes > 0 AND late_minutes === 0 → Normal (green)
- work_minutes > 0 AND late_minutes > 0 → Normal with late indicator

### Pattern 3: File Upload with react-hook-form + Zod v4

[VERIFIED: zod 4.3.6 installed; react-hook-form 7.72.1 installed]

**Zod v4 note:** Import path unchanged (`import { z } from 'zod'`), but some utility methods differ from v3. The `z.instanceof(File)` pattern still works.

```typescript
const novedadSchema = z.object({
  employee_id: z.string().min(1, 'Requerido'),
  department_id: z.string().min(1, 'Requerido'),
  fecha_inicio: z.string().min(1, 'Requerido'),
  fecha_fin: z.string().min(1, 'Requerido'),
  tipo_novedad: z.enum(['medical', 'vacation', 'unpaid', 'manual']),
  justification: z.string().min(10, 'Mínimo 10 caracteres'),
  motivo: z.string().optional(),
  evidence: z
    .instanceof(File)
    .refine(f => f.size <= 5 * 1024 * 1024, 'Máximo 5MB')
    .refine(
      f => ['application/pdf', 'image/jpeg', 'image/png'].includes(f.type),
      'Solo PDF, JPG, PNG'
    )
    .optional(),
  impacto_nomina: z.string().optional(),
  notificar_supervisor: z.boolean().optional(),
})

// Submitting as multipart:
async function onSubmit(values: NovedadFormData) {
  const fd = new FormData()
  fd.append('employee_id', values.employee_id)
  fd.append('from_date', values.fecha_inicio)
  fd.append('to_date', values.fecha_fin)
  fd.append('leave_type', values.tipo_novedad)
  fd.append('justification', values.justification)
  if (values.evidence) fd.append('evidence', values.evidence)
  
  await api.post('/leaves', fd, {
    headers: { 'Content-Type': 'multipart/form-data' }
  })
  queryClient.invalidateQueries({ queryKey: ['leaves'] })
  queryClient.invalidateQueries({ queryKey: ['daily-records'] })
}
```

**File input with react-hook-form:**
```tsx
<input
  type="file"
  accept=".pdf,.jpg,.jpeg,.png"
  onChange={(e) => {
    const file = e.target.files?.[0]
    field.onChange(file)   // pass File object, not FileList
  }}
/>
```

### Pattern 4: Auth Context (JWT Decode, no library)

[ASSUMED — standard pattern]

The access token is stored in JS memory (`setAccessToken()` in `src/lib/api.ts`). JWT payload decoding (no verification needed on client — server verifies):

```typescript
// hooks/use-auth.ts
function decodeJwtPayload(token: string) {
  const [, payload] = token.split('.')
  return JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
}

// Context provides: { role: 'admin' | 'supervisor' | 'viewer', sub: string }
// Use in components:
const { role } = useAuth()
{role === 'admin' && <Button>Registrar Novedad</Button>}
{(role === 'admin' || role === 'supervisor') && <Button>Emitir Reporte</Button>}
```

### Pattern 5: proxy.ts Extension for Auth Guard

[VERIFIED: local Next.js 16 docs + existing proxy.ts]

Extend the existing `proxy.ts` to add JWT cookie check for protected routes:

```typescript
// proxy.ts — extend existing file
const PROTECTED_ROUTES = ['/dashboard', '/timesheet', '/employees', '/devices', '/enrollment']
const PUBLIC_ROUTES = ['/login', '/setup', '/']

export async function proxy(req: NextRequest) {
  const { pathname } = req.nextUrl

  if (pathname.startsWith('/api') || pathname.startsWith('/_next')) {
    return NextResponse.next()
  }

  const isProtected = PROTECTED_ROUTES.some(r => pathname.startsWith(r))
  
  if (isProtected) {
    const cookieValue = req.cookies.get('refresh_token')?.value
    // Optimistic check — if no refresh cookie, redirect to login
    if (!cookieValue) {
      const loginUrl = new URL('/login', req.url)
      loginUrl.searchParams.set('redirect', pathname)
      return NextResponse.redirect(loginUrl)
    }
  }

  // Existing setup check...
  // ... (keep existing setup redirect logic)
  
  return NextResponse.next()
}
```

**Note:** The access token is in JS memory (lost on refresh), so proxy.ts checks the `refresh_token` httpOnly cookie as the session signal. If it exists, allow through — the `Providers` component will call `/auth/refresh` on mount to restore the access token in memory.

### Pattern 6: Global 401 Handler (D-13)

[VERIFIED: existing `src/lib/api.ts` already has 401 interceptor]

The existing axios interceptor in `api.ts` already handles token refresh on 401. Extend to show Sonner toast before redirecting when refresh also fails:

```typescript
// In api.ts catch block (extend existing interceptor):
import { toast } from 'sonner'

// When refresh fails:
toast.error('Tu sesión ha expirado', { duration: 3000 })
setTimeout(() => {
  const redirect = window.location.pathname
  window.location.href = `/login?redirect=${encodeURIComponent(redirect)}`
}, 3000)
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Data tables with sort/filter/pagination | Custom `<table>` + useState | `@tanstack/react-table` v8 | Handles row selection, column pinning, server-side pagination state machine |
| Charts | Canvas drawing code | `recharts` | Donut chart = `<PieChart>` + `innerRadius` prop; 10 lines |
| Toast notifications | Custom toast state | `sonner` | Already shadcn/ui's recommended toast library; handles stacking, duration, dismiss |
| Week date arithmetic | Manual Date math | `date-fns` | `startOfWeek`, `endOfWeek`, `addWeeks`, `subWeeks`, `format` — each a one-liner |
| File size validation | `file.size > X` check in component | Zod `z.instanceof(File).refine(...)` in schema | Validates before submission; displays error in form context |
| SSE reconnection logic | `setTimeout` ad-hoc in component | `useSSE` hook with `BACKOFF_DELAYS` array | Encapsulates state machine; prevents multiple overlapping reconnect timers |
| JWT decode on client | Parse JWT manually per component | `decodeJwtPayload` utility + `useAuth()` context | Single decode point; context prevents prop drilling role checks |

**Key insight:** The dangerous hand-roll in this phase is SSE reconnection. A naive `onerror → setTimeout(connect, 1000)` creates multiple overlapping timers if the component re-renders. The `useSSE` hook must use a `useRef` for the EventSource and clear previous timers before scheduling new ones.

---

## Common Pitfalls

### Pitfall 1: Zod v4 Schema Differences

**What goes wrong:** Code uses `z.string().nonempty()` (removed in v4) or `z.object().partial()` with different behavior.
**Why it happens:** CLAUDE.md documents "Zod v3" but installed version is **4.3.6**.
**How to avoid:** Use `z.string().min(1)` instead of `.nonempty()`. Check [Zod v4 migration guide](https://zod.dev/v4) for any v3 patterns used.
**Warning signs:** `z.string().nonempty is not a function` runtime error.

### Pitfall 2: EventSource Cannot Send Bearer Header

**What goes wrong:** `new EventSource(url, { headers: { Authorization: ... } })` — `EventSource` constructor does not accept a `headers` option. All events return 401.
**Why it happens:** Browser EventSource spec does not support custom headers.
**How to avoid:** SSE endpoint on Rust side must accept `?token=<jwt>` query param OR rely on cookie. Simplest: pass access token as query param since SSE is a GET request.
**Warning signs:** Every SSE connection immediately triggers the `onerror` handler.

### Pitfall 3: TanStack Table Not Installed

**What goes wrong:** `import { useReactTable } from '@tanstack/react-table'` fails at build time.
**Why it happens:** Package is not in package.json (confirmed by ls node_modules check).
**How to avoid:** Wave 0 task must install it. Do not start writing timesheet table code before install.

### Pitfall 4: File Input with react-hook-form — `value` prop conflict

**What goes wrong:** Registering `<input type="file">` via `{...register('evidence')}` causes React warning: "A component is changing an uncontrolled input to be controlled". File inputs cannot use `value` prop.
**Why it happens:** react-hook-form's `register()` sets `value` for controlled components; file inputs must use `onChange` only.
**How to avoid:** Use `Controller` + `field.onChange` manually (see Pattern 3 above). Never `{...register('evidence')}` on a file input.

### Pitfall 5: Recharts in Next.js App Router RSC

**What goes wrong:** `ReferenceError: window is not defined` when Recharts component renders server-side.
**Why it happens:** Recharts uses browser APIs internally; it is not RSC-safe.
**How to avoid:** Add `'use client'` to any component that imports from `recharts`. Do not put Recharts in a Server Component.

### Pitfall 6: Ring Buffer State in SSE Handler

**What goes wrong:** `onMessage` callback captures stale `events` array in closure; ring buffer always shows only the newest 1 item.
**Why it happens:** `useSSE` takes `onMessage` as a prop; if it's defined inline in the component, React recreates it every render, and the `useEffect` dependency array causes reconnect loops or stale closure.
**How to avoid:** Use `useCallback` for the `onMessage` handler, or use `useRef` to hold the latest callback:
```typescript
const onMessageRef = useRef(onMessage)
useEffect(() => { onMessageRef.current = onMessage })
// Inside EventSource: es.onmessage = (e) => onMessageRef.current(JSON.parse(e.data))
```

### Pitfall 7: Week Date Calculation — Monday Start

**What goes wrong:** `startOfWeek(date)` returns Sunday (US locale default). Timesheet shows wrong week (D-7 says Mon–Sun).
**Why it happens:** `date-fns` `startOfWeek` defaults to `weekStartsOn: 0` (Sunday).
**How to avoid:** Always pass `{ weekStartsOn: 1 }`: `startOfWeek(date, { weekStartsOn: 1 })`.

### Pitfall 8: proxy.ts Blocks prefetch Requests

**What goes wrong:** Auth redirect fires for Next.js prefetch requests to protected routes, creating redirect loops or unnecessary redirections during navigation.
**Why it happens:** proxy.ts runs on every request including Link prefetches.
**How to avoid:** The local Next.js docs explicitly say: "only read the session from the cookie (optimistic checks), and avoid database checks to prevent performance issues." The cookie check is fast and correct. Avoid any async calls (no fetch to backend) in proxy.ts beyond the cookie presence check.

---

## Missing Backend Endpoints — Phase 4 Must Add

### 1. SSE Endpoint — `GET /api/v1/events/stream`

**Location:** `backend/src/events/handlers.rs` + `backend/src/events/service.rs`
**Required by:** DASH-02, DASH-03
**Axum SSE pattern** [ASSUMED — standard Axum SSE]:
```rust
// AppState needs a broadcast::Sender<AttendanceEventSSEPayload>
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream;
use tokio::sync::broadcast;

pub async fn events_stream(
    State(state): State<AppState>,
    Query(q): Query<StreamQuery>,  // token: String
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Validate token from query param
    let mut rx = state.event_broadcast.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(payload) => {
                    yield Ok(Event::default().json_data(payload).unwrap())
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

**AppState addition needed:** `event_broadcast: broadcast::Sender<AttendanceEventSSEPayload>` (with `broadcast::channel(100)`). The `events::service::persist_attendance_event` sends to this channel after successful insert.

### 2. Daily Record Overrides — `POST /api/v1/daily-records/{id}/overrides`

**Location:** New handler in `backend/src/daily_records/handlers.rs`
**Required by:** TS-02, TS-03, TS-04, TS-05
**Table already exists:** `daily_record_overrides` (migration 009), audit trigger on INSERT (migration 011).

Schema reminder (from 03-03-SUMMARY.md migration 009):
- `daily_record_id` (FK → daily_records.id)
- `override_work_minutes`, `override_entry_at`, `override_exit_at` (optional overrides)
- `justification` (required text)
- `evidence_path` (server-generated UUID path)
- `overridden_by` (user_id from JWT claims)
- `status`, `version` (optimistic concurrency)

**RBAC:** Admin only (per D-14 — only Admin sees edit icons).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Vitest (not installed yet — must add in Wave 0) |
| Config file | `frontend/vitest.config.ts` — Wave 0 gap |
| Quick run command | `cd frontend && npx vitest run --reporter=dot` |
| Full suite command | `cd frontend && npx vitest run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DASH-01 | KPI aggregation from daily records | unit | `vitest run src/__tests__/kpi.test.ts` | Wave 0 |
| DASH-02 | Device status display | smoke | `vitest run src/__tests__/device-banner.test.tsx` | Wave 0 |
| DASH-03 | Activity feed ring buffer (max 20 items) | unit | `vitest run src/__tests__/activity-feed.test.ts` | Wave 0 |
| TS-01 | Timesheet table renders daily records | component | `vitest run src/__tests__/timesheet-table.test.tsx` | Wave 0 |
| TS-02 | Edit modal opens on edit click | component | `vitest run src/__tests__/novedad-modal.test.tsx` | Wave 0 |
| TS-03 | Justification field required (form blocks submit) | unit | same file | Wave 0 |
| TS-04 | File > 5MB blocked before submit | unit | `vitest run src/__tests__/file-validation.test.ts` | Wave 0 |
| TS-05 | Audit log — manual verification | manual | n/a | n/a |

### Wave 0 Gaps
- [ ] `frontend/vitest.config.ts` — Vitest config
- [ ] `frontend/src/__tests__/` directory + fixture files
- [ ] Install: `npm install -D vitest @vitejs/plugin-react @testing-library/react @testing-library/jest-dom jsdom`

---

## Security Domain

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | JWT in memory; httpOnly refresh cookie; existing axios interceptor |
| V3 Session Management | yes | proxy.ts cookie guard; `/auth/refresh` on mount |
| V4 Access Control | yes | role === 'admin' guards; backend is authoritative; no client-only RBAC |
| V5 Input Validation | yes | Zod v4 schemas for all forms; 5MB file cap before upload |
| V6 Cryptography | no | JWT decoded not verified on client; backend verifies with HS256 secret |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| XSS via face photo `src` | Tampering | Use `<img src={photoUrl}>` with trusted API origin only; never `dangerouslySetInnerHTML` |
| CSRF on multipart POST | Spoofing | Bearer token in header (not cookie-only); SameSite=Lax on refresh cookie |
| Open redirect on `/login?redirect=` | Spoofing | Validate redirect is same-origin before using: `new URL(redirect, window.location.origin).origin === window.location.origin` |
| SSE token in URL (query param) | Info Disclosure | Token appears in server access logs; acceptable for on-premise deployment with controlled log access |
| File upload path traversal | Tampering | Backend already handles: UUID filenames, canonicalize guard — no frontend mitigation needed |

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Frontend build | ✓ | (inferred from npm working) | — |
| npm | Package installs | ✓ | (npm view commands succeed) | — |
| Next.js dev server | All frontend work | ✓ | 16.2.3 (installed) | — |
| Rust backend | API calls | ✓ | Running (Phase 3 complete) | — |
| @tanstack/react-table | Timesheet/employees | ✗ | — | None — must install |
| recharts | Dept donut chart | ✗ | — | None — must install |
| sonner | Toast notifications | ✗ | — | None — must install |
| date-fns | Week navigation | ✗ | — | None — must install |
| vitest | Frontend tests | ✗ | — | None — must install |

**Missing dependencies with no fallback (blocking):**
- `@tanstack/react-table` — timesheet grid cannot be built without it
- All four must be installed in Wave 0 before any screen work begins

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `middleware.ts` default export | `proxy.ts` with `proxy` export | Next.js 16 | Already done in scaffold |
| Zod v3 `z.string().nonempty()` | Zod v4 `z.string().min(1)` | Zod 4.0 | Must use v4 patterns throughout |
| TanStack Query `useQuery` `onError` option | Global `queryClient.setDefaultOptions({ queries: { throwOnError } })` + error boundary | TQ v5 | v5 removed per-query `onError`; use global config or `onError` on mutation |

**Deprecated/outdated:**
- `next/router` → use `next/navigation` (already done in login page)
- TanStack Query v4 `onError` per-query → removed in v5; use `queryClient.getQueryCache().subscribe()` for global error handling or mutation `onError`

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | SSE endpoint accepts `?token=<jwt>` query param for auth | Pattern 1, Missing Endpoints | If backend requires cookie-only auth for SSE, must set access token as a cookie on login |
| A2 | `axum::response::sse` module + `async_stream` crate for SSE impl | Missing Backend Endpoints | May need different crate (tokio-stream); check axum docs before coding |
| A3 | `broadcast::Sender` added to AppState for SSE push | Missing Backend Endpoints | Could use a different pub/sub mechanism (mpsc + fan-out); benchmark under 4-device load |
| A4 | `daily_record_overrides` table primary key is UUID `id`, FK to `daily_records.id` | Missing Backend Endpoints | Need to read migration 009 SQL to confirm column names before writing handler |
| A5 | Vitest works with Next.js 16 App Router without additional config | Validation Architecture | May need `next/jest` transform or specific jsdom config; check after install |
| A6 | `useAuth()` hook decodes JWT from the in-memory `accessToken` variable | Pattern 4 | If access token is not in memory (e.g., after hard refresh before Providers mounts), role will be null until refresh completes |

---

## Open Questions

1. **migration 009 exact schema**
   - What we know: `daily_record_overrides` table was created in migration 009; audit triggers in 011
   - What's unclear: Exact column names for `override_entry_at`, `override_exit_at` — need to read the SQL file before writing the override handler
   - Recommendation: Read `backend/src/db/migrations/009_daily_record_overrides.sql` in plan 04-03 before writing the Rust handler

2. **broadcast channel capacity for SSE**
   - What we know: Up to 4 Hikvision devices, each producing ~1 event/second max under heavy load = 4 events/sec
   - What's unclear: Whether a 100-item broadcast buffer is sufficient or will produce `RecvError::Lagged`
   - Recommendation: Use capacity 256; `Lagged` errors are non-fatal (skip and continue)

---

## Sources

### Primary (HIGH confidence)
- `/Users/gerswin/Proyectos/cronometrix/frontend/package.json` — installed package versions verified
- `/Users/gerswin/Proyectos/cronometrix/frontend/node_modules/next/package.json` — Next.js 16.2.3 confirmed
- `/Users/gerswin/Proyectos/cronometrix/backend/src/main.rs` — all registered routes verified
- `/Users/gerswin/Proyectos/cronometrix/backend/src/daily_records/models.rs` — DailyRecordResponse fields verified
- `/Users/gerswin/Proyectos/cronometrix/backend/src/leaves/handlers.rs` — multipart upload pattern verified
- `/Users/gerswin/Proyectos/cronometrix/frontend/src/lib/api.ts` — auth token pattern verified
- `/Users/gerswin/Proyectos/cronometrix/frontend/src/proxy.ts` — Next.js 16 proxy pattern verified
- Local Next.js docs at `node_modules/next/dist/docs/01-app/02-guides/authentication.md` — proxy auth pattern cited
- Local Next.js docs at `node_modules/next/dist/docs/01-app/03-api-reference/04-functions/use-router.md` — navigation API verified
- `npm view @tanstack/react-table version` → 8.21.3 [VERIFIED]
- `npm view recharts version` → 3.8.1 [VERIFIED]
- `npm view sonner version` → 2.0.7 [VERIFIED]

### Secondary (MEDIUM confidence)
- Phase 3 summaries (03-01-SUMMARY.md, 03-03-SUMMARY.md) — database schema and API contract

### Tertiary (LOW confidence / ASSUMED)
- Axum SSE implementation pattern (A2) — standard pattern, not verified against installed axum version
- Vitest + Next.js 16 compatibility (A5) — not tested in this environment

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified from package.json and node_modules
- Backend API surface: HIGH — verified from main.rs and handler files
- Next.js 16 patterns: HIGH — verified from local node_modules docs
- SSE backend implementation: MEDIUM — Axum SSE API assumed, not verified from axum source
- Pitfalls: HIGH — derived from verified package versions and known issues

**Research date:** 2026-04-23
**Valid until:** 2026-05-23 (stable stack; Next.js 16 minor versions may ship but App Router API is stable)
