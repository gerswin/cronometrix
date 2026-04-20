---
phase: 2
slug: device-integration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-19
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo nextest (Rust) |
| **Config file** | backend/Cargo.toml (dev-dependencies) |
| **Quick run command** | `cd backend && cargo nextest run --no-fail-fast --test-threads=4` |
| **Full suite command** | `cd backend && cargo nextest run --all-features` |
| **Estimated runtime** | ~30 seconds (unit + integration with in-memory libSQL) |

---

## Sampling Rate

- **After every task commit:** Run quick command for the modified crate module
- **After every plan wave:** Run full suite command
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

Populated by planner. See `02-RESEARCH.md` § Validation Architecture for requirement → test mapping (DEV-01..04, EVT-01..04).

---

## Wave 0 Requirements

- [ ] `backend/tests/fixtures/mock_hikvision.rs` — tokio TCP server fixture that serves canned multipart/mixed alertStream bytes
- [ ] `backend/tests/fixtures/alertstream_samples/` — pcap/bytes samples from DS-K1T341 / DS-K1T342 (flagged BLOCKER per STATE.md)
- [ ] `backend/tests/common/mod.rs` — shared test harness (spin up Axum with in-memory libSQL, seeded test users, test DEVICE_CREDS_KEY)
- [ ] cargo-nextest added to dev tooling if not present

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real device handshake + digest auth + multipart parse against DS-K1T341 | DEV-03 / EVT-01 | Requires physical hardware; fixture cannot fully validate firmware quirks | Register a real device, trigger face-scan, verify event lands in DB with correct employee_id and raw_xml |
| Door open command on physical access controller | DEV-04 | Side effect on physical door; not safe in automated test | POST /api/v1/devices/:id/commands {command:"door_open"} and confirm door opens within 10s |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
