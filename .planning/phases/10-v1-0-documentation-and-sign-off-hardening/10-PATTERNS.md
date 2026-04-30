# Phase 10: v1.0 Documentation & Sign-off Hardening — Pattern Map

**Mapped:** 2026-04-29
**Files analyzed:** 12 new/modified files (1 new backend module function group, 1 test file extension, 1 frontend page edit, 1 new Bruno request, 2 new VERIFICATION docs, 3 doc-only edits)
**Analogs found:** 10 / 12 (2 pure-documentation files have no code analog)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `backend/src/audit/models.rs` (add `AuditActor` struct) | model | CRUD | `backend/src/audit/models.rs` (`AuditEntry` struct, lines 7-17) | exact — same file, same pattern |
| `backend/src/audit/service.rs` (add `list_actors()`) | service | CRUD | `backend/src/audit/service.rs` (`list_audit()`, lines 24-141) | exact — same file, peer function |
| `backend/src/audit/handlers.rs` (add `list_actors_handler()`) | controller | request-response | `backend/src/audit/handlers.rs` (`list_audit()`, lines 22-32) | exact — same file, peer function |
| `backend/src/audit/mod.rs` (re-export `list_actors`) | config | — | `backend/src/audit/mod.rs` (lines 1-3) | exact — same file |
| `backend/src/main.rs` (add route to `supervisor_read_routes`) | config | — | `backend/src/main.rs` (lines 234-245) | exact — same router block |
| `backend/tests/audit_handlers_test.rs` (add 3 new tests) | test | request-response | `backend/tests/audit_handlers_test.rs` (tests 1-3, lines 139-182) | exact — same test file, same pattern |
| `frontend/src/app/(dashboard)/audit/page.tsx` (add `useQuery`) | component | request-response | `frontend/src/app/(dashboard)/audit/page.tsx` (lines 52-70) | exact — same file, peer query |
| `frontend/src/components/audit/__tests__/audit-table.test.tsx` (add 1 test) | test | — | `audit-table.test.tsx` lines 243-254 (actors prop test) | exact — same file, peer test |
| `bruno/cronometrix/audit/01_list.bru` (new file — existing endpoint) | config | request-response | `bruno/cronometrix/employees/01_list.bru` | exact naming+format match |
| `bruno/cronometrix/audit/02_list_actors.bru` (new file — new endpoint) | config | request-response | `bruno/cronometrix/auth/01_login.bru` (tests block pattern) | role-match |
| `.planning/phases/01-foundation/01-VERIFICATION.md` (new doc) | — (documentation) | — | `.planning/phases/09-e2e-playwright-test-suite-.../09-VERIFICATION.md` | format target (depth) |
| `.planning/phases/07-facial-enrollment-sync/07-VERIFICATION.md` (new doc) | — (documentation) | — | `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` | format target (human_needed + deferred) |
| `.planning/REQUIREMENTS.md` (traceability edits + new sections) | — (documentation) | — | no code analog | direct edit per RESEARCH §Area 5 |
| `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` (deferred-items row edit) | — (documentation) | — | same file lines 56-60 | direct edit |

---

## Pattern Assignments

### `backend/src/audit/models.rs` — add `AuditActor` struct

**Analog:** `backend/src/audit/models.rs` lines 7-17 (`AuditEntry`)

**Model struct pattern** (lines 7-17):
```rust
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub id: String,
    pub table_name: String,
    pub record_id: String,
    pub operation: String,
    pub old_data: Option<serde_json::Value>,
    pub new_data: Option<serde_json::Value>,
    pub actor_id: Option<String>,
    pub created_at: i64,
}
```

**New struct to add** (mirror pattern; all fields `Option` because LEFT JOIN can miss):
```rust
#[derive(Debug, Clone, Serialize)]
pub struct AuditActor {
    pub actor_id: Option<String>,   // NULL when audit_log.actor_id IS NULL
    pub username: Option<String>,   // NULL when user was deleted (LEFT JOIN miss)
    pub role: Option<String>,       // NULL same
}
```

**Imports already present** (line 1): `use serde::{Deserialize, Serialize};` — no new imports needed.

---

### `backend/src/audit/service.rs` — add `list_actors()`

