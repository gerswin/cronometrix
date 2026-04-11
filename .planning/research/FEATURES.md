# Feature Research

**Domain:** Biometric time & attendance system (on-premise, Hikvision facial recognition)
**Researched:** 2026-04-11
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Clock-in/clock-out event capture | Core product purpose — without this nothing else works | LOW | Hikvision ISAPI webhooks deliver events; must handle deduplication and out-of-order delivery |
| First-entry / last-exit rule | Standard time calculation across multi-reader sites; payroll requires a single "in" and single "out" per day | MEDIUM | Must correlate events across up to 4 devices within a configurable shift window |
| Work hours calculation | Every HR system expects a numeric "hours worked" per day; without it there is no payroll data | MEDIUM | Configurable tolerance windows (±N min), lunch deduction, overnight shifts |
| Late arrival and early departure detection | Supervisors rely on this daily; absence breaks trust in the product | MEDIUM | Requires configured shift schedules per employee or department |
| Overtime calculation | Mandatory for payroll compliance in all jurisdictions; missing = legal exposure | MEDIUM | Requires per-department or global overtime thresholds |
| Department management | All competitors structure employees into departments; required for bulk rule application | LOW | 1:1 employee-department relationship per PROJECT.md |
| Employee directory with status | Active/inactive management is expected; deleting employees is legally risky | LOW | No hard deletes — soft disable with audit record |
| Daily/weekly/monthly attendance reports | Pre-payroll reports are the product's primary deliverable | MEDIUM | Excel and PDF export are both expected |
| Holiday calendar | Attendance products always handle public holidays; missing breaks salary surcharge calculation | MEDIUM | Per-day configuration with salary surcharge percentage |
| Manual timesheet adjustment with justification | Errors in biometric data happen; supervisors need correction capability | MEDIUM | Justification (text) and evidence file (PDF/JPG) upload mandatory per audit requirements |
| Role-based access control | Multi-user admin products require permission boundaries; without RBAC any user can delete everything | MEDIUM | 3-role model: Admin / Supervisor / Viewer |
| Device connection status monitoring | Operators need to know if a reader is offline before it causes missing punches | LOW | Poll ISAPI heartbeat or track webhook gaps |
| Real-time dashboard | Managers expect a live headcount view; absence means operators discover problems late | MEDIUM | KPIs: present count, late count, absentees, device health |
| Payroll period export | The final output of the product — payroll-ready data per period | MEDIUM | Configurable period (weekly, bi-weekly, monthly); Excel primary, PDF secondary |
| Audit trail / change history | Legal requirement in any labor-regulated market; immutability is the differentiator within this category | HIGH | Every mutation must create an immutable log entry with actor, timestamp, before/after values |
| Facial enrollment management | Without enrollment management the biometric hardware is useless | HIGH | Multiple input methods: device camera, webcam capture, JPG upload |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Multi-device facial profile sync | Competitors often require per-device enrollment; syncing to all 4 devices from one UI eliminates a major admin burden | HIGH | Must use Hikvision ISAPI batch enrollment; retry logic and conflict resolution needed |
| Live photo feed from device camera | Operators can visually verify recognition events in real time without logging into device firmware | MEDIUM | Pull snapshot via ISAPI on each access event; display in dashboard |
| Evidence-linked audit trail | Attaching PDF/JPG evidence to timesheet corrections is rare in the market; directly satisfies labor inspection requirements | MEDIUM | File stored locally, linked to immutable audit log entry |
| Configurable tolerance sliders (global rules panel) | Most competitors use hard-coded grace periods; visual sliders per rule type improve operator confidence and reduce support calls | LOW | Tolerance applied at calculation time, not stored on the raw event |
| Lunch deduction mode per department | Some departments do manual lunch breaks, others use device check-in/out; per-department config handles both | MEDIUM | Two modes: fixed deduction minutes, or require explicit lunch punch |
| Bonus minutes configuration | Allows positive adjustments (e.g., overtime grace) without manual timesheet edits; reduces correction volume | LOW | Applied automatically in calculation engine |
| Turso cloud sync for remote access | On-premise products normally lock admins to the LAN; async cloud sync allows off-site report viewing and backup without a VPN | HIGH | libSQL embedded replica with async write-ahead replication to Turso |
| ISAPI command dispatch (door open, reboot, enrollment mode) | Operators can manage hardware from the software UI without physically accessing devices or using Hikvision's own firmware interface | MEDIUM | Covers most common support scenarios without a site visit |
| Per-holiday salary surcharge configuration | Labor law in Latin American markets requires surcharge multipliers on specific holidays; configuring them per calendar day is not standard in international products | MEDIUM | Stored as percentage modifier on each holiday record |
| Medical leave processing | Some competitors treat all leave as the same type; separating medical from other absences allows different salary and compliance treatment | MEDIUM | Medical leave gets different calculation treatment and may require document attachment |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Employee self-service portal | Employees want to see their own hours and request corrections | Adds a separate authentication surface, mobile/responsive requirements, and notification infrastructure; doubles scope for v1 with the primary customer (the admin) gaining no direct value | Supervisors handle corrections via the timesheet editor; move self-service to v2 when the core product is validated |
| Payroll direct integration (push to ADP, Nómina, etc.) | Clients ask for "one click payroll" | Each payroll system has its own API, data model, and auth; supporting even 3 systems is 3 separate integration projects with ongoing maintenance; real-time sync introduces rollback complexity | Export pre-payroll Excel/CSV with the exact columns each payroll system expects; let the payroll operator do the import — this is the standard workflow in the target market |
| Shift scheduling / roster management | Attendance and scheduling seem related | Scheduling is a separate domain requiring template management, rotation logic, swap requests, and manager approval flows; it turns a 4-month project into a 12-month project | Use configured shift start/end times for tolerance calculation only; do not build a full scheduler |
| Support for multiple biometric vendors | Clients have mixed hardware environments | Each vendor has a different protocol (ISAPI, OSDP, proprietary SDK); multi-vendor support creates a combinatorial testing matrix and indefinite maintenance tail | Go deep on Hikvision for v1; add a second vendor only after the product proves revenue; document the abstraction boundary clearly so it is achievable |
| Real-time push notifications to employees | Employees want to know when their clock-in was recorded | Requires a notification service (email/SMS/push), employee contact management, and consent handling; high operational cost for marginal value in an admin-facing product | Show a confirmation display on the device (handled by Hikvision firmware); no software notification needed |
| Multi-client central portal | Resellers want to manage all their client sites from one screen | Each installation is fully independent by design; a central portal requires a different architecture (multi-tenant or federation layer), breaking the local-first guarantee and significantly increasing infrastructure cost | Each client accesses their own installation; reseller support is out-of-scope for v1 |
| Biometric data deletion on employee termination (GDPR) | Required in EU | Complex — must propagate deletion to all enrolled devices via ISAPI, verify success, and log the deletion in an immutable audit record that does not contain the deleted data | Design the system to soft-disable employees and mark their biometric profiles as "pending deletion"; implement actual ISAPI deletion flow as a discrete, audited operation in a later phase |

