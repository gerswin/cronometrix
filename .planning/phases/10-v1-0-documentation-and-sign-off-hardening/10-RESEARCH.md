# Phase 10: v1.0 Documentation & Sign-off Hardening — Research

**Researched:** 2026-04-29
**Domain:** Documentation, retroactive verification, Rust/Axum endpoint, React/TanStack Query, Bruno collection
**Confidence:** HIGH (all findings verified against codebase directly — no external dependencies)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Sub-task 1 — Post-hoc 01-VERIFICATION.md**
- D-01: Full retroactive audit at depth of 09-VERIFICATION.md (21 must-haves, explicit code evidence). Read all Phase 1 PLANs + SUMMARYs, map every Phase 1 REQ (DATA-01..04, AUTH-01..05, EMP-01..04, DEPT-01..03, RULE-01..03 = 19 REQs) to file:line evidence, produce `01-VERIFICATION.md` with `status: passed`.
- D-02: Spawn `gsd-verifier` subagent in worktree isolation (parallel-safe). Match input contract used by recent verifier spawns.
- D-03: If genuine evidence gaps found, write gap as Phase 11 follow-up — do NOT fail the verification.

**Sub-task 2 — Post-hoc 07-VERIFICATION.md**
- D-04: Same approach as D-01..D-03 for Phase 7 (ENRL-01..05 = 5 REQs + ISAPI face-upload integration with Phase 2).
- D-05: Verifier MUST cross-reference Phase 7's `enrollments/pusher.rs → isapi/client.rs` wiring against integration matrix in `v1.0-MILESTONE-AUDIT.md` line 8.

**Sub-task 3 — REQUIREMENTS.md traceability refresh**
- D-06: Mark all 48 v1 REQs `Complete` for shipped phases (1, 2, 3, 4, 5, 6, 7); override stale `Pending` rows.
- D-07: Flip inconsistent `[ ]`/`[x]` checkbox state — ENRL-* checkboxes currently `[x]` but traceability says `Pending`; sync both.
- D-08: Add `## v1 Cross-Cutting Meta-Requirements (Phases 8+)` section after v1 table.
- D-09: DEPL-03 row → `Partial — accepted v1 ship (D-13 in 06-CONTEXT.md); auto-register strict reading deferred to v1.1 backlog as DEPL-03-AUTO`.
- D-10: Update Coverage block total to reflect v1 + meta count.

**Sub-task 4 — /audit/actors endpoint + frontend wiring**
- D-11: `GET /api/v1/audit/actors` → `[{actor_id, username, role}]` from `SELECT DISTINCT al.actor_id, u.username, u.role FROM audit_log al LEFT JOIN users u ON al.actor_id = u.id`. RBAC: Admin + Supervisor read; Viewer 403; Anonymous 401. Register in `supervisor_read_routes`.
- D-12: No pagination — cardinality bounded by admin/supervisor user count.
- D-13: Frontend `audit/page.tsx` adds `useQuery(['audit-actors'], () => api.get('/audit/actors'))` with `staleTime: 5 * 60 * 1000`. Display `{username} ({role})` with `actor_id` value. Test with existing 09-05 audit-page Vitest tests + add new test for actor-dropdown population.
- D-14: Coverage gate applies: backend `audit/handlers.rs` and `audit/service.rs` per-file ≥70%/60%; `src/app/**` is excluded from frontend coverage include set so no new frontend coverage tests required for the page edit, but `__tests__/audit-table.test.tsx` must still pass.
- D-15: Bruno collection `bruno/cronometrix/audit/` MUST get a new `actors.bru` request.

**Sub-task 5 — DEPL-03 final decision record**
- D-16: Accept v1 deferral as final. Token-based connector flow is the documented D-13 design choice in `06-CONTEXT.md`.
- D-17: No new code in Phase 10 for DEPL-03. Closure is documentation only.
- D-18: Add `DEPL-03-AUTO` to new `## v1.1 Backlog` section in `REQUIREMENTS.md` with strict auto-register definition.
- D-19: Update `06-VERIFICATION.md` deferred-items table to cross-reference v1.1 backlog item ID.

**Cross-cutting commit hygiene**
- D-20: 5 atomic commits, one per sub-task. `docs(10-NN):` for verifications/traceability/DEPL-03; `feat(10-NN):` for /audit/actors.
- D-21: Backend `cargo nextest run` MUST report ≥757 passing + new audit/actors tests; frontend `npx vitest run` MUST report 337/338 passing.
- D-22: No new coverage exclusions in `vitest.config.ts` or `Makefile` regex.

### Claude's Discretion
- Wave-grouping for execution: verifications parallel-safe; traceability + /audit/actors + DEPL-03 doc each modify different files.
- Verifier prompt detail: whether to pass `09-VERIFICATION.md` as explicit template or let verifier pull autonomously.
- Bruno collection naming: `actors.bru` vs `list-actors.bru` — match convention already in `bruno/cronometrix/audit/`.

### Deferred Ideas (OUT OF SCOPE)
- DEPL-03-AUTO implementation (v1.1 backlog — documentation only in Phase 10)
- Live-env validation work: Phase 8 Plan 05 live PR runs, Phase 9 CI green/red validation, branch protection toggles, fresh-VM installer smoke, LIC-05 cross-host clone test, real Hikvision alertStream test (ALL Phase 11)
- Audit screen v1.1 polish: actor avatars, date-range presets, diff-view modal
- Audit actors caching strategy (Redis/denormalized view)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SIGNOFF-01-VERIFY | Produce post-hoc 01-VERIFICATION.md for Phase 1 (19 REQs: DATA-01..04, AUTH-01..05, EMP-01..04, DEPT-01..03, RULE-01..03) | Phase 1 evidence map in §Phase 1 Evidence, canonical verification format in §Verification Document Format |
| SIGNOFF-07-VERIFY | Produce post-hoc 07-VERIFICATION.md for Phase 7 (5 REQs: ENRL-01..05 + ISAPI integration) | Phase 7 evidence map in §Phase 7 Evidence, integration matrix cross-ref in §Integration Matrix |
| SIGNOFF-TRACEABILITY | Refresh REQUIREMENTS.md traceability table: Pending→Complete for phases 2/4/5/6/7, DEPL-03 partial note, new meta-REQ section, new v1.1 backlog section | Exact diff plan in §REQUIREMENTS.md Diff Plan |
| SIGNOFF-AUDIT-ACTORS | Build `GET /api/v1/audit/actors` backend + wire frontend actor dropdown + Bruno entry | Full implementation spec in §Audit Actors Endpoint |
| SIGNOFF-DEPL-03-DECISION | Update `06-VERIFICATION.md` deferred-items table; add DEPL-03-AUTO to v1.1 backlog in REQUIREMENTS.md | Exact line/row in §DEPL-03 Deferral Record |
</phase_requirements>

