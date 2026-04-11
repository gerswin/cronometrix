# Project Research Summary

**Project:** Cronometrix
**Domain:** Biometric Time & Attendance — Hikvision Facial Recognition, On-Premise + Cloud Sync Hybrid
**Researched:** 2026-04-11
**Confidence:** HIGH (stack and features), MEDIUM-HIGH (architecture and pitfalls)

## Executive Summary

Cronometrix is a specialized on-premise biometric time and attendance system built around Hikvision facial recognition terminals. The defining characteristic of this product type — and the most important architectural decision — is that events are not pushed to the backend by the device. Instead, the backend initiates and maintains a persistent outbound alertStream connection to each device. This pull model governs the entire device integration layer: every Hikvision DS-K series terminal delivers events over a long-lived GET stream that the backend must supervise, reconnect, and parse as multipart XML. Failure to understand this means missed events, phantom attendance gaps, and failed product validation.

The recommended approach is a Rust/Axum backend with local-first SQLite via the libSQL embedded replica crate, paired with a Next.js 15/React 19 frontend. This stack delivers the performance needed to handle concurrent alertStream connections and high-throughput event processing while keeping deployment simple (no PostgreSQL, no separate message broker). The offline-first persistence model using Turso cloud sync is a genuine product differentiator — it gives on-premise clients remote report access without a VPN — but Turso's offline sync is still beta-quality, so the design must treat local SQLite as the authoritative write target and cloud sync strictly as an async backup and read replica.

The key risks cluster around three areas: (1) Hikvision ISAPI is poorly documented and varies by device model and firmware — the XML parser must be permissive and the integration tested against every target device model before shipping; (2) time calculation correctness is non-trivial — UTC storage, overnight shift attribution, and 30-second duplicate event deduplication windows must all be built correctly from the first migration because retrofitting them is expensive; (3) the audit trail has legal weight — it must be enforced at the database level via triggers, not only in application code, or it will have gaps from direct DB access, sync conflicts, and future developer mistakes.

## Key Findings

### Recommended Stack

The backend is Rust 1.77+ with Axum 0.8.x on Tokio 1.x, using the libSQL crate for the embedded SQLite/Turso persistence layer. Reqwest + diqwest handle outbound ISAPI calls with digest auth. Quick-xml (with serde derives) handles the high-volume multipart XML event parsing from device alertStreams — it is 10x faster than serde-xml-rs with no disadvantage. JWT auth via the jsonwebtoken crate (not a wrapper) gives RBAC flexibility. Argon2id (RustCrypto) is mandatory for password hashing — bcrypt's 72-byte limit disqualifies it.

The frontend is Next.js 15 + React 19 with TanStack Table v8 and TanStack Query v5 for all data grids and server state. Shadcn/ui on Tailwind 4.x provides the component layer. React Hook Form + Zod handles all form validation. The key version constraint is tower-http 0.6.x — it must match Axum 0.8.x; version 0.5.x is incompatible.

**Core technologies:**
- Rust + Axum 0.8.x: HTTP server — memory-safe async with Tower middleware, ideal for long-held alertStream connections and concurrent webhook processing
- libSQL (Turso): persistence — local SQLite embedded replica with async WAL sync to Turso Cloud; local is always the write primary
- reqwest + diqwest: outbound ISAPI client — digest auth challenge-response required by Hikvision devices is handled automatically
- quick-xml: event parsing — permissive serde-integrated XML parser for multipart/mixed alertStream chunks
- Next.js 15 + TanStack Query v5: frontend — SSR dashboard, background refetch, App Router compatible; replaces all ad-hoc fetch/useState patterns
- TanStack Table v8 + shadcn/ui: data grid and components — headless, virtualizable, no UI lock-in

### Expected Features