## Feature Dependencies

```
[Facial Enrollment]
    └──requires──> [Device Manager (IPs, credentials)]
                       └──required by──> [Multi-device Profile Sync]
                       └──required by──> [ISAPI Command Dispatch]
                       └──required by──> [Live Photo Feed]

[Work Hours Calculation]
    └──requires──> [Clock-in/Clock-out Event Capture]
    └──requires──> [Department Management] (shift times, lunch mode)
    └──requires──> [Holiday Calendar] (surcharge classification)
    └──requires──> [Leave Records] (exclude medical/approved leave days)

[Manual Timesheet Adjustment]
    └──requires──> [Audit Trail] (every change must produce an immutable record)
    └──requires──> [RBAC] (Supervisor+ permission required)
    └──enhances──> [Payroll Period Export] (corrected records flow into export)

[Payroll Period Export]
    └──requires──> [Work Hours Calculation]
    └──requires──> [Holiday Calendar]
    └──requires──> [Leave Records]
    └──enhances──> [Audit Trail] (export events are logged)

[Turso Cloud Sync]
    └──requires──> [SQLite local store] (replica source)
    └──enhances──> [All read operations] (remote access to reports)

[Real-time Dashboard]
    └──requires──> [Clock-in/Clock-out Event Capture]
    └──requires──> [Device Connection Status Monitoring]
    └──enhances──> [Live Photo Feed]

[Audit Trail]
    └──required by──> [Manual Timesheet Adjustment]
    └──required by──> [Payroll Period Export]
    └──required by──> [RBAC] (login and permission changes logged)
    └──required by──> [Facial Enrollment] (enrollment changes logged)
```