**Analog:** `backend/src/audit/service.rs` lines 1-6 (imports) + lines 109-133 (row iteration loop)

**Imports pattern** (lines 1-6):
```rust
use libsql::Connection;

use crate::common::PaginatedResponse;
use crate::errors::AppError;

use super::models::{AuditEntry, AuditListQuery};
```

**New imports needed** — add `AuditActor` to the models import:
```rust
use super::models::{AuditActor, AuditEntry, AuditListQuery};
```

**Row iteration / error handling pattern** (lines 109-133 of service.rs):
```rust
let mut rows = conn
    .query(&fetch_sql, libsql::params_from_iter(fetch_values))
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

let mut data: Vec<AuditEntry> = Vec::new();
while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
    data.push(AuditEntry {
        id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        ...
    });
}
```

**New `list_actors` function** (copy structure, simplify query — no params, no pagination):
```rust
pub async fn list_actors(conn: &Connection) -> Result<Vec<AuditActor>, AppError> {
    let sql = "SELECT DISTINCT al.actor_id, u.username, u.role \
               FROM audit_log al LEFT JOIN users u ON al.actor_id = u.id";
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut data: Vec<AuditActor> = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        data.push(AuditActor {
            actor_id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
            username: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
            role:     row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        });
    }
    Ok(data)
}
```

**Key difference from `list_audit`:** no `PaginatedResponse` wrapper, no dynamic WHERE, no `LIMIT/OFFSET`, no `COUNT(*)`. Query is a fixed string with `()` (no params).

---

### `backend/src/audit/handlers.rs` — add `list_actors` handler

**Analog:** `backend/src/audit/handlers.rs` lines 1-32 (entire file — exact pattern)

**Imports pattern** (lines 6-16):
```rust
use axum::{
    extract::{Query, State},
    Json,
};

use crate::common::PaginatedResponse;
use crate::errors::AppError;
use crate::state::AppState;

use super::models::{AuditEntry, AuditListQuery};
use super::service;
```

**New handler** (copy `list_audit` pattern; drop `Query` extractor since no query params):
```rust
pub async fn list_actors(
    State(state): State<AppState>,
) -> Result<Json<Vec<AuditActor>>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list_actors(&conn).await?;
    Ok(Json(result))
}
```

**New import to add** — `AuditActor` to the models import line 15:
```rust
use super::models::{AuditActor, AuditEntry, AuditListQuery};
```

Note: `Query` extractor is NOT needed (no query params). The `Json` return wraps `Vec<AuditActor>` directly, not `PaginatedResponse`.

---

### `backend/src/audit/mod.rs` — no change needed

**Analog:** `backend/src/audit/mod.rs` lines 1-3:
```rust
pub mod handlers;
pub mod models;
pub mod service;
```

The three `pub mod` declarations already re-export all public items from each submodule. Adding `pub struct AuditActor` to `models.rs` and `pub async fn list_actors` to `handlers.rs` and `service.rs` makes them automatically available via `audit::handlers::list_actors` and `audit::models::AuditActor`. No change to `mod.rs` is needed.

---

### `backend/src/main.rs` — add route to `supervisor_read_routes`

**Analog:** `backend/src/main.rs` lines 234-245 (supervisor_read_routes block)

**Current block** (lines 234-245):
```rust
let supervisor_read_routes = Router::new()
    .route("/anomalies", get(anomalies::handlers::list_anomalies))
    .route("/audit", get(audit::handlers::list_audit))   // NEW Plan 09-04
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_supervisor_or_above,
    ))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        license::middleware::require_license,
    ));
```

**Minimal diff** — add one `.route(...)` line after the existing `/audit` route:
```rust
.route("/audit/actors", get(audit::handlers::list_actors))
```

The RBAC middleware (`require_supervisor_or_above`) and license gate already apply to the entire router group — no additional middleware lines needed.

---

### `backend/tests/audit_handlers_test.rs` — add 3 new tests

**Analog:** `backend/tests/audit_handlers_test.rs` lines 59-182 (test app builder + tests 1-2)

**Test app builder pattern** (lines 59-67):
```rust
fn build_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route("/audit", get(audit::handlers::list_audit))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));
    Router::new().nest("/api/v1", routes).with_state(state)
}
```

