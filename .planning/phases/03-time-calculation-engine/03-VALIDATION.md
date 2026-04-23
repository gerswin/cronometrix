---
phase: 3
slug: time-calculation-engine
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-23
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo-nextest` (Rust) |
| **Config file** | `backend/Cargo.toml` (existing); `backend/tests/` directory for integration fixtures |
| **Quick run command** | `cd backend && cargo nextest run --lib calc::` |
| **Full suite command** | `cd backend && cargo nextest run --workspace` |
| **Estimated runtime** | ~45 seconds (quick), ~180 seconds (full) |

---

## Sampling Rate

- **After every task commit:** Run `cd backend && cargo nextest run --lib {touched_module}::`
- **After every plan wave:** Run `cd backend && cargo nextest run --workspace`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds for quick; 180 seconds for full

---

## Per-Task Verification Map

> Populated by planner. Each task must reference a requirement (CALC-01..06, LEAVE-01..04) and an automated verify command or Wave 0 fixture dependency. Planner writes canonical task IDs `03-0N-0M`.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-XX | 01 | 1 | CALC-0X | — | N/A | unit | `cargo nextest run --lib calc::<fn>` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `backend/src/calc/mod.rs` + `backend/src/calc/tests.rs` — pure-domain unit test module
- [ ] `backend/tests/calc_fixtures/` — shared LOTTT scenario fixtures (normal day, late arrival, overnight, Sunday, rest-day, lunch punch-mode missing, OT caps)
- [ ] `backend/tests/common/mod.rs` — test DB bootstrap helper (if reused from Phase 1/2, extend; otherwise create)
- [ ] `proptest` dev-dependency added to `backend/Cargo.toml` (property-based anchor-date + window math tests)
- [ ] `chrono-tz` dependency added (planner sets exact version)

*If existing infrastructure from Phase 1/2 covers any of the above, the planner should note it and skip that bullet.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Evidence upload flow (multipart file to `leaves.evidence_path`) | LEAVE-02 | Filesystem side-effect; integration test covers the happy path but operator UX (malformed file, size cap) is reviewed manually | POST `/api/v1/leaves` with `multipart/form-data`; confirm file lands under configured evidence dir and row references it |
| LOTTT article cross-check against official source | CALC-03, CALC-05, CALC-06 | Legal interpretation — operator / legal reviewer signs off on Art. 117/118/120/173/178 mapping | Compare `calc::overtime_cap_check` constants + comments against INCES PDF Art. 173/178 before phase sign-off |

*Anything else: automated verification required.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags (`cargo watch` forbidden in verify commands — one-shot only)
- [ ] Feedback latency < 60s for quick run
- [ ] `nyquist_compliant: true` set in frontmatter after planner populates the verification map

**Approval:** pending
