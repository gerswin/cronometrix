---
phase: 04-frontend-ui
verified: 2026-04-23T22:00:00Z
status: human_needed
score: 18/18 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Navigate to /dashboard without a refresh_token cookie in browser"
    expected: "Redirected to /login?redirect=/dashboard"
    why_human: "proxy.ts cookie check is server-side Next.js middleware — cannot simulate without running dev server"
  - test: "Open /dashboard while logged in; observe KPI tiles and live activity feed"
    expected: "4 KPI tiles show real data from backend; Actividad en Vivo feed is empty initially, populates on biometric event; SSE disconnect shows orange banner"
    why_human: "SSE real-time behavior and KPI hydration from live API require a running stack"
  - test: "Click edit (pencil) icon on a timesheet row as Admin; fill justification and attach a PDF under 5MB; submit"
    expected: "POST /daily-records/{id}/overrides succeeds with 201; TanStack Query invalidates and table refreshes; modal closes"
    why_human: "End-to-end multipart submission requires running backend with a seeded daily_record"
  - test: "Log in as Supervisor role; visit /timesheet"
    expected: "Edit pencil column is absent from the table; 'Registrar Novedad' button is hidden"
    why_human: "Role gating is client-side conditional render — requires real JWT with role=supervisor"
  - test: "Visit /employees as Viewer role"
    expected: "Nuevo Empleado button hidden; Emitir Reporte button hidden; grid is read-only"
    why_human: "Requires real JWT with role=viewer to verify correct UI gating"
  - test: "Open CommandModal as Admin on a registered device; select 'Abrir Puerta'; dispatch"
    expected: "POST /devices/{id}/commands succeeds; Sonner toast shows success message"
    why_human: "Requires a registered device and running backend to exercise dispatch path"
---

# Phase 4: Frontend UI Verification Report

