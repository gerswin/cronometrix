# Architecture Research

**Domain:** Biometric Time & Attendance — Hybrid On-Premise System
**Researched:** 2026-04-11
**Confidence:** HIGH (component design), MEDIUM (Turso sync — beta-quality caveats apply)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          DEVICE LAYER                                    │
│   ┌────────────────┐  ┌────────────────┐  ┌────────────────┐             │
│   │  Hikvision #1  │  │  Hikvision #2  │  │  Hikvision #3  │  (up to 4) │
│   │  Face Terminal │  │  Face Terminal │  │  Face Terminal │             │
│   └───────┬────────┘  └───────┬────────┘  └───────┬────────┘             │
│           │  alertStream      │  alertStream       │  alertStream         │
│           │  (GET, long-held) │                    │                      │
└───────────┼───────────────────┼────────────────────┼──────────────────────┘
            │                   │                    │
┌───────────▼───────────────────▼────────────────────▼──────────────────────┐
│                          BACKEND (Rust / Axum)                            │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  Device Listener (outbound polling clients, one task per device)   │  │
│  │  - Maintains long-lived GET /ISAPI/Event/notification/alertStream  │  │
│  │  - Parses multipart/mixed XML EventNotificationAlert chunks        │  │
│  │  - Emits internal RawEvent to the Event Bus (tokio broadcast)      │  │
│  └───────────────────────────────┬─────────────────────────────────────┘  │
│                                  │ RawEvent                               │
│  ┌───────────────────────────────▼─────────────────────────────────────┐  │
│  │  Event Processor (domain service)                                  │  │
│  │  - Deduplicates (device re-sends on reconnect)                     │  │
│  │  - Maps employee face ID → employee record                         │  │
│  │  - Determines device direction (entry / exit)                      │  │
│  │  - Stores AttendanceEvent row (idempotent on external_event_id)    │  │
│  │  - Triggers shift pairing via Attendance Engine                    │  │
│  └───────────────────────────────┬─────────────────────────────────────┘  │
│                                  │                                        │
│  ┌───────────────────────────────▼─────────────────────────────────────┐  │
│  │  Attendance Engine (domain service)                                │  │
│  │  - Applies first-entry / last-exit rule across devices per shift  │  │
│  │  - Calculates net worked minutes with tolerance (±N min)           │  │
│  │  - Deducts configured lunch time per department                    │  │
│  │  - Handles holidays, medical leave, manual adjustments            │  │
│  │  - Flags anomalies (no exit, early leave, etc.)                   │  │
│  │  - Writes DailyRecord + WorkMinutes to persistence                │  │
│  └───────────────────────────────┬─────────────────────────────────────┘  │
│                                  │                                        │
│  ┌───────────────────────────────▼─────────────────────────────────────┐  │
│  │  HTTP API (Axum router)                                            │  │
│  │  - REST endpoints for CRUD (employees, departments, devices, etc.) │  │
│  │  - SSE endpoint for real-time dashboard push                       │  │
│  │  - ISAPI command proxy (door open, reboot, enrollment mode)        │  │
│  │  - JWT auth + RBAC middleware (Admin / Supervisor / Viewer)        │  │
│  │  - Report generation (Excel / PDF) on demand                      │  │
│  └───────────────────────────────┬─────────────────────────────────────┘  │
│                                  │                                        │
│  ┌───────────────────────────────▼─────────────────────────────────────┐  │
│  │  Sync Manager (background task)                                    │  │
│  │  - Calls libSQL db.sync() on interval (e.g., every 60s)           │  │
│  │  - Handles offline gracefully — queues locally, flushes on resume  │  │
│  └───────────────────────────────┬─────────────────────────────────────┘  │
└──────────────────────────────────┼──────────────────────────────────────── ┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────────────┐
│                      PERSISTENCE LAYER                                      │
│                                                                             │
│  ┌──────────────────────────────────┐  ┌──────────────────────────────────┐ │
│  │  SQLite (local, embedded)        │  │  Turso Cloud (libSQL replica)    │ │
│  │  - All writes land here first    │  │  - Async sync via WAL frames     │ │
│  │  - Fast file-based reads         │  │  - Remote access + backup        │ │
│  │  - libSQL embedded replica mode  │  │  - Pull on startup, push on sync │ │
│  └──────────────────────────────────┘  └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────────────┐
│                   FRONTEND (Next.js / React)                                │
│                                                                             │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌──────────┐  │
│  │ Dashboard │  │Timesheet  │  │ Employees │  │ Devices   │  │ Reports  │  │
│  │  (SSE)    │  │  Editor   │  │ Directory │  │  Manager  │  │& Payroll │  │
│  └───────────┘  └───────────┘  └───────────┘  └───────────┘  └──────────┘  │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Shared: Auth context, RBAC guards, API client (fetch + React Query)│   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Device Listener | Maintains long-lived alertStream connection per device; parses multipart/mixed XML; emits internal events | One tokio task per device; reconnects on drop |
| Event Processor | Deduplication, face-ID → employee mapping, direction resolution, AttendanceEvent persistence | Idempotent on `external_event_id` from device |
| Attendance Engine | First-entry/last-exit pairing, shift rule application, anomaly flagging, DailyRecord writes | Pure business logic; testable without I/O |
| ISAPI Command Client | Outbound HTTP client wrapping ISAPI: door open, reboot, enrollment sync, face profile push | Per-device, credential-scoped |
| HTTP API | Axum router — REST, SSE, RBAC middleware, report endpoints | Exposes no internal event types directly |
| Sync Manager | Background task calling `db.sync()` on interval; handles offline gracefully | Must survive network absence indefinitely |
| SQLite / libSQL | All application writes; local-first source of truth | Embedded replica — remote is backup, not primary |
| Next.js Frontend | Admin UI — all screens are server-rendered or SPA depending on need | Communicates only with Axum API |

