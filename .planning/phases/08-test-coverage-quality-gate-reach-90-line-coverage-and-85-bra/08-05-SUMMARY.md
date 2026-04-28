---
phase: 08-test-coverage-quality-gate
plan: 05
status: complete-pending-validation
subsystem: infra
tags: [ci, github-actions, coverage, quality-gate, cargo-llvm-cov, vitest]

requires:
  - phase: 08
    provides: "Local coverage tooling (Makefile + scripts/enforce-coverage-floor.sh + rust-toolchain.toml + vitest.config.ts thresholds) and a green coverage baseline (Plan 03/04)"
provides:
  - "GitHub Actions workflow at .github/workflows/ci.yml that runs backend + frontend coverage in parallel on every push and every PR to main"
  - "Hard-fail behavior on threshold miss (no soft-warn, no override label) — D-13"
  - "HTML coverage artifacts uploaded with retention 14 days for both jobs (if: always())"
  - "Least-privilege workflow GITHUB_TOKEN scope (contents: read) per threat model T-08-15"
affects: [08-06]

tech-stack:
  added:
    - "GitHub Actions workflow (yaml)"
    - "actions/checkout@v4"
    - "actions/setup-node@v4"
    - "actions/upload-artifact@v4"
    - "Swatinem/rust-cache@v2"
    - "taiki-e/install-action@v2 (cargo-llvm-cov@0.8.5, cargo-nextest)"
  patterns:
    - "CI mirrors local make commands verbatim (regex parity, threshold parity) — single source of truth for coverage gate"
    - "Two parallel coverage jobs, both REQUIRED, no shared state"
    - "if: always() on artifact upload so failed runs still produce a downloadable HTML report for triage"

key-files:
  created:
    - ".github/workflows/ci.yml"
  modified: []

key-decisions:
  - "CI validation deferred to manual follow-up per user direction — workflow file verified statically (grep + YAML parse), live validation tracked as unchecked checklist in Manual Follow-up section below"
  - "Pinned actions/checkout@v4 (not @v6 even if available) — v4 is the validated baseline for this gate; future bumps are deliberate, separate changes"
  - "permissions: contents: read at workflow level — minimum scope; workflow only reads source and uploads artifacts (no PR comments, no contents: write)"
  - "Exclusion regex parity enforced: CI step uses '(main\\.rs|tests/common/.*)' — identical to Makefile (verified via grep) so local and CI gates cannot drift"

patterns-established:
  - "Pattern 1: CI lives at .github/workflows/ci.yml — single workflow file for the project's quality gate; future workflows (release, deploy) get their own files, not jobs in this one"
  - "Pattern 2: Backend job uses defaults.run.working-directory: backend so the post-processor is invoked as 'bash ../scripts/enforce-coverage-floor.sh lcov.info 85 70 60' (relative ../ traversal from backend/)"
  - "Pattern 3: Frontend job uses defaults.run.working-directory: frontend, npm ci with npm cache keyed on frontend/package-lock.json, and 'npx vitest run --coverage' (vitest.config.ts enforces thresholds)"
  - "Pattern 4: HTML artifact upload uses if: always() — failed coverage runs still produce a triage-ready report (developer downloads HTML, drills into per-file column to find regressed file)"

requirements-completed: [QUALITY-GATE]

duration: 35min
completed: 2026-04-28
started: "2026-04-28T21:50:00Z"
---

# Phase 8 Plan 05: CI Quality Gate Summary

**GitHub Actions workflow committed at .github/workflows/ci.yml with two parallel coverage jobs (backend nightly + cargo-llvm-cov, frontend Node 20 + Vitest+v8), hard-failing on threshold miss, HTML artifacts uploaded for triage; live CI validation deferred to manual follow-up per user direction.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-28T21:50:00Z
- **Completed:** 2026-04-28T22:25:00Z
- **Tasks:** 1 of 2 executed (Task 2 — manual validation — deferred)
- **Files modified:** 1 (created)

## Accomplishments