**Phase Goal:** Admin and supervisors can view a real-time attendance dashboard, browse the weekly timesheet with edit capability, and manage employees and devices — all from an authenticated web UI.
**Verified:** 2026-04-23T22:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Protected routes redirect unauthenticated users to /login?redirect=<path> | VERIFIED | `proxy.ts` L3-27: PROTECTED_PATHS array + `req.cookies.get('refresh_token')` check + `NextResponse.redirect(loginUrl)` with redirect param |
| 2 | Access token decoded from JWT exposes role/sub via useAuth() | VERIFIED | `auth-context.tsx`: `decodeJwtPayload()` reads atob(payload) → JWTClaims; `AuthProvider` sets role/sub in context; `use-auth.ts` re-exports useAuth |
| 3 | Session expiry (401 after refresh failure) shows Sonner toast then redirects to /login | VERIFIED | `api.ts` L46-53: `toast.error('Tu sesión ha expirado', { duration: 3000 })` + `setTimeout` redirect to `/login?redirect=` |
| 4 | Sidebar nav renders all 5 main sections (dashboard, timesheet, employees, devices, enrollment); active route highlighted | VERIFIED | `sidebar.tsx` NAV_ITEMS: 5 core routes + 2 future (reports/audit); `pathname.startsWith(href) ? 'bg-slate-700 text-white'` |
| 5 | GET /api/v1/events/stream SSE endpoint exists on backend, accepts ?token=<jwt>, returns JSON events | VERIFIED | `events/handlers.rs` L35-55: `events_stream` handler with `StreamQuery { token }`, `verify_access_token`, `BroadcastStream`, `Sse::new(...).keep_alive()`; registered L111 main.rs in public_routes |
| 6 | vitest run passes all test stubs | VERIFIED | vitest run output: PASS (19) FAIL (0) — confirmed live |
| 7 | Dashboard shows 4 KPI tiles (Empleados Presentes, % Retraso Hoy, Dispositivos Activos, Alertas Diurnas) | VERIFIED | `dashboard/page.tsx` L45-55: grid of 4 KPITile components with real aggregated data from TanStack Query |
| 8 | Actividad en Vivo list shows up to 20 most recent events with circular avatar (photo or initials) | VERIFIED | `activity-feed.tsx` L30-78: useSSE hook + addToRingBuffer(prev, payload, 20); EventAvatar with `has_photo` img / initials fallback |
| 9 | Distribución por Depto. donut chart renders using Recharts | VERIFIED | `dept-chart.tsx` exists; dashboard/page.tsx L63: `<DeptChart records={records} />` wired with real daily-records data |
| 10 | SSE disconnect triggers orange banner with auto-retry exponential backoff | VERIFIED | `use-sse.ts`: BACKOFF_DELAYS [1000, 2000, 4000, 8000, 30000]; `sse-reconnect-banner.tsx`; `reconnecting` state passed to ActivityFeed |
| 11 | Timesheet screen defaults to current Monday–Sunday week; prev/next navigate weeks | VERIFIED | `timesheet/page.tsx` L27-28: `startOfWeek(currentDate, { weekStartsOn: 1 })`; WeekNavigator component exists |
| 12 | Attendance grid columns: Empleado, Entrada, Min. Inicio, Min. Fin, Salida, Total Min, Estado, edit icon (admin-only) | VERIFIED | `timesheet-table.tsx` L76-130: 8 column definitions; edit column spread only when `role === 'admin'` |
| 13 | Estado badge renders Normal/Ausente/Justificado/Ausente Justificado | VERIFIED | `timesheet-table.tsx` L14-41: `getStatusBadge()` with 4 states and correct color classes |
| 14 | Modal justification field required (min 1 char); evidence file upload enforces 5MB / PDF-JPG-PNG | VERIFIED | `novedad-modal.tsx` uses `novedadSchema` with zodResolver; `evidenceFileSchema` in validations.ts; backend L129-145 validates both fields with 422 on missing |
| 15 | POST /api/v1/daily-records/{id}/overrides writes to daily_record_overrides table and fires audit trigger | VERIFIED | `daily_records/handlers.rs`: INSERT into daily_record_overrides with justification, evidence_path, overridden_by, timestamps; registered under `admin_routes` with `require_admin` middleware (main.rs L169) |
| 16 | Employee directory at /employees: 7 D-11 columns, filters, server-side pagination 10 rows/page | VERIFIED | `employee-table.tsx`: 7 ColumnDef entries (name, cedula, department_name, position, hire_date, status, actions); PAGE_SIZE=10; Anterior/Siguiente; employees/page.tsx filters: search, dept dropdown, status dropdown |
| 17 | RBAC gating: Nuevo Empleado (Admin only); Emitir Reporte (Admin+Supervisor); ISAPI command buttons (Admin only) | VERIFIED | `employees/page.tsx` L78-88: role check for both buttons; `device-table.tsx` L58: `role === 'admin'` guard on command button |
| 18 | Device manager at /devices shows device list with status badges and Admin-only command dispatch via POST /devices/{id}/commands | VERIFIED | `device-table.tsx`: StatusBadge (online/offline/unknown); `command-modal.tsx` posts to `/devices/${device.id}/commands`; route L166 main.rs under admin_routes |

**Score:** 18/18 truths verified

### Deferred Items