### Dependency Notes

- **Facial Enrollment requires Device Manager:** The system cannot push face profiles to hardware without storing device IPs and ISAPI credentials first. Device Manager is the foundation of all hardware interaction.
- **Work Hours Calculation requires Holiday Calendar:** A raw punch event on a public holiday must be classified correctly before calculation; this is not optional for accurate payroll.
- **Manual Timesheet Adjustment requires Audit Trail:** The immutable audit trail is not a reporting add-on — it is the enforcement mechanism that makes manual edits legally defensible. Both must be built together.
- **Turso Cloud Sync enhances all read operations:** Sync is optional for core function (the local SQLite works standalone) but it is a key differentiator; it should be designed in from day one, not retrofitted.
- **Live Photo Feed conflicts with Local-only Deployment:** The photo pull happens via ISAPI on the LAN. If the browser accessing the UI is remote (via Turso sync), the photo endpoint must be proxied through the backend — the browser cannot reach the device directly.

## MVP Definition

### Launch With (v1)

Minimum viable product — what is needed to validate the concept with a paying client.

- [ ] Device Manager — without this, no hardware integration exists
- [ ] Facial Enrollment (device camera + JPG upload) — clients must be able to onboard employees
- [ ] Clock-in/clock-out event capture via ISAPI webhooks — core data pipeline
- [ ] First-entry / last-exit calculation across all devices — single daily attendance record
- [ ] Department management with shift times and lunch mode — calculation rules
- [ ] Holiday calendar with salary surcharge config — payroll accuracy
- [ ] Work hours calculation (tolerance, overtime, lunch deduction) — the core value
- [ ] Leave processing (medical and manual adjustments) — payroll completeness
- [ ] Immutable audit trail — legal defensibility; non-negotiable per PROJECT.md
- [ ] Timesheet editor with justification + evidence file — supervisor correction workflow
- [ ] Employee directory (active/inactive, department assignment) — user management
- [ ] RBAC: Admin / Supervisor / Viewer — multi-user safety
- [ ] Payroll period export (Excel + PDF) — the primary deliverable that clients pay for
- [ ] Real-time dashboard with KPIs and device status — operator situational awareness

### Add After Validation (v1.x)

Features to add once the core pipeline is working and first clients are live.

