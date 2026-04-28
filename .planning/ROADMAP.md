# Roadmap: Cronometrix

## Overview

Cronometrix builds from the ground up: a stable data foundation with auth and core entities first, then the hardware integration layer that feeds attendance data into the system, then the pure calculation engine that transforms raw events into payroll-ready records, then the API surface and frontend dashboards that operators use daily, then reports and payroll export as the primary client deliverable, then licensing and deployment to make each installation commercially viable and remotely accessible, and finally facial enrollment with multi-device sync as the hardware differentiator that eliminates manual setup.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation** - Database schema, auth, user roles, employees, departments, and global rules
- [ ] **Phase 2: Device Integration** - Hikvision alertStream listener, ISAPI client, event capture pipeline
- [ ] **Phase 3: Time Calculation Engine** - Work hours, tolerance, overtime, leave management, overnight shifts
- [ ] **Phase 4: Frontend UI** - Dashboard, timesheet editor, employee directory, device manager screens
- [ ] **Phase 5: Reports & Payroll Export** - Period-based pre-payroll reports in Excel and PDF
- [ ] **Phase 6: Licensing & Deployment** - Hardware-bound licensing, Docker Compose installer, Cloudflare tunnel
- [ ] **Phase 7: Facial Enrollment & Sync** - Enrollment modal, multi-device profile sync

## Phase Details

### Phase 1: Foundation
**Goal**: A running Rust service with correct database schema, authentication, and core data entities so every downstream phase has a stable, auditable data layer to build on
**Depends on**: Nothing (first phase)
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, EMP-01, EMP-02, EMP-03, EMP-04, DEPT-01, DEPT-02, DEPT-03, RULE-01, RULE-02, RULE-03
**Success Criteria** (what must be TRUE):
  1. Admin can log in with username and password and stay logged in across browser refreshes
  2. Admin can create, search, filter, and deactivate employees; no employee can be hard-deleted
  3. Admin can create departments with salary, schedule, and lunch mode; each employee belongs to exactly one department
  4. Admin can adjust global tolerance sliders and bonus minutes; rule changes take effect on next cycle only
  5. Every administrative mutation (user, employee, department, rule) produces an immutable audit log entry in the database
**Plans:** 1/5 plans executed

Plans:
- [x] 01-00-PLAN.md — Test infrastructure (Wave 0): shared fixtures, test stubs, dev-dependencies
- [x] 01-01-PLAN.md — Scaffold Rust project, database schema, audit triggers, Turso sync
- [x] 01-02-PLAN.md — JWT auth service, RBAC middleware, setup wizard backend
- [x] 01-03-PLAN.md — Employee, department, and global rules CRUD endpoints
- [x] 01-04-PLAN.md — Next.js frontend scaffold, setup wizard UI, login page, RBAC verification

### Phase 2: Device Integration
**Goal**: The system maintains live alertStream connections to all registered Hikvision devices, captures attendance events in real time, and operators can manage device configuration from the backend
**Depends on**: Phase 1
**Requirements**: DEV-01, DEV-02, DEV-03, DEV-04, EVT-01, EVT-02, EVT-03, EVT-04
**Success Criteria** (what must be TRUE):
  1. Admin can register a Hikvision device with IP, credentials, and traffic direction; device appears in the system immediately
  2. Admin can edit, disable, or send ISAPI commands (door open, reboot, enrollment mode) to any registered device
  3. System maintains persistent alertStream connections and automatically reconnects when a device drops the TCP connection
  4. Every attendance event from any device is stored with a UTC epoch timestamp; duplicate events within 30 seconds from the same employee are silently discarded
  5. Device connection status (online/offline) is readable from the API so the dashboard can display it
**Plans:** 4 plans

Plans:
- [x] 02-01: Device Manager API — register, edit, disable, ISAPI command dispatch with encrypted credential storage
- [x] 02-02: alertStream listener — one tokio task per device, supervisor/reconnect loop with exponential backoff, multipart XML parser
- [x] 02-03: Event processor — deduplication (30-second idempotency window), face_id-to-employee mapping, AttendanceEvent persistence