---

## Summary

Phase 10 is a pure documentation + small-feature phase that closes 5 audit gaps identified by `v1.0-MILESTONE-AUDIT.md`. All work is fully autonomous — no external infrastructure is required. The two verification documents (01 and 07) require the heaviest reading work (scanning ~200 files of live code) but produce straightforward table-format output. The one code change (`/audit/actors`) is a ~40-line Rust service function + a matching ~30-line handler, wired into an existing router group with a matching 3-function test suite. The remaining three sub-tasks are pure document edits with precise diffs.

The critical risk in this phase is regression: the Phase 8 coverage gate (per-file 70%/60% floor) applies to the new audit module code, and the Phase 9 E2E suite must remain green. Both constraints are satisfied by the approach: the new `list_actors` service function mirrors `list_audit` at the same testability level, and the frontend `/audit/actors` query touches `src/app/**` (excluded from Vitest coverage scope) while `AuditFilters` (in `src/components/audit/`) is already fully tested and does not change its `actors` prop contract.

**Primary recommendation:** Execute sub-tasks 1 + 2 in parallel (independent file paths: `01-VERIFICATION.md` and `07-VERIFICATION.md`); serialize sub-tasks 3 + 5 (both touch `REQUIREMENTS.md`); sub-task 4 is independent code work.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Retroactive verification documents | Documentation only | — | Read-only analysis; no tier owns it; verifier subagent reads codebase |
| `GET /audit/actors` endpoint | API / Backend (Axum) | — | Auth-protected read from DB; no frontend logic needed beyond fetch |
| Actor dropdown display | Frontend Server (Next.js page) | — | `audit/page.tsx` → `AuditFilters` receives the enriched actors list |
| REQUIREMENTS.md traceability update | Documentation only | — | Plain text edit; no tier |
| DEPL-03 deferral record | Documentation only | — | Updates to `06-VERIFICATION.md` and `REQUIREMENTS.md` |

---

## Standard Stack

Phase 10 uses no new libraries. All stack components are inherited from prior phases.

### Core (inherited, no new installs)

| Component | Version | Purpose | Notes |
|-----------|---------|---------|-------|
| Rust / Axum | 0.8.8 | New `/audit/actors` handler | Already in `Cargo.toml` |
| libSQL | current | `SELECT DISTINCT ... JOIN` query | Already in `Cargo.toml` |
| TanStack Query | v5.x | `useQuery(['audit-actors'], ...)` | Already in `frontend/package.json` |
| Vitest | current | New actor-dropdown test | Already in `frontend/package.json` |
| Bruno | n/a | `actors.bru` request file | Already installed per-developer |

**No `cargo add` or `npm install` required for Phase 10.**

---

## Architecture Patterns

### System Architecture Diagram

```
Sub-tasks 1 + 2 (parallel — independent paths):
  gsd-verifier subagent (worktree A)        gsd-verifier subagent (worktree B)
        |                                           |
  reads Phase 1 PLANs/SUMMARYs             reads Phase 7 PLANs/SUMMARYs
  greps backend/src/{auth,employees,        greps backend/src/enrollments/
        departments,rules}/                 + isapi/client.rs
  produces 01-VERIFICATION.md              produces 07-VERIFICATION.md

Sub-task 3 (serialized after sub-tasks 1+2, or parallel since different file):
  edit .planning/REQUIREMENTS.md:
    Pending → Complete for phases 2,4,5,6,7
    DEPL-03 → Partial note
    + new ## v1 Cross-Cutting Meta-Requirements section
    + new ## v1.1 Backlog section

Sub-task 4 (independent — code change):
  backend/src/audit/service.rs   <-- add list_actors()
  backend/src/audit/handlers.rs  <-- add list_actors_handler()
  backend/src/main.rs            <-- .route("/audit/actors", get(audit::handlers::list_actors))
  backend/tests/audit_handlers_test.rs <-- 3 new tests
  frontend/src/app/(dashboard)/audit/page.tsx  <-- add useQuery(['audit-actors'])
  bruno/cronometrix/audit/01_list_actors.bru   <-- new file

Sub-task 5 (serialized after sub-task 3 — both touch REQUIREMENTS.md):
  .planning/REQUIREMENTS.md  <-- add DEPL-03-AUTO to ## v1.1 Backlog
  .planning/phases/06-licensing-deployment/06-VERIFICATION.md
      --> update deferred-items table row 1 "addressed_in" field
```

### Recommended Project Structure

No new directories. Phase 10 adds to existing structures:

```
backend/src/audit/
├── mod.rs              # add pub use handlers::list_actors;
├── handlers.rs         # add list_actors handler function
├── models.rs           # add AuditActor model struct
└── service.rs          # add list_actors() service function

backend/tests/
└── audit_handlers_test.rs  # add 3 new test functions

frontend/src/app/(dashboard)/audit/
└── page.tsx                # add useQuery(['audit-actors']) + update actors useMemo

bruno/cronometrix/audit/    # NEW directory
└── 01_list_audit.bru       # existing (must verify naming)
└── 02_list_actors.bru      # new file (name TBD — see Bruno Naming section)

.planning/
├── REQUIREMENTS.md                       # traceability + meta + backlog + DEPL-03
└── phases/
    ├── 01-foundation/01-VERIFICATION.md  # new
    ├── 06-licensing-deployment/06-VERIFICATION.md  # update deferred-items table
    └── 07-facial-enrollment-sync/07-VERIFICATION.md  # new
```

---

## Research Area 1: /audit/actors Backend Pattern

### JOIN Query

[VERIFIED: direct codebase inspection of `backend/src/db/migrations/001_initial_schema.sql` and `backend/src/audit/service.rs`]

The `users` table schema is:
```sql
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    full_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'supervisor', 'viewer')),
    refresh_token_hash TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    ...
)
```

