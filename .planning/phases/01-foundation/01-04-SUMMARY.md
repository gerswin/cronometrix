---
phase: 01-foundation
plan: 04
subsystem: ui
tags: [nextjs, react, shadcn, tanstack-query, react-hook-form, zod, axios, typescript, tailwind]

requires:
  - phase: 01-02
    provides: JWT auth endpoints (login, refresh, logout) and setup/init API that this frontend calls
  - phase: 01-03
    provides: Employee/Department/Rules CRUD endpoints accessible via the axios client built here

provides:
  - Next.js 16.2.3 frontend scaffold with App Router, TypeScript, Tailwind 4, shadcn/ui
  - TanStack Query client with axios interceptors and in-memory access token + auto-refresh
  - Zod validation schemas for setup and login forms
  - Proxy-based routing (Next.js 16 proxy.ts) for setup redirect logic
  - Setup wizard: centered card, 4 form fields, password toggles, Zod validation, server error handling
  - Login page: username/password fields, generic 401 error (no enumeration), access token storage

affects: [Phase 2, Phase 3, Phase 4 — dashboard screens build on this scaffold and auth client]

tech-stack:
  added:
    - Next.js 16.2.3 (App Router)
    - @tanstack/react-query@5
    - react-hook-form@7
    - @hookform/resolvers
    - zod@3
    - axios
    - lucide-react
    - shadcn/ui (card, input, label, button, form components)
    - tailwindcss@4
    - @radix-ui/react-label, @radix-ui/react-slot
  patterns:
    - Access token stored in memory only (setAccessToken/getAccessToken) — never localStorage (T-01-20)
    - Auto-refresh on 401 via axios response interceptor calling /auth/refresh with cookie
    - QueryClientProvider wrapped in a dedicated providers.tsx client component (App Router pattern)
    - Metadata exported from server-side layout.tsx, not client component pages
    - proxy.ts replaces middleware.ts (Next.js 16 breaking change)
    - aria-disabled on submit button during loading (not HTML disabled — preserves keyboard focus)
    - noValidate on form + Zod resolver = client-side validation before network request

key-files:
  created:
    - frontend/src/lib/api.ts
    - frontend/src/lib/validations.ts
    - frontend/src/proxy.ts
    - frontend/src/components/providers.tsx
    - frontend/src/components/ui/form.tsx
    - frontend/src/app/setup/page.tsx
    - frontend/src/app/setup/layout.tsx
    - frontend/src/app/login/page.tsx
  modified:
    - frontend/src/app/layout.tsx
    - frontend/src/app/page.tsx
    - frontend/src/app/globals.css
    - frontend/.gitignore

key-decisions:
  - "proxy.ts (not middleware.ts): Next.js 16 renamed Middleware to Proxy — function export also renamed to `proxy`"
  - "Metadata in layout.tsx not page.tsx: Next.js 16 forbids metadata export from client components ('use client')"
  - "Providers component: QueryClientProvider must be a client component, isolated from server Root Layout"
  - "frontend/.git removed: create-next-app creates its own git repo; removed to track files in monorepo"

patterns-established:
  - "Form pattern: react-hook-form + zodResolver + shadcn Form/FormField/FormControl/FormMessage for all auth forms"
  - "Error banner pattern: AlertCircle + destructive/10 bg + border-l-4 border-destructive in card top"
  - "Password toggle pattern: relative Input wrapper + absolute button with Eye/EyeOff + aria-label"
  - "Loading button pattern: aria-disabled + onClick preventDefault + Loader2 spinner inline"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05]

duration: 6min
completed: 2026-04-14
---

# Phase 01 Plan 04: Frontend Scaffold — Setup Wizard and Login Summary

**Next.js 16 frontend with shadcn/ui setup wizard and login page wired to Rust backend auth API, using in-memory JWT + httpOnly cookie refresh pattern**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-14T17:52:06Z
- **Completed:** 2026-04-14T17:58:55Z
- **Tasks:** 2 of 3 executed (Task 3 is a human-verify checkpoint)
- **Files modified:** 31 (full frontend scaffold)

## Accomplishments

- Scaffolded Next.js 16.2.3 with App Router, TypeScript, Tailwind 4, shadcn/ui — build passes clean
- Implemented setup wizard with all 4 form fields, password toggles, Zod validation, loading states, error handling, and already-configured redirect logic
- Implemented login page with generic 401 error (no username enumeration per T-01-19), in-memory token storage, and axios auto-refresh interceptor
- API client (api.ts) handles Bearer token injection, 401 auto-refresh via httpOnly cookie, and in-memory token storage safe from XSS (T-01-20)
- Proxy routing (proxy.ts) checks /api/v1/setup/status on every request and redirects to /setup when not initialized

## Task Commits

1. **Task 1: Scaffold Next.js with shadcn/ui, TanStack Query, and proxy routing** - `f96534a` (feat)
2. **Task 2: Setup wizard and login page per UI-SPEC design contract** - `cb892d3` (feat)
3. **Task 3: Human verification checkpoint** - awaiting user approval

## Files Created/Modified

