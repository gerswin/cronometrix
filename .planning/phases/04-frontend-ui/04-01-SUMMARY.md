---
phase: 04-frontend-ui
plan: "01"
subsystem: frontend-shell + backend-sse
tags: [frontend, backend, auth, sse, vitest, next.js, axum]
dependency_graph:
  requires: [03-03]
  provides: [authenticated-app-shell, sse-stream-endpoint, vitest-scaffold, api-types]
  affects: [04-02, 04-03, 04-04]
tech_stack:
  added:
    - "@tanstack/react-table@8"
    - "recharts"
    - "sonner"
    - "date-fns"
    - "vitest + @vitejs/plugin-react + @testing-library/react + @testing-library/jest-dom + jsdom"
    - "tokio-stream 0.1 (sync feature) — Rust"
  patterns:
    - "AuthProvider + useAuth() via React context — decode JWT client-side for role/sub (display only)"
    - "(dashboard) Next.js route group with authenticated layout wrapper"
    - "SSE broadcast channel (tokio::sync::broadcast) — 256-event buffer, non-fatal send"
    - "?token= JWT query param for EventSource (cannot send Bearer headers)"
    - "sonner toast.error + setTimeout redirect on session expiry"
    - "PROTECTED_PATHS cookie guard in proxy.ts (optimistic; backend enforces real auth)"
key_files:
  created:
    - frontend/vitest.config.ts
    - frontend/src/__tests__/setup.ts
    - frontend/src/__tests__/kpi.test.ts
    - frontend/src/__tests__/activity-feed.test.ts
    - frontend/src/__tests__/file-validation.test.ts
    - frontend/src/__tests__/novedad-modal.test.tsx
    - frontend/src/__tests__/timesheet-table.test.tsx
    - frontend/src/__tests__/device-banner.test.tsx
    - frontend/src/types/api.ts
    - frontend/src/lib/kpi-utils.ts
    - frontend/src/lib/ring-buffer.ts
    - frontend/src/contexts/auth-context.tsx
    - frontend/src/hooks/use-auth.ts
    - frontend/src/components/layout/sidebar.tsx
    - frontend/src/components/layout/top-bar.tsx
    - "frontend/src/app/(dashboard)/layout.tsx"
    - "frontend/src/app/(dashboard)/dashboard/page.tsx"
    - "frontend/src/app/(dashboard)/timesheet/page.tsx"
    - "frontend/src/app/(dashboard)/employees/page.tsx"
    - "frontend/src/app/(dashboard)/devices/page.tsx"
    - "frontend/src/app/(dashboard)/enrollment/page.tsx"
  modified:
    - frontend/package.json (5 prod + 5 dev packages added)
    - frontend/src/proxy.ts (PROTECTED_PATHS guard, updated matcher)
    - frontend/src/lib/api.ts (sonner toast import, redirect with ?redirect= param)
    - frontend/src/lib/validations.ts (evidenceFileSchema added)
    - backend/Cargo.toml (tokio-stream added)
    - backend/src/state.rs (AttendanceEventSSEPayload struct, event_broadcast field)
    - backend/src/main.rs (broadcast channel init, SSE route, event_broadcast in AppState)
    - backend/src/events/handlers.rs (events_stream handler)
    - backend/src/events/service.rs (publish_sse_event helper)
    - backend/src/isapi/stream.rs (publish_sse_event call on Inserted)
    - "22 integration test files (event_broadcast: None added to AppState literals)"
decisions:
  - "/events/stream registered in public_routes not viewer_routes — EventSource cannot send Authorization headers; JWT validated inside handler via ?token= query param"
  - "AppState.event_broadcast is Option<broadcast::Sender<...>> — same Option pattern as lifecycle_tx/recompute_tx for test compatibility"
  - "Page shells use minimal placeholder text (Cargando…) — content filled in by 04-02 and 04-03"
  - "evidenceFileSchema uses z.instanceof(File) — compatible with zod v4.3.6"
metrics:
  duration_minutes: 9
  completed_date: "2026-04-24"
  tasks_completed: 2
  files_changed: 43
---

# Phase 4 Plan 01: Foundation + Auth Shell + SSE Endpoint Summary