The `audit_log` table has `actor_id TEXT` (nullable). The locked query from D-11 is:

```sql
SELECT DISTINCT al.actor_id, u.username, u.role
FROM audit_log al
LEFT JOIN users u ON al.actor_id = u.id
```

**Correctness analysis:**
- `LEFT JOIN` is correct — `actor_id` can be NULL (system-generated triggers) or reference a now-deleted user. `INNER JOIN` would lose those records from the actor list.
- `DISTINCT` works correctly on `(actor_id, username, role)` tuples — small cardinality guaranteed.
- Performance: `audit_log.actor_id` has no index (verified: 001 only indexes `table_name`, `record_id`, `created_at`). At v1 scale (bounded admin set, small audit_log) a full scan of actor_id is acceptable. [VERIFIED: confirmed no idx on actor_id in 001_initial_schema.sql]

**Caching decision (D-12 context):** No `Arc<RwLock<HashMap>>` cache needed. Query-on-each-request is correct given:
1. staleTime: 5 min on frontend (D-13) eliminates repeat server calls
2. Actor cardinality = small N (number of users who ever performed an action)
3. No equivalent in-memory cache exists elsewhere in the backend (confirmed: AppState carries `db`, `config`, `paths`, `license_valid`, `cancel_tx`, `purge_tx`, `backfill_tx`, `captures` — no actor cache)

### Model struct (new in `backend/src/audit/models.rs`)

```rust
// Source: mirrors AuditEntry pattern in same file
#[derive(Debug, Clone, Serialize)]
pub struct AuditActor {
    pub actor_id: Option<String>,  // NULL when audit_log.actor_id IS NULL
    pub username: Option<String>,  // NULL when user was deleted (LEFT JOIN miss)
    pub role: Option<String>,      // NULL same
}
```

### Service function pattern

```rust
// Source: mirrors list_audit() in backend/src/audit/service.rs
pub async fn list_actors(conn: &Connection) -> Result<Vec<AuditActor>, AppError> {
    let sql = "SELECT DISTINCT al.actor_id, u.username, u.role \
               FROM audit_log al LEFT JOIN users u ON al.actor_id = u.id";
    let mut rows = conn.query(sql, ()).await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut data: Vec<AuditActor> = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| AppError::Internal(e.into()))? {
        data.push(AuditActor {
            actor_id: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
            username: row.get(1).map_err(|e| AppError::Internal(e.into()))?,
            role: row.get(2).map_err(|e| AppError::Internal(e.into()))?,
        });
    }
    Ok(data)
}
```

### Handler function pattern

```rust
// Source: mirrors list_audit() in backend/src/audit/handlers.rs
pub async fn list_actors(
    State(state): State<AppState>,
) -> Result<Json<Vec<AuditActor>>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list_actors(&conn).await?;
    Ok(Json(result))
}
```

### Route registration (main.rs line 237 region)

```rust
// Source: backend/src/main.rs:235-245 (supervisor_read_routes block)
let supervisor_read_routes = Router::new()
    .route("/anomalies", get(anomalies::handlers::list_anomalies))
    .route("/audit", get(audit::handlers::list_audit))
    .route("/audit/actors", get(audit::handlers::list_actors))  // ADD THIS
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_supervisor_or_above,
    ))
    ...
```

---

## Research Area 2: Verification Document Format

### Canonical format (verified from 3 existing VERIFICATIONs)

[VERIFIED: direct inspection of `02-VERIFICATION.md`, `03-VERIFICATION.md`, `06-VERIFICATION.md`, `09-VERIFICATION.md`]

**Frontmatter contract (required fields):**
```yaml
---
phase: {phase-slug}
verified: {ISO-8601 timestamp}
status: passed | human_needed
score: {N}/{N} must-haves verified
overrides_applied: 0
human_verification: []   # or list of human-only items
deferred: []             # or list of accepted deferrals
---
```

**Body sections (canonical order):**
1. `## Goal Achievement` → `### Observable Truths` → truth table
2. `### Required Artifacts` → artifact table
3. `### Key Link Verification` → wiring table (optional for simpler phases like Phase 3)
4. `### Data-Flow Trace` (optional — Phase 3 omits)
5. `### Behavioral Spot-Checks` → command/result table
6. `### Requirements Coverage` → REQ → status table
7. `### Anti-Patterns Found` (optional)
8. `### Human Verification Required` (if any)
9. `### Gaps Summary` → brief paragraph

**Depth target (from D-01):** 09-VERIFICATION.md depth = 21 must-haves with explicit code evidence (`file.rs:NN` line references). Phase 3 uses 5 truths (minimal format). For Phase 1 (19 REQs) and Phase 7 (5 REQs), target the 02/09-style full format with file:line evidence for every truth.

**Retroactive variant specifics:**
- Status should be `passed` (not `human_needed`) for Phase 1 since all code is verifiable in-codebase without live hardware.
- Phase 7 status should be `human_needed` (matches Phase 7 dependency on live Hikvision hardware for ENRL-01 device-camera capture).
- `verified` timestamp = time of document creation (retroactive).
- D-03/D-04: Evidence gaps → write as Phase 11 follow-up in `human_verification` list.

---

## Research Area 3: Phase 1 Evidence Map

### Phase 1 Requirements → Evidence

[VERIFIED: direct codebase inspection]

