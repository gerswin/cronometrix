---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 03
subsystem: tooling/coverage-gate
tags: [coverage, vitest, cargo-llvm-cov, makefile, lcov, post-processor, rust-toolchain, phase-8-wave-3]
requires:
  - phase: 08-02
    provides: "Backend test fixtures own per-test TempDirs; cargo nextest run is parallel-clean"
provides:
  - "frontend/vitest.config.ts enforces project-wide ≥90/85/90/90 + per-file glob ≥70/60/70/70 thresholds via v8 provider"
  - "Top-level Makefile exposes .PHONY: coverage / coverage-backend / coverage-frontend"
  - "scripts/enforce-coverage-floor.sh post-processes lcov.info enforcing per-file line/branch + project-wide branch gates"
  - "rust-toolchain.toml pins nightly-2026-04-01 + llvm-tools-preview for cargo-llvm-cov --branch (nightly-only)"
  - "08-03-COVERAGE-BASELINE.md captures the gap list (27 backend + 24 frontend = 51 files below floor) Plan 04 inherits"
affects:
  - "Plan 04 (gap-fill) reads BASELINE.md as the authoritative file list and must triage scope (51 files exceeds 15-file cap)"
  - "Plan 05 (CI gate) will copy the Makefile recipe verbatim into .github/workflows/ci.yml; rustup setup-rust-toolchain action picks up the pinned nightly automatically"
  - "Plan 06 (docs) will document the local-vs-CI toolchain differences and the LLVM_COV/LLVM_PROFDATA env-var workaround for non-rustup boxes"
tech-stack:
  added:
    - "cargo-llvm-cov 0.8.5 (cargo-installed dev tool, not a crate)"
  patterns:
    - "Two-level threshold enforcement: built-in tool flags for project-wide, lcov post-processor for per-file"
    - "Pinned nightly via rust-toolchain.toml with quarterly bump cadence documented inline"
    - "Vitest glob-form per-file thresholds (RESEARCH § Pitfall 4 — never combine perFile:true with a glob entry)"
    - "Same Makefile recipe runs locally and in CI — single source of truth for coverage commands"
key-files:
  created:
    - rust-toolchain.toml
    - scripts/enforce-coverage-floor.sh
    - Makefile
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md
  modified:
    - frontend/vitest.config.ts
    - backend/.gitignore
key-decisions:
  - "Rule 3 auto-fix: added lcov.info to backend/.gitignore — Makefile writes backend/lcov.info as a generated artifact, leaving it untracked would surface it in git status forever"
  - "Backend baseline run used stable rustc + Homebrew llvm@21 (LLVM_COV/LLVM_PROFDATA env vars) since local box has no rustup; --branch flag rejected by stable so BRF=0 in baseline lcov — backend branch% will be measured by Plan 05 in CI under nightly. Line coverage numbers are accurate."
  - "BASELINE includes raw FAIL: blocks (script output) + per-file tables + project totals + 'at-or-above-floor' informational tables — Plan 04 has both authoritative gap list AND visibility into already-passing files for regression awareness"
  - "BASELINE explicitly flags scope-cap escalation: 51 files exceeds the 15-file Plan 04 cap; recommended triage paths documented inline (tighten globs, split 04a/04b/04c, or phased threshold ramp)"
  - "Vitest config used glob-form per-file thresholds only (NOT perFile:true) per RESEARCH § Pitfall 4 — perFile:true + glob entry would apply two competing rules to every file"
patterns-established:
  - "Per-file floor enforcement on backend = lcov post-processor (cargo-llvm-cov 0.8.5 has no built-in per-file flag; awk over LCOV's LF/LH/BRF/BRH summary lines is the canonical mechanism)"
  - "Nightly toolchain pinning = rust-toolchain.toml with channel = nightly-YYYY-MM-DD + components = [llvm-tools-preview, rustfmt, clippy] at repo root, never bare 'nightly'"
  - "Coverage Makefile recipe pattern: cd <stack-dir> && tool ... ; bash <repo-root-relative>/script-path — Make doesn't carry cd across recipe lines so script invocations resolve from PWD (repo root)"
requirements-completed: []  # QUALITY-GATE was already marked complete by Plan 02; this plan adds tooling but is a step toward, not a completion of, QUALITY-GATE

# Metrics
duration: ~11min
completed: 2026-04-28
---

# Phase 8 Plan 03: Coverage Tooling + First Measurement Baseline Summary

