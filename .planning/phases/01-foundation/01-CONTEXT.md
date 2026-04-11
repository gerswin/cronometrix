# Phase 1: Foundation - Context

**Gathered:** 2026-04-11
**Status:** Ready for planning

<domain>
## Phase Boundary

A running Rust service with correct database schema, authentication, and core data entities so every downstream phase has a stable, auditable data layer to build on. Covers: database migrations, auth (JWT + RBAC), employee CRUD, department CRUD, global rules API, and audit logging.

</domain>

<decisions>
## Implementation Decisions

### Database Schema Design
- **D-01:** Audit logging via SQLite triggers — triggers fire on INSERT/UPDATE/DELETE and write to an audit_log table automatically; cannot be bypassed by application bugs
- **D-02:** UUID v4 strings for all primary keys — globally unique, safe for Turso sync across independent installations, no collision risk
- **D-03:** Soft-delete uses both a status column (active/inactive) and a deleted_at timestamp — status for business logic, deleted_at for audit trail of when deactivation happened
- **D-04:** Version column on all mutable tables for optimistic concurrency — integer increments on update, prevents lost-update problems, essential for Turso sync conflict detection
- **D-05:** All timestamps stored as UTC epoch integers in the database (established in STATE.md)

### Auth & Session Strategy
- **D-06:** Short-lived access token (15-30 min) in memory + refresh token (7 days) in httpOnly cookie — refresh silently on expiry
- **D-07:** Initial admin created via interactive setup wizard on first boot — browser flow asks for admin credentials when no users exist
- **D-08:** Password policy: minimum 8 characters, no complexity rules — follows NIST 800-63B recommendation; Argon2id hashing
- **D-09:** Viewer role has full read access to all data — 3 roles map to: Admin (full access), Supervisor (edit subset), Viewer (read-only everything)

### API Design Conventions
- **D-10:** Resource-based REST with /api/v1 prefix — plural nouns, standard HTTP verbs (GET /api/v1/employees, POST /api/v1/employees, PATCH /api/v1/employees/:id)
- **D-11:** Structured JSON error responses with machine-readable error codes — `{"error": {"code": "EMPLOYEE_NOT_FOUND", "message": "...", "status": 404}}`
- **D-12:** Offset-based pagination with limit/offset query params — works with TanStack Table's built-in pagination; sufficient for datasets under 10K rows
- **D-13:** Timestamps serialized as ISO 8601 strings in API responses (e.g., "2026-04-11T14:30:00Z") — stored as UTC epoch integers in DB, converted on serialization

### Claude's Discretion
- Rust project module organization and Axum handler/service structure
- Config management approach (dotenvy, environment loading)
- Exact migration file structure and naming
- Loading skeleton and error state patterns for the setup wizard
- Compression algorithm and temp file handling

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

No external specs — requirements fully captured in decisions above and in:

### Project-level
- `.planning/REQUIREMENTS.md` — Full v1 requirement list; Phase 1 covers DATA-01 through DATA-04, AUTH-01 through AUTH-05, EMP-01 through EMP-04, DEPT-01 through DEPT-03, RULE-01 through RULE-03
- `.planning/PROJECT.md` — Constraints, key decisions, tech stack rationale
- `.planning/STATE.md` — Accumulated context including UTC epoch storage decision and SQLite trigger audit decision

### Stack reference
- `CLAUDE.md` — Full technology stack table with version constraints, crate versions, and compatibility matrix

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing code — greenfield project. All patterns established in this phase become the foundation for Phases 2-7.

### Established Patterns
- None yet. This phase defines: module structure, error handling, database access patterns, auth middleware, and CRUD conventions.

### Integration Points
- Database: libSQL with `Builder::new_remote_replica()` for Turso sync
- Auth: JWT middleware applied per Axum router group based on role
- API: All endpoints under /api/v1 namespace; downstream phases add route groups to the same router

</code_context>

<specifics>
## Specific Ideas

- Setup wizard on first boot for admin creation — frontend must handle the "no users exist" state gracefully
- Audit triggers should be comprehensive from migration zero — every table that stores mutable business data gets trigger coverage
- Version column pattern should be consistent: all mutable tables, same column name, same increment behavior

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-04-11*
