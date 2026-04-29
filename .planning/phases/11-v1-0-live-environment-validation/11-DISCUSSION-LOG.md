# Phase 11 — Discussion Log

**Date:** 2026-04-29
**Mode:** discuss (default)

Audit trail of Phase 11 discuss-phase Q&A. NOT consumed by downstream agents — they read `11-CONTEXT.md`.

## Q1 — Gray Area Selection (multi-select)

| Option | Selected? |
|--------|-----------|
| Evidence format & storage | ✓ |
| LIC-05 hardware reality | ✓ |
| Sequencing & completion criteria | ✓ |
| Hikvision live test scope | — (skipped) |

User skipped Hikvision live test → captured as D-10 (out of Phase 11 scope; mock_hikvision sufficient for protocol coverage; first production install is the live test).

## Q2 — Evidence storage

| Option | Selected? |
|--------|-----------|
| Hybrid (Recommended) | — |
| Fully committed evidence dir | ✓ |
| External-only link table | — |

**Chose:** Fully committed → D-01..D-05. Repo bloat budget: 150 MB. `*.html linguist-generated=true` to avoid GitHub language-stat pollution.

## Q3 — LIC-05 cross-host

| Option | Selected? |
|--------|-----------|
| Cloud-VM swap (Recommended) | — |
| Docker container virtualization | — |
| 2 physical machines | — |
| Defer to v1.1 / first prod install | ✓ |

**Chose:** Defer → D-06..D-09. Risk-accept doc + v1.1 backlog entry `LIC-05-CROSS-HOST`. First paying-customer deploy IS the validation event.

## Q4 — Sequencing

| Option | Selected? |
|--------|-----------|
| Split 11.1 (CI) + 11.2 (real infra) (Recommended) | — |
| Incremental (single phase, multi-session) | — |
| Block-until-complete (one sitting) | ✓ |

**Chose:** Block-until-complete → D-11..D-14. Estimated 2-4 hours focused work after prerequisite checklist passes. Resumability via atomic per-item commits preserved as safety net.

## Summary

3 of 4 areas resolved with non-recommended choices in 2 areas (LIC-05, sequencing). User picked the "lighter scope but block-until-ready" combination — committed evidence trades repo size for offline auditability; LIC-05 deferral cuts cloud cost / coordination burden; block-until-complete avoids the unfinished-phase drag.

Net effect on Phase 11 scope: 5 evidence items down to 4 actively executed (LIC-05 captured as deferral doc, not a live test). Hikvision similarly captured as deferral.

Total turns: 2 (gray-area selection + 3-question batch).
