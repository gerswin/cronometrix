# Phase 13 — v1.0 Live Validation

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:executing-plans`; stop at every operator or external-system checkpoint instead of fabricating evidence.

**Goal:** prove the Phase 12 release candidate on GitHub and a fresh Linux host, then produce an auditable sign-off without claiming the two formally deferred hardware validations.

**Architecture:** immutable-candidate validation. Plan 13-01 validates required CI checks, a deliberate red regression, and branch protection. Plan 13-02 installs the exact release manifest on a clean Ubuntu VM through the private distribution path and validates Cloudflare, licensing, same-origin auth/SSE, restart, upgrade, and rollback. Plan 13-03 reconciles evidence and records the release decision.

The approved five-part live-validation design is consolidated here into three executable plans: 13-01 covers CI/protection, 13-02 covers the fresh-VM runtime campaign, and 13-03 covers evidence/deferrals/approval/promotion. This mapping is intentional and is the source of truth for execution.

**Tech Stack:** GitHub Actions/API, GHCR, Docker Compose, Ubuntu 22.04 or 24.04 LTS, Cloudflare Tunnel, DigitalOcean Functions license service, curl/jq/openssl.

## Global Constraints

- Validate one immutable commit and one manifest SHA. Any code or manifest change invalidates downstream evidence and restarts the affected plan.
- Use a dedicated non-production license, client slug, tunnel, VM, and per-installation GHCR read credential.
- Store raw secrets only in the operator's secret manager and ephemeral shell environment. Redact command transcripts before committing.
- Keep text evidence in Git under 1 MiB per file. Keep source-bearing HTML, traces, screenshots, and videos as private CI artifacts for 14 days.
- Never mark a manual check passed from mocks, local assumptions, screenshots without command output, or a different SHA.
- Destroy the test VM, revoke the package credential, and remove the test tunnel after evidence capture.

## Plans

| Plan | Outcome | Depends on |
|---|---|---|
| `13-01-PLAN.md` | green candidate CI, red regression proof, and enforced branch protection | Phase 12 PASS |
| `13-02-PLAN.md` | fresh-VM private installation and runtime validation | 13-01 green candidate |
| `13-03-PLAN.md` | reconciled evidence, accepted deferrals, and v1.0 release verdict | 13-01 and 13-02 |

The phase closes only when `13-03-SUMMARY.md` records either `Verdict: PASS` or a concrete blocking failure. `LIC-05-CROSS-HOST` and `HIKVISION-LIVE` remain `DEFERRED` with owner and due condition, never silently converted to PASS.
