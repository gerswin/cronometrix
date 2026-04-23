---
phase: 3
slug: time-calculation-engine
status: ready
nyquist_compliant: true
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

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | CALC-01..06 (scaffold) | T-3-01, T-3-07 | Migrations apply idempotently; Config.timezone parses TZ env; AppState wires recompute_tx | integration | `cd backend && cargo build --workspace` + `cargo nextest run --lib config::` + migration smoke via `cargo run --bin migrate` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | CALC-01, CALC-02, CALC-03, CALC-04, CALC-06 | T-3-01, T-3-04, T-3-05, T-3-06 | ON CONFLICT upsert (no INSERT OR REPLACE); OT cap rule work+OT > 600; bounded mpsc backpressure; RBAC on /api/v1/anomalies | unit + integration | `cd backend && cargo nextest run --lib calc::` + `cargo nextest run --test daily_record_tests` + `cargo nextest run --test calc_tests` | ✅ | ⬜ pending |
| 03-02-01 | 02 | 2 | CALC-05 | T-3-08 (DST ambiguity) | `.earliest()` not `.single().unwrap()` in overnight.rs; OvernightInferenceAmbiguous emitted on LocalResult::Ambiguous | unit | `cd backend && cargo nextest run --lib calc::overnight::` + `grep -q "earliest()" backend/src/calc/overnight.rs` + `grep -vq ".single().unwrap()" backend/src/calc/overnight.rs` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 2 | CALC-05, CALC-06 | T-3-08 | Overnight fixtures cover 22:00→06:00; proptest asserts anchor = shift_start_date; service event query spans window across midnight | integration + property | `cd backend && cargo nextest run --test calc_tests -- overnight` + `cargo nextest run --test daily_record_tests -- overnight` | ✅ | ⬜ pending |
| 03-03-01 | 03 | 3 | LEAVE-01, LEAVE-02, LEAVE-03, LEAVE-04 (scaffold) | T-3-14, T-3-19 | Migrations 009/010/011 apply with audit triggers; LeaveConflict variant in AppError | integration | `cd backend && cargo build --workspace` + migration smoke + `grep -q "LeaveConflict" backend/src/errors.rs` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 3 | LEAVE-01, LEAVE-02, LEAVE-03, LEAVE-04 | T-3-14, T-3-15, T-3-16, T-3-19 | require_admin on leave writes; mandatory justification; evidence path traversal blocked; overlap detection; D-16 overlay (leave wins); EVENTS_ON_LEAVE_DAY anomaly | integration | `cd backend && cargo nextest run --test leave_tests` + `cargo nextest run --test daily_record_tests -- leave_overlay` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*File Exists: ❌ W0 = created by this task (Wave 0 scaffold) · ✅ = file already exists or was created by a prior task*

---

## Wave 0 Requirements

- [ ] `backend/src/calc/mod.rs` + submodule stubs (`anomalies.rs`, `engine.rs`, `aggregation.rs`, `overtime.rs`, `lunch.rs`, `overnight.rs`) — created by 03-01-01 (overnight.rs by 03-02-01)
- [ ] `backend/tests/fixtures/lottt_scenarios.json` — LOTTT scenarios (normal day, late arrival, Sunday, rest-day, lunch punch-mode missing, OT caps); overnight scenarios added in 03-02-02
- [ ] `backend/tests/calc_tests.rs`, `backend/tests/daily_record_tests.rs` — integration test files created by 03-01-01
- [ ] `backend/tests/leave_tests.rs` — created by 03-03-01
- [ ] `backend/tests/common/mod.rs` — extended or created by 03-01-01 (DB bootstrap helper)
- [ ] `proptest` dev-dependency added to `backend/Cargo.toml` by 03-01-01
- [ ] `chrono-tz = "0.10.4"` added to `backend/Cargo.toml` by 03-01-01

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Evidence upload flow (multipart file to `leaves.evidence_path`) | LEAVE-02 | Filesystem side-effect; integration test covers the happy path but operator UX (malformed file, size cap) is reviewed manually | POST `/api/v1/leaves` with `multipart/form-data`; confirm file lands under configured evidence dir and row references it |
| LOTTT article cross-check against official source | CALC-03, CALC-05, CALC-06 | Legal interpretation — operator / legal reviewer signs off on Art. 117/118/120/173/178 mapping | Compare `calc::overtime_cap_check` constants + comments against INCES PDF Art. 173/178 before phase sign-off |
| Nightly reconcile 02:00 local-time execution | CALC-01 | Real-clock behavior — proving the sleep_until math hits 02:00 America/Caracas requires observing a live run over ~24h | Deploy to staging with log tracing on; observe reconcile log line at 02:00 local for two consecutive nights |

*Anything else: automated verification required.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (6/6)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (every task has a one-shot cargo command)
- [x] Wave 0 covers all MISSING references (calc module stubs, fixtures, test files, deps)
- [x] No watch-mode flags (`cargo watch` forbidden — only `cargo nextest run`, one-shot)
- [x] Feedback latency < 60s for quick run
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-04-23
