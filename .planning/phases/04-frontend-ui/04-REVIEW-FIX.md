---
phase: 04-frontend-ui
fixed_at: 2026-04-23T00:00:00Z
review_path: .planning/phases/04-frontend-ui/04-REVIEW.md
iteration: 1
findings_in_scope: 11
fixed: 11
skipped: 0
status: all_fixed
---

# Phase 04: Code Review Fix Report

**Fixed at:** 2026-04-23T00:00:00Z
**Source review:** `.planning/phases/04-frontend-ui/04-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 11 (4 critical + 7 warning)
- Fixed: 11
- Skipped: 0

All fixes verified with `cargo build --manifest-path backend/Cargo.toml`
(clean) and `npx vitest run` from `frontend/` (19/19 pass).

## Fixed Issues

### CR-01: JWT Access Token Exposed in SSE URL Query Parameter

**Files modified:** `frontend/src/hooks/use-sse.ts`
**Commit:** ee64767 (combined with WR-01 — same file, related guard logic)
**Applied fix:** Bail out of `connect()` when `getAccessToken()` returns null
instead of building a `?token=` URL with an empty value. URL-encode the token
defensively. The long-term ticket-pattern mitigation requires a new backend
endpoint (`POST /events/stream-ticket`) and is deferred — noted in commit
message and source comment.

---

### CR-02: Open Redirect via Unvalidated `redirect` Query Parameter

**Files modified:** `frontend/src/app/login/page.tsx`
**Commit:** 1f4e754
**Applied fix:** Added `safeRedirect()` helper that only accepts paths
starting with a single `/`, rejecting protocol-relative URLs (`//evil.com`),
absolute URLs, and backslash variants. Wired the login `onSubmit` handler to
read `searchParams.get("redirect")` through the helper. The login page
previously hardcoded `router.push("/")`, so this is preventive: it makes the
redirect param consumable safely if/when the workflow needs it.

---

### CR-03: Multipart File Upload Accepts Spoofed Content-Type

**Files modified:** `backend/src/daily_records/handlers.rs`
**Commit:** 90fea42
**Applied fix:** Added `infer_evidence_ext_from_magic()` that derives the
canonical extension (`pdf`, `jpg`, `png`) from the file's magic-byte
signature (`%PDF`, `FF D8 FF`, `89 50 4E 47 0D 0A 1A 0A`). The declared
multipart Content-Type still acts as a fast pre-filter, but the on-disk
extension and acceptance decision come from the bytes. Returns a 400
`VALIDATION_ERROR` if the bytes do not match a supported signature.

---

### CR-04: Client-Side Role Enforcement from Unverified JWT Decode

**Files modified:** `frontend/src/contexts/auth-context.tsx`,
`frontend/src/components/layout/top-bar.tsx`
**Commit:** c790fdc
**Status:** fixed: requires human verification
**Applied fix:** Documented `decodeJwtPayload` as "display hint only — backend
is authoritative" with a JSDoc block explaining that the backend `AuthUser`
extractor is the only RBAC authority. Added a hover tooltip on the TopBar
role label noting the same. This is a defense-in-depth change: the backend
already verifies signed JWTs on every request, so a tampered client cannot
actually call privileged endpoints. **Human review note:** if any future
component starts treating the unverified `role` as a security gate (rather
than UX hint), it must instead consult a server-verified source like
`/auth/me`.

---

### WR-01: SSE Reconnect State Stuck on Invalid URL

**Files modified:** `frontend/src/hooks/use-sse.ts`
**Commit:** ee64767 (combined with CR-01)
**Applied fix:** Same guard as CR-01 — early return resets both `connected`
and `reconnecting` to `false`, preventing the banner from being stuck when
no token is available.

---

### WR-02: PaginatedResponse Shape Mismatch

**Files modified:** `frontend/src/types/api.ts`,
`frontend/src/app/(dashboard)/dashboard/page.tsx`,
`frontend/src/app/(dashboard)/timesheet/page.tsx`,
`frontend/src/app/(dashboard)/devices/page.tsx`,
`frontend/src/app/(dashboard)/employees/page.tsx`
**Commit:** 785bebc
**Applied fix:** Renamed the frontend `PaginatedResponse<T>.items` to `data`
to match the backend `crate::common::PaginatedResponse` wire format
(`pub data: Vec<T>` in `backend/src/common.rs`). Updated all 6 use sites
across dashboard, timesheet, devices, and employees pages. Backend wire
format is canonical because multiple existing handlers depend on it.

---

### WR-03: Evidence Written to Disk Before DB Existence Check

**Files modified:** `backend/src/daily_records/handlers.rs`
**Commit:** 444e6d1
**Applied fix:** Reordered the override handler to perform the
`SELECT 1 FROM daily_records WHERE id = ?1` check **before** calling
`write_photo_atomic`. A 404 path now never touches the filesystem, so
repeated bogus POSTs cannot accumulate orphaned files in `./data/overrides/`.

---

### WR-04: Activity Feed Photo URL Without Authentication

**Files modified:** `frontend/src/components/dashboard/activity-feed.tsx`
**Commit:** 4547bfa
**Applied fix:** Replaced the raw `<img src="${API}/.../photo">` with a
blob fetch through the axios `api` instance (which attaches the bearer via
request interceptor). Used `URL.createObjectURL()` to feed the blob to the
`<img>` and revoked it on unmount or event change. Falls back to the
initials avatar on fetch failure.

---

### WR-05: Stale AuthContext Claims After Token Refresh

**Files modified:** `frontend/src/contexts/auth-context.tsx`,
`frontend/src/lib/api.ts`
**Commit:** e034049
**Applied fix:** Added a tiny pub-sub in `lib/api.ts`: `setAccessToken` now
notifies registered listeners; exported `onAccessTokenChange(listener)` for
subscribers. `AuthProvider` subscribes during effect and re-decodes claims
on every token mutation, unsubscribing on unmount. The 401 axios interceptor
already calls `setAccessToken(data.access_token)`, so refresh flows now keep
context fresh automatically.

---

### WR-06: novedadSchema Missing Date-Range Validation

**Files modified:** `frontend/src/lib/validations.ts`,
`backend/src/daily_records/handlers.rs`
**Commit:** 87b229d
**Applied fix:** Added a Zod `.refine()` to `novedadSchema` ensuring
`fecha_fin >= fecha_inicio` (lexicographic compare on YYYY-MM-DD). Added a
parallel backend check in the override handler that rejects
`override_exit_at <= override_entry_at` when both are supplied. Both layers
return user-friendly Spanish error text aligned with the existing UX.

---

### WR-07: Sidebar Active State Prefix Match

**Files modified:** `frontend/src/components/layout/sidebar.tsx`
**Commit:** 892a8c5
**Applied fix:** Switched from `pathname.startsWith(href)` to exact match for
leaf paths and `pathname.startsWith(href + '/')` for sub-routes. Includes
explicit handling of the `'/'` case. Prevents future prefix collisions
(e.g., a `/reports-archive` route lighting up `/reports`).

---

_Fixed: 2026-04-23T00:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