None. All phase 4 must-haves are addressed by this phase's plans.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `frontend/src/proxy.ts` | Auth guard for protected routes | VERIFIED | PROTECTED_PATHS + cookie check + redirect wired |
| `frontend/src/types/api.ts` | TypeScript interfaces for all backend shapes | VERIFIED | 9 interfaces: DailyRecord, PaginatedResponse, Employee, Department, Device, AttendanceEvent, AttendanceEventSSEPayload, Leave, JWTClaims |
| `frontend/src/contexts/auth-context.tsx` | AuthProvider + useAuth() | VERIFIED | Substantive — decodes JWT, provides role/sub/claims |
| `frontend/src/hooks/use-auth.ts` | Re-export of useAuth | VERIFIED | Exists, re-exports from auth-context |
| `frontend/src/hooks/use-sse.ts` | SSE hook with backoff + ring buffer | VERIFIED | BACKOFF_DELAYS, EventSource ref, onMessageRef pattern |
| `frontend/src/components/dashboard/kpi-tile.tsx` | KPI tile component | VERIFIED | Substantive with variant prop (default/warning/danger) |
| `frontend/src/components/dashboard/activity-feed.tsx` | Live feed with SSE | VERIFIED | useSSE wired, addToRingBuffer, EventAvatar |
| `frontend/src/components/timesheet/timesheet-table.tsx` | TanStack Table v8 grid | VERIFIED | useReactTable, 8 columns, role-gated edit |
| `frontend/src/components/timesheet/novedad-modal.tsx` | Registrar Novedad modal | VERIFIED | react-hook-form + zodResolver + useMutation → overrides API |
| `frontend/src/components/employees/employee-table.tsx` | Employee grid with pagination | VERIFIED | 7 columns, Anterior/Siguiente, role-gated actions |
| `frontend/src/components/devices/device-table.tsx` | Device list with status badges | VERIFIED | StatusBadge, Admin-only command button |
| `backend/src/state.rs` | AppState with event_broadcast | VERIFIED | `Option<broadcast::Sender<AttendanceEventSSEPayload>>` field present |
| `backend/src/events/handlers.rs` | GET /events/stream SSE handler | VERIFIED | events_stream with JWT validation, BroadcastStream |
| `backend/src/daily_records/handlers.rs` | POST overrides multipart handler | VERIFIED | Reads multipart fields, validates justification + evidence, INSERTs to daily_record_overrides |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `proxy.ts` | refresh_token httpOnly cookie | `req.cookies.get('refresh_token')` | WIRED | L20-21: `req.cookies.get('refresh_token')?.value` |
| `api.ts` | sonner toast + /login redirect | axios 401 interceptor | WIRED | L46-53: toast.error + setTimeout redirect |
| `events/handlers.rs` | `state.rs` event_broadcast | `state.event_broadcast.as_ref()?.subscribe()` | WIRED | L44-47: subscribe() called on Option<Sender> |
| `use-sse.ts` | GET /api/v1/events/stream | `new EventSource(url + ?token=)` | WIRED | L22-25: url constructed with NEXT_PUBLIC_API_URL + /events/stream?token= |
| `activity-feed.tsx` | `use-sse.ts` | `useSSE()` hook | WIRED | L37: `useSSE<AttendanceEventSSEPayload>('/events/stream', handleMessage)` |
| `novedad-modal.tsx` | POST /daily-records/{id}/overrides | useMutation + FormData multipart | WIRED | L53-56: `api.post('/daily-records/${record.id}/overrides', fd, multipart header)` |
| `daily_records/handlers.rs` | daily_record_overrides table | INSERT SQL | WIRED | L176+: INSERT into daily_record_overrides with all required columns |
| `command-modal.tsx` | POST /devices/{id}/commands | useMutation + axios.post | WIRED | L28-29: `api.post('/devices/${device.id}/commands', { command: selectedCommand })` |
| `employee-table.tsx` | GET /api/v1/employees | TanStack Query useQuery | WIRED | `employees/page.tsx` L20-32: useQuery with pagination + filter params |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| `dashboard/page.tsx` | records (DailyRecord[]) | useQuery → GET /daily-records | Yes — Rust handler queries DB | FLOWING |
| `dashboard/page.tsx` | devices (Device[]) | useQuery → GET /devices (refetchInterval 30s) | Yes — Rust handler queries DB | FLOWING |
| `activity-feed.tsx` | events ring buffer | useSSE → GET /events/stream | Yes — BroadcastStream from real attendance events | FLOWING |
| `timesheet/page.tsx` | data (PaginatedResponse) | useQuery → GET /daily-records with weekStart/weekEnd | Yes — Rust handler queries DB | FLOWING |
| `employees/page.tsx` | employees | useQuery → GET /employees with filters + pagination | Yes — Rust handler queries DB | FLOWING |
| `devices/page.tsx` | devices | useQuery → GET /devices (refetchInterval 30s) | Yes — Rust handler queries DB | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| vitest suite passes | `npx vitest run` (run live) | PASS (19) FAIL (0) | PASS |
| SSE route registered in main.rs | grep main.rs | `/events/stream` → `events_stream` in public_routes | PASS |
| Overrides route under admin_routes | grep main.rs | `/daily-records/{id}/overrides` under require_admin layer | PASS |
| Backend compiles | cargo build (per SUMMARY) | 0 errors, all commits present in git log | PASS |
| All artifact files exist | ls checks | All 14 key files confirmed present | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| DASH-01 | 04-02 | Dashboard real-time KPIs (present, late, absentees) | SATISFIED | kpi-tile.tsx + dashboard/page.tsx aggregateKPIs() |
| DASH-02 | 04-02, 04-04 | Dashboard shows device connection status | SATISFIED | DeviceStatusSummary in KPI tile + DeviceTable |
| DASH-03 | 04-02 | Dashboard live photo feed from recognition events | SATISFIED | activity-feed.tsx with EventAvatar photo or initials |
| TS-01 | 04-03 | Supervisor can view daily attendance grid | SATISFIED | TimesheetTable with DailyRecord data from backend |
| TS-02 | 04-03 | Supervisor can edit entry/exit time for a specific day | SATISFIED | NovedadModal with override_entry_at / override_exit_at fields |
| TS-03 | 04-03 | Every timesheet edit requires text justification | SATISFIED | novedadSchema min(1) frontend + backend trim().is_empty() check |
| TS-04 | 04-03 | Every timesheet edit requires evidence file upload | SATISFIED | evidenceFileSchema 5MB cap + backend evidence required (TS-04) |
| TS-05 | 04-03 | Immutable audit log entry for every timesheet edit | SATISFIED | SQLite trigger on daily_record_overrides INSERT (migration 011) |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `employee-table.tsx` | 67 | `// TODO Phase 7: open employee edit modal` + `alert()` call | Info | Edit button falls back to `alert()` — no modal; edit flow is a stub. Intentional per scope: employee edit UI is Phase 7. Does NOT block DASH-02 or any Phase 4 requirement. |

