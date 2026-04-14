# Requirements: Cronometrix

**Defined:** 2026-04-11
**Core Value:** Accurate, auditable time tracking that turns raw biometric events into payroll-ready data — with zero manual calculation and full legal traceability.

## v1 Requirements

### Device Management

- [ ] **DEV-01**: Admin can register a Hikvision device with IP, ISAPI credentials, and traffic direction (entry/exit)
- [ ] **DEV-02**: Admin can view real-time connection status of all registered devices
- [ ] **DEV-03**: Admin can send ISAPI commands to a device (door open, reboot, enrollment mode)
- [ ] **DEV-04**: Admin can edit or disable a registered device

### Facial Enrollment

- [ ] **ENRL-01**: Admin can capture a facial profile via Hikvision device camera
- [ ] **ENRL-02**: Admin can upload a JPG photo for facial enrollment
- [ ] **ENRL-03**: Admin can capture a facial profile via webcam
- [ ] **ENRL-04**: System syncs enrolled facial profile to all registered devices simultaneously
- [ ] **ENRL-05**: Admin can see per-device sync status during enrollment (progress/success/failure)

### Event Capture

- [ ] **EVT-01**: System maintains persistent alertStream connections to all registered Hikvision devices
- [ ] **EVT-02**: System automatically reconnects when an alertStream connection drops
- [ ] **EVT-03**: System deduplicates events received within a 30-second window from the same employee
- [ ] **EVT-04**: System stores all raw attendance events with UTC epoch timestamps

### Time Calculation

- [ ] **CALC-01**: System applies first-entry/last-exit rule across all devices within the configured shift window
- [ ] **CALC-02**: System calculates work minutes with configurable tolerance margins (±N minutes)
- [ ] **CALC-03**: System detects and flags late arrivals and early departures
- [ ] **CALC-04**: System calculates overtime hours based on department thresholds
- [ ] **CALC-05**: System deducts lunch time per department config (fixed minutes or explicit punch)
- [ ] **CALC-06**: System handles overnight shifts correctly using anchor-date logic

### Leave Management

- [ ] **LEAVE-01**: Admin can register medical leave for an employee with date range
- [ ] **LEAVE-02**: Admin can register manual adjustments (permissions, special leave) with justification
- [ ] **LEAVE-03**: System excludes approved leave days from attendance calculations
- [ ] **LEAVE-04**: Medical leave receives different salary treatment than other absence types

### Employee Management

- [x] **EMP-01**: Admin can create an employee with unique ID, name, department assignment, and status
- [x] **EMP-02**: Admin can search and filter employees by name, department, and status
- [x] **EMP-03**: Admin can deactivate an employee (soft delete — no hard deletes)
- [x] **EMP-04**: Each employee belongs to exactly one department (1:1 relationship)

### Department Management

- [x] **DEPT-01**: Admin can create a department with base salary and shift schedule (start/end times)
- [x] **DEPT-02**: Admin can configure lunch mode per department (fixed deduction or explicit punch)
- [x] **DEPT-03**: Admin can edit department settings (salary, schedule, lunch mode)

### Global Rules

- [x] **RULE-01**: Admin can configure tolerance margins via visual sliders (late arrival, early departure)
- [x] **RULE-02**: Admin can configure bonus minutes (grace period applied automatically)
- [x] **RULE-03**: Rule changes take effect on the next calculation cycle (not retroactive)

### Timesheet Editor

- [ ] **TS-01**: Supervisor can view daily attendance grid per employee
- [ ] **TS-02**: Supervisor can edit an employee's entry/exit time for a specific day
- [ ] **TS-03**: Every timesheet edit requires a text justification (mandatory field)
- [ ] **TS-04**: Every timesheet edit requires an evidence file upload (PDF or JPG)
- [ ] **TS-05**: System generates an immutable audit log entry for every timesheet edit

### Dashboard

- [ ] **DASH-01**: Dashboard displays real-time KPIs (present count, late count, absentees)
- [ ] **DASH-02**: Dashboard shows connection status of all registered devices
- [ ] **DASH-03**: Dashboard displays live photo feed from device recognition events

### Payroll Export

- [ ] **PAY-01**: Admin can generate pre-payroll report for a configurable period (weekly/bi-weekly/monthly)
- [ ] **PAY-02**: Report includes work minutes, overtime, late deductions, and leave summary per employee
- [ ] **PAY-03**: Report exports to Excel format
- [ ] **PAY-04**: Report exports to PDF format

### Access Control (RBAC)

- [x] **AUTH-01**: User can log in with username and password
- [x] **AUTH-02**: Admin role has full access to all features
- [x] **AUTH-03**: Supervisor role can edit timesheets, manage employees, and view reports
- [x] **AUTH-04**: Viewer role has read-only access to dashboards and reports
- [x] **AUTH-05**: User session persists across browser refresh (JWT-based)

### Licensing & Deployment

- [ ] **LIC-01**: System requires a license key on first run before allowing configuration
- [ ] **LIC-02**: System generates a hardware fingerprint (CPU, MAC, disk serial) and binds the license to it
- [ ] **LIC-03**: License validation happens via DigitalOcean Functions (license server)
- [ ] **LIC-04**: Validated license is cached locally as a signed JWT for offline operation
- [ ] **LIC-05**: System rejects activation if hardware fingerprint doesn't match (anti-cloning)
- [ ] **DEPL-01**: One-command installer script (`curl | bash`) for Linux servers with Docker
- [ ] **DEPL-02**: Docker Compose deployment with 3 services: api, web, cloudflared
- [ ] **DEPL-03**: Installer auto-registers a Cloudflare tunnel with `{client-slug}.cronometrix.com`
- [ ] **DEPL-04**: System works locally even when internet connection is unavailable (after initial activation)

