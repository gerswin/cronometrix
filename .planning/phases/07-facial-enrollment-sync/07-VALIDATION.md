---
phase: 7
slug: facial-enrollment-sync
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-27
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (backend)** | `cargo nextest` (Rust 1.77+, tokio test feature) |
| **Framework (frontend)** | Vitest + React Testing Library + happy-dom |
| **Config file (backend)** | `backend/Cargo.toml` |
| **Config file (frontend)** | `frontend/vitest.config.ts` |
| **Quick run command (backend)** | `cd backend && cargo nextest run --test enrollments_*` |
| **Quick run command (frontend)** | `cd frontend && pnpm vitest run src/components/enrollment` |
| **Full suite command (backend)** | `cd backend && cargo nextest run` |
| **Full suite command (frontend)** | `cd frontend && pnpm vitest run && pnpm tsc --noEmit && pnpm next build` |
| **Estimated runtime (quick)** | ~30s backend, ~15s frontend |
| **Estimated runtime (full)** | ~3m backend, ~2m frontend (build dominates) |

---

## Sampling Rate

- **After every task commit:** Run quick run command for the touching layer (backend or frontend)
- **After every plan wave:** Run full suite for the layer that wave touched
- **Before `/gsd-verify-work`:** Full suite (both layers) green; manual hardware smoke test for ENRL-03/04 logged
- **Max feedback latency:** 60 seconds for quick, 300 seconds for full

---

## Per-Task Verification Map

> Filled by planner. Every task in 07-01-PLAN.md and 07-02-PLAN.md must appear here with an automated verify command OR a Wave 0 stub reference. The planner is responsible for binding `T-NN` threat refs once the threat model lands.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-01-W0-01 | 01 | 0 | ENRL-01..05 | — | n/a | scaffold | `test -f backend/tests/enrollments_test.rs` | ❌ W0 | ⬜ pending |
| 07-01-W0-02 | 01 | 0 | ENRL-04 | — | n/a | scaffold | `test -f backend/tests/multi_device_push_test.rs` | ❌ W0 | ⬜ pending |
| 07-02-W0-01 | 02 | 0 | ENRL-02 | — | n/a | scaffold | `test -f frontend/src/components/enrollment/__tests__/enrollment-modal.test.tsx` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `backend/tests/enrollments_test.rs` — integration test stubs for `POST /api/v1/enrollments` (multipart accept, downscale loop, JoinSet fan-out)
- [ ] `backend/tests/multi_device_push_test.rs` — stubs for D-06 concurrency + D-08 partial failure semantics + D-16 backfill cap (Semaphore=4)
- [ ] `backend/tests/face_capture_test.rs` — stubs for D-02 2-step capture-from-device state machine
- [ ] `backend/tests/enrollment_lifecycle_test.rs` — stubs for D-14 (re-enroll), D-15 (purge on deactivate), D-16 (backfill on new device)
- [ ] `backend/tests/common/mod.rs` — shared fixtures: mock Hikvision ISAPI server returning canned `Record` + `FaceDataRecord` responses, sample 200KB JPG, sample 4MB JPG (downscale input)
- [ ] `frontend/src/components/enrollment/__tests__/enrollment-modal.test.tsx` — stubs for tab switching, AI validation gating, per-device sync polling, modal-close persistence
- [ ] `frontend/src/components/enrollment/__tests__/ai-validation.test.tsx` — stubs for `@vladmandic/face-api` lazy load + the 3 quality checks
- [ ] If `vitest.config.ts` lacks `happy-dom` setup: install + configure as part of W0

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real Hikvision DS-K1T34x face enrollment via `Record` + `FaceDataRecord` | ENRL-03 | Requires real device + admin physical presence; ISAPI multipart format varies by firmware (research A1, A2) | (1) Register a real DS-K1T341/342 in dev env. (2) Enroll via webcam tab. (3) Confirm `device_face_mappings` row written, status=success. (4) Confirm employee can clock in at the device using the enrolled face. |
| Kiosk capture-from-device 2-step flow (D-02) on real hardware | ENRL-01 | Device-side capture timing + JPG retrieval format only verifiable on hardware | (1) Open enrollment modal, choose Lector Hikvision tab. (2) Select registered device, click Iniciar Captura. (3) Walk to device, present face. (4) Confirm preview JPG appears in modal. (5) Click Aceptar, confirm push to all devices. |
| Phone-camera upload tab with 4MB photo + downscale | ENRL-02 | End-to-end requires real upload + ISAPI accept | (1) Capture photo with phone (4MB+). (2) Drag to upload tab. (3) Submit. (4) Verify backend logs show downscale loop converged ≤200KB. (5) Verify Hikvision device accepts and face_id mapping created. |
| Auto-purge on employee deactivation (D-15) | ENRL-04 | Cross-system: SQLite trigger → tokio worker → ISAPI delete | (1) Enroll employee, confirm `device_face_mappings`. (2) Deactivate employee. (3) Tail logs, confirm `UserInfoDetail/Delete` ISAPI calls per device. (4) Confirm mapping rows gone or `state=pending_delete`. |
| Auto-backfill on new device registration (D-16) | ENRL-04 | Requires real new device + Semaphore concurrency observation | (1) Register new device. (2) Watch backend logs for backfill job — confirm ≤4 ISAPI calls in flight. (3) Confirm all active employees' faces land on new device. |
| Modal close mid-sync, app-level toast persists (D-09) | ENRL-05 | Background TanStack Query polling behavior across screens | (1) Start enrollment. (2) Close modal during sync. (3) Verify toast/badge "Enrolamiento en curso — X/Y dispositivos". (4) Navigate to other screens, badge persists. (5) Reopen enrollment, see terminal status. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (planner binds during plan generation)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags (`--watch`, `--ui`) in any verify command
- [ ] Feedback latency < 60s for quick, < 300s for full
- [ ] `nyquist_compliant: true` set in frontmatter (planner flips after binding)

**Approval:** pending