## Recommended Project Structure

```
cronometrix/
├── backend/                        # Rust workspace crate
│   ├── src/
│   │   ├── main.rs                 # App bootstrap, tokio runtime, router, background tasks
│   │   ├── config.rs               # Config loading (env / file)
│   │   │
│   │   ├── device/                 # Device layer
│   │   │   ├── mod.rs
│   │   │   ├── listener.rs         # alertStream long-lived client (one task per device)
│   │   │   ├── isapi_client.rs     # Outbound ISAPI commands (door, reboot, enrollment)
│   │   │   ├── parser.rs           # Multipart/mixed XML → RawEvent
│   │   │   └── types.rs            # RawEvent, DeviceConfig
│   │   │
│   │   ├── domain/                 # Pure business logic — no I/O dependencies
│   │   │   ├── mod.rs
│   │   │   ├── attendance/
│   │   │   │   ├── engine.rs       # first-entry/last-exit, minute calc, tolerance
│   │   │   │   ├── rules.rs        # Tolerance config, lunch deduction, holiday logic
│   │   │   │   └── types.rs        # AttendanceEvent, DailyRecord, Shift
│   │   │   ├── employee/
│   │   │   │   └── types.rs        # Employee, Department, FaceProfile
│   │   │   └── audit/
│   │   │       └── types.rs        # AuditEntry (immutable)
│   │   │
│   │   ├── persistence/            # DB layer — libSQL / SQLite
│   │   │   ├── mod.rs
│   │   │   ├── db.rs               # Connection pool, migrations
│   │   │   ├── repositories/
│   │   │   │   ├── attendance.rs
│   │   │   │   ├── employee.rs
│   │   │   │   ├── device.rs
│   │   │   │   └── audit.rs
│   │   │   └── sync.rs             # Turso sync manager (background task)
│   │   │
│   │   ├── api/                    # Axum HTTP layer
│   │   │   ├── mod.rs
│   │   │   ├── router.rs           # Route registration
│   │   │   ├── middleware/
│   │   │   │   ├── auth.rs         # JWT extraction + validation
│   │   │   │   └── rbac.rs         # Role guard extractor
│   │   │   └── handlers/
│   │   │       ├── dashboard.rs    # SSE stream for live KPIs
│   │   │       ├── attendance.rs
│   │   │       ├── employees.rs
│   │   │       ├── devices.rs
│   │   │       ├── timesheet.rs
│   │   │       ├── reports.rs
│   │   │       └── auth.rs
│   │   │
│   │   └── services/               # Application services (orchestration)
│   │       ├── event_processor.rs  # Wires RawEvent → domain → persistence
│   │       ├── enrollment.rs       # Face profile sync across all devices
│   │       └── report_builder.rs   # Excel/PDF generation
│   │
│   ├── migrations/                 # SQL migration files (numbered)
│   └── Cargo.toml
│
└── frontend/                       # Next.js application
    ├── src/
    │   ├── app/                    # Next.js App Router pages
    │   │   ├── (auth)/             # Login route group
    │   │   └── (admin)/            # Protected route group
    │   │       ├── dashboard/
    │   │       ├── employees/
    │   │       ├── attendance/
    │   │       ├── devices/
    │   │       ├── reports/
    │   │       └── settings/
    │   ├── components/             # Shared UI components
    │   ├── lib/
    │   │   ├── api.ts              # Typed fetch client
    │   │   └── auth.ts             # Session / JWT handling
    │   └── hooks/                  # React Query hooks per domain
    └── package.json
```