- `frontend/src/lib/api.ts` — Axios client with Bearer interceptor, in-memory token, auto-refresh on 401
- `frontend/src/lib/validations.ts` — Zod schemas: setupSchema (full_name, username, password, confirm_password with match refinement) and loginSchema
- `frontend/src/proxy.ts` — Next.js 16 proxy (formerly middleware): checks setup/status, redirects to /setup when not initialized
- `frontend/src/components/providers.tsx` — Client-side QueryClientProvider wrapper for App Router
- `frontend/src/components/ui/form.tsx` — shadcn Form component (react-hook-form wrapper with aria attributes)
- `frontend/src/app/layout.tsx` — Root layout: Inter font, Cronometrix metadata, Providers wrapper
- `frontend/src/app/page.tsx` — Root page: server-side redirect to /login
- `frontend/src/app/setup/page.tsx` — Setup wizard client component per UI-SPEC
- `frontend/src/app/setup/layout.tsx` — Setup route server layout exporting "Cronometrix — Setup" metadata
- `frontend/src/app/login/page.tsx` — Login page client component
- `frontend/.env.example` — NEXT_PUBLIC_API_URL=http://localhost:3001

## Decisions Made

- **proxy.ts not middleware.ts:** Next.js 16 renamed Middleware to Proxy. The build rejects `middleware.ts` with a deprecation error when `proxy.ts` also exists. Used `proxy.ts` with `export function proxy(req)` as the new API.
- **Metadata in layout.tsx:** Next.js 16 forbids exporting `metadata` from client components (`"use client"`). Created `setup/layout.tsx` as a server component to export the page title.
- **frontend/.git removed:** `create-next-app` initializes its own git repo. Removed to allow tracking in the project monorepo.
- **Providers component pattern:** QueryClientProvider requires `"use client"`. Isolated in `src/components/providers.tsx` so the root `layout.tsx` remains a server component.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Next.js 16 renames middleware.ts to proxy.ts**
- **Found during:** Task 1 (scaffold and build verification)
- **Issue:** Next.js 16.2.3 deprecated the `middleware.ts` convention in favor of `proxy.ts`. Build errored when both files existed: "Both middleware file and proxy file are detected. Please use proxy.ts only."
- **Fix:** Created `src/proxy.ts` with `export function proxy(req)` (new API), removed `src/middleware.ts`
- **Files modified:** `frontend/src/proxy.ts` (new), `frontend/src/middleware.ts` (deleted)
- **Verification:** Build passes cleanly with no warnings
- **Committed in:** f96534a (Task 1 commit)

**2. [Rule 1 - Bug] Cannot export metadata from client component**
- **Found during:** Task 2 (build verification after setup wizard)
- **Issue:** Setup wizard uses `"use client"` for form state. Next.js 16 forbids `export const metadata` in client components.
- **Fix:** Removed metadata export from `setup/page.tsx`, created `setup/layout.tsx` as a server component that exports `metadata: { title: "Cronometrix — Setup" }`
- **Files modified:** `frontend/src/app/setup/page.tsx`, `frontend/src/app/setup/layout.tsx` (new)
- **Verification:** Build passes; `/setup` route renders correctly
- **Committed in:** cb892d3 (Task 2 commit)

**3. [Rule 3 - Blocking] create-next-app initialized a nested git repository**
- **Found during:** Task 1 (git add after scaffold)
- **Issue:** `create-next-app` ran `git init` inside `frontend/`, making it a submodule from the parent repo's perspective. `git add frontend/` staged it as a gitlink entry, not individual files.
- **Fix:** Ran `git rm --cached -f frontend`, removed `frontend/.git`, re-ran `git add frontend/`
- **Files modified:** `frontend/` directory (removed nested .git)
- **Verification:** `git diff --cached --stat` shows 31 files staged correctly
- **Committed in:** f96534a (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (2 Next.js 16 breaking changes, 1 blocking git issue)
**Impact on plan:** All auto-fixes necessary for build success and correct file tracking. No scope creep.

## Known Stubs

None — all form fields are wired to real API endpoints. Token management is wired to the live `/auth/refresh` endpoint via axios interceptor.

## Threat Flags

No new threat surface beyond what the plan's threat model already covers (T-01-19 through T-01-22 all addressed).

## Issues Encountered

- `create-next-app` with flags like `--typescript` produced npm warnings ("Unknown cli config") — flags are passed as positional args in newer npm. Worked around by using `node -e "execSync(...)"` to run commands in the correct working directory.
- `npx shadcn add form` silently skipped the form component — added manually as `src/components/ui/form.tsx` following the standard shadcn form implementation pattern.

## User Setup Required

None — no external service configuration required for the frontend. Backend env vars were covered in Plan 01-02.

To run locally:
1. `cd frontend && cp .env.example .env.local`
2. `npm run dev`
3. Backend must be running at `http://localhost:3001`

## Next Phase Readiness

- Frontend scaffold ready — all auth screens functional
- Task 3 checkpoint requires human verification of end-to-end setup → login flow and RBAC boundaries
- After checkpoint approval: Phase 1 foundation is complete

## Self-Check: PASSED

- All created files exist on disk
- Commits f96534a and cb892d3 verified in git log
- Build passes: `npm run build` exits 0 with routes /, /login, /setup, /_not-found

---
*Phase: 01-foundation*
*Completed: 2026-04-14*