The product has a well-defined MVP scope. The table stakes features are non-negotiable for a paying client: event capture via alertStream, first-entry/last-exit aggregation across all devices, shift-based work hours calculation with tolerance and lunch deduction, holiday calendar with salary surcharge percentages, an immutable audit trail with evidence file attachment, a timesheet editor with mandatory justification, employee directory and RBAC, and payroll period export in Excel and PDF. The real-time dashboard is also P1 — operators need device health and headcount visibility without navigating to a separate page.

The primary differentiators are multi-device facial profile sync (eliminating per-device enrollment), Turso cloud sync for off-site report access (unique in the on-premise market segment), and per-holiday salary surcharge configuration (required for Latin American labor law compliance). These belong in v1.x — after the core pipeline is validated — not in v1.

**Must have (table stakes):**
- Clock-in/clock-out event capture via alertStream — the entire product depends on this pipeline
- First-entry / last-exit aggregation across up to 4 devices — single daily presence record
- Work hours calculation (tolerance, lunch deduction, overtime) — the payroll-ready output
- Holiday calendar with salary surcharge percentages — payroll accuracy and labor law compliance
- Leave management (medical + manual adjustments) — payroll completeness
- Immutable audit trail with justification + evidence file upload — legal defensibility
- Employee directory (soft disable, no hard deletes) + 3-role RBAC — multi-user safety
- Facial enrollment (device camera + JPG upload) — required to use the hardware
- Device Manager with connection status monitoring — hardware integration foundation
- Payroll period export (Excel + PDF) — the primary deliverable clients pay for
- Real-time dashboard with KPIs and device health — operator situational awareness

**Should have (competitive differentiators, v1.x):**
- Multi-device facial profile sync — eliminates manual per-device enrollment; high admin value
- Turso cloud sync as client-facing feature — remote report access without VPN
- Live photo feed from device on access events — visual verification in dashboard
- ISAPI command dispatch (door open, reboot, enrollment mode) — reduces on-site support
- Bonus minutes and configurable tolerance sliders — reduces correction volume

**Defer (v2+):**
- Employee self-service portal — adds separate auth surface and mobile requirements; validate demand first
- Biometric GDPR deletion workflow — EU-specific; not the initial target geography
- Second biometric vendor support — only after Hikvision integration proves revenue
- Shift scheduling / roster management — different domain; 12-month project scope creep

### Architecture Approach

The architecture is a local-first monolith with a clean domain boundary. The Rust backend is organized into four independent layers: device/ (alertStream listener + ISAPI client), domain/ (pure business logic with no I/O), persistence/ (libSQL repository layer), and api/ (thin Axum handlers + middleware). A tokio broadcast channel decouples device listeners from the event processor service, which in turn calls the pure Attendance Engine for time calculations. A background Sync Manager task calls db.sync() every 60 seconds to replicate WAL frames to Turso Cloud. The frontend communicates exclusively with the Axum API over REST + SSE; the browser never touches the Hikvision device directly.

**Major components:**
1. Device Listener — one tokio task per device; maintains long-lived GET alertStream; reconnects with exponential backoff; emits RawEvent to broadcast channel
2. Event Processor — deduplicates events (idempotent on external_event_id), maps face_id to employee, persists AttendanceEvent, triggers Attendance Engine
3. Attendance Engine — pure domain service; first-entry/last-exit aggregation, tolerance windows, lunch deduction, holiday/leave overlay, anomaly flagging; upserts DailyRecord
4. HTTP API (Axum) — thin REST handlers, SSE dashboard stream, JWT + RBAC middleware; no business logic in handlers
5. Sync Manager — background task; local SQLite is always the write primary; cloud sync is async and failure-tolerant
6. Next.js Frontend — dashboard (SSE consumer), timesheet editor, employee directory, device manager, reports — all via TanStack Query

### Critical Pitfalls