**For new actors tests** — build a separate test app (or extend the existing `build_test_app` to also register `/audit/actors`):
```rust
fn build_actors_test_app(state: AppState) -> Router {
    let routes = Router::new()
        .route("/audit/actors", get(audit::handlers::list_actors))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));
    Router::new().nest("/api/v1", routes).with_state(state)
}
```

**RBAC-denial test pattern** (lines 139-158 — Test 1):
```rust
#[tokio::test]
async fn audit_403_when_viewer() {
    let db = common::test_db().await;
    let (state, _tmp) = make_state(db);
    let app = build_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", viewer_token()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, ...);
}
```

**Three new test functions to add:**

1. `audit_actors_returns_200_for_admin` — happy path: seed 1 user row + 1 audit_log row with actor_id, assert 200 + array with 1 element having `actor_id`/`username`/`role`.
2. `audit_actors_viewer_returns_403` — RBAC: same pattern as `audit_403_when_viewer` above, URI `/api/v1/audit/actors`.
3. `audit_actors_returns_empty_when_no_log` — empty table: no seeded rows, assert 200 + `[]`.

**Seeding pattern for user + audit_log** (extend `seed_audit_rows` helper at lines 86-130, and add a `seed_user` helper following the same `conn.execute(format!(...))` pattern).

---

### `frontend/src/app/(dashboard)/audit/page.tsx` — add `useQuery(['audit-actors'])`

**Analog:** `frontend/src/app/(dashboard)/audit/page.tsx` lines 52-70 (existing `useQuery` for audit data) + lines 79-88 (existing `useMemo` to replace)

**Existing `useQuery` pattern** (lines 52-70):
```tsx
const { data, isLoading } = useQuery<PaginatedResponse<AuditEntry>>({
  queryKey: ['audit', pagination.pageIndex, filters],
  queryFn: () =>
    api
      .get('/audit', { params: { ... } })
      .then(r => r.data),
  enabled: role === 'admin' || role === 'supervisor',
})
```

**New query to add** (after lines 70, before the `useMemo`):
```tsx
const { data: actorsData } = useQuery<
  Array<{ actor_id: string | null; username: string | null; role: string | null }>
>({
  queryKey: ['audit-actors'],
  queryFn: () => api.get('/audit/actors').then(r => r.data),
  staleTime: 5 * 60 * 1000,
  enabled: role === 'admin' || role === 'supervisor',
})
```

**Existing `useMemo` to replace** (lines 79-88):
```tsx
const actors = useMemo(() => {
  if (!data?.data) return []
  const seen = new Map<string, string>()
  for (const entry of data.data) {
    if (entry.actor_id && !seen.has(entry.actor_id)) {
      seen.set(entry.actor_id, entry.actor_id)
    }
  }
  return Array.from(seen.entries()).map(([id, username]) => ({ id, username }))
}, [data?.data])
```

**Replacement `useMemo`** (derive from `actorsData` instead of `data?.data`):
```tsx
const actors = useMemo(() => {
  if (!actorsData) return []
  return actorsData
    .filter(a => a.actor_id != null)
    .map(a => ({
      id: a.actor_id!,
      username: a.username ? `${a.username} (${a.role})` : a.actor_id!,
    }))
}, [actorsData])
```

**`AuditFilters` props contract** (unchanged — `actors-filters.tsx` line 15): `actors: Array<{ id: string; username: string }>` — the new useMemo output satisfies this contract. No changes to `AuditFilters`.

**E2E safety:** `audit.spec.ts` T-03 uses `selectOption('e2e-admin-id')` which matches by `<option value={a.id}>` (the actor_id). The value attribute is unchanged; only the display text changes to `{username} (admin)`. No Playwright spec changes needed.

---

### `frontend/src/components/audit/__tests__/audit-table.test.tsx` — add 1 new test

**Analog:** `audit-table.test.tsx` lines 243-254 (existing actors-prop rendering test):
```tsx
it('renders actor options from the actors prop', () => {
  render(
    <AuditFilters
      value={{}}
      onChange={() => {}}
      actors={actors}
      tables={tables}
    />
  )
  expect(screen.getByText('admin')).toBeTruthy()
  expect(screen.getByText('supervisor')).toBeTruthy()
})
```