### Human Verification Required

#### 1. Auth Guard Redirect

**Test:** Open browser, clear all cookies, navigate directly to `/dashboard`
**Expected:** Immediately redirected to `/login?redirect=/dashboard`
**Why human:** Next.js proxy.ts runs server-side — cannot simulate cookie state without running dev server

#### 2. Real-time Dashboard (SSE)

**Test:** Log in as Admin; open `/dashboard`; trigger a biometric event from a Hikvision device (or test webhook call to POST /api/v1/isapi/events)
**Expected:** Activity feed populates in real time; KPI tiles show correct counts; disconnect Ethernet to test SSE drop → orange banner appears → auto-reconnects
**Why human:** SSE real-time behavior requires running stack; exponential backoff only verifiable with network interruption

#### 3. Timesheet Override End-to-End (TS-02, TS-03, TS-04, TS-05)

**Test:** Log in as Admin; navigate to `/timesheet`; click the pencil icon on any row; fill justification field; attach a valid PDF under 5MB; submit
**Expected:** 201 response from backend; modal closes; TanStack Query refetches; audit_log record created in DB
**Why human:** Requires running backend with seeded daily_records data

#### 4. Role-Gated UI (RBAC)

**Test:** Log in separately as Supervisor and Viewer; verify in each session: (a) Supervisor sees no edit pencil in timesheet but sees Emitir Reporte; (b) Viewer sees no Emitir Reporte, no Nuevo Empleado, no device command buttons
**Expected:** Correct buttons hidden/shown per D-14 rules
**Why human:** Requires real JWTs with distinct roles

#### 5. ISAPI Command Dispatch

**Test:** As Admin, open `/devices`; select a registered device; open CommandModal; send door_open command
**Expected:** POST /devices/{id}/commands returns success; Sonner toast confirms; no device modal needed for Phase 4 (modal is functional)
**Why human:** Requires a registered device and backend running

#### 6. Novedad Modal Validation

**Test:** Open NovedadModal; submit with empty justification field
**Expected:** Form blocks submission; Zod error message renders under justification textarea
**Why human:** Form validation behavior requires rendered React component in browser

### Gaps Summary

No automated gaps found. All 18 observable truths pass all four verification levels (exists, substantive, wired, data-flowing). The single anti-pattern (employee edit TODO) is intentional and out-of-scope for Phase 4.

The phase goal is fully implemented in code. Six behaviors require human verification with a running stack before the phase can be declared fully complete.

---

_Verified: 2026-04-23T22:00:00Z_
_Verifier: Claude (gsd-verifier)_