- `.github/workflows/ci.yml` written from the verbatim RESEARCH § CI Workflow Skeleton, with action versions pinned per the plan's pin-verification rules (`@v4`, `@v2`, `cargo-llvm-cov@0.8.5`)
- Static verification passed: file exists, YAML parses, all grep-based acceptance criteria green (name: CI, push branches `**`, pull_request branches `[main]`, both jobs present, `--fail-under-lines 90`, post-processor invocation `85 70 60`, `actions/upload-artifact@v4`, `permissions: contents: read`)
- Exclusion regex parity confirmed: CI's `--ignore-filename-regex '(main\.rs|tests/common/.*)'` matches Makefile exactly — no drift between local and CI coverage scope
- Threat model mitigations applied at workflow level: T-08-14 (action pinning), T-08-15 (least-privilege GITHUB_TOKEN), T-08-17 (no secrets in PR-triggered runs), T-08-18 (rust-toolchain.toml controls actual nightly date)
- Plan 06 (CLAUDE.md docs) is unblocked — Plan 05's contract (workflow file in place) is satisfied; the live-validation steps do not gate Plan 06 because Plan 06 documents the design, not the runtime behavior

## Task Commits

1. **Task 1: Create .github/workflows/ci.yml with two parallel jobs and HTML artifact upload** — `d3e2802` (ci)
2. **Task 2: Manual gate validation** — DEFERRED (see Manual Follow-up section)

**Plan metadata:** _(this commit)_ — `docs(08-05): finalize CI gate plan with deferred manual validation`

## Files Created/Modified

- `.github/workflows/ci.yml` — GitHub Actions workflow defining backend-coverage + frontend-coverage jobs; triggers on push (any branch) + PR to main; uploads HTML artifacts; hard-fails on threshold miss; least-privilege GITHUB_TOKEN scope

## Decisions Made

- **Defer live CI validation** — User directed "Skip CI validation — finalize 05 SUMMARY now" at the human-verify checkpoint. The workflow file is correct based on inline review + grep checks + YAML parse. Real GitHub Actions runtime validation (positive run, negative regression PR, branch protection setup) is captured below as an explicit unchecked checklist for the user to execute.
- **Pin to actions/checkout@v4 (not @v6)** — Per plan's pin-verification rules, v4 is the validated baseline; bumping is a separate deliberate change with its own acceptance run.
- **Pin cargo-llvm-cov to 0.8.5** — Matches local development version (rust-toolchain.toml ecosystem); avoids drift between local `make coverage` and CI coverage.
- **`permissions: contents: read` at workflow level (not job level)** — Single declaration applies to both jobs; workflow has no need for write scopes (no PR comments, no auto-fix bots, artifact upload uses implicit action token).

## Deviations from Plan

**1. [User direction] Task 2 (manual gate validation) deferred to Manual Follow-up**
- **Found during:** Task 2 checkpoint (human-verify gate)
- **Issue:** User responded "Skip CI validation — finalize 05 SUMMARY now" instead of running the three validation steps (A positive, B negative regression, C branch protection)
- **Resolution:** Captured all three validation steps as an explicit, executable checklist in the Manual Follow-up section below. Status frontmatter set to `complete-pending-validation` to make the partial-completion state machine-readable.
- **Files modified:** `.planning/phases/.../08-05-SUMMARY.md` (this file)
- **Impact:** Plan 06 (CLAUDE.md docs) is NOT blocked — Plan 06 describes the design + conventions, not the runtime validation outcome. The manual checklist must complete before Phase 8 is declared truly green, but it does not gate Plan 06's documentation work.

---

**Total deviations:** 1 user-directed deferral (no auto-fixes; static verification passed cleanly on first write)
**Impact on plan:** Workflow file ships correct; runtime validation moves to a human-driven follow-up.

## Manual Follow-up

The following items MUST be completed by the user before Phase 8's quality gate is considered truly active. Each item includes the concrete steps so they can be executed without re-reading the plan.

### A. Positive verification — gate passes on a clean PR

- [ ] **Push the current branch and confirm both jobs run green**
  ```bash
  # From repo root, current branch
  git push -u origin "$(git rev-parse --abbrev-ref HEAD)"
  ```
  Then visit:
  - GitHub Actions tab: `https://github.com/<owner>/<repo>/actions`
  - Confirm both jobs appear in the workflow run: **Backend Coverage** and **Frontend Coverage**
  - Wait for both jobs to complete (backend ~10-15min cold, frontend ~3-5min)
  - Confirm both show green check (✓)