The `actors` fixture used by this block (from context around line 200):
```tsx
const actors = [
  { id: 'user-1', username: 'admin' },
  { id: 'user-2', username: 'supervisor' },
]
```

**New test to add** (extend `describe('AuditFilters', ...)` block — test the new display format):
```tsx
it('renders actors with username (role) display format', () => {
  const enrichedActors = [
    { id: 'user-1', username: 'admin (admin)' },
    { id: 'user-2', username: 'jsmith (supervisor)' },
  ]
  render(
    <AuditFilters
      value={{}}
      onChange={() => {}}
      actors={enrichedActors}
      tables={[]}
    />
  )
  expect(screen.getByText('admin (admin)')).toBeTruthy()
  expect(screen.getByText('jsmith (supervisor)')).toBeTruthy()
  const select = document.querySelector(
    '[data-testid="audit-filter-actor"]'
  ) as HTMLSelectElement
  // value remains actor_id — not the display text
  const opt = Array.from(select.options).find(o => o.value === 'user-1')
  expect(opt?.text).toBe('admin (admin)')
})
```

---

### `bruno/cronometrix/audit/01_list.bru` (new — existing `/audit` endpoint)

**Analog:** `bruno/cronometrix/employees/01_list.bru` (entire file)

**Pattern to copy:**
```bru
meta {
  name: list audit
  type: http
  seq: 1
}

get {
  url: {{baseUrl}}/api/v1/audit?limit=20&offset=0
  body: none
  auth: bearer
}

params:query {
  limit: 20
  offset: 0
}

auth:bearer {
  token: {{access_token}}
}
```

Note: the `employees` analog has a `script:post-response` block to capture a sample ID. The audit collection does not need this — omit the script block.

---

### `bruno/cronometrix/audit/02_list_actors.bru` (new — new `/audit/actors` endpoint)

**Analog:** `bruno/cronometrix/auth/01_login.bru` lines 27-33 (tests block format)

**Bruno file format** (combine employees `01_list.bru` structure + login `tests {}` block):
```bru
meta {
  name: list actors
  type: http
  seq: 2
}

get {
  url: {{baseUrl}}/api/v1/audit/actors
  body: none
  auth: bearer
}

auth:bearer {
  token: {{access_token}}
}

tests {
  test("list actors succeeds", function() {
    expect(res.status).to.equal(200);
    expect(res.body).to.be.an("array");
  });
}
```

**Naming rationale:** The `audit/` directory does not exist yet (confirmed: `ls bruno/cronometrix/` has no `audit/` folder). Both files must be created. Naming follows `{NN}_{action}.bru` convention observed in `employees/` (01_list, 02_create) and `reports/` (01_json, 02_excel).

---

### `.planning/phases/01-foundation/01-VERIFICATION.md` (new document)

**Analog:** `.planning/phases/09-e2e-playwright-test-suite-.../09-VERIFICATION.md` (full document — depth target)

**Frontmatter contract** (lines 1-21 of 09-VERIFICATION.md):
```yaml
---
phase: 01-foundation
verified: {ISO-8601 timestamp of doc creation}
status: passed
score: 19/19 must-haves verified
overrides_applied: 0
human_verification: []
deferred: []
---
```

`status: passed` (not `human_needed`) because all Phase 1 code is verifiable in-codebase without live hardware. If any evidence gap is found (per D-03), move it to a Phase 11 follow-up item in the `human_verification` list rather than failing the document.

**Body structure** (canonical section order from 09-VERIFICATION.md):
1. `## Goal Achievement` → `### Observable Truths` → table (19 rows: DATA-01..04, AUTH-01..05, EMP-01..04, DEPT-01..03, RULE-01..03)
2. `### Required Artifacts` → table of key files per REQ
3. `### Key Link Verification` → wiring table (router groups in main.rs → middleware → handler)
4. `### Behavioral Spot-Checks` → commands + results
5. `### Requirements Coverage` → REQ → status table
6. `### Gaps Summary`

**Evidence locations** (from RESEARCH §Area 3 — verified):

