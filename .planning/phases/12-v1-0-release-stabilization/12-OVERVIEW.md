# Phase 12 — v1.0 Release Stabilization

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:executing-plans` for one plan at a time, and `superpowers:test-driven-development` for every product-code change.

**Goal:** convert baseline `1dd6d758fc1ed775189a4fff3f20d6a7c1800e34` into a reproducible, private, release-ready Cronometrix build whose functional, persistence, audit, deployment, and CI contracts are green on one commit.

**Architecture:** gate-first and sequential at shared-domain boundaries. Plan 12-01 repairs the development and CI harness. Plan 12-02 lands the final functional contracts before 12-03 starts, because both plans touch enrollment/event/daily-record code. Plan 12-04 consumes their merged result to produce the private deployment bundle. Plan 12-05 is the convergence gate and must run from a clean checkout of one immutable SHA.

**Tech Stack:** Rust/Axum/libSQL, Tokio, Next.js/React/TypeScript, Vitest, Playwright, Docker Compose, Nginx, Cloudflare Tunnel, GitHub Actions, GHCR.

## Global Constraints

- Work from a `codex/` branch and an isolated git worktree for each active plan.
- Preserve the untracked local `.codex/` directory and `AGENTS.md`; never stage them.
- Pull or fetch `origin/main` before creating each worktree. Do not rebase or merge a dirty tree.
- Every behavior change begins with a failing test and ends with the narrow test, the affected suite, and the plan gate green.
- Run tests before every commit. Commit only files listed by the current task plus required lockfiles.
- Do not add coverage exclusions or weaken thresholds.
- Do not use direct SQLite writes outside the allowlist established by 12-03.
- Do not log, commit, or attach licenses, Cloudflare tokens, GHCR tokens, JWT secrets, device credentials, `.env` files, SQLite databases, face images, or source-bearing HTML reports.
- Keep real Hikvision validation and cross-host LIC-05 validation explicitly deferred; mocks are not evidence that those live checks passed.
- A plan is complete only when its summary records the tested SHA, exact commands, exit codes, and remaining risks.
- Plans 12-02 through 12-04 close scoped checkpoints, not the release gate. Their
  summaries may record `Verdict: PASS — scoped <name> checkpoint` only when
  their owned behavior and every production file they modified meet the
  repository per-file floors. Raw project-wide coverage failures remain
  `Release gate: FAIL — deferred to 12-05`; they are never normalized away.
- Plan 12-05 is the only plan allowed to record an unqualified release `PASS`.

## Execution graph

```text
12-01 Rebaseline and harness
          |
          v
12-02 Functional contracts
          |
          v
12-03 Persistence/audit
          |
          v
12-04 Private distribution
          |
          v
12-05 Full release gate
          |
          v
Phase 13 live validation
```

## Plans

| Plan | Outcome | Depends on | Parallel-safe |
|---|---|---|---|
| `12-01-PLAN.md` | reproducible Node/Rust harness and truthful planning baseline | design approval | no |
| `12-02-PLAN.md` | auth, devices, enrollment, timesheet, and SSE contracts fixed | 12-01 | no; integrate before 12-03 starts |
| `12-03-PLAN.md` | bounded single-writer queue, atomic domains, immutable audit log | integrated 12-02 scoped functional PASS | no |
| `12-04-PLAN.md` | same-origin gateway, private GHCR images, private installer bundle | 12-02 and 12-03 scoped PASS | no |
| `12-05-PLAN.md` | fmt, lint, type, tests, coverage, E2E, containers all green on one SHA | 12-04 | no |

## Required summaries

Each executor creates `12-0N-SUMMARY.md` beside its plan with:

- branch, worktree, start SHA, and tested implementation SHA before the summary commit;
- tasks and commits completed;
- commands and exit codes;
- CI run URL when applicable;
- security-sensitive evidence represented only by redacted identifiers or hashes;
- deferred or newly discovered risks with an owner and next action.

The summary commit is evidence after the tested implementation. Identify it in
the handoff/git history; never try to embed its not-yet-created SHA inside its
own file.

The phase closes only when all five summaries exist and `12-05-SUMMARY.md`
records the sole unqualified `Verdict: PASS`. Earlier summaries preserve their
scoped verdicts and their raw release-gate failures until that convergence.
