---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 06
subsystem: docs
tags: [docs, claude-md, conventions, coverage-gate, phase-8-wave-6, phase-closure]
requires:
  - phase: 08-05
    provides: ".github/workflows/ci.yml committed; coverage gate code-complete with deferred Manual Follow-up checklist"
  - phase: 08-04C
    provides: "Frontend coverage GREEN; D-09 file-specific exclusions documented in vitest.config.ts"
  - phase: 08-04B
    provides: "Backend coverage GREEN on Linux CI; macOS-only license/* exclusion candidates surfaced"
  - phase: 08-01
    provides: "AppState carries Paths substruct; Paths::from_env at startup, Paths::for_test in tests"
provides:
  - "CLAUDE.md ## Test Coverage section (D-22) — install, local commands, thresholds, exclusion policy, HTML reports, CI gate, failing-run triage, public-vs-private note, pending-validation pointer"
  - "CLAUDE.md Conventions § Filesystem-root injection (D-23) — env-var-and-default table for state.paths.* fields + tests guidance"
  - "Protective HTML comment ('Phase 8 D-23 — DO NOT remove on conventions sync') above the Filesystem-root subsection so future automated sync tools cannot silently revert the gate-binding rule"
  - "In-doc pointer to .planning/phases/.../08-05-SUMMARY.md Manual Follow-up so the deferred CI validation work cannot be lost"
  - "Phase 8 closure-ready (5 deliverables landed across Plans 01–05; 06 documents the binding rules)"
affects:
  - "Future Claude Code sessions and human contributors find the gate rules + path-injection convention in the canonical project-instructions surface"
  - "A future plan-checker or human reviewer can diff Makefile / vitest.config.ts against CLAUDE.md to detect undocumented exclusions (T-08-19 mitigation)"
tech-stack:
  added: []
  patterns:
    - "Insert binding code conventions INSIDE GSD-managed markers (<!-- GSD:conventions-start --> ... <!-- GSD:conventions-end -->) so a future sync preserves them"
    - "Protective HTML comment ('DO NOT remove on conventions sync') as a cheap, grep-friendly signal for both human reviewers and future automation"
    - "Document deployed values verbatim from source (env vars copy-pasted from paths.rs; thresholds copy-pasted from Makefile + scripts/enforce-coverage-floor.sh + vitest.config.ts; CI job names copy-pasted from .github/workflows/ci.yml)"
key-files:
  created:
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-06-SUMMARY.md
  modified:
    - CLAUDE.md
key-decisions:
  - "Insert Filesystem-root injection rule INSIDE the GSD:conventions markers (lines 185-212) so a hypothetical future GSD conventions-sync command preserves the rule rather than overwriting it; no such sync command exists today, but the markers are present and a human-driven sync from CONVENTIONS.md is still possible"
  - "Add the protective HTML comment 'Phase 8 D-23 — DO NOT remove on conventions sync; this rule is a binding code convention, not a placeholder.' immediately above the subsection — defensive in-band signal for future automation and human readers alike"
  - "Document the three Plan 04C D-09 file-specific frontend exclusions (providers.tsx, top-bar.tsx, access-restricted.tsx) in CLAUDE.md's exclusion table — the plan flagged that '04 may have added file-specific exclusions; ADD ROWS to the exclusion table' and these were the actual additions"
  - "Document the macOS-only backend exclusion exception (license/fingerprint.rs + license/service.rs) as a separate paragraph beneath the exclusion table — it is host-platform-conditional, not a true exclusion, so it does not belong in the main exclusion table"
  - "Document the pinned nightly date (nightly-2026-04-01) and the bump cadence ('quarterly, or when nightly introduces an ICE/strict lint that blocks CI') so future contributors know the upgrade contract"
  - "Reference the deferred-validation checklist from 08-05-SUMMARY.md ('Pending live validation (Plan 05 deferred)' subsection) so the Manual Follow-up is discoverable from the canonical project-instructions surface; without this pointer the manual work could be lost between sessions"
requirements-completed: [QUALITY-GATE]

# Metrics
duration: ~25min
completed: 2026-04-28
---

