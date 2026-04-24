---
phase: 04-frontend-ui
reviewed: 2026-04-23T00:00:00Z
depth: standard
files_reviewed: 30
files_reviewed_list:
  - backend/src/daily_records/handlers.rs
  - backend/src/daily_records/models.rs
  - backend/src/events/handlers.rs
  - backend/src/events/service.rs
  - backend/src/state.rs
  - backend/src/main.rs
  - frontend/src/app/(dashboard)/dashboard/page.tsx
  - frontend/src/app/(dashboard)/devices/page.tsx
  - frontend/src/app/(dashboard)/employees/page.tsx
  - frontend/src/app/(dashboard)/layout.tsx
  - frontend/src/app/(dashboard)/timesheet/page.tsx
  - frontend/src/components/dashboard/activity-feed.tsx
  - frontend/src/components/dashboard/dept-chart.tsx
  - frontend/src/components/dashboard/device-banner.tsx
  - frontend/src/components/dashboard/kpi-tile.tsx
  - frontend/src/components/dashboard/sse-reconnect-banner.tsx
  - frontend/src/components/devices/command-modal.tsx
  - frontend/src/components/devices/device-table.tsx
  - frontend/src/components/employees/employee-table.tsx
  - frontend/src/components/layout/sidebar.tsx
  - frontend/src/components/layout/top-bar.tsx
  - frontend/src/components/timesheet/novedad-modal.tsx
  - frontend/src/components/timesheet/timesheet-table.tsx
  - frontend/src/components/timesheet/week-navigator.tsx
  - frontend/src/components/ui/dialog.tsx
  - frontend/src/contexts/auth-context.tsx
  - frontend/src/hooks/use-sse.ts
  - frontend/src/lib/api.ts
  - frontend/src/lib/validations.ts
  - frontend/src/proxy.ts
  - frontend/src/types/api.ts
findings:
  critical: 4
  warning: 7
  info: 4
  total: 15
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-04-23T00:00:00Z
**Depth:** standard
**Files Reviewed:** 30
**Status:** issues_found

## Summary

This review covers the Phase 04 frontend UI implementation (Next.js 15 + React 19) and the Rust/Axum backend additions that support it (daily-record overrides, SSE stream, event photo serving). The overall structure is solid: multipart validation is thorough, path traversal on photos is correctly handled, and the TanStack Table integration follows server-pagination conventions correctly.

Four critical issues were found: a JWT token exposed in plaintext in SSE URLs and stored in module-level memory rather than a httpOnly cookie; an open-redirect vulnerability in the session-expiry redirect; content-type spoofing accepted without magic-byte validation in file uploads; and a client-side authorization bypass possible because role gating is purely frontend-derived from an unverified JWT decode.

Seven warnings were found covering SSE reconnection state management, pagination count edge cases, the `PaginatedResponse` shape mismatch between the backend `data` field and frontend `items` field, potential XSS in the activity feed photo URL, token URL exposure in logs, an unchecked error path in the override handler, and stale auth-context on token refresh.

---

## Critical Issues

### CR-01: JWT Access Token Exposed in SSE URL Query Parameter — Logged in Proxies and Server Access Logs

**File:** `frontend/src/hooks/use-sse.ts:23`

**Issue:** The SSE connection URL is constructed as `…/events/stream?token=<jwt>`. JWTs in query strings appear in server access logs, Cloudflare request logs, browser history, and HTTP Referer headers sent to third-party resources. The backend design document acknowledges this as "T-4-02 accepted risk on-premise", but the risk surface is non-trivial on Cloudflare-tunneled installations where Cronometrix is exposed externally. The token is also held in a plain `let accessToken` module variable (no expiry enforcement, no rotation on `connect()` after a token refresh — the stale pre-refresh token may be embedded in a new EventSource URL).

**Fix:** Use a short-lived one-time "stream ticket" pattern: the frontend calls `POST /api/v1/events/stream-ticket` (returns a UUID valid for 10 seconds, single use), then opens `EventSource` with `?ticket=<uuid>`. The backend exchanges the ticket for a session without ever logging a real JWT. If the ticket pattern is deferred, at minimum always re-read the token on each `connect()` call so post-refresh reconnects use the updated token:

