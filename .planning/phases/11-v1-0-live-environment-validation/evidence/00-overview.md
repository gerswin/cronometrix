# Phase 11 Evidence — v1.0 Live Environment Validation

**Status:** in progress
**Last updated:** 2026-04-29

This directory holds the auditable evidence for every live-environment
validation item Phase 11 executes. Per D-13, ALL items must show
`Verdict: PASS` (or `Verdict: DEFERRED` with a risk-accept doc) before
STATE.md flips milestone v1.0 to `complete`.

## Evidence Items

| # | Item | Plan | Requirement | Verdict |
|---|------|------|-------------|---------|
| 01 | Live CI green run (Backend Coverage + Frontend Coverage + E2E Tests) | [11-01](../11-01-PLAN.md) | VALIDATE-CI-GREEN | [pending](./01-ci-green/README.md) |
| 02 | Local `make e2e` against real dev stack | [11-01](../11-01-PLAN.md) | VALIDATE-CI-GREEN (supplemental) | [pending](./02-local-make-e2e/README.md) |
| 03 | Live CI red regression (deliberate broken PR fails all 3 gates) | [11-02](../11-02-PLAN.md) | VALIDATE-CI-RED | [pending](./03-ci-red/README.md) |
| 04 | Branch protection — 3 status checks required on `main` | [11-04](../11-04-PLAN.md) | VALIDATE-BRANCH-PROTECTION | [pending](./04-branch-protection/README.md) |
| 05 | Fresh-VM installer smoke (Ubuntu 22.04 + Docker + CF tunnel + DO Functions) | [11-05](../11-05-PLAN.md) | VALIDATE-INSTALLER-SMOKE | [pending](./05-installer-smoke/README.md) |
| 06 | LIC-05 cross-host clone test — risk-accept deferral to first prod install | [11-03](../11-03-PLAN.md) | VALIDATE-LIC-05-CLONE | [pending](./06-lic-05-deferral/README.md) |
| 07 | Real Hikvision alertStream live test — risk-accept deferral to first prod install | [11-03](../11-03-PLAN.md) | VALIDATE-HIKVISION-LIVE | [pending](./07-hikvision-deferral/README.md) |

## Evidence README schema (per D-02)

Every item README MUST have these sections in this order:

- Date captured (YYYY-MM-DD)
- Captured by (operator name / username)
- Command run / action (exact command or UI step)
- Expected (what should happen)
- Actual (what did happen)
- Verdict (`PASS` | `FAIL` | `DEFERRED`)
- Artifacts (list of files in this dir)
- External refs (GH Actions run URL, PR number — if any)

## Size budget (per D-03)

Total committed evidence across this directory MUST stay under **150 MB**.
Per-item HTML reports above 50 MB MUST be `tar.gz`-compressed before commit.
Use `du -sh evidence/` to spot-check before each commit.

## Phase 11 verdict (per D-13)

| Block | Status |
|-------|--------|
| All 5 active items PASS | pending |
| Both deferral items have risk-accept docs | pending |
| STATE.md milestone flip approved | pending |