### Phase 3: Time Calculation Engine
**Goal**: The Attendance Engine correctly transforms raw attendance events into payroll-ready daily records, handling tolerance windows, lunch deductions, overtime, leave overlays, and overnight shifts as pure domain logic
**Depends on**: Phase 2
**Requirements**: CALC-01, CALC-02, CALC-03, CALC-04, CALC-05, CALC-06, LEAVE-01, LEAVE-02, LEAVE-03, LEAVE-04
**Success Criteria** (what must be TRUE):
  1. System applies first-entry/last-exit rule across all devices within the configured shift window and materializes a single DailyRecord per employee per day
  2. System correctly flags late arrivals and early departures based on configurable tolerance margins
  3. System calculates overtime above department-configured thresholds and deducts lunch time per department mode (fixed minutes or explicit punch)
  4. Admin can register medical leave or manual adjustments with justification; approved leave days are excluded from attendance calculations with correct salary treatment
  5. Overnight shifts are attributed to the correct anchor date regardless of which calendar day the event occurs on
**Plans:** 3 plans

Plans:
- [x] 03-01-PLAN.md — Attendance Engine: pure domain calc (first-entry/last-exit, tolerance, lunch, overtime + LOTTT Art. 178 caps, anomalies), persistence with ON CONFLICT DO UPDATE upsert, recompute worker (tokio mpsc + 500ms debounce), nightly 02:00 reconcile, migrations 007/008/012, read endpoints for daily-records + anomalies
- [x] 03-02-PLAN.md — Overnight shift support: chrono-tz integration, anchor-date = shift-start date (D-05), is_overnight_shift opt-in flag (D-06), DST-safe .earliest() path + OVERNIGHT_INFERENCE_AMBIGUOUS anomaly (Venezuela = America/Caracas, no DST, future-proofed for Colombia/Ecuador)
- [x] 03-03-PLAN.md — Leave management API: migrations 009/010/011, leaves CRUD with mandatory justification + evidence upload (multipart, 10MB cap, PDF/JPEG/PNG), soft-delete cancellation, optimistic concurrency, leave overlay integration (D-16) with EVENTS_ON_LEAVE_DAY anomaly, audit triggers on leaves + daily_record_overrides

### Phase 4: Frontend UI
**Goal**: Operators can perform all daily workflows through a web interface: monitoring the live dashboard, editing timesheets with mandatory justification, managing employees, and configuring devices
**Depends on**: Phase 3
**Requirements**: DASH-01, DASH-02, DASH-03, TS-01, TS-02, TS-03, TS-04, TS-05
**Success Criteria** (what must be TRUE):
  1. Dashboard shows real-time KPIs (present count, late count, absentees) and device connection status; device offline events surface as a prominent banner
  2. Dashboard displays the live photo feed from device recognition events via SSE
  3. Supervisor can view the daily attendance grid per employee and edit entry/exit times with a mandatory text justification and evidence file upload
  4. Every timesheet edit produces an immutable audit log entry visible via the backend; the justification field cannot be skipped or left empty
  5. Admin can manage employees and configure devices from the same UI without leaving the application
**Plans:** 4 plans

Plans:
- [x] 04-01-PLAN.md — packages + vitest + auth shell + proxy.ts + SSE backend endpoint (Wave 1)
- [x] 04-02-PLAN.md — Dashboard: KPI tiles, SSE activity feed, dept donut chart, device banners (Wave 2)
- [x] 04-03-PLAN.md — Timesheet: TanStack Table grid + Novedad modal + overrides backend endpoint (Wave 2)
- [x] 04-04-PLAN.md — Employee directory + Device manager + ISAPI command dispatch (Wave 2)
**UI hint**: yes