| REQ | Description | Primary Evidence Path |
|-----|-------------|----------------------|
| DATA-01 | All data stored locally in SQLite via libSQL | `backend/src/db/mod.rs` + `Cargo.toml` libsql dependency |
| DATA-02 | Data syncs async to Turso cloud | `backend/src/main.rs` `Builder::new_remote_replica()` call; `config.rs` `turso_url`/`turso_token` fields |
| DATA-03 | Local SQLite authoritative — cloud is replica | `Builder::new_remote_replica()` creates embedded replica; local write primary pattern in all service files |
| DATA-04 | Every admin mutation generates immutable audit log entry | `backend/src/db/migrations/002_audit_triggers.sql` — triggers on employees, departments, global_rules, daily_records, etc. |
| AUTH-01 | User can log in with username and password | `backend/src/auth/handlers.rs:35` SELECT from users; `backend/tests/auth_tests.rs` |
| AUTH-02 | Admin role has full access | `backend/src/auth/rbac.rs` RBAC middleware; admin_routes in `main.rs` |
| AUTH-03 | Supervisor role can edit timesheets, manage employees | `supervisor_read_routes` + `supervisor_routes` in `main.rs` |
| AUTH-04 | Viewer role read-only | `viewer_routes` in `main.rs`; Viewer 403 on mutating endpoints |
| AUTH-05 | Session persists across browser refresh | `backend/src/auth/handlers.rs:121` refresh token flow; httpOnly cookie |
| EMP-01 | Create employee with unique ID, name, department, status | `backend/src/employees/handlers.rs` POST handler; `migrations/001_initial_schema.sql` employees table |
| EMP-02 | Search/filter by name, department, status | `backend/src/employees/service.rs` dynamic WHERE clause with positional params |
| EMP-03 | Soft delete (status=inactive, deleted_at set) | `backend/src/employees/handlers.rs` DELETE handler; employees.status + deleted_at pattern |
| EMP-04 | Each employee belongs to exactly one department | `backend/src/db/migrations/001_initial_schema.sql` `department_id TEXT NOT NULL REFERENCES departments(id)` |
| DEPT-01 | Create department with base salary and shift schedule | `backend/src/departments/handlers.rs` POST handler; schema in 001 migration |
| DEPT-02 | Configure lunch mode per department | `lunch_mode TEXT CHECK(lunch_mode IN ('fixed', 'punch'))` in schema |
| DEPT-03 | Edit department settings | `backend/src/departments/handlers.rs` PATCH handler |
| RULE-01 | Configure tolerance margins via visual sliders | `backend/src/rules/handlers.rs` + `frontend/src/components/` rule sliders |
| RULE-02 | Configure bonus minutes | `global_rules.bonus_minutes` in schema + rules handler |
| RULE-03 | Rule changes take effect on next calculation cycle | `backend/src/rules/service.rs` `effective_from` always updated on any PATCH (per STATE.md decision) |

**Verifier reading list for Phase 1:**
- `backend/src/db/migrations/001_initial_schema.sql` — DATA-01, DATA-02, DATA-03, DATA-04, EMP-01..04, DEPT-01..03, RULE-01..03
- `backend/src/db/migrations/002_audit_triggers.sql` — DATA-04
- `backend/src/auth/{handlers,middleware,rbac,service}.rs` — AUTH-01..05
- `backend/src/employees/{handlers,service,models}.rs` — EMP-01..04
- `backend/src/departments/{handlers,service,models}.rs` — DEPT-01..03
- `backend/src/rules/{handlers,service,models}.rs` — RULE-01..03
- `backend/src/main.rs` — route group wiring for all RBAC REQs
- `backend/tests/{auth,employee,department,rules}_tests.rs` — test evidence
- `backend/src/state.rs` + `backend/src/config.rs` — Turso sync config (DATA-02)
- Phase 1 PLANs (01-00 through 01-04) and SUMMARYs — planned vs. delivered

---

## Research Area 4: Phase 7 Evidence Map

### Phase 7 Requirements → Evidence

[VERIFIED: direct inspection of `07-01-SUMMARY.md`, `07-02-SUMMARY.md`, `07-CONTEXT.md`]

| REQ | Description | Primary Evidence Path |
|-----|-------------|----------------------|
| ENRL-01 | Admin captures facial profile via Hikvision device camera | `backend/src/enrollments/handlers.rs` capture_from_device endpoint; `backend/src/isapi/client.rs:233` `capture_face_image` |
| ENRL-02 | Admin uploads JPG for facial enrollment | `backend/src/enrollments/handlers.rs` POST /enrollments with `captured_via = "upload"` |
| ENRL-03 | Admin captures via webcam | `frontend/src/components/enrollment/` webcam tab; `captured_via = "webcam"` |
| ENRL-04 | System syncs enrolled facial profile to all registered devices simultaneously | `backend/src/enrollments/pusher.rs` JoinSet fan-out; `isapi/client.rs:108,144` `upsert_user`/`upload_face` |
| ENRL-05 | Admin sees per-device sync status during enrollment | `backend/src/enrollments/service.rs` enrollment_device_pushes table; GET /enrollments/:id polling returns push rows |

**Integration matrix cross-ref (from `v1.0-MILESTONE-AUDIT.md` dimension 8):**
- `enrollments/pusher.rs:173-187` → `isapi/client.rs:108,144,233` [VERIFIED: confirmed in audit file]
- The verifier MUST read `backend/src/enrollments/pusher.rs` lines 173-187 and confirm they call `isapi::client::upsert_user`, `upload_face`, or `capture_face_image`.

**Verifier reading list for Phase 7:**
- `backend/src/enrollments/{mod,handlers,service,pusher,image_pipeline,isapi_face,models}.rs`
- `backend/src/workers/{backfill,purge}.rs` — D-15/D-16 workers
- `backend/src/isapi/client.rs` — upsert_user (line 108), upload_face (line 144), capture_face_image (line 233)
- `backend/src/db/migrations/016_enrollments.sql` + `017_phase7_audit_triggers.sql`
- `frontend/src/components/enrollment/` — modal + capture tabs
- `backend/tests/{enrollments_test,multi_device_push_test,face_capture_test,enrollment_lifecycle_test}.rs`
- `07-01-PLAN.md`, `07-02-PLAN.md`, `07-01-SUMMARY.md`, `07-02-SUMMARY.md`

---

## Research Area 5: REQUIREMENTS.md Traceability Diff Plan

### Current state (verified by inspection of REQUIREMENTS.md lines 152-216)

[VERIFIED: direct inspection — confirmed against actual file content]

**Rows currently wrong:**
- Phase 2 rows (DEV-01..04, EVT-01..04): status = `Pending` → should be `Complete`
- Phase 4 rows (DASH-01..03, TS-01..05): status = `Pending` → should be `Complete`
- Phase 5 rows (PAY-01..04): status = `Pending` → should be `Complete`
- Phase 6 rows (LIC-01..05, DEPL-01, DEPL-02, DEPL-04): status = `Pending` → should be `Complete`
- Phase 6 row DEPL-03: status = `Pending` → should be `Partial — accepted v1 ship (D-13 in 06-CONTEXT.md); auto-register strict reading deferred to v1.1 backlog as DEPL-03-AUTO`
- Phase 7 rows (ENRL-01..05): status = `Pending` → should be `Complete` (traceability column)