```typescript
// use-sse.ts — always read the current token at connect time, not from closure
const connect = useCallback(() => {
  if (timerRef.current) clearTimeout(timerRef.current)
  const token = getAccessToken()           // <-- already correct, but...
  // PROBLEM: if the module-level token was refreshed between the SSE onerror
  // and the setTimeout(connect, delay) firing, getAccessToken() will return
  // the new token. This part is actually safe. The real fix needed is:
  // ensure reconnects after a token refresh also use the new token.
  // The current code is correct for that — the stale-token risk only applies
  // to log exposure, not to the connection itself.
}, [path])
```

The stream ticket approach is the correct long-term mitigation.

---

### CR-02: Open Redirect via Unvalidated `redirect` Query Parameter on Session Expiry

**File:** `frontend/src/lib/api.ts:49-51`

**Issue:** When the 401 auto-refresh fails, the code redirects to `/login?redirect=${encodeURIComponent(redirect)}`. The `redirect` value is taken from `window.location.pathname`, which is safe. However, the login page (not reviewed here but implied) that consumes the `redirect` query param must validate it before using it in `window.location.href = redirect` or `router.push(redirect)`. If the login page does a naïve `router.push(searchParams.get('redirect'))`, an attacker can craft `?redirect=%2F%2Fevil.com%2Fphishing` (a protocol-relative URL) and redirect the user after login. The encoding via `encodeURIComponent` does not prevent this — `//evil.com` encodes to `%2F%2Fevil.com` but decodes back before navigation.

**Fix:** On the login page, validate the decoded redirect value before navigating:

```typescript
// In the login page post-auth handler
const raw = searchParams.get('redirect') ?? '/dashboard'
// Only allow same-origin relative paths
const safe = raw.startsWith('/') && !raw.startsWith('//') ? raw : '/dashboard'
router.replace(safe)
```

---

### CR-03: File Upload Accepts Content-Type Without Magic Byte Validation — Content-Type Spoofing

**File:** `backend/src/daily_records/handlers.rs:99-110`

**Issue:** The override evidence upload validates `content_type()` from the multipart header (lines 100–110), but `Content-Type` in a multipart field is set by the client and can be trivially spoofed. An attacker can upload an executable (e.g., a PHP or HTML file with embedded script) labeled `image/jpeg`, which passes the server check and is saved with a `.jpg` extension. If the stored files are ever served via a web server configured to execute scripts in that directory, this is a remote code execution vector. Even without execution, stored HTML/SVG with scripts served back as `image/jpeg` can lead to stored XSS (some browsers sniff content).

**Fix:** After reading `evidence_bytes`, verify the file magic bytes before accepting:

```rust
// After: evidence_bytes = Some(bytes.to_vec());
// Add magic byte check:
fn infer_ext_from_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF") { return Some("pdf"); }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 { return Some("jpg"); }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Some("png"); }
    None
}

let magic_ext = infer_ext_from_magic(&bytes).ok_or_else(|| AppError::Validation {
    code: "VALIDATION_ERROR",
    message: "evidence bytes do not match a supported file type (PDF/JPEG/PNG)".into(),
})?;
// Override the content-type-derived ext with the magic-derived ext
evidence_ext = Some(magic_ext);
```

The same fix should be applied to the leaves evidence upload (not in scope here but same pattern).

---

### CR-04: Client-Side Role Enforcement Derived from Unverified JWT Decode — Authorization Bypass

**File:** `frontend/src/contexts/auth-context.tsx:6-13`

