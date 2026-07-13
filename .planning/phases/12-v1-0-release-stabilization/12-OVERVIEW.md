# Phase 12 — v1.0 Release Stabilization

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:executing-plans` for one plan at a time, and `superpowers:test-driven-development` for every product-code change.

**Goal:** convert baseline `1dd6d758fc1ed775189a4fff3f20d6a7c1800e34` into a reproducible, private, release-ready Cronometrix build whose functional, persistence, audit, deployment, and CI contracts are green on one commit.

**Architecture:** gate-first. Plan 12-01 repairs the development and CI harness. Plans 12-02 and 12-03 can start in isolated worktrees, but they share enrollment/event/daily-record files and therefore require an explicit integration checkpoint after 12-02 before overlapping 12-03 domain migrations land. Plan 12-04 consumes the merged result to produce the private deployment bundle. Plan 12-05 is the convergence gate and must run from a clean checkout of one immutable SHA.

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

## Execution graph

```text
12-01 Rebaseline and harness
          |
          +---------------------+
          |                     |
          v                     v
12-02 Functional contracts  12-03 Persistence/audit
          |                     |
          +----------+----------+
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
| `12-02-PLAN.md` | auth, devices, enrollment, timesheet, and SSE contracts fixed | 12-01 | partial; integrate before overlapping 12-03 work |
| `12-03-PLAN.md` | bounded single-writer queue, atomic domains, immutable audit log | 12-01 plus 12-02 integration checkpoint | no |
| `12-04-PLAN.md` | same-origin gateway, private GHCR images, private installer bundle | 12-02 and 12-03 | no |
| `12-05-PLAN.md` | fmt, lint, type, tests, coverage, E2E, containers all green on one SHA | 12-04 | no |

## Required summaries

Each executor creates `12-0N-SUMMARY.md` beside its plan with:

- branch, worktree, start SHA, and final SHA;
- tasks and commits completed;
- commands and exit codes;
- CI run URL when applicable;
- security-sensitive evidence represented only by redacted identifiers or hashes;
- deferred or newly discovered risks with an owner and next action.

The phase closes only when all five summaries exist and `12-05-SUMMARY.md` records `Verdict: PASS`.
