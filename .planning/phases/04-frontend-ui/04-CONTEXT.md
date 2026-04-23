# Phase 4: Frontend UI - Context

**Gathered:** 2026-04-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a working web UI so operators can perform all daily workflows without touching the backend directly: monitor the live dashboard, edit timesheets with mandatory justification and evidence, manage employees, and configure devices. All business logic lives in the Rust backend; the frontend is a display + form layer.

**In scope (04-01 → 04-04):**
- Next.js 15 project scaffold (App Router, shadcn/ui, TanStack Query v5, Tailwind 4)
- Dashboard screen: KPI tiles, live activity feed (SSE), device health banners
- Timesheet editor: attendance grid per week, "Registrar Novedad" modal
- Employee directory: search/filter table, employee CRUD entry point
- Device manager: device list, ISAPI command dispatch, connection status

**Out of scope (Phase 4):**
- Payroll export (Phase 5)
- Facial enrollment workflow (Phase 7) — placeholder only
- Mobile/responsive (desktop-only this phase)
- Leave approval workflow (backend is immediate-approve; D-1)
</domain>

<decisions>

## Visual Design

**D-1 — Novedad state field is decorative**
The "Registrar Novedad" modal shows "Estado Inicial" — this is a read-only display field showing "Aprobado" (not "Pendiente de Aprobación" as the mockup draft had). Backend approves leaves immediately (D-15 from Phase 3). No approval workflow exists; the field is visual labeling only.

**D-2 — Activity feed uses real face-capture photos**
Dashboard "Actividad en Vivo" list shows mini-thumbnail of the JPEG captured by the Hikvision device at event time. Backend already stores the photo (`attendance_events.photo_path`). Frontend fetches via `GET /api/v1/events/{id}/photo` and shows as a small circular avatar. Fallback to initials if no photo exists.

**D-3 — Desktop-only, minimum 1280px**
No responsive breakpoints in Phase 4. On-premise tool used at workstations. Tailwind breakpoints are not added; layout assumes ≥1280px viewport. Mobile support deferred to a future phase.

## Dashboard

**D-4 — SSE disconnection UX**
When the SSE connection drops: show a discrete orange banner at the top of the dashboard content area — "Conexión perdida — reconectando…". Auto-retry with exponential backoff (1s, 2s, 4s, 8s, max 30s). Banner disappears automatically on reconnect. No user action required. No modal, no page reload.

**D-5 — Dashboard layout (from mockup 6I0fB)**
Four KPI tiles top row: Empleados Presentes, % Retraso Hoy, Dispositivos Activos (X/total), Alertas Diurnas.
Left panel: "Actividad en Vivo" — scrollable list, most recent event on top. Each row: face-capture thumbnail (40px circle), employee name, time, location/department, status badge (Entrada / Salida).
Right panel: "Distribución por Depto." — donut chart with department breakdown.
Device offline events surface as a banner/badge inside the KPI tile "Dispositivos Activos" (yellow/red if any offline).

**D-6 — Live feed ring buffer**
Keep the 20 most recent events in the feed. Oldest drops off automatically. No "Ver todo" pagination in Phase 4 — the link routes to the Timesheet screen filtered to today.

## Timesheet

**D-7 — Default view: current week (Mon–Sun)**
Opening the Timesheet screen loads the current calendar week (Monday–Sunday). Navigation: prev/next week arrows on either side of the date range display. Date range picker (shadcn Calendar Popover) allows free-jump to any week.

**D-8 — Timesheet table columns (from mockup atcTy)**
Columns: Empleado, Entrada, Min. Inicio, Min. Fin, Salida, Total Min, Estado + edit icon.
Estado badges: Normal (green), Ausente (red), Justificado (yellow), Ausente Justificado (amber). Colors from design system variables.
Department filter dropdown (all departments or single). Search by employee name.

**D-9 — "Registrar Novedad" modal fields (from mockup RETHd)**
Required fields (*): Empleado, Departamento, Fecha Inicio, Fecha Fin, Tipo de Novedad, Descripción/Justificación.
Optional: Motivo (short text), Adjuntar soporte (PDF/JPG/PNG, max **5MB** — frontend cap; backend allows 10MB), Impacto en Nómina (select), Notificar al supervisor (checkbox).
Estado Inicial: read-only label "Aprobado" (D-1).
Submit button: "Registrar Novedad". Cancel closes modal without saving.

