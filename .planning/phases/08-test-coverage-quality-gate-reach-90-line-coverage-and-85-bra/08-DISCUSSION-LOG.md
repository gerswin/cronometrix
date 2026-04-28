# Phase 8: Test Coverage & Quality Gate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-28
**Phase:** 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
**Areas discussed:** CI platform & pipeline, Coverage exclusions philosophy, Gate behavior on miss, leave_tests cwd fix scope

---

## CI Platform & Pipeline

### Q1: Where does the coverage threshold gate run?

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Actions (Recommended) | Add .github/workflows/ci.yml. Runs on push/PR. Standard for OSS-style repos. Free for public, generous for private. Fits the on-prem product model (CI is dev-side, not deployment). | |
| Local-only via Makefile/justfile | Provide make coverage / just coverage targets that run cargo-llvm-cov + vitest with thresholds. No remote CI. Devs run before pushing. Simplest, no infra. Risk: gate is honor-system. | |
| Both: local target + GitHub Actions | Ship a Makefile/justfile target that runs locally AND a GitHub Actions workflow that runs the same command. CI enforces, devs reproduce locally with one command. | ✓ |

**User's choice:** Both: local target + GitHub Actions

### Q2: Which events trigger the coverage gate?

| Option | Description | Selected |
|--------|-------------|----------|
| Every push + PR to main (Recommended) | Run on every push to any branch + every PR. Catches regressions early. Standard. | ✓ |
| PR to main only | Only run when opening/updating PRs targeting main. Saves CI minutes on feature branch pushes. Slower feedback during dev. | |
| PR + manual workflow_dispatch | PR-triggered + manual button to re-run on demand. No push-trigger noise. | |

**User's choice:** Every push + PR to main

### Q3: Upload coverage reports as CI artifacts?

| Option | Description | Selected |
|--------|-------------|----------|
| HTML reports as workflow artifacts (Recommended) | Upload backend lcov.html + frontend coverage/index.html as workflow artifacts. Zero external service. Click in GitHub UI to download. No Codecov account needed. | ✓ |
| Codecov / Coveralls integration | Push to external service for PR comments + history graph. Requires account + token. Nicer UX but adds external dep. | |
| Terminal summary only | Print coverage summary to CI log. No artifacts, no uploads. Minimal noise. | |

**User's choice:** HTML reports as workflow artifacts

---

## Coverage Exclusions Philosophy

### Q1: How aggressive should exclusions be to reach 90% line / 85% branch?

| Option | Description | Selected |
|--------|-------------|----------|
| Pragmatic exclusions (Recommended) | Exclude what genuinely shouldn't be tested: main.rs/bin entrypoints, generated code, build.rs, trivial Display/Debug derives, panic-only error paths. Keep all business logic and handlers in scope. Add tests where coverage falls short. | |
| Minimal exclusions, write more tests | Exclude only main.rs and generated code. Force everything else to be tested. Higher quality bar but bigger scope (likely many new tests). | ✓ |
| Aggressive exclusions to hit bar fast | Exclude entrypoints, error variants, Display impls, dev-only modules, anything < 5 lines. Fastest path to 90% but lowers actual quality signal. | |

**User's choice:** Minimal exclusions, write more tests

### Q2: Frontend: which paths to include in coverage?

| Option | Description | Selected |
|--------|-------------|----------|
| src/components, src/hooks, src/lib only (Recommended) | Cover reusable units: components/, hooks/, lib/, utils/. Exclude src/app/ (Next.js route shells), shadcn/ui copies (vendored), styles, type-only files. Targets the logic that benefits most from testing. | ✓ |
| Everything except shadcn/ui | Include src/app/ route pages too. Higher bar — need page-level integration tests. More work but more confidence. | |
| src/lib + src/hooks only (logic) | Strictest: only test pure logic. Components excluded. Easy to hit threshold but zero component-level confidence. | |

**User's choice:** src/components, src/hooks, src/lib only

### Q3: Backend: how to handle integration test paths in coverage?

| Option | Description | Selected |
|--------|-------------|----------|
| Combined unit + integration coverage (Recommended) | Run cargo-llvm-cov over both src/ unit tests AND tests/ integration tests. Combined report. Excludes tests/common/ helpers. Realistic view of what's exercised. | ✓ |
| Unit tests only | Cover only #[cfg(test)] inline modules. Faster but undercount — most behavior in this repo is exercised via tests/. Likely fails target. | |
| Integration tests only | Skip inline unit tests; report coverage from tests/ runs. Misses small inline tests. | |