### Data Persistence

- [x] **DATA-01**: All data stored locally in SQLite via libSQL
- [x] **DATA-02**: Data syncs asynchronously to Turso cloud for remote access and backup
- [x] **DATA-03**: Local SQLite is authoritative — cloud is a replica, not the primary
- [x] **DATA-04**: Every administrative mutation generates an immutable audit log entry (backend)

## v2 Requirements

### Audit Trail UI

- **AUDIT-01**: Admin can view immutable change history in a dedicated panel
- **AUDIT-02**: Audit entries display actor, timestamp, before/after values, and linked evidence
- **AUDIT-03**: Admin can filter audit logs by employee, date range, and change type

### Holiday Calendar

- **HOL-01**: Admin can mark dates as holidays on an interactive calendar
- **HOL-02**: Admin can configure salary surcharge percentage per holiday
- **HOL-03**: System applies holiday classification to attendance calculations automatically

### Additional Features

- **FEED-01**: Live photo feed from device camera on access events (dashboard enhancement)
- **CMD-01**: Batch ISAPI command dispatch to multiple devices simultaneously

## Out of Scope

| Feature | Reason |
|---------|--------|
| Employee self-service portal | Doubles scope with separate auth surface; admin-facing product for v1 |
| Direct payroll system integration | Each payroll system has different API; Excel/CSV export is standard workflow |
| Shift scheduling / roster management | Separate domain; configured shift times sufficient for v1 |
| Multiple biometric vendors | Single-vendor deep focus (Hikvision); add second vendor after revenue proven |
| Central management portal | Each installation is independent by design |
| Mobile app | Web-first; Tauri desktop wrapper is nearer-term option |
| GDPR biometric deletion workflow | Not targeting EU markets initially |
| Real-time push notifications to employees | Device firmware handles confirmation display |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DATA-01 | Phase 1 | Complete |
| DATA-02 | Phase 1 | Complete |
| DATA-03 | Phase 1 | Complete |
| DATA-04 | Phase 1 | Complete |
| AUTH-01 | Phase 1 | Complete |
| AUTH-02 | Phase 1 | Complete |
| AUTH-03 | Phase 1 | Complete |
| AUTH-04 | Phase 1 | Complete |
| AUTH-05 | Phase 1 | Complete |
| EMP-01 | Phase 1 | Complete |
| EMP-02 | Phase 1 | Complete |
| EMP-03 | Phase 1 | Complete |
| EMP-04 | Phase 1 | Complete |
| DEPT-01 | Phase 1 | Complete |
| DEPT-02 | Phase 1 | Complete |
| DEPT-03 | Phase 1 | Complete |
| RULE-01 | Phase 1 | Complete |
| RULE-02 | Phase 1 | Complete |
| RULE-03 | Phase 1 | Complete |
| DEV-01 | Phase 2 | Pending |
| DEV-02 | Phase 2 | Pending |
| DEV-03 | Phase 2 | Pending |
| DEV-04 | Phase 2 | Pending |
| EVT-01 | Phase 2 | Pending |
| EVT-02 | Phase 2 | Pending |
| EVT-03 | Phase 2 | Pending |
| EVT-04 | Phase 2 | Pending |
| CALC-01 | Phase 3 | Pending |
| CALC-02 | Phase 3 | Pending |
| CALC-03 | Phase 3 | Pending |
| CALC-04 | Phase 3 | Pending |
| CALC-05 | Phase 3 | Pending |
| CALC-06 | Phase 3 | Pending |
| LEAVE-01 | Phase 3 | Pending |
| LEAVE-02 | Phase 3 | Pending |
| LEAVE-03 | Phase 3 | Pending |
| LEAVE-04 | Phase 3 | Pending |
| DASH-01 | Phase 4 | Pending |
| DASH-02 | Phase 4 | Pending |
| DASH-03 | Phase 4 | Pending |
| TS-01 | Phase 4 | Pending |
| TS-02 | Phase 4 | Pending |
| TS-03 | Phase 4 | Pending |
| TS-04 | Phase 4 | Pending |
| TS-05 | Phase 4 | Pending |
| PAY-01 | Phase 5 | Pending |
| PAY-02 | Phase 5 | Pending |
| PAY-03 | Phase 5 | Pending |
| PAY-04 | Phase 5 | Pending |
| LIC-01 | Phase 6 | Pending |
| LIC-02 | Phase 6 | Pending |
| LIC-03 | Phase 6 | Pending |
| LIC-04 | Phase 6 | Pending |
| LIC-05 | Phase 6 | Pending |
| DEPL-01 | Phase 6 | Pending |
| DEPL-02 | Phase 6 | Pending |
| DEPL-03 | Phase 6 | Pending |
| DEPL-04 | Phase 6 | Pending |
| ENRL-01 | Phase 7 | Pending |
| ENRL-02 | Phase 7 | Pending |
| ENRL-03 | Phase 7 | Pending |
| ENRL-04 | Phase 7 | Pending |
| ENRL-05 | Phase 7 | Pending |

**Coverage:**
- v1 requirements: 48 total
- Mapped to phases: 48
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-11*
*Last updated: 2026-04-11 — traceability table populated after roadmap creation*