1. **Silent alertStream disconnect** — the device drops TCP without signaling; implement a per-device supervisor task with last-event-at heartbeat tracking, exponential backoff reconnection, and a dashboard alert when no event arrives in >5 minutes during business hours.
2. **UTC timestamps — never local time** — parse Hikvision timestamps as DateTime<FixedOffset> and immediately convert to UTC epoch integers for storage; DST edge cases and overnight shifts are impossible to fix retroactively.
3. **Duplicate event deduplication must span all devices** — a 30-second idempotency window on (employee_id, epoch/30) prevents double-counting when multiple readers cover the same door; this must be in the database constraint, not just application logic.
4. **Audit trail must be database-enforced** — SQLite triggers on AFTER UPDATE/DELETE on sensitive tables are the primary enforcement; application-layer calls are secondary; add a hash chain per entry for tamper detection.
5. **Turso sync conflict resolution is absent (beta)** — treat local SQLite as the exclusive write primary; never write directly to the Turso remote URL; add row versioning so conflicts are detectable rather than silently lost.
6. **XML event format varies by device model and firmware** — use a permissive parser that never fails on unknown fields; map both employeeNo (integer) and employeeNoString (string) to a normalized string; log raw XML of unrecognized events for analysis.

## Implications for Roadmap

Based on combined research, the natural phase structure follows the architectural build order identified in ARCHITECTURE.md, with pitfall prevention woven into each phase.

### Phase 1: Foundation — Database, Config, Auth

**Rationale:** Every other component reads and writes the database. UTC timestamp handling, audit trigger schema, and row versioning must be correct from migration zero — these cannot be retrofitted. Auth is a prerequisite for any protected API endpoint.
**Delivers:** Runnable Rust service with libSQL embedded replica, Turso sync wired, migrations, JWT issuance and validation, RBAC middleware, basic user management.
**Addresses:** Employee directory (partial), RBAC (Admin/Supervisor/Viewer)
**Avoids:** UTC timestamp pitfall, Turso conflict pitfall, application-only audit enforcement pitfall

### Phase 2: Device Integration — alertStream + Device Manager

**Rationale:** The event pipeline is the product's entire data source. Nothing downstream works without real attendance events flowing into the database.
**Delivers:** Device Manager UI with credential storage (encrypted), alertStream listener tasks with supervisor/reconnect loop, multipart XML parser with permissive field mapping, AttendanceEvent persistence, device health status on dashboard.
**Addresses:** Clock-in/clock-out event capture, Device Manager, device connection status monitoring
**Avoids:** Silent disconnect pitfall, XML variation pitfall, plain-text credential pitfall, webhook endpoint security pitfall, duplicate event pitfall (idempotency window at insert)

### Phase 3: Time Calculation Engine — Business Logic Core

**Rationale:** The Attendance Engine is the product's core value. It must be built as pure domain logic, fully testable without I/O, before the API exposes it. Holiday and leave tables are required inputs for correct calculation.
**Delivers:** First-entry/last-exit aggregation, tolerance windows, lunch deduction (fixed or punch-based per department), overtime calculation, holiday surcharge classification, leave overlay, overnight shift attribution (anchor-date model), DailyRecord persistence.
**Addresses:** Work hours calculation, overtime, late/early detection, holiday calendar, leave management
**Avoids:** Midnight shift wrong-day attribution, duplicate aggregation across devices, performance trap of recalculating on every dashboard load (materialize DailyRecord)

### Phase 4: HTTP API — REST Endpoints + SSE Dashboard

**Rationale:** The API surface must be stable before the frontend is built against it. Thin handlers call services; no business logic in handlers.
**Delivers:** Full REST API for employees, departments, devices, attendance, timesheet adjustment (with audit write in same transaction), SSE dashboard stream, payroll period report endpoints.
**Addresses:** All API-exposed features from Phase 1–3, real-time dashboard, RBAC enforcement at endpoint level
**Avoids:** Business logic in handlers anti-pattern; audit-append write pattern enforced at service layer

### Phase 5: Frontend UI — Admin Dashboard and Core Screens

