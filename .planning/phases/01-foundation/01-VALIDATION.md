---
phase: 1
slug: foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + tokio::test for async |
| **Config file** | Cargo.toml `[dev-dependencies]` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | DATA-01 | — | Schema creates all tables | integration | `cargo test schema` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 1 | DATA-02 | — | Audit triggers fire on mutations | integration | `cargo test audit` | ❌ W0 | ⬜ pending |
| 1-01-03 | 01 | 1 | DATA-03 | — | UTC epoch storage verified | unit | `cargo test epoch` | ❌ W0 | ⬜ pending |
| 1-01-04 | 01 | 1 | DATA-04 | — | Turso sync connects | integration | `cargo test turso` | ❌ W0 | ⬜ pending |
| 1-02-01 | 02 | 1 | AUTH-01 | T-1-01 | Login returns JWT | integration | `cargo test auth_login` | ❌ W0 | ⬜ pending |
| 1-02-02 | 02 | 1 | AUTH-02 | T-1-02 | Argon2id hashing verified | unit | `cargo test password` | ❌ W0 | ⬜ pending |
| 1-02-03 | 02 | 1 | AUTH-03 | T-1-03 | RBAC middleware blocks unauthorized | integration | `cargo test rbac` | ❌ W0 | ⬜ pending |
| 1-02-04 | 02 | 1 | AUTH-04 | — | JWT refresh works | integration | `cargo test refresh` | ❌ W0 | ⬜ pending |
| 1-02-05 | 02 | 1 | AUTH-05 | — | Setup wizard creates admin | integration | `cargo test setup` | ❌ W0 | ⬜ pending |
| 1-03-01 | 03 | 2 | EMP-01 | — | CRUD employee endpoints | integration | `cargo test employee` | ❌ W0 | ⬜ pending |
| 1-03-02 | 03 | 2 | EMP-02 | — | Soft-delete only, no hard-delete | integration | `cargo test soft_delete` | ❌ W0 | ⬜ pending |
| 1-03-03 | 03 | 2 | EMP-03 | — | Search and filter employees | integration | `cargo test employee_search` | ❌ W0 | ⬜ pending |
| 1-03-04 | 03 | 2 | EMP-04 | — | Employee-department constraint | integration | `cargo test emp_dept` | ❌ W0 | ⬜ pending |
| 1-03-05 | 03 | 2 | DEPT-01 | — | CRUD department endpoints | integration | `cargo test department` | ❌ W0 | ⬜ pending |
| 1-03-06 | 03 | 2 | DEPT-02 | — | Department has salary/schedule/lunch | unit | `cargo test dept_fields` | ❌ W0 | ⬜ pending |
| 1-03-07 | 03 | 2 | DEPT-03 | — | 1:1 employee-department enforced | integration | `cargo test dept_constraint` | ❌ W0 | ⬜ pending |
| 1-04-01 | 04 | 2 | RULE-01 | — | Tolerance sliders endpoint | integration | `cargo test rules` | ❌ W0 | ⬜ pending |
| 1-04-02 | 04 | 2 | RULE-02 | — | Bonus minutes config | integration | `cargo test bonus` | ❌ W0 | ⬜ pending |
| 1-04-03 | 04 | 2 | RULE-03 | — | Rules take effect next cycle only | integration | `cargo test rule_cycle` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/common/mod.rs` — shared test fixtures (in-memory libSQL database, test auth tokens)
- [ ] `tests/integration/` — integration test directory structure
- [ ] Dev dependencies: `tower` (test features), `serde_json`, `http-body-util`

*Existing infrastructure covers framework (cargo test built-in).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Browser refresh preserves login | AUTH-01 | Requires browser context | Login via API, store JWT in cookie, verify subsequent requests succeed |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