**Checkbox inconsistency:**
- ENRL-01..05: checkboxes in requirements list say `[x]` (lines 17-27) but traceability table says `Pending`
- DEV-01..04, EVT-01..04: checkboxes say `[ ]` (not yet marked complete in list) — leave checkbox as-is since they are in `Pending` category in the original design (Phase 2 hardware-gated requirements); only traceability table and Phase 7 ENRL checkboxes need sync per D-07
- ACTUALLY: Per D-07, "Flip the inconsistent `[ ]`/`[x]` checkbox state to match the traceability column. ENRL-* checkboxes are currently `[x]` but traceability column says `Pending` — bring both into sync." → Both checkbox AND traceability should say `Complete` for ENRL-*

**Exact DEPL-03 row update (D-09):**
```markdown
| DEPL-03 | Phase 6 | Partial — accepted v1 ship (D-13 in 06-CONTEXT.md); auto-register strict reading deferred to v1.1 backlog as DEPL-03-AUTO |
```

**New section to add after the v1 traceability table (D-08):**
```markdown
## v1 Cross-Cutting Meta-Requirements (Phases 8+)

| Requirement | Phase | Status |
|-------------|-------|--------|
| QUALITY-GATE | Phase 8 | Complete |
| E2E-TOOLING..E2E-SELECTORS (21 IDs) | Phase 9 | Complete |
```

**New v1.1 Backlog section (D-18):**
```markdown
## v1.1 Backlog

| ID | Description | Notes |
|----|-------------|-------|
| DEPL-03-AUTO | Installer auto-registers a Cloudflare tunnel by calling the Cloudflare API with a CF API token (not just a tunnel TOKEN), creating the tunnel + DNS route + cloudflared service config in one step. | v1.1 should evaluate whether `cloudflared tunnel create` CLI invocation suffices vs full Go SDK call. |
```

**Coverage block update (D-10):** Change from `Mapped to phases: 48` to:
```markdown
**Coverage:**
- v1 requirements: 48 total
- Mapped to phases: 48
- v1 Cross-Cutting Meta-Requirements: QUALITY-GATE (Phase 8) + 21 E2E-* (Phase 9) = 22 additional
- Unmapped: 0 ✓
```

---

## Research Area 6: Frontend /audit/actors Wiring

### Current state (verified)

[VERIFIED: direct inspection of `frontend/src/app/(dashboard)/audit/page.tsx` and `frontend/src/components/audit/audit-filters.tsx`]

The current `page.tsx` (lines 79-88) derives `actors` from current page data via `useMemo`:
```tsx
const actors = useMemo(() => {
  if (!data?.data) return []
  const seen = new Map<string, string>()
  for (const entry of data.data) {
    if (entry.actor_id && !seen.has(entry.actor_id)) {
      seen.set(entry.actor_id, entry.actor_id)  // username = actor_id (raw UUID)
    }
  }
  return Array.from(seen.entries()).map(([id, username]) => ({ id, username }))
}, [data?.data])
```

**Minimum diff to implement D-13:**