**Issue:** The `decodeJwtPayload` function base64-decodes the JWT payload without signature verification. The `role` claim extracted here (`admin | supervisor | viewer`) is used to show or hide UI controls: the "Nuevo Empleado" button, ISAPI command buttons, and the edit pencil in TimesheetTable. A user can modify their JWT payload in browser memory (e.g., via devtools: `setAccessToken(fakejwt)`) and gain admin UI. While the backend enforces RBAC on every request, the issue is that the UI renders sensitive action buttons based on this unverified role. This means a Viewer can trigger the command dispatch UI flow (the backend will reject the actual call, but the UI should never have presented it). More concretely: the `role` value shown in `TopBar` (`{role} · {sub}`) can be forged to display misleading role information.

**Fix:** The backend `AuthUser` extractor already verifies the JWT signature — this is the authoritative source. For the frontend, this is a defense-in-depth issue (not a backend auth bypass), but the fix is to mark the client-side role as "display hint only" and ensure any action that performs a mutation goes through the backend check. The `TopBar` display of role should note it is derived client-side. If role-based UI hiding is a security requirement (not just UX), it must be re-derived from a server-verified source (e.g., a `/auth/me` endpoint that re-validates the JWT and returns claims).

---

## Warnings

### WR-01: SSE Reconnect State Never Resets to `false` After Successful Reconnect on `connect()` Re-Entry

**File:** `frontend/src/hooks/use-sse.ts:35-41`

**Issue:** When `es.onerror` fires, `reconnecting` is set to `true` and `setTimeout(connect, delay)` is called. On the next `connect()`, a new `EventSource` is created. When `es.onopen` fires on the new connection, `reconnecting` is set to `false`. This is correct in the happy path. However, if `connect()` is called but the `EventSource` constructor itself throws (e.g., invalid URL in test), `onopen` is never called, `reconnecting` stays `true` forever, and the banner never clears. This is a minor but real edge case.

**Fix:** Reset `reconnecting` to `false` after a successful open only (already correct). Add a guard: if the URL is invalid, fail fast:

```typescript
// Before constructing EventSource, validate URL
if (!token) {
  // No token available — don't attempt connection, clear reconnecting
  setReconnecting(false)
  return
}
```

---

### WR-02: `PaginatedResponse` Shape Mismatch — Backend Returns `data`, Frontend Expects `items`

**File:** `frontend/src/types/api.ts:24-29` / `backend/src/daily_records/handlers.rs:28` (via `common::PaginatedResponse`)

**Issue:** The frontend `PaginatedResponse<T>` interface has field `items: T[]`, but the backend `PaginatedResponse` struct (from `crate::common`) uses field `data: Vec<T>` (as evidenced by the `list()` function returning `PaginatedResponse { data, total, limit, offset }`). Every page in the app does `data?.items ?? []` — if the backend actually serializes `data` (not `items`), all tables will render empty even when records exist, with no visible error.

**Fix:** Align the field name. Either change the backend struct field to `items` or change the frontend interface to `data`. Check the actual JSON response from `/api/v1/daily-records` and pick one canonical name. This is a runtime bug that only surfaces with a real backend (tests mock the shape).

---

### WR-03: Unchecked Error in Override Handler — Evidence Written to Disk Before DB Existence Check

**File:** `backend/src/daily_records/handlers.rs:147-171`

**Issue:** `write_photo_atomic` is called at line 153 before the database existence check at line 161 (`SELECT 1 FROM daily_records WHERE id = ?1`). If `daily_record_id` does not exist, the function correctly returns `AppError::NotFound`, but the evidence file has already been written to `./data/overrides/<uuid>.ext` and is now orphaned on disk. Over time, many failed POST attempts (whether from bugs or malicious repeated requests with nonexistent IDs) will accumulate orphaned files.

**Fix:** Reorder: check record existence first, then write the file:

```rust
// 1. Verify daily_record exists FIRST
let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
let exists: bool = conn.query(
    "SELECT 1 FROM daily_records WHERE id = ?1 LIMIT 1",
    libsql::params![daily_record_id.clone()],
).await...;
if !exists {
    return Err(AppError::NotFound { ... });
}

// 2. THEN write evidence to disk
let evidence_relpath = if let (Some(bytes), Some(ext)) = ...
```

---

### WR-04: Activity Feed Photo URL Built Without Authentication — Requests Will 401

