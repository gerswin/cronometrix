# Cronometrix

## What This Is

Cronometrix is a biometric time & attendance product for businesses using Hikvision facial recognition devices. It runs on-premise at each client site, connects to up to 4 biometric readers, calculates work hours with configurable tolerance rules, and syncs data to Turso cloud for remote access and backup. Built as a commercial product — each installation is independent.

## Core Value

Accurate, auditable time tracking that turns raw biometric events into payroll-ready data — with zero manual calculation and full legal traceability.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Receive real-time attendance webhooks from Hikvision devices
- [ ] Send ISAPI commands (door open, reboot, enrollment mode) to devices
- [ ] Sync facial profiles across all devices simultaneously
- [ ] Calculate work minutes with configurable tolerance (±20 min) and lunch deduction
- [ ] Process holidays, medical leave, and manual adjustments
- [ ] Apply first-entry/last-exit rule across multiple devices
- [ ] Store data locally in SQLite with async Turso cloud sync per client
- [ ] Generate immutable audit logs for every administrative change
- [ ] Dashboard with real-time KPIs, device status, and live photo feed
- [ ] Department management with base salary and lunch mode config
- [ ] Global rules panel (tolerance sliders, bonus minutes)
- [ ] Employee directory with advanced filters and active/inactive status
- [ ] Facial enrollment modal (device camera, webcam, or JPG upload)
- [ ] Device manager (IPs, ISAPI credentials, entry/exit direction)
- [ ] Interactive holiday calendar with salary surcharge config
- [ ] Timesheet editor with mandatory justification (PDF/JPG) for changes
- [ ] Audit trail panel with immutable change history and evidence viewer
- [ ] Reports and pre-payroll export (Excel/PDF) by period
- [ ] Role-based access: Admin (full), Supervisor (edit timesheets, view reports), Viewer (read-only)
- [ ] Hardware-bound licensing: machine fingerprint + DO Functions license server + signed JWT cached locally
- [ ] One-command installer script (`curl | bash`) for Linux servers with Docker
- [ ] Docker Compose deployment (api + web + cloudflared services)
- [ ] Cloudflare tunnel auto-registration with `{client}.cronometrix.com` subdomain

### Out of Scope

- Central management portal for multiple clients — each installation is independent
- Mobile app — web-first, mobile later
- Biometric vendors other than Hikvision — single vendor focus for v1
- Real-time chat or messaging features — not relevant to core value
- Employee self-service portal — admin-facing only for v1

## Context

- **Hardware:** Hikvision facial recognition biometric devices (up to 4 per site), communicating via ISAPI protocol and webhooks
- **Deployment model:** Hybrid on-premise — runs locally with Turso (libSQL) cloud sync for remote access and backup. Each client installation is fully independent
- **Business rules:** 1:1 employee-department relationship. No data deletion — all changes are audited. First entry/last exit across devices within shift range
- **Target market:** Businesses that use Hikvision biometrics and need payroll-ready attendance reports with legal audit trails
- **Multi-client architecture:** Each client gets their own on-premise installation with embedded SQLite + Turso replica. No shared infrastructure between clients

## Constraints

- **Tech stack (backend):** Rust with Axum — performance-critical for real-time webhook processing and time calculations
- **Tech stack (frontend):** React/Next.js with TypeScript — mature ecosystem for data-heavy admin screens
- **Tech stack (database):** SQLite (local) + Turso (cloud sync) via libSQL — local-first architecture
- **Hardware dependency:** Must support Hikvision ISAPI protocol — this is non-negotiable
- **Audit compliance:** Every mutation to attendance records must generate an immutable audit log entry with justification
- **Desktop option (future):** Architecture should allow wrapping in Tauri later for desktop deployment
- **Deployment:** Docker Compose on Linux servers, one-command install via shell script
- **Licensing:** Hardware-bound via DO Functions — prevents unauthorized cloning across servers
- **Network access:** Cloudflare tunnel per client → `{client-slug}.cronometrix.com`

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust + Axum for backend | Performance for real-time webhooks, type safety for business logic, Turso/libSQL native support | — Pending |
| React/Next.js for frontend | Mature component ecosystem (grids, calendars, forms), large talent pool, Tauri-compatible | — Pending |
| SQLite + Turso for persistence | Local-first operation, async cloud sync per client, no shared infrastructure needed | — Pending |
| Independent client installations | Simpler architecture, no multi-tenant complexity, each client owns their data | — Pending |
| 3-role RBAC (Admin/Supervisor/Viewer) | Balance between access control and simplicity for v1 | — Pending |
| Hardware-bound licensing via DO Functions | Prevent unauthorized cloning, machine fingerprint + signed JWT | — Pending |
| Docker Compose + shell installer | One-command deployment on Linux, minimal client-side ops knowledge needed | — Pending |
| Cloudflare tunnel per client | Remote access without VPN, `{client}.cronometrix.com` subdomains | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-28 — Phase 6 (Licensing & Deployment) complete; license backend module + Tower middleware gate + Docker/cloudflared deploy stack + DO Functions activate/renew server live. LIC-01..LIC-05 validated; DEPL-01/02/04 validated; DEPL-03 partial (auto-register CF tunnel deferred). 6 HUMAN-UAT items pending Linux VM smoke.*
