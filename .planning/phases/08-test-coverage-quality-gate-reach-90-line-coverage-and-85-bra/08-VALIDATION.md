---
phase: 8
slug: test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-28
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (backend)** | cargo-nextest + cargo-llvm-cov (Rust 1.77+ stable for tests; nightly toolchain in coverage CI job for `--branch`) |
| **Framework (frontend)** | Vitest 4.1.5 + @vitest/coverage-v8 4.1.5 |
| **Config file (backend)** | `backend/Cargo.toml` (test deps), new `Makefile`, new `scripts/enforce-coverage-floor.sh` |
| **Config file (frontend)** | `frontend/vitest.config.ts` (extend with `coverage.thresholds` + `coverage.thresholds.perFile`) |
| **Quick run command (backend)** | `cd backend && cargo nextest run` |
| **Quick run command (frontend)** | `cd frontend && npm run test -- --run` |
| **Full suite command (backend)** | `make coverage-backend` |
| **Full suite command (frontend)** | `make coverage-frontend` |
| **Combined coverage gate** | `make coverage` (runs backend + frontend; fails on threshold miss) |
| **Estimated runtime** | ~120s backend coverage, ~60s frontend coverage, ~30s nextest unit |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run` (touched crate) or `npm run test -- --run` (touched dir)
- **After every plan wave:** Run `make coverage-backend` or `make coverage-frontend` for the side that changed
- **Before `/gsd-verify-work`:** `make coverage` must exit 0 with project-wide AND per-file thresholds met
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | 01–N | 1–M | (none — quality gate) | — | — | unit/integration/CI | `cargo nextest run` / `npm run test -- --run` / `make coverage` | ✅ / ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `backend/src/state/` — add `Paths` struct (or fields on `AppState`) with `leaves_root`, `events_root`, `enrollments_root`, `captures_tmp_root`, `data_dir` (overrides) — D-18, D-19
- [ ] `backend/src/leaves/service.rs` — `leaves_root()` reads from `&AppState` instead of env
- [ ] `backend/src/events/service.rs` — `events_root()` reads from `&AppState`; remove inline `static Mutex<()>` test guard
- [ ] `backend/src/daily_records/handlers.rs:201-203` — replace inline `DATA_DIR` env read with AppState field
- [ ] `backend/tests/common/test_state.rs` — helper that builds `AppState` with `tempfile::TempDir` paths held alive for test scope
- [ ] `backend/tests/leave_tests.rs` — remove `LeavesRootGuard`, switch to test_state helper
- [ ] `backend/tests/event_tests.rs` — remove `EventsRootGuard`, switch to test_state helper
- [ ] `backend/tests/listener_tests.rs` — remove `EventsRootGuard`, switch to test_state helper
- [ ] `frontend/vitest.config.ts` — add `coverage.thresholds` (lines 90, branches 85) + `coverage.thresholds.perFile` (lines 70, branches 60) + `coverage.include` / `coverage.exclude`
- [ ] `Makefile` — `coverage`, `coverage-backend`, `coverage-frontend` targets
- [ ] `scripts/enforce-coverage-floor.sh` — lcov.info post-processor (per-file 70/60 floor)
- [ ] `.github/workflows/ci.yml` — backend job + frontend job, both required

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| HTML coverage report renders correctly in browser | D-04 | Visual rendering of `target/llvm-cov/html/index.html` and `frontend/coverage/index.html` | After `make coverage`, open both files in browser; confirm file tree, drill-down, and per-line annotation work |
| GitHub Actions workflow artifact upload visible in PR UI | D-04 | Requires real PR run | Open a draft PR; verify workflow run uploads "backend-coverage" + "frontend-coverage" artifacts that download correctly |
| CI gate hard-fails on coverage drop | D-13, D-15 | Requires deliberate red PR to validate | Open a PR that intentionally deletes a high-coverage test file; confirm CI red, merge button blocked |
| Per-file floor catches a 0% file | D-14 | Requires synthetic violation | Open a PR that introduces a new module with no tests; confirm `enforce-coverage-floor.sh` exit≠0 even when project-wide ≥90% |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (AppState injection lands before coverage measurement)
- [ ] No watch-mode flags (`--watch`, `--ui`) in CI commands
- [ ] Feedback latency < 120s for unit/nextest, < 5min for full coverage
- [ ] `nyquist_compliant: true` set in frontmatter after planner fills the per-task map

**Approval:** pending