- [ ] **Confirm HTML artifacts are downloadable**
  - Click into the workflow run summary page
  - Scroll to the "Artifacts" section at the bottom
  - Confirm `backend-coverage-html` and `frontend-coverage-html` are both listed and downloadable
  - Download `backend-coverage-html.zip`, unzip, open `index.html` in a browser, confirm per-file drill-down works
  - Repeat for `frontend-coverage-html.zip`

### B. Negative regression PR — confirms the gate is not a no-op

Per RESEARCH § "Validating the Gate Itself" (lines 925-934):

- [ ] **Open a deliberate red PR and confirm it fails**
  ```bash
  # On a throwaway branch
  git checkout -b chore/coverage-gate-regression-test main
  cat > backend/src/dead_code.rs <<'EOF'
  pub fn dead_function_one() -> u32 {
      // Intentionally untested — should trip the per-file coverage floor.
      let x = 1 + 1;
      x * 2
  }

  pub fn dead_function_two(input: &str) -> usize {
      // Intentionally untested — second un-callable path.
      input.len()
  }
  EOF
  # Wire it into the lib so the file is actually compiled
  printf '\npub mod dead_code;\n' >> backend/src/lib.rs
  git add backend/src/dead_code.rs backend/src/lib.rs
  git commit -m "test: deliberate coverage regression to validate CI gate"
  git push -u origin chore/coverage-gate-regression-test
  gh pr create --base main --head chore/coverage-gate-regression-test \
    --title "TEST: validate CI gate fails on regression" \
    --body "Do not merge — proves the coverage gate hard-fails on sub-threshold files."
  ```
  Then verify on GitHub:
  - The `Backend Coverage` job FAILS at the "Enforce branch + per-file thresholds" step
  - Failure output contains `FAIL: backend/src/dead_code.rs line coverage 0.00% < floor 70%` (or equivalent message from `scripts/enforce-coverage-floor.sh`)
  - The PR's "Merge" button is BLOCKED (assuming branch protection from step C is configured)
- [ ] **Close the regression PR and delete the throwaway branch**
  ```bash
  gh pr close <pr-number>
  git push origin --delete chore/coverage-gate-regression-test
  git checkout main && git branch -D chore/coverage-gate-regression-test
  ```

### C. Branch protection — one-time GitHub UI configuration

- [ ] **Configure required status checks on `main`**
  Visit: `https://github.com/<owner>/<repo>/settings/branches`
  - Click **Add branch protection rule** (or edit the existing rule for `main`)
  - Branch name pattern: `main`
  - Enable: **Require a pull request before merging**
  - Enable: **Require status checks to pass before merging**
    - Click **Add status check** and search for / select:
      - `Backend Coverage`
      - `Frontend Coverage`
  - Enable: **Require branches to be up to date before merging** (recommended)
  - Click **Save changes**
- [ ] **Re-verify step B** — re-open the negative regression PR briefly and confirm the "Merge" button is now disabled (then close again).

### Tracking

- A project-level note in `CLAUDE.md` (added by Plan 06) will reference this checklist so future contributors know the live-validation step was deferred from Plan 05's automated execution.
- Phase 8 is NOT considered fully green until A, B, and C all pass on the live GitHub Actions runner with branch protection active.

## Issues Encountered

None during the executed task. Workflow file passed grep + YAML-parse acceptance on first write.

## User Setup Required

None for Plan 05 file changes. The Manual Follow-up section above (steps A/B/C) is the user setup required to activate the gate at runtime — but those steps are tracked separately as deferred validation, not as Phase-8 setup.

## Next Phase Readiness

- **Plan 06 unblocked.** Plan 06 (CLAUDE.md docs) consumes Plan 05's deliverable (a committed CI workflow file) — Plan 06 documents the gate's design, exclusions, and the path-injection convention. The manual validation deferral does NOT block Plan 06 because the documentation describes the contract, not the runtime outcome.
- **Phase 8 closure depends on Manual Follow-up A/B/C.** When the user completes those steps, Phase 8 transitions from "code complete" to "gate active in production CI."

## Self-Check: PASSED

- [x] `.github/workflows/ci.yml` — FOUND
- [x] Commit `d3e2802` — FOUND in git log

---
*Phase: 08-test-coverage-quality-gate*
*Plan: 05*
*Completed (file changes): 2026-04-28*
*Pending live validation: see Manual Follow-up section*