### Structure Rationale

- **device/**: Isolated from business logic. Protocol changes (ISAPI version, XML schema) touch only this folder.
- **domain/**: Zero infrastructure dependencies. Allows unit testing attendance rules without database or network.
- **persistence/**: All SQL lives here. Swapping SQLite for another DB only touches this layer.
- **api/**: Thin handlers — validate input, call service, return response. No business logic in handlers.
- **services/**: Orchestration glue — coordinates domain + persistence + device layers for multi-step operations.

## Architectural Patterns

### Pattern 1: Outbound alertStream Polling (Pull Model for Events)

**What:** The backend initiates a persistent GET connection to each device's alertStream endpoint. The device pushes XML event chunks over the long-held connection as `multipart/mixed` with a heartbeat every 5s. The backend never waits for the device to push a webhook to it.

**When to use:** Always — Hikvision devices do not support inbound webhook configuration in the access control terminal product line. The device is the server; the backend is the client.

**Trade-offs:** Requires the backend to know all device IPs at startup. Must handle reconnection when devices reboot or the network drops. No firewall inbound rule needed for the backend machine.

**Implementation sketch:**
```rust
// One tokio task per device — spawned on startup and on device registration
async fn listen_device(device: DeviceConfig, tx: broadcast::Sender<RawEvent>) {
    loop {
        match connect_alert_stream(&device).await {
            Ok(stream) => parse_multipart_stream(stream, &tx).await,
            Err(e) => {
                tracing::warn!("Device {} disconnected: {e}", device.id);
                tokio::time::sleep(RECONNECT_BACKOFF).await;
            }
        }
    }
}
```

### Pattern 2: First-Entry / Last-Exit Aggregation

**What:** When an employee badge event arrives from any device, it is stored as an `AttendanceEvent`. At the end of a shift window (or on demand for reports), the Attendance Engine queries all events for an employee within the shift period and selects the chronologically earliest entry-direction event and the latest exit-direction event.

**When to use:** This is the core business rule for calculating presence. Applied on DailyRecord creation and re-applied on manual adjustment.

**Trade-offs:** Simple and correct for single-shift days. Requires device direction config (entry vs exit) to be accurate. Multi-shift or split-shift days need additional shift-window bounds to avoid cross-day contamination.

**Implementation sketch:**
```rust
fn resolve_daily_record(events: &[AttendanceEvent], shift: &ShiftConfig) -> DailyRecord {
    let entries = events.iter().filter(|e| e.direction == Direction::Entry);
    let exits   = events.iter().filter(|e| e.direction == Direction::Exit);
    let first_entry = entries.min_by_key(|e| e.occurred_at);
    let last_exit   = exits.max_by_key(|e| e.occurred_at);
    DailyRecord::from_bounds(first_entry, last_exit, shift)
}
```

### Pattern 3: Audit-Append Writes

**What:** Every mutation to timesheet data (attendance records, manual adjustments) writes both the mutation and an immutable `AuditEntry` in a single database transaction. Audit entries are never updated or deleted — only appended.

**When to use:** Every admin-triggered data change. Enforced at the service layer, not optional.

**Trade-offs:** Audit table grows unbounded — partition or archive annually. Justification files (PDF/JPG) stored on local disk with path recorded in the audit entry.

**Implementation sketch:**
```rust
async fn adjust_timesheet(db: &Db, adjustment: Adjustment, actor: &User) -> Result<()> {
    let mut tx = db.begin().await?;
    update_daily_record(&mut tx, &adjustment).await?;
    insert_audit_entry(&mut tx, AuditEntry::from_adjustment(&adjustment, actor)).await?;
    tx.commit().await?;
    Ok(())
}
```

### Pattern 4: Offline-First with Turso Embedded Replica

**What:** The backend uses the libSQL `Builder::new_remote_replica()` to open a local SQLite file that periodically syncs with a Turso Cloud database. All reads and writes go to the local file. A background task calls `db.sync().await` on a configurable interval (60–300 seconds). On startup, a pull sync loads any changes from cloud (important after offline periods).

**When to use:** Always — this is the fundamental persistence model for Cronometrix.

**Trade-offs:** Turso Offline Sync is beta-quality as of April 2026. No bi-directional conflict resolution is documented — remote is treated as the replication destination, not a second write primary. Multiple on-premise installations writing to the same Turso database simultaneously would create conflicts (not applicable here — each client has their own Turso database).

**Implementation sketch:**
```rust
let db = Builder::new_remote_replica("file:cronometrix.db", turso_url, turso_token)
    .build()
    .await?;

// Background sync task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Err(e) = db.sync().await {
            tracing::warn!("Sync failed (offline?): {e}");
        }
    }
});
```

## Data Flow

### Flow 1: Biometric Event to Stored Attendance Record

```
Hikvision Device
    │  GET /ISAPI/Event/notification/alertStream (long-held)
    │  → multipart/mixed XML chunks pushed by device
    │
Device Listener (Rust async task)
    │  parse_multipart_chunk() → EventNotificationAlert XML
    │  → extract: face_id, device_id, timestamp, direction
    │
    ├─ broadcast::Sender<RawEvent>
    │
Event Processor (service)
    │  deduplicate by external_event_id (idempotent insert)
    │  resolve face_id → employee_id (lookup table)
    │  persist AttendanceEvent row to SQLite
    │
Attendance Engine (domain service, called by Event Processor)
    │  load today's AttendanceEvents for employee
    │  apply first-entry / last-exit rule
    │  apply tolerance, lunch deduction, holiday table
    │  upsert DailyRecord (worked_minutes, flags)
    │
SQLite (local file)
    │  written immediately, available for API reads
    │
Sync Manager (background)
    │  db.sync() → WAL frames pushed to Turso Cloud
    │
Turso Cloud
    └─ Remote replica updated (accessible from anywhere)
```

### Flow 2: Manual Timesheet Adjustment (Audit-Safe)

```
Supervisor/Admin in UI
    │  POST /api/timesheet/{id}/adjust  { minutes, justification_file_id }
    │
RBAC Middleware
    │  validates JWT, confirms role >= Supervisor
    │
Axum Handler
    │  validates input, calls TimeSheetService::adjust()
    │
TimeSheetService (service layer)
    │  BEGIN TRANSACTION
    │  1. UPDATE daily_records SET worked_minutes = ?
    │  2. INSERT INTO audit_log (record_id, actor, before, after, justification)
    │  COMMIT
    │
SQLite → Sync Manager → Turso Cloud
```

### Flow 3: Payroll Report Generation

```
Admin requests report  POST /api/reports/payroll { period, department_id }
    │
Report Builder (service)
    │  query DailyRecords for period + department
    │  join Employees, Departments (base salary, lunch mode)
    │  join Holidays (surcharge multipliers)
    │  join LeaveRequests (medical, vacations)
    │
Calculation
    │  for each employee:
    │    sum(worked_minutes) + adjustments - leaves
    │    apply holiday surcharges
    │    derive payable hours + deductions
    │
Output
    │  Excel via rust_xlsxwriter or calamine
    │  PDF via printpdf or headless Chrome (chromium call)
    │
    └─ File returned as binary download to browser
```

### Flow 4: Real-Time Dashboard (SSE)

```
Browser opens SSE connection  GET /api/dashboard/stream
    │
Axum SSE handler
    │  subscribe to broadcast::Receiver<DashboardEvent>
    │  streams JSON events as they arrive
    │
Event Processor (on new AttendanceEvent)
    │  publishes DashboardEvent { employee, device, time, photo_url }
    │  to broadcast channel
    │
Browser renders
    │  live attendance feed, device status, KPI counters updated in-place
```

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Hikvision Device (alertStream) | Outbound GET long-poll from backend to device | Backend is HTTP client; device is HTTP server; basic auth |
| Hikvision Device (ISAPI commands) | Outbound POST/PUT from backend to device | Door open, reboot, face profile CRUD — synchronous calls |
| Turso Cloud | libSQL embedded replica sync via WAL frames | Periodic background push/pull; no Turso SDK beyond libSQL crate |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Device Listener → Event Processor | tokio `broadcast::channel<RawEvent>` | Decoupled; processor can be restarted without losing listener |
| Event Processor → Attendance Engine | Direct function call (same process) | Engine is pure — takes data, returns result, no I/O |
| Attendance Engine → Persistence | Repository trait abstraction | Allows mock in unit tests |
| API Handlers → Services | Direct async fn call via shared `Arc<AppState>` | State injected via Axum `State` extractor |
| Backend → Frontend | HTTP REST + SSE (no WebSockets) | SSE sufficient for one-way push; no bidirectional needed |

## Build Order (Phase Dependencies)

The following order respects hard dependencies between components:

```
Phase 1 — Foundation
    Database schema + migrations
    libSQL embedded replica + Turso sync wiring
    Config loading (device IPs, Turso URL/token, JWT secret)
    Reason: Everything else reads/writes the DB

Phase 2 — Device Integration
    Device Listener (alertStream client + multipart parser)
    ISAPI Command Client (door, reboot)
    Requires: Phase 1 (persistence for event storage)
    Reason: Cannot process events without somewhere to store them

Phase 3 — Business Logic Core
    Attendance Engine (first-entry/last-exit, minute calc, tolerance)
    Holiday / Leave tables and rules
    Audit-append write pattern
    Requires: Phase 1, Phase 2 (raw events must flow first)
    Reason: Engine needs real events to validate rule correctness

Phase 4 — HTTP API + Auth
    Axum router, JWT middleware, RBAC guards
    REST endpoints for employees, departments, devices, attendance
    SSE dashboard stream
    Requires: Phase 1, Phase 3 (domain must be correct before exposing)

Phase 5 — Frontend UI
    Next.js project with API client
    Dashboard, Timesheet Editor, Employee Directory, Device Manager
    Requires: Phase 4 (API must be stable to build against)

Phase 6 — Reports & Export
    Payroll pre-export (Excel/PDF)
    Audit trail panel
    Requires: Phase 3 (calculations must be complete)

Phase 7 — Enrollment & Profile Sync
    Facial profile push across multiple devices simultaneously
    Enrollment modal (device camera / webcam / JPG upload)
    Requires: Phase 2 (ISAPI client) + Phase 5 (UI)
```

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1–4 devices, <200 employees | Current design is fine — single process, SQLite, all in-memory event bus |
| 5–20 devices | Increase listener task pool; consider per-device reconnection budget; SQLite WAL handles concurrent reads well |
| 50+ devices | alertStream polling becomes I/O-heavy; consider grouping devices behind a dedicated listener service; evaluate PostgreSQL over SQLite |

### Scaling Priorities

1. **First bottleneck:** alertStream listener tasks — each device consumes a long-lived TCP connection + tokio task. At 4 devices this is trivial. At 50 it needs careful resource accounting.
2. **Second bottleneck:** SQLite write contention during high-traffic check-in peaks (shift start/end). WAL mode handles concurrent reads; writes are serialized. For high-frequency events, batch-insert with a short buffer (100ms aggregation window) before committing.

## Anti-Patterns

### Anti-Pattern 1: Treating Turso Cloud as the Write Primary

**What people do:** Send writes directly to Turso remote URL, treating it like a hosted database, then hoping local SQLite stays in sync.

**Why it's wrong:** Violates the offline-first guarantee. If the network is down and writes go to remote, the local app is blocked.

**Do this instead:** All writes go to local SQLite via the libSQL embedded replica. Turso Cloud receives those writes asynchronously via `.sync()`. The backend is always operational regardless of cloud reachability.

### Anti-Pattern 2: Placing Business Rules in HTTP Handlers

**What people do:** Calculating worked minutes, tolerance windows, or holiday surcharges inside Axum handler functions.

**Why it's wrong:** Logic becomes impossible to unit test (requires HTTP context), duplicates across endpoints, and tightly couples protocol to domain.

**Do this instead:** Handlers validate input and call a service function. The service calls a pure domain function. Domain functions take typed structs, return typed results, no I/O.

### Anti-Pattern 3: Using Webhooks (Inbound) for Hikvision Events

**What people do:** Configure a webhook URL in the Hikvision device admin panel, expecting the device to POST events to the backend.

**Why it's wrong:** Access control terminals (the DS-K series) use alertStream (outbound long-poll from backend to device) for event delivery. Inbound webhook configuration exists on camera models but is unreliable or unsupported on face recognition terminals. Relying on inbound webhooks leads to missed events with no reconnect mechanism.

**Do this instead:** Backend initiates the alertStream connection. Backend owns the reconnect loop. Events are never lost due to network blips because the backend reconnects and the device re-sends events from its buffer.

### Anti-Pattern 4: Mutable Audit Logs

**What people do:** Update or delete audit log rows to "fix" incorrect entries.

**Why it's wrong:** Destroys legal traceability. Regulatory compliance for payroll records requires an unbroken chain of who changed what and when.

**Do this instead:** Corrections are new audit entries. An incorrect record gets a correction entry referencing the original. The original entry is never touched. The timesheet editor enforces this at the service layer with database-level triggers or application constraints.

## Sources

- Hikvision ISAPI alertStream documentation: [How to get real-time alarm/event in HTTP listening mode](https://www.hikvisioneurope.com/eu/portal/portal/Technology%20Partner%20Program/03-How%20to/How%20to%20get%20real-time%20event%20in%20listening%20mode.pdf)
- Hikvision alertStream endpoint reference: [TPP Wiki — alertStream](https://tpp.hikvision.com/Wiki/ISAPI/Access%20Control%20on%20Person/GUID-C8398309-7417-4540-AF4F-4DA909E766D2.html)
- Turso embedded replicas: [Embedded Replicas Introduction](https://docs.turso.tech/features/embedded-replicas/introduction)
- Turso offline sync beta: [Offline Sync Public Beta](https://turso.tech/blog/turso-offline-sync-public-beta)
- libSQL repository: [tursodatabase/libsql](https://github.com/tursodatabase/libsql)
- Axum webhook patterns: [Receive Webhooks with Rust (Axum) — Svix](https://www.svix.com/guides/receiving/receive-webhooks-with-rust-axum/)
- Axum multipart streaming: [axum::extract::Multipart docs](https://docs.rs/axum/latest/axum/extract/struct.Multipart.html)
- Rust Axum DDD structure: [Hexagonal Architecture in Rust](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust)
- HikVision event receiver reference implementation: [peku33/HikVision-EventReceiver](https://github.com/peku33/HikVision-EventReceiver)
- Offline-first sync patterns 2026: [Offline sync & conflict resolution patterns](https://www.sachith.co.uk/offline-sync-conflict-resolution-patterns-crash-course-practical-guide-apr-8-2026/)

---
*Architecture research for: Biometric Time & Attendance — Cronometrix*
*Researched: 2026-04-11*