**User's choice:** Combined unit + integration coverage

---

## Gate Behavior on Miss

### Q1: What happens when coverage falls below threshold?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard fail — block merge (Recommended) | CI exits non-zero, PR cannot merge. Standard quality gate. Forces tests to land with code. Aligns with audit-compliance goal of the product. | ✓ |
| Soft warn — PR comment, no block | Print warning, don't fail CI. Allows merge with regression. Trust-based. | |
| Hard fail with manual override label | Fail by default; an admin can apply override-coverage label to bypass for a single PR. Escape hatch for emergencies. | |

**User's choice:** Hard fail — block merge

### Q2: Per-file ratchet or project-wide threshold?

| Option | Description | Selected |
|--------|-------------|----------|
| Project-wide thresholds (Recommended) | Single 90/85 number for whole backend, single 90/85 for whole frontend. Simple. Allows trade-offs across files. | |
| Per-file minimum + project-wide | Project must meet 90/85 AND every file must hit a lower floor (e.g., 70%). Catches outliers — stops one untested file from hiding behind well-tested ones. | ✓ |
| Per-crate / per-package thresholds | Set thresholds per backend crate (cronometrix-api, etc.) and per frontend area. More granular policy. More config to maintain. | |

**User's choice:** Per-file minimum + project-wide

### Q3: Allow regression below current value?

| Option | Description | Selected |
|--------|-------------|----------|
| Block any drop below thresholds (Recommended) | If line < 90 or branch < 85, fail. No regression allowed below the contract. Doesn't track current value — just the floor. | ✓ |
| Ratchet — never go below current measured value | Store current coverage and reject any PR that drops it. Stricter than threshold. Requires baseline file in repo. | |

**User's choice:** Block any drop below thresholds

### Q4: What's the per-file floor?

| Option | Description | Selected |
|--------|-------------|----------|
| 70% line / 60% branch (Recommended) | Loose floor — catches truly untested files (e.g., 0%) without forcing every file to 90%. Allows variation across files while preventing dark spots. | ✓ |
| 80% line / 70% branch | Tighter — most files must be well-tested. May force tests on files that are inherently low-value to test. | |
| 50% line / 40% branch | Loosest — only catches near-zero files. Easier to maintain but lets sloppy files pass. | |

**User's choice:** 70% line / 60% branch

---

## leave_tests cwd Fix Scope

### Q1: How to fix the leave_tests cwd-dependent failure?

| Option | Description | Selected |
|--------|-------------|----------|
| Inject leaves_root into AppState (Recommended) | Promote leaves_root from process-global env-var lookup to a field on AppState/Config. Tests build AppState with a tempdir path. Eliminates env-var race AND cwd dependence in one shot. Mirror the same fix for events_root since it has the same pattern. Larger blast radius but root-cause fix. | ✓ |
| Surgical: serialize tests + force absolute cwd | Add a #[serial] guard / global mutex around tests using LeavesRootGuard. Set absolute path in default. Smallest patch, leaves env-var pattern in place. Risk: same bug recurs if new tests are added without the guard. | |
| Make default an absolute path under target/ | Default leaves_root to an absolute path (e.g., resolved from CARGO_MANIFEST_DIR). Tests still use guards. Removes cwd dep but env-var race remains. | |

**User's choice:** Inject leaves_root into AppState

### Q2: Apply the same fix to events_root and any other relative-path roots?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — sweep all relative-path roots (Recommended) | Audit src/ for ./data/* defaults. Apply same AppState injection pattern to events_root and any siblings. Prevents the bug from recurring on future test runs. | ✓ |
| Only fix leaves_root in this phase | Stay narrow. Other roots get fixed when they break. Faster but technical debt remains. | |

**User's choice:** Yes — sweep all relative-path roots

---

## Claude's Discretion

- Choice between `Makefile` vs `justfile` (default Makefile).
- Exact mechanism for per-file floor enforcement on the backend (post-process script vs cargo-llvm-cov flag).
- Whether to factor coverage commands into a shared shell helper or duplicate.
- Specific test additions to close the coverage gap.
- Whether `Config` gains the path fields directly or a new `Paths` substruct holds them.
- Order of operations within the phase.

## Deferred Ideas

- Codecov / Coveralls integration
- Ratchet baseline ("never go below current")
- Mutation testing (cargo-mutants)
- E2E / Playwright tests for `src/app/` route pages
- Per-crate / per-package thresholds
- Performance benchmarks
- Property-based test expansion beyond threshold needs
- Snapshot tests for API responses
- Manual override label for emergency merges