**One-liner:** Land the coverage measurement + gate infrastructure (Vitest thresholds with v8 provider + lcov reporter, top-level Makefile with three .PHONY targets, awk-based lcov post-processor for per-file floor on the backend, pinned nightly Rust toolchain) and produce the authoritative gap list (27 backend + 24 frontend files below 70/60 floor) that Plan 04 will close.

## What Got Built

A two-task tooling drop. After this plan, `make coverage-frontend` runs end-to-end locally with strict threshold enforcement; `make coverage-backend` is wired identically and runs once nightly Rust is on the PATH.

### Task 1 — Vitest coverage block + rust-toolchain.toml (commit `a4ab4ec`)

- `frontend/vitest.config.ts` extended with the `coverage` block: v8 provider, `['text', 'html', 'lcov']` reporters, `./coverage` reportsDirectory, three `include` globs (`src/components/**`, `src/hooks/**`, `src/lib/**` per D-10), five `exclude` globs (vendored shadcn `ui/**`, `__tests__/**`, `*.test.{ts,tsx}`, `*.spec.{ts,tsx}`, `*.d.ts`), and the two-level threshold shape: project-wide `lines/branches/functions/statements: 90/85/90/90` plus the glob-form per-file floor `'**/*.{ts,tsx}': { lines: 70, branches: 60, functions: 70, statements: 70 }`. NO `perFile: true` (RESEARCH § Pitfall 4). The 14-line minimal config grew to 45 lines; preserved the existing `plugins`/`environment`/`globals`/`setupFiles`/`resolve` rhythm.
- `rust-toolchain.toml` created at the repo root (`/Users/gerswin/Proyectos/cronometrix/rust-toolchain.toml`) with `channel = "nightly-2026-04-01"`, `components = ["llvm-tools-preview", "rustfmt", "clippy"]`, and an inline comment documenting the bump cadence (quarterly, or on ICE/strict-lint break) per RESEARCH § Pitfall 2.

### Task 2 — Script + Makefile + first measurement run (commit `143d021`)

- **`scripts/enforce-coverage-floor.sh`** — bash `set -euo pipefail` wrapper around a single-pass awk over lcov.info that (a) flags any file with line% < $3 or branch% < $4, (b) sums LF/LH/BRF/BRH across all records and flags project-wide branch% < $2, (c) exits 1 on any failure. Guards `BRF=0` files (RESEARCH § Pitfall 5) so derive-only modules don't divide-by-zero. Sanity-tested against `/dev/null` (empty lcov → exit 0). Made executable via `chmod +x`.
- **`Makefile`** at repo root with three TAB-indented `.PHONY` recipes:
  - `coverage` → depends on `coverage-backend coverage-frontend`; success log on completion.
  - `coverage-backend` → `cd backend && cargo llvm-cov nextest --branch --all-features --ignore-filename-regex '(main\.rs|tests/common/.*)' --fail-under-lines 90 --lcov --output-path lcov.info` then `bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60` then `cd backend && cargo llvm-cov --branch --all-features --no-clean --html` (the `--no-clean` second invocation reuses the .profraw cache from the first to skip re-running tests).
  - `coverage-frontend` → `cd frontend && npx vitest run --coverage`.
- **`backend/.gitignore`** updated to exclude `lcov.info` (Rule 3 auto-fix — without this, every `make coverage-backend` run leaves a 480 KB untracked artifact in `git status`).
- **`.planning/.../08-03-COVERAGE-BASELINE.md`** generated FROM the first measurement run. Structure (per plan contract):
  - Run-time provenance (commands, toolchain caveat, target gates).
  - Backend section with raw `FAIL:` block (27 file fails) + structured per-file table + at-or-above-floor informational table + project total (line=63.09%, branch=N/A on stable).
  - Frontend section with raw `FAIL:` block (41 file fails + 1 project-wide branch fail) + structured per-file table + at-or-above-floor table + project totals (line=51.81%, branch=44.79%, functions=50.53%, statements=50.87%).
  - File count summary (27 backend + 24 frontend = **51 total** below floor).
  - **Scope-cap escalation note:** 51 > 15 cap → Plan 04 must triage before execution; three triage paths documented (tighten globs, split 04a/04b/04c, phased threshold ramp).

### First Measurement Run Outcome