**Rationale:** Frontend is built against the stable Phase 4 API. TanStack Query manages all server state — no ad-hoc fetch/useState.
**Delivers:** Dashboard (SSE consumer, device health banners), Timesheet Editor (justification required field, evidence upload), Employee Directory, Device Manager UI, Attendance grid (TanStack Table with virtualization).
**Addresses:** All operator-facing workflows for table stakes features
**Avoids:** Showing raw UTC timestamps (convert in presentation layer), hiding device offline status (surface as prominent dashboard banner), optional justification field (must be required + minimum length)

### Phase 6: Reports and Payroll Export

**Rationale:** Reports require correct calculations from Phase 3 and stable API from Phase 4. Excel/PDF generation is a separate concern that should not block core pipeline validation.
**Delivers:** Payroll period export (Excel primary, PDF secondary), audit trail panel (Admin-only read), configurable payroll period (weekly/bi-weekly/monthly).
**Addresses:** Payroll period export, audit trail panel, pre-payroll data deliverable
**Avoids:** Loading all audit log rows in memory (use OFFSET/LIMIT with index on created_at)

### Phase 7: Facial Enrollment and Profile Sync

**Rationale:** Enrollment requires both a working ISAPI client (Phase 2) and a UI (Phase 5). Multi-device sync is complex enough to be its own phase after the single-device path is validated.
**Delivers:** Enrollment modal (device camera / JPG upload with quality score gate), per-device enrollment status display, multi-device profile push with concurrent tokio tasks and per-device success/failure status.
**Addresses:** Facial enrollment, multi-device profile sync (v1.x differentiator)
**Avoids:** Enrollment modal closing without showing per-device status, accepting low-quality enrollment photos, blocking UI on synchronous multi-device push

### Phase Ordering Rationale

- Foundation before everything else: UTC schema and audit triggers cannot be added after data exists without a costly migration.
- Device integration before business logic: the Attendance Engine needs real event flows to validate correctness; pure unit tests are necessary but not sufficient.
- Business logic before API: the domain must be correct before it is exposed; thin handlers only.
- API before frontend: building a UI against an unstable API wastes frontend iterations.
- Reports after core pipeline: payroll export is the product's deliverable but it is last in the dependency chain — it correctly lands in Phase 6.
- Enrollment last: it requires both the ISAPI client and the UI; its complexity deserves isolation after the core pipeline is proven.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 2 (Device Integration):** Hikvision alertStream protocol has sparse public documentation and device-model variation. Recommend capturing real alertStream traffic from each target device model (DS-K1T341, DS-K1T342) before implementation. Reference: peku33/HikVision-EventReceiver for real-world parsing patterns.
- **Phase 3 (Time Calculation Engine):** Overnight shift anchor-date model and DST edge cases for the target region (Mexico) need explicit test fixture construction. DST dates vary by state within Mexico — confirm which timezone applies.
- **Phase 7 (Enrollment + Sync):** ISAPI batch face profile enrollment API (`PUT /ISAPI/AccessControl/UserInfo/SetUp`) behavior on partial failure (e.g., 3 of 4 devices succeed) is undocumented. Needs hands-on testing with physical hardware.

Phases with standard patterns (research phase can be skipped):