```tsx
// ADD: new useQuery for actors (after existing useQuery for audit data)
const { data: actorsData } = useQuery<Array<{actor_id: string|null, username: string|null, role: string|null}>>({
  queryKey: ['audit-actors'],
  queryFn: () => api.get('/audit/actors').then(r => r.data),
  staleTime: 5 * 60 * 1000,
  enabled: role === 'admin' || role === 'supervisor',
})

// REPLACE: existing useMemo with actors derived from actorsData
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

**Impact on `AuditFilters`:** Zero changes needed. The `actors` prop type is `Array<{ id: string; username: string }>` — the new actors list satisfies this contract identically. The `<option key={a.id} value={a.id}>{a.username}</option>` pattern renders `{username} ({role})` as the display text automatically.

**Impact on existing Playwright tests:** The `audit.spec.ts` T-03 (actor filter) uses `selectOption('e2e-admin-id')`. With the new `/audit/actors` endpoint, the select `<option value="e2e-admin-id">` still exists (actor_id is still the value). HOWEVER: the option text changes from `e2e-admin-id` (raw UUID) to `e2e-admin (admin)` (username + role). The Playwright `selectOption('e2e-admin-id')` call uses the **value** not the label text, so it remains correct. [VERIFIED: Playwright `selectOption(value)` matches by value attribute by default]

**Impact on Vitest coverage:** `src/app/(dashboard)/audit/page.tsx` is in `src/app/**` which is NOT in the Vitest coverage `include` array (`src/components/**`, `src/hooks/**`, `src/lib/**`). So the new `useQuery` call in `page.tsx` is NOT in coverage scope. The `AuditFilters` component is in `src/components/audit/` and IS in coverage scope — but its code does not change. [VERIFIED: `vitest.config.ts` coverage.include]

**New Vitest test needed (D-13):** A test for actor-dropdown population. This test belongs in `audit-table.test.tsx` (the existing test file at `frontend/src/components/audit/__tests__/audit-table.test.tsx`). The test pattern: render `AuditFilters` with `actors` prop containing `[{id: 'user-1', username: 'admin (admin)'}]` and verify the option renders with the new display format. The existing test at line 243-254 already tests `actors` prop rendering — a simple extension.

---

## Research Area 7: Bruno Collection Convention

### Current convention (verified)

[VERIFIED: `find /Users/gerswin/Proyectos/cronometrix/bruno -name "*.bru"` output]

```
cronometrix/auth/ 01_login.bru 02_refresh.bru 03_logout.bru
cronometrix/enrollments/ 01_create_enrollment.bru 02_get_enrollment.bru ...
cronometrix/reports/ 01_json.bru 02_excel.bru
```

**Naming pattern:** `{NN}_{action_or_entity}.bru` where NN is sequential. BUT there is no `bruno/cronometrix/audit/` directory yet (confirmed: `ls bruno/cronometrix/` shows no audit folder — only the gitStatus shows `?? bruno/cronometrix/audit/` which means it may exist as untracked).

**Check gitStatus:** `?? bruno/cronometrix/audit/` is listed in the gitStatus as untracked. This means the `audit/` folder exists but is not committed. The CONTEXT.md D-15 says to add `actors.bru` — so a `list_audit.bru` (for the existing endpoint) and `actors.bru` (for the new endpoint) likely both belong there.

**Recommended file name:** Based on the convention `{NN}_{action}.bru`, the actors endpoint should be:
- `01_list.bru` for `GET /audit` (existing)
- `02_list_actors.bru` for `GET /audit/actors` (new)

OR if the audit folder already has `01_list.bru`, name the new one `02_list_actors.bru`. The CONTEXT D-15 uses `actors.bru` as the name — Claude's Discretion says match the convention; the planner should verify the existing file name in the untracked `audit/` folder before deciding.

**Bruno file template for `GET /audit/actors`:**
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

---

## Research Area 8: DEPL-03 Deferral Record — Exact Location

### Current state of 06-VERIFICATION.md deferred-items table (verified)

[VERIFIED: direct inspection of `06-VERIFICATION.md` lines 27-31]

```markdown
### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Auto-registers a Cloudflare tunnel via CF API (strict reading of DEPL-03) | Phase 7 / future v2 release | Current architecture: token-based connector... Documented as design choice D-13... |
```

**Required change (D-19):** Update the `Addressed In` field from `Phase 7 / future v2 release` to `v1.1 Backlog — DEPL-03-AUTO` (cross-referencing the new backlog entry in `REQUIREMENTS.md`).

**Final deferred-items table row after update:**
```markdown
| 1 | Auto-registers a Cloudflare tunnel via CF API (strict reading of DEPL-03) | v1.1 Backlog — DEPL-03-AUTO (see REQUIREMENTS.md §v1.1 Backlog) | Current architecture: token-based connector to a pre-registered CF Zero Trust tunnel. Slug flows to .env for future automation. Documented as design choice D-13 in 06-CONTEXT.md. Accepted as v1 ship decision. |
```

**Does `06-CONTEXT.md` D-13 need changing?** No (verified by reading D-16: "Accept the v1 deferral as final. The v1 ship of Phase 6 uses operator-driven Cloudflare Zero Trust (D-13 design choice in 06-CONTEXT.md)."). D-13 in `06-CONTEXT.md` is the correct rationale — it stays unchanged.

---

## Research Area 9: Regression Risk Analysis

### Coverage gate at risk

[VERIFIED: `vitest.config.ts` and CLAUDE.md §Test Coverage]

**Backend: new `audit/handlers.rs` and `audit/service.rs` functions**
- Per-file floor: ≥70% lines, ≥60% branches
- Current test count for `audit_handlers_test.rs`: 14 tests passing [VERIFIED by running `cargo nextest run --test audit_handlers_test`]
- New tests needed to keep existing tests + cover new endpoint:
  1. `audit_actors_returns_200_for_admin` — happy path with data
  2. `audit_actors_viewer_returns_403` — RBAC enforcement
  3. `audit_actors_returns_empty_when_no_log` — empty audit_log case

These 3 tests cover 100% of the new `list_actors()` service (1 branch: empty vs non-empty) and 100% of the new handler (1 happy path).

**Frontend: coverage impact**
- `src/app/(dashboard)/audit/page.tsx` — NOT in coverage include set → no impact
- `src/components/audit/audit-filters.tsx` — in coverage scope; code does NOT change → no impact
- Existing `audit-table.test.tsx` tests (12 AuditTable + 14 AuditFilters = 26 tests) must remain passing → no change to AuditFilters props contract

**Phase 9 E2E impact:**
- `audit.spec.ts` T-03 (actor filter): uses `selectOption('e2e-admin-id')` by VALUE — unaffected by display text change [VERIFIED: analysis above]
- `mock_hikvision` configuration: not affected (no device-related changes)
- `test_reset` endpoint: not affected (actors endpoint is read-only)

**Backend test baseline:** Currently 757+ tests passing per D-21. The 3 new audit/actors tests bring the new baseline to ≥760. The `cargo nextest run` command must be run from `backend/` directory.

---

## Research Area 10: Atomic Commit Grouping

### Optimal commit sequence (D-20)

| Sub-task | Files Modified | Commit Prefix | Order |
|----------|---------------|---------------|-------|
| 1: 01-VERIFICATION.md | `.planning/phases/01-foundation/01-VERIFICATION.md` | `docs(10-01):` | Wave 1 (parallel) |
| 2: 07-VERIFICATION.md | `.planning/phases/07-facial-enrollment-sync/07-VERIFICATION.md` | `docs(10-02):` | Wave 1 (parallel) |
| 4: /audit/actors | `backend/src/audit/{handlers,service,models,mod}.rs`, `backend/tests/audit_handlers_test.rs`, `frontend/src/app/(dashboard)/audit/page.tsx`, `bruno/cronometrix/audit/XX_list_actors.bru` | `feat(10-04):` | Wave 2 (independent) |
| 3: REQUIREMENTS.md | `.planning/REQUIREMENTS.md` | `docs(10-03):` | Wave 2 (must serialize before sub-task 5) |
| 5: DEPL-03 record | `.planning/REQUIREMENTS.md` (v1.1 Backlog section), `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` | `docs(10-05):` | Wave 3 (after sub-task 3) |

**File overlap analysis:**
- Sub-tasks 1 + 2: completely independent (different files, different directories) → parallel-safe in Wave 1
- Sub-task 4: touches `backend/`, `frontend/`, `bruno/` — no overlap with 1, 2, 3, or 5 → can run concurrently with Wave 1
- Sub-task 3: touches `REQUIREMENTS.md` → must complete before sub-task 5 (which also adds to `REQUIREMENTS.md`)
- Sub-task 5: touches `REQUIREMENTS.md` + `06-VERIFICATION.md` → serialize after sub-task 3

**Recommended wave layout:**
- Wave 1: Sub-tasks 1, 2, 4 in parallel (entirely different file sets)
- Wave 2: Sub-task 3 (REQUIREMENTS.md main refresh)
- Wave 3: Sub-task 5 (REQUIREMENTS.md v1.1 Backlog + 06-VERIFICATION.md update)

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Username JOIN for actors | Custom lookup cache | Direct SQL LEFT JOIN | Query is fast at v1 scale; TanStack Query staleTime provides client-side caching |
| Actor display in dropdown | Custom formatting component | Inline template literal in `page.tsx` useMemo | AuditFilters already accepts `{id, username}` — just format username as `{username} ({role})` |
| Retroactive verification document structure | Custom format | Mirror 09-VERIFICATION.md exactly | Consistent format is the contract; deviating creates audit ambiguity |

---

## Common Pitfalls

### Pitfall 1: Breaking the E2E actor filter test
**What goes wrong:** Changing the option display text without checking how Playwright selects the actor.
**Why it happens:** `audit.spec.ts` T-03 uses `selectOption('e2e-admin-id')` — looks like it selects by text, but it selects by VALUE.
**How to avoid:** The `<option value={a.id}>{a.username}</option>` pattern keeps `actor_id` as the value. `selectOption('e2e-admin-id')` matches the value attribute, not the visible label. No E2E spec change needed.
**Warning signs:** If the Playwright test fails with "option not found" after the change, it means the option value changed (not just the label).

### Pitfall 2: Serializing sub-tasks 3 + 5 incorrectly
**What goes wrong:** Sub-task 5 adds `v1.1 Backlog` to `REQUIREMENTS.md` — if sub-task 3 also touches the same section, merge conflicts or double-additions occur.
**Why it happens:** Both sub-tasks touch `REQUIREMENTS.md`.
**How to avoid:** Sub-task 3 only touches the traceability table section and the Coverage block. Sub-task 5 only adds the NEW `## v1.1 Backlog` section and edits `06-VERIFICATION.md`. These do NOT overlap — sub-task 3 must complete first so sub-task 5 can append cleanly.

### Pitfall 3: Missing the `AuditActor` model in `models.rs`
**What goes wrong:** Adding `list_actors` to `service.rs` and `handlers.rs` without adding the `AuditActor` struct to `models.rs` causes a compile error.
**Why it happens:** The existing `service.rs` imports only `AuditEntry` and `AuditListQuery` from `models.rs`.
**How to avoid:** Add `pub struct AuditActor` to `models.rs` and re-export from `mod.rs` before touching `service.rs`.

### Pitfall 4: Frontend coverage floor violation
**What goes wrong:** Adding a new function to `audit-filters.tsx` or `audit-table.tsx` without a test drops per-file coverage below 70%.
**Why it happens:** Coverage denominator increases but tests don't cover the new code.
**How to avoid:** Phase 10 does NOT modify `AuditFilters` or `AuditTable` — the actors data flows in via the existing `actors` prop. Only `page.tsx` changes, which is excluded from coverage. If a test for actor-dropdown population is added to `audit-table.test.tsx`, it must test behavior already in `AuditFilters`, not new code.

### Pitfall 5: 06-VERIFICATION.md deferred-items "Addressed In" field inaccuracy
**What goes wrong:** Updating `Addressed In` to `Phase 7` (the old value) instead of `v1.1 Backlog — DEPL-03-AUTO`.
**Why it happens:** The existing text says "Phase 7 / future v2 release" — it's easy to conflate with the current update.
**How to avoid:** The exact replacement text is: `v1.1 Backlog — DEPL-03-AUTO (see REQUIREMENTS.md §v1.1 Backlog)`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Backend framework | cargo-nextest (installed, pinned) |
| Backend config file | `backend/rust-toolchain.toml` (nightly-2026-04-01) |
| Backend quick run | `cd backend && cargo nextest run --test audit_handlers_test` |
| Backend full suite | `cd backend && cargo nextest run` |
| Frontend framework | Vitest 3.x |
| Frontend config file | `frontend/vitest.config.ts` |
| Frontend quick run | `cd frontend && npx vitest run src/components/audit` |
| Frontend full suite | `cd frontend && npx vitest run` |
| Coverage check | `make coverage` (both sides) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SIGNOFF-AUDIT-ACTORS (backend) | `GET /audit/actors` returns 200 + actor list for Admin | integration | `cargo nextest run --test audit_handlers_test` | ❌ Wave 0 (3 new tests) |
| SIGNOFF-AUDIT-ACTORS (RBAC) | Viewer gets 403 on `/audit/actors` | integration | `cargo nextest run --test audit_handlers_test` | ❌ Wave 0 |
| SIGNOFF-AUDIT-ACTORS (empty) | Empty audit_log returns `[]` | integration | `cargo nextest run --test audit_handlers_test` | ❌ Wave 0 |
| SIGNOFF-AUDIT-ACTORS (frontend) | Actor dropdown shows `{username} ({role})` display | unit | `cd frontend && npx vitest run src/components/audit` | ❌ Wave 0 (1 new test) |
| SIGNOFF-01-VERIFY | 01-VERIFICATION.md file exists with `status: passed` | manual (doc review) | `cat .planning/phases/01-foundation/01-VERIFICATION.md` | ❌ Wave 0 (doc created) |
| SIGNOFF-07-VERIFY | 07-VERIFICATION.md file exists with appropriate status | manual (doc review) | `cat .planning/phases/07-facial-enrollment-sync/07-VERIFICATION.md` | ❌ Wave 0 (doc created) |
| SIGNOFF-TRACEABILITY | All shipped phase rows show `Complete` in traceability | manual (doc review) | `grep "Pending" .planning/REQUIREMENTS.md` (should have 0 delivered-phase results) | ❌ Wave 0 (doc edit) |
| SIGNOFF-DEPL-03-DECISION | 06-VERIFICATION.md deferred-items row updated | manual (doc review) | `grep "DEPL-03-AUTO" .planning/phases/06-licensing-deployment/06-VERIFICATION.md` | ❌ Wave 0 (doc edit) |

### Sampling Rate

- **Per task commit (code tasks only):** `cd backend && cargo nextest run --test audit_handlers_test` + `cd frontend && npx vitest run src/components/audit`
- **Per wave merge:** `cd backend && cargo nextest run` + `cd frontend && npx vitest run`
- **Phase gate:** Full suite green (`make coverage`) before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `backend/tests/audit_handlers_test.rs` — 3 new test functions: `audit_actors_returns_200_for_admin`, `audit_actors_viewer_returns_403`, `audit_actors_returns_empty_when_no_log`
- [ ] `frontend/src/components/audit/__tests__/audit-table.test.tsx` — 1 new test: `renders actors with username (role) display format`
- [ ] No new framework install needed — all tooling present

*(Existing infrastructure fully covers all remaining phase requirements via doc review)*

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | JWT Bearer via existing `require_auth` middleware — no change |
| V3 Session Management | no | Session management unchanged |
| V4 Access Control | yes | `require_supervisor_or_above` already applied in `supervisor_read_routes` — `/audit/actors` inherits this automatically by being added to the same router |
| V5 Input Validation | no | `GET /audit/actors` has no query params — no input to validate |
| V6 Cryptography | no | No new crypto operations |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Unauthorized actor enumeration | Information Disclosure | `require_supervisor_or_above` middleware blocks Viewer (403) and Anonymous (401) — same pattern as existing `/audit` |
| Username leakage via LEFT JOIN | Information Disclosure | Acceptable — only users who have already performed an audited action are revealed; Supervisor already sees `actor_id` raw UUID in audit log |
| NULL actor_id in result | Tampering | `AuditActor.actor_id: Option<String>` — frontend filters null actors before display |

---

## State of the Art

No library upgrades or API changes relevant to this phase. All stack components are stable.

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| W6 OPTION A (actor_id raw string in dropdown) | `/audit/actors` username-join (OPTION B) | Phase 10 | Actor dropdown shows `{username} ({role})` instead of raw UUID; value remains actor_id |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `bruno/cronometrix/audit/` folder exists (shown as untracked in gitStatus) with at least one `.bru` file for the existing `/audit` endpoint | Bruno Naming | If folder doesn't exist, planner must create it first and add both `01_list.bru` and `02_list_actors.bru` |

**All other claims in this research were verified directly from codebase files in this session — no unverified assumptions.**

---

## Open Questions

1. **Bruno `audit/` directory contents**
   - What we know: `?? bruno/cronometrix/audit/` appears as untracked in gitStatus
   - What's unclear: Whether it already contains a `list.bru` or `list_audit.bru` file for the existing endpoint
   - Recommendation: Planner should `ls bruno/cronometrix/audit/` at execution start to determine the naming for the new actors file

2. **Phase 7 verification status: passed vs human_needed**
   - What we know: Phase 7 ENRL-01 (Hikvision device camera capture) depends on live hardware
   - What's unclear: Whether the verifier should mark the whole document `human_needed` or mark only that specific truth `HUMAN` while the document status is `passed`
   - Recommendation: Mirror Phase 6's approach — status `human_needed`, with the device-capture smoke test in `human_verification` list; the code path for ENRL-01 is verified in `face_capture_test.rs` against the mock Hikvision, so the implementation is correct; only the live-hardware smoke is deferred

---

## Environment Availability

Step 2.6 findings (code-only changes — no new external tools required):

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo-nextest | Backend tests | ✓ | (installed per CLAUDE.md) | `cargo test` |
| Vitest | Frontend tests | ✓ | (installed in frontend/package.json) | — |
| cargo-llvm-cov | Coverage gate | ✓ | 0.8.5 (pinned per CLAUDE.md) | — |
| Bruno (CLI or GUI) | `actors.bru` validation | ✓ | (per-developer tool, not CI) | — |

**Missing dependencies with no fallback:** None.

---

## Sources

### Primary (HIGH confidence)
- `backend/src/audit/handlers.rs` — existing handler pattern for new actors handler [VERIFIED]
- `backend/src/audit/service.rs` — existing service pattern for new actors service [VERIFIED]
- `backend/src/audit/mod.rs` — module re-export pattern [VERIFIED]
- `backend/src/audit/models.rs` — model struct pattern for AuditActor [VERIFIED]
- `backend/src/main.rs:234-245` — supervisor_read_routes registration site [VERIFIED]
- `backend/src/db/migrations/001_initial_schema.sql` — users table schema + audit_log schema [VERIFIED]
- `backend/tests/audit_handlers_test.rs` — test pattern for new actors tests [VERIFIED]
- `frontend/src/app/(dashboard)/audit/page.tsx` — current page with OPTION A actors [VERIFIED]
- `frontend/src/components/audit/audit-filters.tsx` — actors prop contract [VERIFIED]
- `frontend/src/components/audit/__tests__/audit-table.test.tsx` — existing test patterns [VERIFIED]
- `frontend/vitest.config.ts` — coverage include/exclude scopes [VERIFIED]
- `.planning/REQUIREMENTS.md` — current traceability table (lines 152-216) [VERIFIED]
- `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` — current deferred-items table [VERIFIED]
- `.planning/v1.0-MILESTONE-AUDIT.md` — gap list + integration matrix [VERIFIED]
- `.planning/phases/09-e2e-playwright-test-suite-.../09-VERIFICATION.md` — depth target format [VERIFIED]
- `.planning/phases/02-device-integration/02-VERIFICATION.md` — format reference [VERIFIED]
- `.planning/phases/03-time-calculation-engine/03-VERIFICATION.md` — minimal format reference [VERIFIED]
- `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` — full format with deferred items [VERIFIED]
- `bruno/cronometrix/auth/01_login.bru` — Bruno file format reference [VERIFIED]
- `find /Users/gerswin/Proyectos/cronometrix/bruno -name "*.bru"` — Bruno naming convention [VERIFIED]

### Secondary (MEDIUM confidence)
- `find /Users/gerswin/Proyectos/cronometrix/.planning/phases/01-foundation -name "*.md"` — Phase 1 file inventory [VERIFIED]
- `07-01-SUMMARY.md` + `07-02-SUMMARY.md` — Phase 7 deliverables for evidence map [VERIFIED]

---

## Metadata

**Confidence breakdown:**
- Audit actors endpoint: HIGH — pattern directly mirrors existing service/handler; schema verified
- Verification document format: HIGH — compared 4 existing VERIFICATIONs directly
- Phase 1 evidence map: HIGH — file paths verified by directory listing + file inspection
- Phase 7 evidence map: HIGH — SUMMARY files verified; integration matrix from AUDIT file
- REQUIREMENTS.md diff plan: HIGH — current file content read and each row status confirmed
- Frontend wiring: HIGH — exact diff derived from reading current `page.tsx` and `audit-filters.tsx`
- Bruno convention: HIGH — all .bru files enumerated
- DEPL-03 record: HIGH — current `06-VERIFICATION.md` deferred-items table read directly

**Research date:** 2026-04-29
**Valid until:** 2026-05-29 (30-day window; all findings are from local codebase — no external dependency)