- [ ] Multi-device facial profile sync — high value but the enrollment flow works device-by-device in v1; add when the manual sync is confirmed as a pain point
- [ ] Live photo feed from device on access events — enhances operator UX; requires stable ISAPI connection layer from v1
- [ ] Bonus minutes global configuration — reduces manual correction volume; add when correction frequency is measured
- [ ] ISAPI command dispatch (door open, reboot) — reduces on-site support calls; add when support tickets confirm the need
- [ ] Turso async cloud sync — core architecture supports it from day one; enable as a client-facing feature once sync reliability is validated internally

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] Employee self-service portal — requires separate auth surface and responsive design; validate that admins want to offload this first
- [ ] Biometric GDPR deletion workflow — required for EU markets; not the initial target geography
- [ ] Second biometric vendor support — only after Hikvision integration is proven and a second vendor opportunity is confirmed
- [ ] Mobile companion app — web-first per PROJECT.md; Tauri desktop wrapper is a nearer-term option
- [ ] Shift scheduling / roster management — different domain; only if clients explicitly request it after v1 validation

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Clock-in/clock-out event capture | HIGH | MEDIUM | P1 |
| Work hours calculation | HIGH | MEDIUM | P1 |
| Payroll period export (Excel/PDF) | HIGH | MEDIUM | P1 |
| Audit trail (immutable) | HIGH | HIGH | P1 |
| Facial enrollment | HIGH | HIGH | P1 |
| Device Manager | HIGH | MEDIUM | P1 |
| Employee directory + RBAC | HIGH | MEDIUM | P1 |
| Holiday calendar + leave management | HIGH | MEDIUM | P1 |
| Timesheet editor with justification | HIGH | MEDIUM | P1 |
| Real-time dashboard | MEDIUM | MEDIUM | P1 |
| Multi-device profile sync | HIGH | HIGH | P2 |
| Turso cloud sync | HIGH | HIGH | P2 |
| ISAPI command dispatch | MEDIUM | MEDIUM | P2 |
| Live photo feed | MEDIUM | MEDIUM | P2 |
| Bonus minutes + tolerance sliders | MEDIUM | LOW | P2 |
| Employee self-service portal | MEDIUM | HIGH | P3 |
| Shift scheduling | LOW | HIGH | P3 |
| Multi-vendor biometric support | LOW | HIGH | P3 |
| GDPR deletion workflow | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | AMGtime (on-prem) | Invixium IXM Time | Truein (cloud) | Our Approach |
|---------|-------------------|-------------------|----------------|--------------|
| Biometric integration | Multi-vendor SDK | Own hardware only | Cloud camera | Single-vendor deep (Hikvision ISAPI) |
| Facial enrollment | Fingerprint-primary | Own terminals | Mobile phone | Device cam + webcam + JPG upload |
| Multi-device sync | Manual per-device | Automatic (own HW) | N/A | Automatic ISAPI batch, all devices |
| Cloud backup | Optional add-on | Not available | Native cloud | Turso async replica (local-first) |
| Audit trail | Basic change log | Basic | Limited | Immutable with evidence files |
| Holiday surcharge config | Per-rule | Not configurable | Basic | Per-day percentage on calendar |
| Payroll export | 80+ templates | Basic Excel | CSV | Excel + PDF, configurable period |
| Self-service portal | Yes (adds cost) | Yes | Yes | Deferred to v2 |
| RBAC | Granular (complex) | Basic | Basic | 3-role simplified (Admin/Super/Viewer) |
| On-premise deployment | Yes | Yes | No | Yes — local SQLite + cloud sync |

## Sources

- [Top 4 Biometric Time Clock Systems 2026 — Factorial](https://factorialhr.com/blog/best-biometric-time-clock-systems/)
- [7 Best Face Recognition Attendance Systems 2026 — Timeero](https://timeero.com/post/best-face-recognition-attendance-system/)
- [Biometric Attendance Systems Ultimate Guide 2025 — MiHCM](https://mihcm.com/resources/blog/biometric-attendance-systems-the-ultimate-guide/)
- [Hikvision MinMoe Face Recognition Terminals — Hikvision](https://www.hikvision.com/content/dam/hikvision/en/brochures-download/product-brochures/access-control/MinMoe-Face-Recognition-Terminals-Brochure.pdf)
- [Hikvision ISAPI & OTAP Developer Guide — TPP](https://tpp.hikvision.com/download/ISAPI_OTAP)
- [Access Control Integration — Hikvision TPP](https://tpp.hikvision.com/tpp/ACIntegration)
- [BioSyn: Hikvision DS-K1T804AMF Attendance System Integration](https://biosyn.online/blog/2025/01/10/hikvision-ds-k1t804amf-integration/)
- [Payroll Time and Attendance Complete Guide 2025 — Factorial](https://factorialhr.com/blog/payroll-time-attendance/)
- [Enterprise Biometric Time and Attendance — Invixium IXM Time](https://www.invixium.com/ixm-time-biometric-attendance-system/)
- [Truein Biometric Attendance System Guide](https://truein.com/blogs/biometric-attendance-system)
- [8 Pros & Cons of Biometric Time Clocks — Workforce.com](https://workforce.com/news/the-pros-and-cons-of-biometric-time-clocks)
- [The Top 25 On-Prem Time & Attendance Software 2025](https://topbusinesssoftware.com/categories/time-attendance/on-premise/)
- [AMGtime Employee Time Tracking Solutions](https://amgtime.com/)
- [Leave and Holiday Management Software Guide — MiHCM](https://mihcm.com/resources/blog/a-complete-guide-to-leave-and-holiday-management-software/)

---
*Feature research for: Biometric time & attendance system — Hikvision facial recognition, on-premise*
*Researched: 2026-04-11*