# Phase 8 Plan 06: CLAUDE.md Convention + Coverage Gate Documentation Summary

**One-liner:** Two surgical edits to root `CLAUDE.md` — replaced the Conventions placeholder with the Filesystem-root injection rule (D-23, inside the GSD markers, with a protective HTML comment) and inserted a new top-level `## Test Coverage` section (D-22) documenting install, local commands, thresholds (90/85 project; 70/60 per-file), the exclusion policy (including the three Plan 04C D-09 frontend file exclusions), HTML reports, CI gate contract, failing-run triage, public-vs-private note, and the pending-validation pointer to 08-05's Manual Follow-up.

## Performance

- **Started:** 2026-04-28
- **Completed:** 2026-04-28
- **Duration:** ~25 min
- **Tasks:** 1 of 1
- **Files modified:** 1 (CLAUDE.md)

## Where the new content lives

- **`CLAUDE.md` line 188** — Protective HTML comment: `<!-- Phase 8 D-23 — DO NOT remove on conventions sync; this rule is a binding code convention, not a placeholder. -->`
- **`CLAUDE.md` line 189** — `### Filesystem-root injection (Phase 8)` subsection inside the GSD-managed `## Conventions` block (between `<!-- GSD:conventions-start -->` line 185 and `<!-- GSD:conventions-end -->` line 212)
- **`CLAUDE.md` line 214** — `## Test Coverage` top-level section (between the closed Conventions block at line 212 and the GSD-managed Architecture block starting at line 387)

## Task Commits

| # | Subject | Hash |
|---|---------|------|
| 1 | docs(08-06): document coverage gate + Filesystem-root injection convention | a1c223b |

## Verification

All 17 grep-based acceptance criteria from the plan pass on the post-edit file:

```
$ grep -c "^## " CLAUDE.md
17                          # was 16 — exactly one new top-level section added
$ grep -E "^## Test Coverage" CLAUDE.md
214:## Test Coverage
$ grep -E "Filesystem-root injection" CLAUDE.md
189:### Filesystem-root injection (Phase 8)
$ grep -F "Phase 8 D-23 — DO NOT remove on conventions sync" CLAUDE.md
188 (line)                  # protective HTML comment present
$ grep -F "Conventions not yet established" CLAUDE.md
(no match)                  # placeholder removed
```

All other required tokens present (verified by grep): `make coverage`,
`cargo-llvm-cov`, `rust-toolchain.toml`, `CRONOMETRIX_LEAVES_ROOT`,
`CRONOMETRIX_EVENTS_ROOT`, `ENROLLMENTS_DIR`, `CRONOMETRIX_CAPTURES_TMP`,
`DATA_DIR`, `test_state_with_tmpdir`, `.github/workflows/ci.yml`, `90%`,
`85%`, `70%`, `60%`.

### Source-verbatim checks

The documented values were copy-pasted from the actual deployed configuration:

- Backend ignore-regex `(main\.rs|tests/common/.*)` — verified against
  `Makefile` line 17 + `.github/workflows/ci.yml` line 47 (parity confirmed).
- Backend thresholds `--fail-under-lines 90` then `85 70 60` — verified
  against `Makefile` lines 17–18 and `scripts/enforce-coverage-floor.sh`
  comment at line 7.
- Frontend project-wide thresholds (lines 90, branches 85, functions 90,
  statements 90) and per-file floor (70/60/70/70) — verified against
  `frontend/vitest.config.ts` lines 30–43.
- Frontend exclusion list (`src/components/ui/**`, `providers.tsx`,
  `top-bar.tsx`, `access-restricted.tsx`, `__tests__/**`, `*.test.{ts,tsx}`,
  `*.spec.{ts,tsx}`, `*.d.ts`) — verified against `frontend/vitest.config.ts`
  lines 20–29 (all 7 entries documented; `src/app/**` documented as
  implicitly excluded via the whitelist `include` array on lines 15–19).
- Path env vars + defaults (`CRONOMETRIX_LEAVES_ROOT=./data/leaves`,
  `CRONOMETRIX_EVENTS_ROOT=./data/events`, `ENROLLMENTS_DIR=./data/enrollments`,
  `CRONOMETRIX_CAPTURES_TMP=/tmp/enrollments-captures`, `DATA_DIR + overrides
  = ./data/overrides`) — verified against `backend/src/state/paths.rs::Paths::from_env`
  lines 19–30.
