# Phase 1: Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 01-foundation
**Areas discussed:** Database schema design, Auth & session strategy, API design conventions

---

## Database Schema Design

### Audit Logging Approach

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite triggers | Triggers fire on INSERT/UPDATE/DELETE, write to audit_log automatically. Can't be bypassed by app bugs | ✓ |
| Application-level middleware | Axum middleware intercepts mutations. More flexible but can be bypassed | |
| Hybrid: triggers + app context | Triggers capture raw change, app layer enriches with actor ID and justification | |

**User's choice:** SQLite triggers
**Notes:** Aligns with STATE.md decision: "Audit trail enforced via SQLite triggers, not application code only"

### ID Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| UUID v4 strings | Globally unique, safe for Turso sync, no collision risk | ✓ |
| Auto-increment integers | Compact, fast, but risky with Turso sync (ID collisions) | |
| ULID (sortable UUID) | Lexicographically sortable by creation time, globally unique | |

**User's choice:** UUID v4 strings

### Soft-Delete Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Status column (active/inactive) | Simple enum-style column, clear business meaning | |
| deleted_at timestamp | NULL = active, standard pattern | |
| Both: status + deleted_at | Status for business logic, deleted_at for audit trail timing | ✓ |

**User's choice:** Both: status column + deleted_at timestamp

### Row Versioning

| Option | Description | Selected |
|--------|-------------|----------|
| Version column on all mutable tables | Integer version increments on update, prevents lost-update problems | ✓ |
| Only on conflict-prone tables | Add version to employees, departments, daily_records, rules only | |
| No versioning, last-write-wins | Simpler schema, rely on audit log for conflict detection | |

**User's choice:** Version column on all mutable tables

---

## Auth & Session Strategy

### JWT Token Management

| Option | Description | Selected |
|--------|-------------|----------|
| Short-lived access + refresh token | Access (15-30 min) in memory, refresh (7 days) in httpOnly cookie | ✓ |
| Long-lived single JWT | One token with 7-day expiry in httpOnly cookie | |
| Session-based with server state | Server stores session in SQLite, client holds session ID cookie | |

**User's choice:** Short-lived access + refresh token

### Initial Admin Creation

| Option | Description | Selected |
|--------|-------------|----------|
| Environment variables | ADMIN_USERNAME/PASSWORD in .env, create on first boot | |
| Interactive setup wizard | First-time browser flow asks for admin credentials | ✓ |
| CLI seed command | `cronometrix seed-admin` command run manually | |

**User's choice:** Interactive setup wizard

### Password Policy

| Option | Description | Selected |
|--------|-------------|----------|
| Minimum 8 chars, no complexity rules | NIST 800-63B recommendation, length over complexity | ✓ |
| 8+ chars with complexity requirements | Traditional uppercase/lowercase/number/special | |
| You decide | Claude picks during implementation | |

**User's choice:** Minimum 8 chars, no complexity rules

### Viewer Role Data Access

| Option | Description | Selected |
|--------|-------------|----------|
| Full read access to all data | Viewer sees everything Admin sees, just can't edit | ✓ |
| Restricted: only aggregated/anonymized data | Stats and reports but not individual employee details | |
| Configurable per installation | Admin toggles what Viewer sees | |

**User's choice:** Full read access to all data

---

## API Design Conventions

### REST Naming Convention

| Option | Description | Selected |
|--------|-------------|----------|
| Resource-based with /api/v1 prefix | Versioned prefix, plural nouns, standard HTTP verbs | ✓ |
| Flat routes without version prefix | Simpler URLs, add versioning later if needed | |
| You decide | Claude picks based on Axum best practices | |

**User's choice:** Resource-based with /api/v1 prefix

### Error Response Format

| Option | Description | Selected |
|--------|-------------|----------|
| Structured JSON with error code | Machine-readable codes + human message | ✓ |
| Simple message-only | Minimal {error: "..."} format | |
| RFC 7807 Problem Details | Standard format but verbose | |

**User's choice:** Structured JSON with error code

### Pagination Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Offset-based with limit/offset | Simple, works with TanStack Table, good for <10K rows | ✓ |
| Cursor-based pagination | Better for large/real-time datasets, more complex | |
| No pagination in Phase 1 | Return all records, add later | |

**User's choice:** Offset-based with limit/offset params

### Timestamp Serialization

| Option | Description | Selected |
|--------|-------------|----------|
| ISO 8601 strings in API responses | Human-readable, date-fns parses natively | ✓ |
| UTC epoch integers everywhere | Consistent with DB storage, less developer-friendly | |
| You decide | Claude picks based on Axum/serde stack | |

**User's choice:** ISO 8601 strings in API responses

---

## Claude's Discretion

- Rust project module organization and Axum handler/service structure
- Config management approach
- Exact migration file structure and naming
- Setup wizard loading/error states

## Deferred Ideas

None — discussion stayed within phase scope