Installed 10 npm packages (5 prod, 5 dev), wired the authenticated Next.js (dashboard) route group with sidebar navigation and AuthProvider, extended proxy.ts auth guard for 5 protected routes with session toast on expiry, defined all 9 TypeScript API interfaces, and implemented the GET /api/v1/events/stream SSE endpoint in the Rust backend with broadcast channel.

## Tasks Completed

| Task | Name | Commit | Key Deliverables |
|------|------|--------|-----------------|
| 1 | Install packages, vitest, types, proxy/api extensions | 7ee33e7 | 10 npm packages, vitest.config.ts, 6 test stubs, api.ts (9 interfaces), kpi-utils.ts, ring-buffer.ts, evidenceFileSchema, proxy.ts PROTECTED_PATHS, api.ts toast |
| 2 | Auth context, app shell, SSE endpoint | 1c994cb | AuthProvider, useAuth(), Sidebar (7 items), TopBar, (dashboard) layout, 5 page shells, AttendanceEventSSEPayload, event_broadcast in AppState, events_stream handler, publish_sse_event helper |

## Verification Results

- `npx vitest run`: 10/10 tests pass (3 real unit test suites + 3 placeholders)
- `cargo build`: 328 crates compiled, 0 errors
- All 22 integration test AppState literals updated with `event_broadcast: None`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] SSE route moved from viewer_routes to public_routes**
- **Found during:** Task 2
- **Issue:** The plan placed `/events/stream` inside `viewer_routes` which applies `require_auth` Bearer middleware. EventSource (browser API) cannot send custom headers, so the Bearer token cannot be passed via Authorization header — every SSE connection would receive 401.
- **Fix:** Registered `/events/stream` in `public_routes` instead. JWT validation is performed inside the `events_stream` handler itself via `auth_service::verify_access_token(&q.token, ...)` on the `?token=` query param. This matches the plan's documented T-4-02 threat (accepted risk on-premise) and Pitfall 2 annotation in the handler spec.
- **Files modified:** backend/src/main.rs, backend/src/events/handlers.rs
- **Commit:** 1c994cb

**2. [Rule 3 - Blocking] AppState.event_broadcast made Option<> + 22 test files updated**
- **Found during:** Task 2
- **Issue:** Adding `event_broadcast: broadcast::Sender<...>` as a required field broke all 22 integration tests that construct AppState without a broadcast channel.
- **Fix:** Used `Option<broadcast::Sender<...>>` following the same pattern as `lifecycle_tx` and `recompute_tx`. Added `event_broadcast: None` to all 22 test AppState literals. `publish_sse_event` guards on `state.event_broadcast.as_ref()` so a None channel silently skips (no active SSE clients in tests).
- **Files modified:** backend/src/state.rs, all 22 backend/tests/*.rs files
- **Commit:** 1c994cb

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `Cargando dashboard…` | frontend/src/app/(dashboard)/dashboard/page.tsx | Content implemented in 04-02 |
| `Cargando marcaciones…` | frontend/src/app/(dashboard)/timesheet/page.tsx | Content implemented in 04-03 |
| `Cargando empleados…` | frontend/src/app/(dashboard)/employees/page.tsx | Content implemented in 04-02 |
| `Cargando dispositivos…` | frontend/src/app/(dashboard)/devices/page.tsx | Content implemented in 04-02 |
| `Próximamente — Enrolamiento Facial` | frontend/src/app/(dashboard)/enrollment/page.tsx | Content implemented in 04-04 (Phase 7) |
| placeholder test stubs (3 files) | frontend/src/__tests__/novedad-modal, timesheet-table, device-banner | Real assertions added in 04-02/04-03 |

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: information-disclosure | backend/src/events/handlers.rs | JWT token in ?token= URL query param — logged in server access logs. Accepted per T-4-02 (on-premise deployment; short-lived access token) |

## Self-Check: PASSED

- `frontend/vitest.config.ts` — FOUND
- `frontend/src/types/api.ts` — FOUND
- `frontend/src/contexts/auth-context.tsx` — FOUND
- `frontend/src/components/layout/sidebar.tsx` — FOUND
- `frontend/src/app/(dashboard)/layout.tsx` — FOUND
- `backend/src/state.rs` (event_broadcast field) — FOUND
- `backend/src/events/handlers.rs` (events_stream) — FOUND
- Commit 7ee33e7 — FOUND
- Commit 1c994cb — FOUND
- `cargo build` — PASSES (0 crates compiled on second run)
- `npx vitest run` — PASSES (10/10)