- **`make coverage-frontend`** ran end-to-end: 105 tests passed across 20 test files, lcov.info + HTML produced in `frontend/coverage/`. Threshold violations correctly fired both the project-wide gate and the per-file glob — exactly the configured behavior. Failure is **measurement** (51.81% vs 90% target), not config.
- **`make coverage-backend`** under the Makefile recipe failed on this developer's box because the recipe uses `--branch` (nightly-only) and the local Rust install is Homebrew stable 1.93.0 with no rustup. To produce the BASELINE numbers, an off-recipe command was used: `cargo llvm-cov nextest --all-features --ignore-filename-regex ... --lcov --output-path lcov.info` (no `--branch`) with `LLVM_COV=/opt/homebrew/opt/llvm/bin/llvm-cov` and `LLVM_PROFDATA=/opt/homebrew/opt/llvm/bin/llvm-profdata`. All 319 tests passed (22 skipped); 480 KB lcov.info produced. Branch% is N/A in the baseline (BRF=0 across all records); will be measured by Plan 05's CI run under nightly.
- The Makefile recipe itself is **correct** — Plan 05 sets up `rustup toolchain install nightly` + `rustup component add llvm-tools-preview` in the GitHub Actions job, at which point the recipe runs identically locally (with rustup) and in CI.

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Glob-form per-file thresholds in Vitest config | RESEARCH § Pitfall 4: `perFile: true` + glob entry applies two rules per file; "strictest wins" produces unintended behavior. Glob form alone is the verified mechanism for D-14's two-level threshold. |
| Pin nightly to a specific date (`nightly-2026-04-01`) | RESEARCH § Pitfall 2 + Security Domain: bare `nightly` lets nightly drift break CI on unrelated rustc changes; pinning a date is the supply-chain mitigation. Bump cadence (quarterly) documented in the file's header comment. |
| Per-file floor enforced via awk post-processor (NOT a Rust binary) | RESEARCH § Per-File Floor Mechanism: cargo-llvm-cov 0.8.5 has `--fail-under-lines/-functions/-regions` but NO per-file flag and NO `--fail-under-branches`. A Rust binary would be over-engineered for ~50 lines of pattern matching; awk is portable and idempotent. |
| Backend baseline measured without `--branch` | Local box has no rustup; stable rustc rejects `-Z coverage-options=branch`. Line% is the load-bearing measurement for Plan 04's gap-fill scope; branch% will be measured by Plan 05 in CI. The BASELINE explicitly documents this limitation so Plan 04 isn't surprised. |
| BASELINE includes raw `FAIL:` blocks + structured tables + at-or-above-floor list | Raw blocks satisfy the plan's frontmatter `contains: "FAIL:"` contract. Structured tables give Plan 04 a parseable file list. At-or-above-floor list gives Plan 04 visibility into files already covered (regression awareness — adding tests in one file shouldn't drop another below floor). |
| BASELINE flags scope-cap escalation inline | Plan 04's stated cap is "if N+M > 15 OR estimated work > 10 hours, STOP and escalate." The first measurement gives N+M=51, so the BASELINE must surface this immediately rather than letting Plan 04 hit the wall mid-execution. Three triage paths documented so the orchestrator/planner has options. |
| `lcov.info` added to `backend/.gitignore` (Rule 3 auto-fix) | Without this, every `make coverage-backend` run leaves a 480 KB untracked artifact in `git status` permanently — confuses every subsequent `git status` review and risks accidental commit. Frontend already covered by `frontend/.gitignore`'s `/coverage` rule. |
| LLVM_COV / LLVM_PROFDATA documented in BASELINE for non-rustup boxes | This developer's machine (and any other Homebrew-installed Rust user) won't have `llvm-tools-preview` available without rustup. Documenting the Homebrew `llvm@21` workaround in BASELINE saves Plan 04/05 executors the same investigation. |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `lcov.info` to `backend/.gitignore`**

