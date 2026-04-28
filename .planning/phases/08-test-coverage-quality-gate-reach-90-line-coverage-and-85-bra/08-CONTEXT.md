# Phase 8: Test Coverage & Quality Gate - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase establishes a project-wide test-coverage quality gate. It delivers:

1. **Backend coverage** — `cargo-llvm-cov` instrumentation over both `src/` (inline `#[cfg(test)]`) and `tests/` (integration) producing a single combined report. Threshold: ≥90% line, ≥85% branch.
2. **Frontend coverage** — Vitest + `@vitest/coverage-v8` (already installed: `vitest 4.1.5`, `@vitest/coverage-v8 4.1.5`). Threshold: ≥90% line, ≥85% branch.
3. **Per-file floor** — On top of the project-wide thresholds, every counted file must hit ≥70% line / ≥60% branch (catches dark spots — files at 0% can't hide behind well-tested ones).
4. **CI gate** — A new `.github/workflows/ci.yml` that runs the backend + frontend coverage commands on every push and every PR to `main`, hard-fails (non-zero exit, blocks merge) when any threshold is missed, and uploads the HTML coverage reports as workflow artifacts.
5. **Local reproducibility** — A `Makefile` (or `justfile`, planner picks one) target that runs the exact same coverage commands locally, so devs can reproduce CI before pushing.
6. **leave_tests cwd-dependent failure fix** — Root-cause fix: promote `leaves_root` from a process-global env-var/relative-path lookup to a field on `AppState`/`Config`. Sweep `events_root` and any sibling `./data/*` defaults so the cwd + env-var-race class of bug stops recurring.
7. **Documentation** — Add a "Test Coverage" section to `CLAUDE.md` documenting the local commands, thresholds, exclusion policy, and how to interpret the HTML reports.

**Out of scope:**
- Adding Codecov / Coveralls or any external coverage service (decision: HTML artifacts only)
- Ratchet-style "never decrease from current value" policy (decision: threshold-only)
- Mutation testing (cargo-mutants etc.)
- Property-test expansion beyond what's needed to meet the threshold
- E2E / Playwright / browser tests (frontend coverage is unit + integration via Vitest)
- Performance benchmarking
- Restructuring the existing test suite beyond what the leave_tests fix requires

</domain>

<decisions>
## Implementation Decisions

### CI Platform & Pipeline
- **D-01:** CI gate lives in **GitHub Actions** at `.github/workflows/ci.yml`. The repo currently has no `.github/workflows/` directory — this is greenfield.
- **D-02:** A `Makefile` (planner may substitute a `justfile` if it surveys the repo and finds one preferred — Makefile is the default) ships the same coverage commands so devs reproduce CI locally with one invocation. Suggested targets: `make coverage`, `make coverage-backend`, `make coverage-frontend`.
- **D-03:** CI triggers: **`push` to any branch** AND **`pull_request` targeting `main`**. Catches regressions early on feature branches; PR run is the merge-blocking instance.
- **D-04:** Coverage reports are uploaded as **GitHub Actions workflow artifacts** (HTML for both backend `lcov.html` and frontend `coverage/index.html`). No external service (no Codecov, no Coveralls).
- **D-05:** CI must run backend coverage and frontend coverage as **separate jobs** so failures are attributable and cache keys are independent. Both jobs are required for the gate to pass.

### Coverage Tooling
- **D-06:** Backend uses **`cargo-llvm-cov`**. Add it to CI via `taiki-e/install-action@cargo-llvm-cov` (or equivalent). Local devs install via `cargo install cargo-llvm-cov` (document in CLAUDE.md). It is NOT a Cargo dependency — it is a tool.
- **D-07:** Backend coverage runs over **both unit and integration tests in a single combined run** (`cargo llvm-cov --workspace --all-features`). The combined report is what the gate evaluates. Excludes `tests/common/` helpers from the denominator.
- **D-08:** Frontend uses **Vitest's built-in v8 coverage** (`vitest run --coverage`). Already installed.

### Coverage Scope & Exclusions Philosophy
- **D-09:** **Exclusions are minimal — write tests instead of shrinking the denominator.** The 90/85 bar must be reached by adding tests, not by hiding code. Allowed exclusions are limited to genuinely-uncoverable code:
  - Backend: `main.rs` / binary entrypoints, `build.rs` (if any), generated code, dead `Display`/`Debug` derives, and unreachable error variants only after explicit justification per case.
  - Frontend: vendored shadcn/ui components in `src/components/ui/` (these are upstream copies — covered by upstream), Next.js boilerplate (`layout.tsx` shells with no logic), pure type files, generated code, MSW handlers if used as test infra.
- **D-10:** **Frontend coverage scope:** include `src/components/`, `src/hooks/`, `src/lib/`. Exclude `src/app/` (Next.js route pages — covered by future E2E phase, not Vitest), vendored `src/components/ui/` shadcn copies, type-only files. Configured via Vitest `coverage.include` / `coverage.exclude`.
- **D-11:** **Backend coverage scope:** all of `src/`. Exclude `src/main.rs`, `src/bin/*`, and `tests/common/*`. Anything else needs an explicit justification comment + listed in CLAUDE.md exclusions section.
- **D-12:** Filling the gap: the planner is expected to **identify the current coverage delta and propose which modules need new tests** to reach the bar. Likely candidates based on the repo: handler error paths, validator-failure branches, ISAPI digest-auth retry paths, license-gate edge cases, frontend hooks not yet covered.

### Gate Behavior
- **D-13:** **Hard fail on miss** — CI exits non-zero, PR cannot merge. No "soft warn" mode. No manual override label. Aligns with the audit-compliance ethos of the product.
- **D-14:** **Two-level threshold:** project-wide AND per-file.
  - Project-wide: ≥90% line, ≥85% branch (backend); ≥90% line, ≥85% branch (frontend). One number per side, not per-crate.
  - Per-file floor: ≥70% line, ≥60% branch. Every counted file must hit this. Catches files at 0% that the project-wide average would otherwise paper over.
- **D-15:** **No ratcheting.** The gate compares against the fixed threshold, not against a stored baseline. A PR that drops coverage from 95% → 91% passes (still above 90); from 91% → 89% fails.
- **D-16:** Per-file floor enforcement: cargo-llvm-cov supports `--fail-under-functions` / `--fail-under-lines` for project-wide. For per-file, the planner should pick the simplest mechanism — a small post-processing script that parses the `lcov.info` output is acceptable, as is a config-file approach if cargo-llvm-cov gains it. Frontend Vitest config natively supports `coverage.thresholds.perFile`.

### leave_tests cwd Fix
- **D-17:** **Root cause:** `backend/src/leaves/service.rs::leaves_root()` reads env `CRONOMETRIX_LEAVES_ROOT` and falls back to the **relative path** `./data/leaves`. This is cwd-dependent (fails when cargo-llvm-cov / nextest run from a different cwd) AND racy (env vars are process-global; parallel tests using `LeavesRootGuard` clobber each other). The same anti-pattern exists in `events_root()`.
- **D-18:** **Fix approach: AppState injection.** Promote `leaves_root` from a free function reading process env to a field on `AppState` (or `Config`, planner decides which fits the existing layering). Production startup populates it from env-or-default. Tests build `AppState` with a tempdir path. Handlers and services receive the path via `State<AppState>` instead of calling a global function.
- **D-19:** **Sweep scope:** Apply the same AppState-injection pattern to `events_root` and any other `./data/*` defaults discovered during the audit (planner runs a grep for `PathBuf::from("./` and `env::var(.*ROOT)` in `backend/src/`). One pattern, one fix, one phase — don't leave a sibling time bomb.
- **D-20:** **Test-side cleanup:** Once roots are injected, remove the `LeavesRootGuard` / `EventsRootGuard` env-var manipulation from tests. Tests just pass the tempdir path when constructing AppState. This eliminates `#[serial]` requirements and the env-var race in one move.
- **D-21:** **Backwards compatibility:** Production startup behavior is preserved — same env vars, same defaults read once at startup, same on-disk layout.

### Documentation
- **D-22:** Add a **"Test Coverage"** section to root `CLAUDE.md` documenting: install commands (cargo-llvm-cov), local commands (`make coverage`, `make coverage-backend`, `make coverage-frontend`), thresholds (project + per-file), exclusion policy with rationale, where the HTML report lands locally, and how the CI gate works (which workflow file, which jobs).
- **D-23:** Document the `leaves_root` / `events_root` AppState pattern in `CLAUDE.md` Conventions section so future code that needs filesystem roots follows the same injection pattern.

### Claude's Discretion
- Choice between `Makefile` vs `justfile` (default Makefile; switch if planner finds the repo already trends toward `just`).
- Exact mechanism for per-file floor enforcement on the backend (post-process `lcov.info` script vs newer cargo-llvm-cov flag if available at planning time).
- Whether to factor coverage commands into a small shell helper (`scripts/coverage.sh`) that both Make and CI invoke, or duplicate the command in both. Planner picks based on command length / DRY pressure.
- Specific test additions to close the coverage gap — planner identifies modules below 90/85 and proposes targeted tests.
- Whether the `Config` struct gains the path fields directly or a new `Paths` substruct holds them.
- Order of operations within the phase: leave_tests/sweep fix likely first (so coverage runs cleanly), then exclusion config, then add tests, then CI gate, then docs.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & requirements
- `.planning/PROJECT.md` — Stack constraints, audit-compliance ethos that drives the hard-fail gate
- `.planning/REQUIREMENTS.md` — All v1 requirements; phase 8 has no REQ-IDs of its own but enforces quality across all of them
- `.planning/STATE.md` — Current phase status; phase 8 is the new quality-gate work

### Current codebase relevant to this phase
- `backend/Cargo.toml` — Existing test deps (axum-test, proptest, wiremock, tempfile); cargo-llvm-cov is NOT a dep, it's a tool
- `backend/src/leaves/service.rs` §leaves_root (lines 28-32) — The buggy relative-path/env-var pattern to be replaced
- `backend/src/leaves/handlers.rs` — Call sites for `service::leaves_root()` (lines 167, 276) — must accept injected path
- `backend/src/events/service.rs` §events_root — Same anti-pattern, included in the sweep
- `backend/tests/leave_tests.rs` §LeavesRootGuard (lines ~45-60) — Process-global env-var guard pattern to be removed once roots are injected
- `backend/tests/common/` — Test harness helpers; excluded from coverage denominator
- `frontend/vitest.config.ts` — Current Vitest config (no coverage.thresholds yet) — extend with thresholds + include/exclude
- `frontend/package.json` — Confirms `vitest@4.1.5` + `@vitest/coverage-v8@4.1.5` already installed; no install needed

### Codebase intel (already analyzed)
- `.planning/codebase/STACK.md` — Stack versions, frameworks
- `.planning/codebase/STRUCTURE.md` — Repo layout
- `.planning/codebase/ARCHITECTURE.md` — Layering, AppState pattern
- `.planning/codebase/INTEGRATIONS.md` — External integrations (relevant for which paths get tested)

### External docs (planner should fetch via WebFetch / Context7 during research)
- `cargo-llvm-cov` README — install, basic usage, threshold flags, lcov output: https://github.com/taiki-e/cargo-llvm-cov
- Vitest coverage docs — `coverage.thresholds`, `coverage.thresholds.perFile`, `coverage.include`/`exclude`: https://vitest.dev/guide/coverage
- `taiki-e/install-action` — preferred GitHub Action for installing cargo-llvm-cov in CI: https://github.com/taiki-e/install-action

### Project conventions
- `/Users/gerswin/Proyectos/cronometrix/CLAUDE.md` — Backend stack table; existing tooling table (cargo-nextest mentioned). New "Test Coverage" section gets appended here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`AppState` pattern** (`backend/src/state/`) — Already used to thread DB connections, JWT secret, config into handlers. Adding `leaves_root: PathBuf` and `events_root: PathBuf` fields fits the established shape; no new abstraction needed.
- **`tempfile::TempDir`** (already in dev-deps) — Tests can build per-test tempdirs and pass them into AppState directly. No new crate needed.
- **`cargo-nextest`** (mentioned in CLAUDE.md tooling) — cargo-llvm-cov supports `--cargo-test-runner=nextest`; combine for speed.
- **Vitest setup file** (`frontend/src/__tests__/setup.ts`) — Already wired; no setup-file changes likely needed beyond config.
- **`@testing-library/react` + `@testing-library/jest-dom` + `msw`** — Already installed; component test infra ready for the gap-filling tests.

### Established Patterns
- **`#[tokio::test]` + `axum-test`** for handler integration tests — Standard pattern; new tests added to close the coverage gap follow the same shape.
- **Tests live in `backend/tests/*.rs`** with shared helpers in `backend/tests/common/`. No `#[cfg(test)] mod tests` proliferation in `src/` for handler/service code (though some pure functions have inline tests).
- **`thiserror`-derived error enums** at module boundaries — Coverage gap is often in error variants (e.g., DB conflict branch, validation-fail branch). Planner should target these.
- **Atomic file writes** (`write_photo_atomic`) — Enrollment + leaves both use this pattern; coverage of the error path matters for compliance.

### Integration Points
- New `.github/workflows/ci.yml` is the integration entry — no existing workflows to extend.
- New top-level `Makefile` — no existing one (verify during research; if exists, extend).
- `CLAUDE.md` gains a new section — does not replace existing content.
- `AppState` struct (location: `backend/src/state/`) gains fields — every test that constructs AppState needs a one-line update (tempdir path).

### Risks / Watch-outs
- The `LeavesRootGuard`/`EventsRootGuard` env-var pattern is used across multiple test files. Removing it requires touching every call site. Planner must enumerate them in research.
- `tempfile::TempDir` is dropped at end of scope — tests that retain a path string but drop the TempDir will see the dir disappear. New AppState pattern must keep the TempDir alive for the test's lifetime.
- Per-file floor on cargo-llvm-cov may require a custom script — Vitest supports it natively. Asymmetry is acceptable.
- Coverage runs are slower than `cargo test`. CI job timeout must be raised if needed.
- `cargo-llvm-cov` requires `llvm-tools-preview` rustup component — install step in CI must include it.
- `next/font` and other Next.js features may produce uncoverable shims; exclude as discovered.

</code_context>

<specifics>
## Specific Ideas

- The phase number/name is unusually long because it embeds the goal in the title. Don't try to rename it during planning.
- Branch coverage on Rust via `cargo-llvm-cov` historically uses `--branch` (recently `--branch` is supported behind a flag — planner verifies current state during research).
- Frontend coverage exclusions go through Vitest config (`coverage.exclude`), NOT through `.gitignore` or any Next.js config.
- The `bruno/` collection and `docs/` directory are NOT in coverage scope — neither produces code that runs in tests.
- The repo currently has untracked changes (new test directories, bruno docs, etc.). The phase should not depend on those landing first; plan around the committed state.
- `frontend/src/__tests__/setup.ts` already exists — extend, don't recreate.
- Avoid the temptation to also fix unrelated test flakiness in this phase. If a test is flaky but unrelated to cwd/env-var roots, file it as deferred.

</specifics>

<deferred>
## Deferred Ideas

- **Codecov/Coveralls integration** — Decided against for v1 of the gate; HTML artifacts are sufficient. Revisit if the team wants PR-comment coverage diffs.
- **Ratchet baseline** — Decided against; threshold-only is simpler and the threshold itself is high. Revisit if drift becomes a problem.
- **Mutation testing** (cargo-mutants) — Out of scope; line + branch coverage is the contract for this phase.
- **E2E / Playwright tests covering `src/app/` route pages** — Excluded from this phase's frontend coverage scope; belongs to a future phase.
- **Per-crate or per-package thresholds on the backend** — Decided against; one project-wide number per side. Revisit if the workspace grows multiple crates with very different testing characteristics.
- **Performance benchmarks / cargo-criterion gate** — Out of scope.
- **Property-based test expansion beyond what's needed to hit the threshold** — Out of scope.
- **Snapshot tests for serialized API responses** — Out of scope (could be a future phase to lock the API contract).
- **Manual override label for emergency merges** — Decided against; gate is hard-fail. Revisit only if a real emergency justifies the escape hatch.

</deferred>

---

*Phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra*
*Context gathered: 2026-04-28*