- Pinned nightly `nightly-2026-04-01` — verified against `rust-toolchain.toml`
  line 7.
- CI workflow path + job names (`Backend Coverage`, `Frontend Coverage`) +
  permissions (`contents: read`) + action pins (checkout@v4, setup-node@v4,
  upload-artifact@v4, install-action@v2, rust-cache@v2, cargo-llvm-cov@0.8.5)
  — verified against `.github/workflows/ci.yml` lines 5–86.

## GSD Conventions Sync Defensive Step (Plan §DEFENSIVE STEP)

- **Detection result:** No GSD conventions-sync command exists in
  `.claude/commands/` (`ls .claude/commands/ | grep -i convention` returns
  empty), and `gsd-sdk query --help` does not surface a `conventions` or
  `sync` verb. However, the **GSD-managed markers DO exist** in `CLAUDE.md`
  (`<!-- GSD:conventions-start source:CONVENTIONS.md -->` at line 185 +
  `<!-- GSD:conventions-end -->` at line 212).
- **Action taken:** Inserted the new content INSIDE the markers (lines
  186–212 inclusive of the `## Conventions` heading and the protective
  comment) so a future sync from a hypothetical `CONVENTIONS.md` source file
  would see the markers as the authoritative content boundary. The
  protective HTML comment at line 188 makes the rule's status explicit to any
  future automation that respects HTML comments as in-band signals.
- **Dry-run verification:** No sync command exists to dry-run; this defensive
  step is a precautionary measure for any future sync mechanism. The risk is
  bounded: if a future sync ignores the protective comment AND ignores the
  fact that the content lives inside the GSD markers, that sync is broken on
  arrival and the breakage will be visible in the next plan-check.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Add missing critical functionality] Document the three Plan 04C D-09 file-specific frontend exclusions**

- **Found during:** Reading `frontend/vitest.config.ts` per the plan's `<read_first>` step.
- **Issue:** The plan's exclusion-table template listed only the broad-glob frontend exclusions (`src/components/ui/**`, `src/app/**`, test files, type files). `frontend/vitest.config.ts` lines 22–24 add three specific D-09 exclusions: `providers.tsx`, `top-bar.tsx`, `access-restricted.tsx`. The plan instructed "If Plan 04 added file-specific exclusions, ADD ROWS to the exclusion table for each, with the justification copied from `08-04-SUMMARY.md`."
- **Fix:** Added three rows to the exclusion table with the verbatim D-09 justifications from `frontend/vitest.config.ts` inline comments ("pure QueryClientProvider wrapper, no logic", "pure display, no logic", "pure display, no logic").
- **Files modified:** `CLAUDE.md`.
- **Severity:** Documentation correctness — without this, T-08-19 (undocumented exclusions repudiation threat) would not be fully mitigated; a future PR could add another D-09-style exclusion without diff-checkable provenance.
- **Committed in:** a1c223b.

**2. [Rule 2 — Add missing critical functionality] Document the macOS-only backend exclusion exception**

- **Found during:** Reading `08-04B-SUMMARY.md` per the plan's `<read_first>` step.
- **Issue:** Plans 04B and 04C surfaced `backend/src/license/fingerprint.rs` + `backend/src/license/service.rs` as macOS-only exclusion candidates (Linux CI under nightly measures them at full coverage). The plan's exclusion-table template did not have a row for these, but documenting them is essential — a future macOS dev who runs `make coverage-backend` and sees two FAILs needs the doc to explain that this is expected and informational.
- **Fix:** Added a paragraph beneath the exclusion table titled "Backend note (macOS dev)" documenting the host-platform asymmetry and the "CI is authoritative" contract.
- **Files modified:** `CLAUDE.md`.
- **Severity:** Documentation correctness — without this, a future macOS dev could mistake the FAIL output for a real regression and either revert work or add an unjustified exclusion.
- **Committed in:** a1c223b.