- **Phase 1 (Foundation):** Axum + libSQL + JWT is well-documented with official guides; standard Rust project setup.
- **Phase 4 (HTTP API):** Axum REST + SSE is a documented pattern; RBAC middleware pattern is established for this stack.
- **Phase 5 (Frontend):** Next.js 15 + TanStack Query + shadcn/ui has official integration guides; standard App Router patterns apply.
- **Phase 6 (Reports):** Excel generation via Rust (rust_xlsxwriter) and client-side PDF (jspdf-autotable) are well-documented; no novel patterns required.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core Rust/Axum/libSQL/Next.js stack verified against current official docs; all versions confirmed |
| Features | HIGH | Feature landscape cross-referenced against 4 competitors and multiple industry guides; MVP scope well-validated |
| Architecture | HIGH (components), MEDIUM (Turso sync) | Component design follows established Rust DDD patterns; Turso offline sync is beta-quality with documented limitations |
| Pitfalls | MEDIUM-HIGH | Most pitfalls verified against multiple sources; Hikvision ISAPI internals remain LOW confidence due to closed proprietary protocol and sparse public documentation |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **Hikvision device model compatibility:** XML schema variations between DS-K1T341, DS-K1T342, and DS-K1T604 (if used) need hands-on testing during Phase 2. Do not rely on documentation alone.
- **Turso offline sync durability:** Beta status means behavior may change before v1 launch. Design the sync manager so it can be disabled (falling back to local-only SQLite) without data loss. Validate sync reliability internally before enabling as a client-facing feature.
- **Mexico DST timezone boundaries:** Confirm which IANA timezone applies to the initial deployment region; some Mexican states observe CST/CDT, others MST/MDT, and some do not observe DST at all. This affects overnight shift test fixtures.
- **ISAPI batch enrollment failure handling:** Behavior when enrolling a face profile to 4 devices and one fails mid-batch is not publicly documented. Test retry and rollback behavior with physical hardware before designing the enrollment modal.
- **Report column mapping:** The specific Excel column layout expected by the target payroll system (if any) needs to be confirmed with the first client before Phase 6 begins.

## Sources

### Primary (HIGH confidence)
- [Axum 0.8.8 docs.rs](https://docs.rs/axum/latest/axum/) — routing, extractors, middleware
- [Tokio 1.51.1 docs.rs](https://docs.rs/tokio/latest/tokio/) — async runtime
- [reqwest 0.13.2 docs.rs](https://docs.rs/reqwest/latest/reqwest/) — HTTP client
- [jsonwebtoken 10.3.0 docs.rs](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/) — JWT
- [Turso Rust Quickstart](https://docs.turso.tech/sdk/rust/quickstart) — embedded replica API
- [Turso Embedded Replicas Introduction](https://docs.turso.tech/features/embedded-replicas/introduction) — sync model
- [RustCrypto password-hashes](https://github.com/RustCrypto/password-hashes) — argon2 crate
- [TanStack Table v8](https://tanstack.com/table/latest) — headless data grid
- [TanStack Query v5](https://tanstack.com/query/latest) — server state, SSR hydration
- [shadcn/ui Data Table docs](https://ui.shadcn.com/docs/components/radix/data-table) — TanStack Table integration pattern
- [react-hook-form + Zod shadcn guide](https://ui.shadcn.com/docs/forms/react-hook-form) — form validation pattern

### Secondary (MEDIUM confidence)
- [Hikvision ISAPI Event Listening PDF](https://www.hikvisioneurope.com/eu/portal/portal/Technology%20Partner%20Program/03-How%20to/How%20to%20get%20real-time%20event%20in%20listening%20mode.pdf) — alertStream multipart event format
- [Hikvision TPP Integration Center](https://tpp.hikvision.com/) — digest auth requirement, ISAPI endpoints
- [Turso Offline Sync Public Beta](https://turso.tech/blog/turso-offline-sync-public-beta) — conflict resolution not implemented, durability caveats
- [diqwest crate docs](https://docs.rs/diqwest) — digest auth reqwest extension
- [peku33/HikVision-EventReceiver](https://github.com/peku33/HikVision-EventReceiver) — alertStream connection handling and multipart parsing patterns
- [Hexagonal Architecture in Rust](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust) — domain layer isolation pattern
- [Recharts v3 changelog](https://blog.logrocket.com/best-react-chart-libraries-2025/) — v3 release mid-2025

### Tertiary (LOW confidence)
- [tauri-plugin-libsql](https://dev.to) — Tauri desktop wrapper integration with libSQL (future milestone only)
- Hikvision DS-K1T342 Value Series ISAPI guide (2024) — field name variants between device models; not fully cross-referenced with Pro Series guide

---
*Research completed: 2026-04-11*
*Ready for roadmap: yes*