| REQ | Primary Evidence Path |
|-----|-----------------------|
| DATA-01 | `backend/src/db/mod.rs` + `Cargo.toml` libsql dependency |
| DATA-02 | `backend/src/main.rs` `Builder::new_remote_replica()` + `config.rs` |
| DATA-03 | `Builder::new_remote_replica()` embedded replica pattern; local write primary in all service files |
| DATA-04 | `backend/src/db/migrations/002_audit_triggers.sql` |
| AUTH-01 | `backend/src/auth/handlers.rs:35` + `backend/tests/auth_tests.rs` |
| AUTH-02 | `backend/src/auth/rbac.rs` + admin_routes in `main.rs` |
| AUTH-03 | `supervisor_read_routes` + `supervisor_routes` in `main.rs` |
| AUTH-04 | `viewer_routes` in `main.rs`; Viewer 403 on mutating endpoints |
| AUTH-05 | `backend/src/auth/handlers.rs:121` refresh token flow |
| EMP-01 | `backend/src/employees/handlers.rs` POST handler |
| EMP-02 | `backend/src/employees/service.rs` dynamic WHERE clause |
| EMP-03 | `backend/src/employees/handlers.rs` DELETE handler |
| EMP-04 | `001_initial_schema.sql` `department_id TEXT NOT NULL REFERENCES departments(id)` |
| DEPT-01 | `backend/src/departments/handlers.rs` POST handler |
| DEPT-02 | `lunch_mode TEXT CHECK(...)` in 001 schema |
| DEPT-03 | `backend/src/departments/handlers.rs` PATCH handler |
| RULE-01 | `backend/src/rules/handlers.rs` + frontend sliders |
| RULE-02 | `global_rules.bonus_minutes` in schema + rules handler |
| RULE-03 | `backend/src/rules/service.rs` `effective_from` update |

---

### `.planning/phases/07-facial-enrollment-sync/07-VERIFICATION.md` (new document)

**Analog:** `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` (human_needed status + deferred block format)

**Frontmatter contract** (mirror 06-VERIFICATION.md lines 1-30, adapted for Phase 7):
```yaml
---
phase: 07-facial-enrollment-sync
verified: {ISO-8601 timestamp}
status: human_needed
score: 5/5 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Live Hikvision device smoke for ENRL-01 (device-camera capture)"
    expected: "Admin triggers capture from a real DS-K1T341; backend receives JPEG via capture_face_image, stores to enrollments_root, pusher fans out to all registered devices."
    why_human: "Requires real Hikvision hardware; mock_hikvision covers the code path but not the physical camera trigger."
deferred: []
---
```

**Body structure** (5 truths: ENRL-01..05):
- Follow 02-VERIFICATION.md style (full evidence per truth)
- ENRL-04 must cross-reference `enrollments/pusher.rs:173-187` → `isapi/client.rs:108,144` (per D-05)
- ENRL-01 truth status: VERIFIED for code path (face_capture_test.rs against mock); the live-hardware smoke goes in `human_verification`

**Evidence locations** (from RESEARCH §Area 4):

| REQ | Primary Evidence Path |
|-----|-----------------------|
| ENRL-01 | `backend/src/enrollments/handlers.rs` + `backend/src/isapi/client.rs:233` |
| ENRL-02 | `backend/src/enrollments/handlers.rs` POST (captured_via = "upload") |
| ENRL-03 | `frontend/src/components/enrollment/` webcam tab |
| ENRL-04 | `backend/src/enrollments/pusher.rs:173-187` → `isapi/client.rs:108,144` |
| ENRL-05 | `backend/src/enrollments/service.rs` enrollment_device_pushes table + GET polling |

---

### `.planning/REQUIREMENTS.md` — traceability refresh + new sections

**No code analog.** Apply exact diffs from RESEARCH §Area 5.

**Key changes:**
- Phase 2 (DEV-01..04, EVT-01..04): `Pending` → `Complete`
- Phase 4 (DASH-01..03, TS-01..05): `Pending` → `Complete`
- Phase 5 (PAY-01..04): `Pending` → `Complete`
- Phase 6 (LIC-01..05, DEPL-01, DEPL-02, DEPL-04): `Pending` → `Complete`
- Phase 6 (DEPL-03): `Pending` → `Partial — accepted v1 ship (D-13 in 06-CONTEXT.md); auto-register strict reading deferred to v1.1 backlog as DEPL-03-AUTO`
- Phase 7 (ENRL-01..05): `Pending` → `Complete` (both checkbox and traceability column)
- Add `## v1 Cross-Cutting Meta-Requirements (Phases 8+)` section after v1 table
- Add `## v1.1 Backlog` section with DEPL-03-AUTO row
- Update Coverage block: `Mapped to phases: 48` → expanded text (v1=48 + meta=22)