**3. [Rule 2 — Add missing critical functionality] Reference 08-05 Manual Follow-up checklist**

- **Found during:** Reading `08-05-SUMMARY.md` per the user's `<context_notes>` direction.
- **Issue:** Plan 05 deferred live CI validation to a manual checklist (push + verify positive run, open negative regression PR, configure branch protection). The 08-05-SUMMARY.md "Tracking" section explicitly says: "A project-level note in CLAUDE.md (added by Plan 06) will reference this checklist so future contributors know the live-validation step was deferred from Plan 05's automated execution."
- **Fix:** Added a "Pending live validation (Plan 05 deferred)" subsection at the end of `## Test Coverage` that summarizes the three checklist items and points to `08-05-SUMMARY.md` for the exact commands. Also added an explicit "Phase 8 is NOT considered fully green until A, B, and C all pass" warning so the deferred work cannot be silently dropped.
- **Files modified:** `CLAUDE.md`.
- **Severity:** Documentation correctness + phase-closure tracking — without this pointer, the deferred work would only be discoverable by reading the SUMMARY archive.
- **Committed in:** a1c223b.

**Total deviations:** 3 Rule-2 additions to make the documentation match the deployed state and the deferred-validation tracking. No auto-fixed bugs (no source code changes); no scope creep (all additions were to the same `CLAUDE.md` edit). No checkpoints encountered.

## Authentication Gates

None encountered.

## Issues Encountered

None.

## Self-Check: PASSED

- CLAUDE.md modifications:
  - `^## Test Coverage` heading at line 214 — FOUND
  - `### Filesystem-root injection (Phase 8)` heading at line 189 — FOUND
  - Protective HTML comment at line 188 — FOUND
  - Placeholder line "Conventions not yet established. Will populate as patterns emerge during development." — REMOVED (verified absent)
  - Top-level heading count went 16 → 17 — VERIFIED
  - All 14 deployed-value tokens present (env vars × 5, threshold % × 4, command names × 3, CI path, test helper) — VERIFIED via grep
  - Three D-09 file-specific frontend exclusions documented in table — VERIFIED
  - macOS-only backend exclusion exception documented — VERIFIED
  - "Pending live validation (Plan 05 deferred)" subsection points to 08-05-SUMMARY.md Manual Follow-up — VERIFIED
- Commit a1c223b — FOUND in git log (`docs(08-06): document coverage gate + Filesystem-root injection convention`)

## Threat Flags

None — this plan only modifies project documentation. No new network endpoints, no new auth paths, no schema changes, no file-access patterns added or removed. The threat-model `<threat_register>` mitigations (T-08-19 undocumented-exclusions repudiation, T-08-20 public-artifact disclosure note, T-08-21 convention-drift tampering) are now actively realized in CLAUDE.md.

## Phase 8 Closure Status

With Plan 06 committed, Phase 8 has 6 of 6 plans complete in code-and-docs:

| Plan | Subject | Status |
|------|---------|--------|
| 08-01 | AppState Paths substruct (path injection) | done |
| 08-02 | common::test_state_with_tmpdir test fixture | done |
| 08-03 | Coverage tooling (Makefile + enforcer + rust-toolchain.toml + vitest config) | done |
| 08-04A | Backend domain coverage gap-fill (16 modules) | done |
| 08-04B | Backend infrastructure coverage gap-fill (9 modules + 2 macOS exclusion candidates) | done |
| 08-04C | Frontend coverage gap-fill (21 bucket + 6 branch-bump) + composite checkpoint | done |
| 08-05 | CI gate workflow file (.github/workflows/ci.yml) | code-complete; live validation deferred |
| 08-06 | CLAUDE.md docs (Test Coverage section + Filesystem-root injection rule) | done |

**Phase is code-and-docs complete.** Phase 8 transitions from "code complete" to "gate active in production CI" only after the Plan 05 Manual Follow-up steps (positive verification, negative regression PR, branch protection) are executed by a human on the live GitHub Actions runner. CLAUDE.md now carries the pointer to those steps in `## Test Coverage` § "Pending live validation".

---

*Phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra*
*Plan: 06*
*Completed: 2026-04-28*
