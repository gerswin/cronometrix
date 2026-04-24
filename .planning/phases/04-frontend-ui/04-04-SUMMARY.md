---
phase: 04-frontend-ui
plan: 04
subsystem: ui
tags: [react, tanstack-table, next.js, shadcn, typescript, rbac]

requires:
  - phase: 04-01
    provides: AuthProvider/useAuth(), TypeScript API types (Employee, Device), authenticated app shell

provides:
  - Employee Directory with TanStack Table v8, server-side pagination, search/filter, RBAC-gated actions
  - Device Manager with status table, ISAPI command dispatch modal, role-gated controls
  - shadcn Dialog primitive (frontend/src/components/ui/dialog.tsx)

affects: [05-reports]

tech-stack:
  added: [shadcn/ui Dialog]
  patterns: [TanStack Table v8 with server-side pagination, role-gated action buttons via useAuth()]

key-files:
  created:
    - frontend/src/components/employees/employee-table.tsx
    - frontend/src/components/devices/device-table.tsx
    - frontend/src/components/devices/command-modal.tsx
    - frontend/src/components/ui/dialog.tsx
  modified:
    - frontend/src/app/(dashboard)/employees/page.tsx
    - frontend/src/app/(dashboard)/devices/page.tsx

key-decisions:
  - "EmployeeTable uses server-side pagination (10 rows/page) via offset/limit query params"
  - "Nuevo Empleado button Admin-only; Emitir Reporte Admin+Supervisor; Viewer sees read-only grid (D-14)"
  - "CommandModal sends ISAPI commands (door_open, reboot, sync_profiles) Admin-only via POST /devices/{id}/command"
  - "StatusBadge: Activo=green, Pendiente=yellow, Inactivo=slate — consistent with D-11 mockup"
  - "Dialog primitive added to shadcn/ui to support CommandModal — base for future modal needs"

patterns-established:
  - "Role-gated buttons: const { role } = useAuth(); if (role !== 'admin') return null"
  - "TanStack Table server-side pagination: manual pagination mode + query state for offset/limit"
  - "Filter state colocated in page component, passed as query params to TanStack Query"

requirements-completed: [DASH-03, TS-05]

duration: 8min
completed: 2026-04-23
---

# Phase 04-04: Employee Directory + Device Manager Summary

**Employee Directory with role-gated RBAC and Device Manager with ISAPI command dispatch — both using TanStack Table v8 with server-side pagination.**

## Performance

- **Duration:** ~8 min
- **Completed:** 2026-04-23
- **Tasks:** 2/2
- **Files modified:** 6

## Accomplishments

### Task 1: Employee Directory
- `EmployeeTable` — TanStack Table v8, 7 columns (Nombre, Cédula, Departamento, Cargo, Fecha Ingreso, Estatus, Acciones)
- Server-side pagination: 10 rows/page, Anterior/Siguiente navigation
- Filter controls: free-text search + Departamento dropdown + Estatus dropdown
- StatusBadge component: Activo (green), Pendiente (yellow), Inactivo (slate)
- Role-gated buttons: Nuevo Empleado (Admin only), Emitir Reporte (Admin+Supervisor), Edit icon in Acciones (Admin only)

### Task 2: Device Manager + Command Modal
- `DeviceTable` — device name, IP, direction, status badge, last seen, actions
- Online/Offline/Unknown status badges with color coding
- `CommandModal` — Admin-only ISAPI command dispatch (door_open, reboot, sync_profiles) via POST /devices/{id}/command
- `dialog.tsx` shadcn Dialog primitive added as base component
- Viewer role sees read-only grid with no action buttons

## Self-Check: PASSED

- Employee table renders with 7 D-11 columns ✓
- Admin sees all action buttons; Supervisor sees Emitir Reporte only; Viewer sees none ✓
- Device command modal Admin-only ✓
- Server-side pagination with offset/limit ✓
- TypeScript types from 04-01 (Employee, Device) used correctly ✓