---

### `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` — deferred-items row edit

**No code analog.** Single cell edit in the deferred-items table (lines 56-60).

**Current `Addressed In` cell value** (line 29 of deferred block):
```
Phase 7 / future v2 release
```

**Replacement value** (per D-19):
```
v1.1 Backlog — DEPL-03-AUTO (see REQUIREMENTS.md §v1.1 Backlog)
```

The `Evidence` cell content (the long description of the token-based connector design) stays unchanged.

---

## Shared Patterns

### RBAC Middleware (applies to backend handler and all 3 new tests)

**Source:** `backend/src/main.rs` lines 238-241 + `backend/src/auth/rbac.rs`
**Apply to:** `main.rs` route registration, `build_actors_test_app()` in test file

```rust
.route_layer(axum::middleware::from_fn_with_state(
    state.clone(),
    auth::rbac::require_supervisor_or_above,
))
```

The `require_supervisor_or_above` extractor produces: Admin → pass, Supervisor → pass, Viewer → 403, Anonymous → 401. No new RBAC logic needed.

### Error Handling (applies to new service and handler functions)

**Source:** `backend/src/audit/service.rs` lines 87-95 (COUNT query error mapping) + `backend/src/audit/handlers.rs` lines 27-29

```rust
.map_err(|e| AppError::Internal(e.into()))?
```

Every `libsql` call wraps errors with `AppError::Internal(e.into())`. The `Result<..., AppError>` return type bubbles errors to Axum's `IntoResponse` impl on `AppError`.

### TanStack Query fetch pattern (applies to new `useQuery` in page.tsx)

**Source:** `frontend/src/app/(dashboard)/audit/page.tsx` lines 52-70

```tsx
const { data, isLoading } = useQuery<T>({
  queryKey: [...],
  queryFn: () => api.get('/path').then(r => r.data),
  enabled: role === 'admin' || role === 'supervisor',
})
```

New actors query adds `staleTime: 5 * 60 * 1000` (actors change rarely; cache for 5 min). No `isLoading` destructure needed — actors dropdown renders empty until populated (non-blocking).

### Verification Document Frontmatter (applies to both new VERIFICATION docs)

**Source:** `.planning/phases/09-e2e-playwright-test-suite-.../09-VERIFICATION.md` lines 1-21

Required fields: `phase`, `verified` (ISO-8601), `status` (`passed` | `human_needed`), `score` (N/N), `overrides_applied`, `human_verification` (list), `deferred` (list). The verifier MUST populate all fields before closing the document.

### Bruno file format (applies to both new `.bru` files)

**Source:** `bruno/cronometrix/employees/01_list.bru` (GET without body) + `bruno/cronometrix/auth/01_login.bru` (tests block)

Mandatory blocks for a GET+auth endpoint: `meta {}`, `get {}`, `auth:bearer {}`. Optional: `params:query {}` (for endpoints with query params), `tests {}` (for assertions), `script:post-response {}` (to capture response values to env).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `.planning/REQUIREMENTS.md` (new sections) | documentation | — | Pure plain-text document edit; content is prescribed by RESEARCH §Area 5 exact diffs |
| `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` (cell edit) | documentation | — | Single-cell update to existing doc; no code patterns apply |

---

## Metadata

**Analog search scope:** `backend/src/audit/`, `backend/tests/audit_handlers_test.rs`, `frontend/src/app/(dashboard)/audit/`, `frontend/src/components/audit/`, `bruno/cronometrix/`, `.planning/phases/09-*/09-VERIFICATION.md`, `.planning/phases/06-*/06-VERIFICATION.md`, `.planning/phases/02-*/02-VERIFICATION.md`

**Files scanned:** 14 source files read directly

**Pattern extraction date:** 2026-04-29
