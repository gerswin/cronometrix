---
name: coverage-gate
description: Run, install, and debug the Cronometrix test-coverage gate (backend cargo-llvm-cov + frontend Vitest). Use when running `make coverage`, setting up coverage tooling, reading a failing Backend Coverage or Frontend Coverage CI job, or checking coverage thresholds and HTML reports.
---

# Test coverage gate (Phase 8)

Phase 8 established hard-failing coverage jobs. Every PR to `main` runs the
same checks documented below; Phase 13 separately proves branch protection
blocks merge when a required threshold is missed.

The exclusion policy and its justifications live in the root `CLAUDE.md`
(always loaded) because adding an exclusion is a policy decision, not a
procedure.

## Install (one-time per developer)

```bash
# Backend coverage tooling (cargo-llvm-cov is a tool, NOT a Cargo dependency)
cargo install cargo-llvm-cov --locked --version 0.8.5

# Nightly Rust is required for branch coverage (--branch is unstable on stable rustc).
# The repo's rust-toolchain.toml pins a specific nightly date; rustup honors it
# automatically. To install that exact toolchain explicitly:
NIGHTLY=$(grep '^channel' rust-toolchain.toml | sed 's/.*"\(.*\)".*/\1/')
rustup toolchain install "$NIGHTLY" --component llvm-tools-preview

# Frontend coverage tooling is already installed
# (vitest + @vitest/coverage-v8 in frontend/package.json)
nvm use && npm install --global npm@11.12.1 && cd frontend && npm ci
```

Frontend installs are pinned to Node `24.15.0` and npm `11.12.1` via
the root `.nvmrc`, `package.json` engines, and `packageManager`. CI and the web
image must use the same pair and `npm ci`; a lockfile mismatch is a hard failure.

The pinned nightly is currently `nightly-2026-04-01`. Bump cadence is quarterly
(or earlier if nightly introduces an ICE / strict lint that blocks CI). Bump =
update `rust-toolchain.toml` + verify `make coverage-backend` still green.

## Local commands

```bash
make coverage           # Backend + frontend; both must pass
make coverage-backend   # Backend only (cargo-llvm-cov + per-file enforcer)
make coverage-frontend  # Frontend only (Vitest --coverage)
make test-ci-config     # Verify every setup-node version-file exists
```

The same coverage commands run in CI (`.github/workflows/ci.yml`), so local
green is the required pre-push evidence. It does not prove the live GitHub run
or branch protection; Phase 13 records those external results.

## Thresholds

| Side | Scope | Lines | Branches | Functions | Statements |
|------|-------|-------|----------|-----------|------------|
| Backend | Project-wide | >=90% | >=85% | — | — |
| Backend | Per file | >=70% | >=60% | — | — |
| Frontend | Project-wide | >=90% | >=85% | >=90% | >=90% |
| Frontend | Per file | >=70% | >=60% | >=70% | >=70% |

Thresholds are fixed (no ratchet): the gate compares against the threshold,
not against a stored baseline. A PR that drops coverage from 95% to 91%
passes; from 91% to 89% fails.

Backend project-wide line gate is enforced by `cargo llvm-cov nextest
--fail-under-lines 90`; backend project-wide branch gate + per-file floor are
enforced by `scripts/enforce-coverage-floor.sh lcov.info 85 70 60` (project
branch min / per-file line min / per-file branch min). Frontend gates are
enforced natively by Vitest from `frontend/vitest.config.ts`.

## HTML reports

Local:
- Backend: `backend/target/llvm-cov/html/index.html`
- Frontend: `frontend/coverage/index.html`

CI: artifacts named `backend-coverage-html` and `frontend-coverage-html` are
attached to every workflow run (retention: 14 days). Download from the GitHub
Actions run page even when the gate is red — the report helps drill into the
failing file.

## CI gate

Workflow file: `.github/workflows/ci.yml`

Triggers: push to any branch, pull_request targeting `main`.

Coverage jobs (intended required checks; live protection is verified in Phase 13):
- `Backend Coverage` — installs nightly Rust + cargo-llvm-cov + cargo-nextest;
  runs `cargo llvm-cov nextest --branch --all-features --ignore-filename-regex
  '(main\.rs|tests/common/.*)' --fail-under-lines 90 --lcov --output-path
  lcov.info`, then `bash ../scripts/enforce-coverage-floor.sh lcov.info 85 70
  60`. Threshold miss makes the job red; verified Phase 13 protection then
  blocks merge.
- `Frontend Coverage` — reads Node `24.15.0` from the root `.nvmrc`, pins npm
  `11.12.1`, and runs `npx vitest run --coverage`.
  Vitest enforces both project-wide and per-file thresholds natively from
  `frontend/vitest.config.ts`.

Both jobs run with `permissions: contents: read` (least privilege per
threat model T-08-15) and pin actions (`actions/checkout@v4`,
`actions/setup-node@v4`, `actions/upload-artifact@v4`,
`taiki-e/install-action@v2`, `Swatinem/rust-cache@v2`,
`cargo-llvm-cov@0.8.5`).

The exclusion regex `(main\.rs|tests/common/.*)` is identical between
`Makefile` and `.github/workflows/ci.yml` — DO NOT change one without the
other; drift between local and CI scope makes the gate untrustworthy.

The hard-fail behavior is locked-in (no soft-warn, no override label).
Aligns with the audit-compliance ethos of the product (D-13).

## Reading a failing run

1. Open the failing job's logs in the Actions tab.
2. For backend: the post-processor prints `FAIL: <file> line coverage X% < floor 70%`
   (or branch). Click the file in the HTML artifact to see uncovered lines.
3. For frontend: Vitest prints a threshold table per file; uncovered lines are
   highlighted in the HTML report.
4. Add tests to bring the file above the floor. Don't add an exclusion unless
   the file is genuinely uncoverable in this phase.

## Note on private vs public repo

HTML reports include source code excerpts. The repo is currently private, so
artifacts are scoped to repo collaborators. If the repo ever goes public,
revisit the artifact retention policy and consider scrubbing sensitive
comment patterns from the HTML output.

## Pending live validation (Plan 05 deferred)

Plan 05 (CI gate) shipped the workflow file but the live runtime
validation was deferred per user direction. Three checklist items remain
in
`.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-05-SUMMARY.md`
under "Manual Follow-up":

1. **Positive verification** — push the branch, confirm both jobs pass green
   on GitHub Actions, confirm HTML artifacts are downloadable.
2. **Negative regression PR** — open a deliberate red PR (add an untested
   `dead_code.rs`), confirm `Backend Coverage` FAILS at the post-processor
   step with `FAIL: backend/src/dead_code.rs line coverage 0.00% < floor 70%`,
   then close the PR.
3. **Branch protection** — in GitHub UI (Settings → Branches), require
   `Backend Coverage` and `Frontend Coverage` as status checks before merge to
   `main`.

Phase 8 is NOT considered fully green until A, B, and C all pass on the live
GitHub Actions runner with branch protection active. Anyone resuming this work
should consult `08-05-SUMMARY.md` for the exact commands.