**D-10 — Timesheet "Zona de Justificación"**
The bottom zone in the mockup ("Adjunta un escrito y/o documento de soporte") is the file upload call-to-action area displayed when a row's edit icon is clicked (it activates the modal, not an inline zone). The bottom text is hint/guide copy, not an independent component.

## Employee & Device Management

**D-11 — Employee table columns (from mockup TYS9Z)**
Columns: Nombre, Cédula, Departamento, Cargo, Fecha Ingreso, Estatus, Acciones.
Filters: Departamento (dropdown), Cargo (dropdown), Estatus (dropdown: Activo/Pendiente/Inactivo), free-text search.
Pagination: server-side, 10 rows/page, Anterior / page numbers / Siguiente.
Actions per row: edit icon (pencil), expand/detail icon.
Top-right: "Nuevo Empleado" button (Admin only), "Emitir Reporte" button.

**D-12 — Enrollment entry point: sidebar placeholder**
"Enrolamiento" appears as a sidebar nav item (matching mockup sidebar). Clicking renders a placeholder screen: title "Enrolamiento Facial", brief description "Próximamente — Sincronización de perfiles faciales con dispositivos Hikvision", and a simple illustration. No functionality, no buttons. The nav item is NOT disabled or hidden — it's visitable, just empty.

## Authentication & Session

**D-13 — Session expiry UX**
TanStack Query's global `onError` handler intercepts HTTP 401. Triggers: (1) toast notification "Tu sesión ha expirado" displayed for 3 seconds via shadcn Sonner, (2) redirect to `/login` after toast closes. The login page preserves the intended destination via `?redirect=<path>` so the user lands back after re-authentication. Applies to all queries and mutations.

## RBAC UI Gating

**D-14 — Role-based element visibility**

| Element | Admin | Supervisor | Viewer |
|---------|:-----:|:----------:|:------:|
| "Registrar Novedad" button | ✓ | ✗ hidden | ✗ hidden |
| Edit icon in Timesheet rows | ✓ | ✗ hidden | ✗ hidden |
| "Nuevo Empleado" button | ✓ | ✗ hidden | ✗ hidden |
| ISAPI command buttons (Device) | ✓ | ✗ hidden | ✗ hidden |
| All read views (Dashboard, Timesheet, Employees, Devices) | ✓ | ✓ | ✓ |
| "Emitir Reporte" button | ✓ | ✓ | ✗ hidden |

Role is decoded from the JWT claims on the client. A React context `useAuth()` exposes `role`. Components use `role === 'admin'` guards (no third-party CASL/RBAC library — simple conditional rendering).

## Tech Stack (locked — from CLAUDE.md)

- **Framework:** Next.js 15 (App Router) + React 19 + TypeScript 5
- **UI:** shadcn/ui (Radix UI + Tailwind 4), Lucide React icons
- **Data:** TanStack Query v5 (server state), no Redux/Zustand
- **Tables:** TanStack Table v8
- **Forms:** react-hook-form v7 + Zod v3
- **Real-time:** Server-Sent Events (SSE) via native `EventSource`
- **HTTP:** axios or native fetch inside TanStack Query queryFns
- **Auth:** JWT stored in `httpOnly` cookie; Next.js middleware reads cookie for SSR redirect
- **Lint/format:** Biome
- **Testing:** Vitest + React Testing Library

</decisions>

<deferred>

## Deferred to Future Phases

- **Mobile/responsive design** — Desktop-only in Phase 4. Future phase if client demand arises.
- **Leave approval workflow** — Backend immediate-approve (D-15/Phase 3). If multi-actor approval is needed later, requires backend state machine extension first.
- **Facial enrollment UI** — Phase 7. Placeholder screen only in Phase 4.
- **Payroll export UI** — Phase 5 (Reportes y Pre-Nómina screen visible in nav but not built in Phase 4).
- **Audit log screen** ("Panel de Auditoría" mockup 73MPC) — Visible in sidebar nav, placeholder or read-only table in Phase 4 (backend audit_log table already exists).

</deferred>

<open_questions>

None — all gray areas resolved.

</open_questions>
