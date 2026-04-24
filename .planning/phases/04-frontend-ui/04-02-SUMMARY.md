---
phase: 04-frontend-ui
plan: "02"
subsystem: frontend-dashboard
tags: [dashboard, sse, recharts, kpi, real-time, vitest, tdd]
dependency_graph:
  requires:
    - 04-01  # types, api.ts, ring-buffer, kpi-utils, app shell
  provides:
    - dashboard-page-full
    - use-sse-hook
    - activity-feed-component
    - kpi-tile-component
    - dept-chart-component
    - device-banner-component
    - sse-reconnect-banner-component
  affects:
    - frontend/src/app/(dashboard)/dashboard/page.tsx
tech_stack:
  added:
    - recharts (PieChart donut — already in package.json)
    - EventSource API (native browser SSE)
  patterns:
    - useRef-stable-EventSource (Pitfall 6 avoidance)
    - onMessageRef-fresh-callback (stale closure avoidance)
    - exponential-backoff-retry (SSE reconnect)
    - ring-buffer-newest-first (20-item cap)
    - TanStack-Query-refetchInterval (30s device polling)
key_files:
  created:
    - frontend/src/hooks/use-sse.ts
    - frontend/src/components/dashboard/sse-reconnect-banner.tsx
    - frontend/src/components/dashboard/activity-feed.tsx
    - frontend/src/components/dashboard/device-banner.tsx
    - frontend/src/components/dashboard/kpi-tile.tsx
    - frontend/src/components/dashboard/dept-chart.tsx
  modified:
    - frontend/src/__tests__/device-banner.test.tsx
    - frontend/src/app/(dashboard)/dashboard/page.tsx
decisions:
  - "Named import { api } from @/lib/api used in dashboard page — api.ts has no default export"
  - "TDD cycle applied to device-banner: RED commit 97efcdf, GREEN commit e5f2d0e"
  - "node_modules symlinked from main repo to worktree for vitest execution"
metrics:
  duration: "~2 minutes"
  completed_date: "2026-04-24"
  tasks_completed: 2
  files_changed: 8
requirements_delivered:
  - DASH-01
  - DASH-02
  - DASH-03
---

# Phase 04 Plan 02: Dashboard Screen (SSE + KPI Tiles + Donut Chart) Summary

## One-liner

Full real-time dashboard with 4 KPI tiles, SSE-powered live activity feed with exponential backoff reconnect, Recharts donut chart, and device offline badge — all wired on `/dashboard`.

## What Was Built

### Task 1 (TDD): useSSE Hook + Activity Feed + Device Banner + SSE Reconnect Banner

**`frontend/src/hooks/use-sse.ts`** — EventSource hook with:
- `useRef` for EventSource (never recreated on re-render)
- `onMessageRef` pattern to avoid stale closure on message callback
- `BACKOFF_DELAYS = [1000, 2000, 4000, 8000, 30000]` exponential backoff
- `timerRef` cleared before each retry (prevents reconnect storm — T-4-07)
- Token passed as `?token=<jwt>` URL param (EventSource cannot send Bearer headers)
- Returns `{ connected, reconnecting }` state

**`frontend/src/components/dashboard/sse-reconnect-banner.tsx`** — Orange banner auto-shown when `reconnecting=true`, auto-hidden when connection restored. No user action needed.

**`frontend/src/components/dashboard/activity-feed.tsx`** — SSE-powered live feed:
- 20-item ring buffer (newest first via `addToRingBuffer`)
- 40px circular avatar: `<img>` from `/api/v1/events/{id}/photo` if `has_photo`, initials fallback otherwise
- Direction badge: green "Entrada" / blue "Salida"
- "Ver todo" link → `/timesheet?from_date=TODAY&to_date=TODAY`

**`frontend/src/components/dashboard/device-banner.tsx`** — `DeviceStatusSummary` component:
- All online → `X/X en línea` (green)
- Some offline → `N desconectado(s)` (yellow)
- All offline → `N OFFLINE` (red)

**`frontend/src/__tests__/device-banner.test.tsx`** — 3 real assertions replacing placeholder.

### Task 2: KPI Tile + DeptChart + Full Dashboard Page

**`frontend/src/components/dashboard/kpi-tile.tsx`** — Reusable KPI card with `default | warning | danger` variants (border color changes).

**`frontend/src/components/dashboard/dept-chart.tsx`** — Recharts `PieChart` donut:
- Groups `DailyRecord[]` by `department_id` counting `work_minutes > 0`
- `'use client'` directive required (Recharts uses browser APIs)
- 7-color palette, shows "Sin datos para hoy" when empty

**`frontend/src/app/(dashboard)/dashboard/page.tsx`** — Full dashboard screen:
- 4 KPI tiles in a CSS grid row: Empleados Presentes, % Retraso Hoy, Dispositivos Activos (with DeviceStatusSummary sub-slot), Alertas Diurnas
- Left panel (3/5 cols): `ActivityFeed` (SSE-powered)
- Right panel (2/5 cols): `DeptChart` (Recharts donut)
- `daily-records-today` query: staleTime 60s
- `devices` query: `refetchInterval: 30_000`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Named import for `api` in dashboard page**
- **Found during:** Task 2 implementation
- **Issue:** Plan's dashboard code used `import api from '@/lib/api'` (default import), but `api.ts` exports named `export const api = axios.create(...)` — no default export
- **Fix:** Changed to `import { api } from '@/lib/api'`
- **Files modified:** `frontend/src/app/(dashboard)/dashboard/page.tsx`
- **Commit:** 7f310a3

**2. [Rule 3 - Blocking] node_modules missing in worktree**
- **Found during:** Task 1 verification
- **Issue:** Worktree has no `node_modules/` — `npx vitest` resolved to a stale global binary that couldn't find `vitest/config`
- **Fix:** Symlinked `/frontend/node_modules` from main repo to worktree frontend directory
- **Commit:** Not committed (runtime-only symlink, not tracked)

## TDD Gate Compliance

| Gate | Commit | Message |
|------|--------|---------|
| RED (test) | 97efcdf | `test(04-02): add failing device-banner tests (TDD RED)` |
| GREEN (feat) | e5f2d0e | `feat(04-02): useSSE hook + activity-feed + device-banner + sse-reconnect-banner (TDD GREEN)` |

Both gates present. RED commit correctly failed (module not found — component didn't exist). GREEN commit correctly passed 3 assertions.

## Known Stubs

None. All components receive real data from TanStack Query or SSE. Activity feed starts empty (correct — no SSE events until backend connects).

## Threat Flags

No new threat surface beyond the plan's `<threat_model>`. All STRIDE mitigations confirmed:

| Flag | Status | Evidence |
|------|--------|---------|
| T-4-06 Photo URL | Mitigated | `activity-feed.tsx`: `${API}/api/v1/events/${event.id}/photo` — API_BASE constant, no user input, no dangerouslySetInnerHTML |
| T-4-07 SSE reconnect storm | Mitigated | `use-sse.ts`: `timerRef` cleared before each `setTimeout`; EventSource `.close()` called before retry |

## Self-Check: PASSED

All 8 key files exist on disk. All 3 task commits (97efcdf, e5f2d0e, 7f310a3) found in git log. All 12 vitest tests pass (6 test files).