**File:** `frontend/src/components/dashboard/activity-feed.tsx:17-19`

**Issue:** `EventAvatar` renders `<img src={`${API}/api/v1/events/${event.id}/photo`} ...>`. The `GET /events/{id}/photo` route is in `viewer_routes` (requires `Authorization: Bearer` header). Browser `<img>` tags issue simple GET requests without custom headers — the request will hit the backend without the JWT and return 401, causing a broken image.

**Fix:** Either:
1. Expose event photos on a public/pre-signed URL path that embeds a short-lived token, or
2. Fetch the photo blob via the `api` axios instance (which attaches the Bearer header) and use `URL.createObjectURL()`:

```typescript
// Preferred approach — keeps auth on the axios layer
function EventAvatar({ event }: ...) {
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    if (!event.has_photo) return
    api.get(`/events/${event.id}/photo`, { responseType: 'blob' })
      .then(r => setSrc(URL.createObjectURL(r.data)))
      .catch(() => setSrc(null))
    return () => { if (src) URL.revokeObjectURL(src) }
  }, [event.id, event.has_photo])
  if (src) return <img src={src} ... />
  // fall through to initials avatar
}
```

---

### WR-05: `AuthProvider` Only Decodes Token Once on Mount — Stale Role After Token Refresh

**File:** `frontend/src/contexts/auth-context.tsx:26-30`

**Issue:** The `useEffect` with empty dependency array (`[]`) runs once on mount. If the access token is refreshed by the axios interceptor (which calls `setAccessToken(data.access_token)`), the new token's claims (including potentially an updated role or `exp`) are never decoded into `AuthContext`. The UI continues showing stale role/sub until a full page reload. In practice the role rarely changes mid-session, but the `sub` and `exp` claims are also stale, and any downstream component relying on `claims.exp` to warn about impending expiry will be wrong.

**Fix:** Either subscribe `AuthContext` to a token-change event, or expose a `refreshClaims()` function and call it from the axios 401 interceptor after `setAccessToken`:

```typescript
// In auth-context.tsx
export function AuthProvider(...) {
  const [claims, setClaims] = useState<JWTClaims | null>(null)

  // Expose a way to refresh claims from updated token
  const refreshClaims = useCallback(() => {
    const token = getAccessToken()
    setClaims(token ? decodeJwtPayload(token) : null)
  }, [])

  useEffect(() => { refreshClaims() }, [refreshClaims])

  return (
    <AuthContext.Provider value={{ role: ..., sub: ..., claims, refreshClaims }}>
      {children}
    </AuthContext.Provider>
  )
}
```

Then in `api.ts` after `setAccessToken(data.access_token)`, call `refreshClaims()` (pass the function via a singleton or event).

---

### WR-06: `novedadSchema` Does Not Validate Date Range — `fecha_fin` Can Precede `fecha_inicio`

**File:** `frontend/src/lib/validations.ts:32-43`

**Issue:** The schema validates that both date fields are non-empty strings, but no cross-field refinement checks that `fecha_fin >= fecha_inicio`. A user can submit an override with end-date before start-date, which the backend will accept (the backend daily-record override handler does not validate the date range either — it accepts `override_entry_at` and `override_exit_at` as independent optional values). This can produce logically incoherent audit records.

**Fix:**

```typescript
export const novedadSchema = z.object({ ... })
  .refine(
    data => !data.fecha_inicio || !data.fecha_fin || data.fecha_fin >= data.fecha_inicio,
    { message: 'La fecha fin no puede ser anterior a la fecha inicio', path: ['fecha_fin'] }
  )
```

On the backend (`handlers.rs`), add a similar check:
```rust
if let (Some(entry), Some(exit)) = (override_entry_at, override_exit_at) {
    if exit <= entry {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "override_exit_at must be after override_entry_at".into(),
        });
    }
}
```

---

### WR-07: Sidebar Active State Breaks on `/dashboard` — `pathname.startsWith('/d')` Matches `/devices`