### Phase 5: Reports & Payroll Export
**Goal**: Admin and supervisors can generate a pre-payroll report for any configurable period and export it to Excel or PDF, producing the primary deliverable clients pay for
**Depends on**: Phase 4
**Requirements**: PAY-01, PAY-02, PAY-03, PAY-04
**Success Criteria** (what must be TRUE):
  1. Admin can select a report period (weekly, bi-weekly, or monthly) and generate a pre-payroll report covering all employees
  2. Report includes work minutes, overtime hours, late deductions, and leave summary per employee for the selected period
  3. Report downloads as a correctly formatted Excel file
  4. Report downloads as a PDF file with the same data
**Plans:** 4 plans

Plans:
- [x] 05-01-PLAN.md — tenant_info CRUD (singleton + audit triggers + audit_log CHECK rebuild) + employees position/hire_date columns
- [x] 05-02-PLAN.md — Reports calculation JSON API: money math (LOTTT premiums), period boundary math, SQL aggregation with override-merge, REPORT_EXPORT audit
- [x] 05-03-PLAN.md — Excel export endpoint via rust_xlsxwriter 0.94 with branding header, per-dept subtotals, anomaly tinting, calamine round-trip tests
- [x] 05-04-PLAN.md — Frontend Reports screen + Settings/Datos de Empresa + jspdf-autotable PDF + sidebar nav update

### Phase 6: Licensing & Deployment
**Goal**: Each installation requires a valid hardware-bound license before it can be configured, and a single `curl | bash` command installs and connects the full system — making it commercially deployable and remotely accessible
**Depends on**: Phase 5
**Requirements**: LIC-01, LIC-02, LIC-03, LIC-04, LIC-05, DEPL-01, DEPL-02, DEPL-03, DEPL-04
**Success Criteria** (what must be TRUE):
  1. System blocks all configuration on first run until a valid license key is entered; license is bound to the machine's hardware fingerprint
  2. License validation contacts the DO Functions license server; a signed JWT is cached locally so the system operates offline after initial activation
  3. System rejects activation if the hardware fingerprint does not match the license (anti-cloning protection)
  4. Running `curl | bash` on a fresh Linux server installs Docker Compose with api, web, and cloudflared services and registers a `{client-slug}.cronometrix.com` Cloudflare tunnel
  5. System operates fully when the internet is unavailable (after initial activation and tunnel registration)
**Plans:** 4 plans

Plans:
- [x] 06-01-PLAN.md — License backend module: hardware fingerprint (cpu+mac+disk SHA256), RS256 JWT verifier with embedded public key, DO Functions activation call, cached JWT I/O, renewal task, AppError::Unlicensed, full integration tests
- [x] 06-02-PLAN.md — License gate wiring: AppState.license_valid + require_license middleware + setup_activate handler + extended setup_status + frontend /setup/license activation page; updates 16 existing test fixtures for the new AppState field
- [x] 06-03-PLAN.md — Deployment stack: multi-stage Dockerfiles (Rust 1.93, Node 24-alpine), 3-service docker-compose (api/web/cloudflared:2026.3.0), curl|bash installer with idempotent secret handling, .dockerignore, frontend next.config.ts standalone output
- [x] 06-04-PLAN.md — DO Functions license server: activate/renew handlers (RS256 signing, fingerprint binding, Postgres-backed store), project.yml deployment config, Node test harness, operator README

### Phase 7: Facial Enrollment & Sync
**Goal**: Admin can enroll an employee's facial profile through the web UI using a device camera, webcam, or JPG upload, and the system simultaneously pushes the profile to all registered devices with per-device status feedback
**Depends on**: Phase 4
**Requirements**: ENRL-01, ENRL-02, ENRL-03, ENRL-04, ENRL-05
**Success Criteria** (what must be TRUE):
  1. Admin can capture a facial profile via the Hikvision device camera or a connected webcam and save it linked to an employee
  2. Admin can upload a JPG photo for enrollment as an alternative to live capture
  3. After enrollment, the system automatically pushes the facial profile to all registered devices concurrently
  4. Admin can see per-device sync status (in progress, success, failure) during and after the push without the modal blocking