- **Found during:** Task 2, Step 3 (first measurement run produced `backend/lcov.info` and `git status` immediately showed it as untracked).
- **Issue:** Makefile's `coverage-backend` recipe writes `backend/lcov.info` (480 KB), but `backend/.gitignore` only excludes `/target`. Every coverage run would leave a tracked-but-uncommitted artifact in `git status`, confusing future executors and risking accidental `git add .` of binary-ish output.
- **Fix:** Appended a 2-line block to `backend/.gitignore` excluding `lcov.info` with a header comment explaining its provenance. (Frontend's `frontend/.gitignore` already has `/coverage` so frontend lcov is handled.)
- **Files modified:** `backend/.gitignore` (+3 lines).
- **Verification:** `git status` after the measurement run shows no untracked coverage artifacts.
- **Committed in:** `143d021` (Task 2 commit, alongside the BASELINE).

**2. [Rule 3 - Blocking] Backend `make coverage-backend` failed locally on stable rustc — captured baseline via off-recipe command**

- **Found during:** Task 2, Step 3.
- **Issue:** The Makefile recipe uses `--branch` (correct per the plan), but stable rustc 1.93.0 (Homebrew) rejects `-Z coverage-options=branch` ("the option `Z` is only accepted on the nightly compiler"). The plan acknowledges this scenario: Plan 05 will install nightly via rustup in CI; local devs need rustup to run the recipe verbatim. Without nightly available locally, no baseline could be produced under the recipe — but Task 2 Step 3's purpose is precisely "capture the gap list as Plan 04's input."
- **Fix:** Ran cargo-llvm-cov outside the Makefile recipe with `--branch` removed and Homebrew's llvm@21 as the `LLVM_COV`/`LLVM_PROFDATA` source: `LLVM_COV=/opt/homebrew/opt/llvm/bin/llvm-cov LLVM_PROFDATA=/opt/homebrew/opt/llvm/bin/llvm-profdata cargo llvm-cov nextest --all-features --ignore-filename-regex '(main\.rs|tests/common/.*)' --lcov --output-path lcov.info`. Produced 480 KB lcov.info with line/function data; branch data absent (BRF=0 across all 61 records). Documented this caveat front-and-center in the BASELINE.
- **The Makefile recipe was NOT modified** — it remains correct for the canonical (CI / rustup-equipped) environment. The deviation is an environment-specific workaround for the local-baseline run only.
- **Files modified:** None beyond the BASELINE documentation.
- **Verification:** Backend lcov.info exists, parsed cleanly by the script, produced 27 file fails. Plan 05 CI will produce the missing branch numbers.

### No deviations from acceptance criteria

All acceptance criteria from the plan checked off:
- vitest.config.ts has the verbatim `coverage` block from the plan (provider, reporter array, include/exclude globs, two-level thresholds via glob form).
- Makefile has all three .PHONY targets with the verbatim recipes from the plan.
- scripts/enforce-coverage-floor.sh has `set -euo pipefail` + the awk block parsing all six lcov record types.
- rust-toolchain.toml pins a specific nightly date and lists `llvm-tools-preview`.
- BASELINE.md exists at the canonical path and contains 69 lines matching `FAIL:`, satisfying the frontmatter `contains: "FAIL:"` contract.

## Verification

```
$ test -x scripts/enforce-coverage-floor.sh && echo OK
OK

$ awk '/^[a-z-]+:/ { inrec=1; next } inrec && /^\t/ { print "TAB-OK:", NR; next } inrec && /^[ ]/ { print "SPACE-BUG:", NR }' Makefile | head
TAB-OK: 13
TAB-OK: 16
TAB-OK: 17
TAB-OK: 18
TAB-OK: 19
TAB-OK: 20
TAB-OK: 21
TAB-OK: 24
TAB-OK: 25
(no SPACE-BUG output → all recipe lines TAB-indented)

$ make -n coverage-backend | head -3
cd backend && cargo llvm-cov nextest --branch --all-features \
	  --ignore-filename-regex '(main\.rs|tests/common/.*)' \
	  --fail-under-lines 90 --lcov --output-path lcov.info

$ make -n coverage-frontend
cd frontend && npx vitest run --coverage
echo "Frontend HTML: frontend/coverage/index.html"

$ bash scripts/enforce-coverage-floor.sh /dev/null 85 70 60; echo "exit=$?"
exit=0

$ awk '/FAIL:/' .planning/phases/08-.../08-03-COVERAGE-BASELINE.md | wc -l
69

$ npx vitest --version
vitest/4.1.5 darwin-arm64 node-v25.5.0

$ make coverage-frontend
... 105 tests passed, 51.8% lines, 44.78% branches → measured failure (config OK) ...

$ cargo llvm-cov nextest --all-features --ignore-filename-regex ... --lcov --output-path lcov.info
... 319 passed, 22 skipped, 480 KB lcov.info produced ...

$ bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60 > /tmp/backend-fails.txt; echo "exit=$?"
exit=1

$ awk '/^FAIL:/' /tmp/backend-fails.txt | wc -l
27
```

## Issues Encountered

- **Local box has no rustup** → backend `--branch` measurement deferred to Plan 05 CI. Documented in BASELINE.
- **RTK proxy mangled `grep`/`cat -A` calls** during verification (cosmetic, not load-bearing); used `awk` as alternative. The committed files are correct.
- **Plan 02's "1 leaky test under nextest"** noted in 08-02-SUMMARY.md did NOT recur in this run — all 319 backend tests passed cleanly under cargo-llvm-cov nextest. May have been a transient resource leak; logged for visibility but no action required.

## Self-Check: PASSED

- `frontend/vitest.config.ts` — modified, contains `provider: 'v8'`, `reporter` with `lcov`, `lines: 90` (project), `branches: 85` (project), `lines: 70` (per-file), `branches: 60` (per-file), the `'**/*.{ts,tsx}'` glob entry, NO `perFile: true` — VERIFIED
- `rust-toolchain.toml` — created at repo root, contains `channel = "nightly-2026-04-01"` and `llvm-tools-preview` — VERIFIED
- `scripts/enforce-coverage-floor.sh` — created, executable bit set, contains `#!/usr/bin/env bash` first line and `set -euo pipefail`, contains awk block parsing `^SF:` / `^LF:` / `^LH:` / `^BRF:` / `^BRH:` / `^end_of_record` — VERIFIED
- `Makefile` — created at repo root, contains `.PHONY: coverage coverage-backend coverage-frontend`, recipe lines TAB-indented, contains `cargo llvm-cov nextest --branch --all-features`, `--fail-under-lines 90`, `bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60`, `npx vitest run --coverage` — VERIFIED
- `backend/.gitignore` — modified to include `lcov.info` (Rule 3 auto-fix) — VERIFIED
- `.planning/.../08-03-COVERAGE-BASELINE.md` — created at canonical path, contains 69 `FAIL:` lines, contains backend gap table (27 entries), frontend gap table (24 entries), totals, scope-cap escalation note — VERIFIED
- Commit `a4ab4ec` (Task 1) — FOUND in `git log`
- Commit `143d021` (Task 2) — FOUND in `git log`
- `make -n coverage-backend` and `make -n coverage-frontend` print recipe lines without parse errors — VERIFIED
- `bash scripts/enforce-coverage-floor.sh /dev/null 85 70 60` exits 0 (sanity) — VERIFIED
- `bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60` exits 1 with 27 file FAILs (measurement) — VERIFIED
- `make coverage-frontend` ran end-to-end producing `frontend/coverage/lcov.info` + `frontend/coverage/index.html`; failure is measurement (51.8% vs 90%), not config — VERIFIED

## Threat Flags

None — this plan introduces no network endpoints, auth paths, or trust-boundary changes. Per the plan's threat model:

- **T-08-08 (Tampering — bash injection):** mitigated. `set -euo pipefail` enforces strict mode; positional args are passed to awk via `-v` (not interpolated into the awk program text); `${1:?...}` enforces argument presence; no `eval`, no `$(...)` over user input.
- **T-08-09 (Tampering — toolchain drift):** mitigated. `rust-toolchain.toml` pins `nightly-2026-04-01`. Bare `nightly` is not used. Bump cadence documented in-file.
- **T-08-10 (Information Disclosure — coverage HTML):** accept. Repo is private; HTML reports are local artifacts (Plan 03) or scoped CI artifacts (Plan 05 future). No new disclosure surface introduced by this plan.

## Next Phase Readiness

- **Plan 04 (gap-fill):** BASELINE provides the authoritative file list (27 backend + 24 frontend = 51 files). Scope-cap escalation already flagged inline — Plan 04 must triage (tighten globs, split into 04a/04b/04c, or phased threshold ramp) before execution. The "at or above floor" tables give regression-awareness context.
- **Plan 05 (CI gate):** Makefile recipe is the canonical source — copy verbatim into `.github/workflows/ci.yml`. Use `actions-rust-lang/setup-rust-toolchain@v1` with `toolchain: nightly-2026-04-01` (or read from `rust-toolchain.toml` automatically). Install cargo-llvm-cov via `taiki-e/install-action@cargo-llvm-cov`. The pinned nightly + `llvm-tools-preview` component guarantees `--branch` works in CI.
- **Plan 06 (docs):** Document the local-vs-CI toolchain story in CLAUDE.md (rustup install, cargo-llvm-cov install, LLVM_COV/LLVM_PROFDATA workaround for Homebrew Rust users).

---
*Phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra*
*Completed: 2026-04-28*