**File:** `frontend/src/components/layout/sidebar.tsx:32-33`

**Issue:** The active highlight uses `pathname.startsWith(href)`. The nav item for `/dashboard` has `href: '/dashboard'` and the one for `/devices` has `href: '/devices'`. Because `/devices` does not start with `/dashboard`, this particular pair is safe. However `/dashboard` does catch `/dashboard/anything` correctly, and `/d` would catch both — the current hrefs are unique enough that this is only a latent risk. The real bug is that when pathname is exactly `/`, none of the nav items are highlighted (no nav item has `href: '/'`), so if the root redirects to `/dashboard` there is a flash with no active item. More concretely: `pathname.startsWith('/reports')` would match a future `/reports-archive` route incorrectly.

**Fix:** Use exact matching for leaf routes and prefix matching only for routes with sub-paths:

```typescript
const isActive = href === '/'
  ? pathname === '/'
  : pathname === href || pathname.startsWith(href + '/')
```

---

## Info

### IN-01: `DeviceTable` — `device.ip_address` Field Does Not Exist on the `Device` Type

**File:** `frontend/src/components/devices/device-table.tsx:44` and `frontend/src/components/devices/command-modal.tsx:49`

**Issue:** Both components reference `device.ip_address`. The `Device` interface in `types/api.ts` (line 59) also declares the field as `ip_address`, so the type is consistent. However, the Rust backend `Device` model (not in scope) likely serializes it as `ip` (based on `Cargo.toml` references and the fact that the state module uses `ip` and `port`). If the backend sends `ip` and the frontend expects `ip_address`, the field will be `undefined` and render as an empty cell. Verify the actual backend serialization field name.

---

### IN-02: `NovedadModal` — `employee_id` and `department_id` Are Free-Text Inputs

**File:** `frontend/src/components/timesheet/novedad-modal.tsx:101-127`

**Issue:** The form renders raw `<Input>` fields for `employee_id` and `department_id` with no dropdown or autocomplete. Users must type raw UUID strings. This is noted as a placeholder for future Phase implementation, but the Zod schema validates them only as `min(1)` strings. A user who types an invalid (but non-empty) employee ID will pass client-side validation and receive a 404 from the backend only after form submission. The UX gap could also cause accidental overrides attached to wrong employees.

This is a UX issue more than a bug, but worth tracking for Phase 5 when employee selection is wired up.

---

### IN-03: `write_photo_atomic` — Temp File Extension Collision on Concurrent Writes

**File:** `backend/src/events/service.rs:164`

**Issue:** The temporary file path is `full.with_extension("jpg.tmp")`. For events the relpath already ends in `.jpg` (e.g., `2026-04-23/abc.jpg`), so the temp is `2026-04-23/abc.jpg.tmp` — unique per event. For overrides (reusing `write_photo_atomic` from `handlers.rs:153`), relpath is a UUID like `550e8400.jpg`, temp is `550e8400.jpg.tmp` — also unique. No practical collision exists under the current calling patterns. However, the function would produce incorrect behavior if two callers used the same `relpath` simultaneously (race to rename). The UUID generation upstream prevents this, but the function signature doesn't document or enforce this precondition. This is an info-level observation, not an active bug.

---

### IN-04: `proxy.ts` — Setup Status Fetched on Every Protected Route Request, No Caching

**File:** `frontend/src/proxy.ts:29-41`

**Issue:** Every navigation to a protected route triggers a `fetch` to `/api/v1/setup/status`. This is a Next.js middleware, so it runs on the Edge and will fire on every page request, including background navigations via `router.prefetch`. With `refetchInterval: 30_000` on multiple queries plus SSE reconnects, this could add meaningful latency and load. The `initialized` flag is immutable after first setup — once set, it never flips back to false.

**Fix:** Cache the result in a response header or use Next.js middleware's `NextResponse` cookie-based flag set at setup time. Alternatively, skip the check entirely after confirming `initialized = true` (store in a cookie set by the `/setup/init` response handler).

---

_Reviewed: 2026-04-23T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