**Plans:** 2 plans

Plans:
- [ ] 07-01-PLAN.md — Enrollment backend: migrations 016/017 (face_id/current_face_enrollment_id/state columns + 3 new tables + 9 audit triggers), `enrollments/` module (handlers, service, pusher with JoinSet fan-out, image_pipeline ≤200KB downscale, isapi_face), `workers/` (PurgeWorker + BackfillWorker with Semaphore=4), `isapi/client.rs` extended with upsert_user/upload_face/delete_user/capture_face_image, RequestBodyLimitLayer(3MB), Bruno collection, ~23 integration tests
- [ ] 07-02-PLAN.md — Enrollment modal UI: 6 shadcn primitives (tabs/progress/select/sonner/badge/skeleton), vendored @vladmandic/face-api@1.7.15 + tinyFaceDetector model (~190KB), 11 enrollment components (EnrollmentModal centerpiece, 3 capture tabs, ValidationPanel + SyncPanel/Row, picker + in-progress list), shared AccessRestricted, /enrollment page replacement, Employees table row action, Zod schema + types, 6 vitest test files + manual smoke checkpoint
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 1/5 | In Progress|  |
| 2. Device Integration | 0/3 | Not started | - |
| 3. Time Calculation Engine | 0/3 | Not started | - |
| 4. Frontend UI | 0/4 | Not started | - |
| 5. Reports & Payroll Export | 0/2 | Not started | - |
| 6. Licensing & Deployment | 0/4 | Not started | - |
| 7. Facial Enrollment & Sync | 0/2 | Not started | - |

### Phase 8: Test Coverage & Quality Gate — Reach >=90% line coverage and >=85% branch coverage on backend (cargo-llvm-cov) and frontend (Vitest+v8); add CI thresholds; fix leave_tests cwd-dependent failure; document coverage commands in CLAUDE.md

**Goal:** Hard-fail coverage gate active in CI: backend (cargo-llvm-cov + nightly --branch) and frontend (Vitest+v8) both enforce >=90% line / >=85% branch project-wide and >=70/60 per-file; the leave_tests cwd-dependent failure is fixed via AppState `Paths` injection; CLAUDE.md documents the gate, exclusions, and the path-injection convention so future phases cannot regress them silently.
**Requirements**: QUALITY-GATE (cross-cutting; D-01..D-23 from 08-CONTEXT.md are the binding contract)
**Depends on:** Phase 7
**Plans:** 6 plans

Plans:
- [x] 08-01-PLAN.md — AppState `Paths` substruct + 5 source-side call-site updates (leaves/events/enrollments/daily_records); preserves env vars + defaults (D-17/D-18/D-19/D-21)
- [x] 08-02-PLAN.md — Test-helper migration: `common::test_state_with_tmpdir` + remove `LeavesRootGuard`/`EventsRootGuard` from leave/event/listener tests + 4 other AppState construction sites (D-20)
- [x] 08-03-PLAN.md — Coverage tooling: vitest.config.ts thresholds + Makefile + scripts/enforce-coverage-floor.sh + rust-toolchain.toml (D-02/D-08/D-14/D-16)
- [x] 08-04-PLAN.md — Coverage gap-fill tests (backend + frontend) — split into 04A (backend domain), 04B (backend infrastructure), 04C (frontend + composite checkpoint); `make coverage-frontend` exits 0; backend GREEN on Linux CI (2 macOS-only exclusions: license/fingerprint.rs + license/service.rs)
- [x] 08-05-PLAN.md — `.github/workflows/ci.yml` with two parallel jobs + HTML artifacts + negative-regression validation (D-01/D-03/D-04/D-05/D-13) — _live CI validation deferred to manual follow-up; see 08-05-SUMMARY.md_
- [ ] 08-06-PLAN.md — CLAUDE.md docs: Test Coverage section + Conventions § Filesystem-root injection (D-22/D-23)
