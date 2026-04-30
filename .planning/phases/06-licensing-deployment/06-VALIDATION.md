---
phase: 6
slug: licensing-deployment
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-27
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo nextest (Rust unit/integration tests) |
| **Config file** | `backend/Cargo.toml` (workspace) |
| **Quick run command** | `cd backend && cargo nextest run --test-threads 4` |
| **Full suite command** | `cd backend && cargo nextest run && cd ../frontend && npm test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd backend && cargo nextest run --test-threads 4`
- **After every plan wave:** Run `cd backend && cargo nextest run && cd ../frontend && npm test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | LIC-02 | — | Fingerprint = SHA256(cpu+mac+disk), deterministic | unit | `cargo nextest run -p cronometrix -- license::fingerprint` | ❌ W0 | ⬜ pending |
| 6-01-02 | 01 | 1 | LIC-01, LIC-04 | — | Cached JWT loads at startup, offline works | unit | `cargo nextest run -p cronometrix -- license::cache` | ❌ W0 | ⬜ pending |
| 6-01-03 | 01 | 1 | LIC-03 | — | Activation call sends fingerprint to DO Functions | integration | `cargo nextest run -p cronometrix -- license::activation` | ❌ W0 | ⬜ pending |
| 6-02-01 | 02 | 1 | LIC-01, LIC-05 | — | License gate rejects with 403 when unlicensed | unit | `cargo nextest run -p cronometrix -- license::middleware` | ❌ W0 | ⬜ pending |
| 6-02-02 | 02 | 1 | LIC-05 | — | Fingerprint mismatch → 403 on all routes | unit | `cargo nextest run -p cronometrix -- license::anticheat` | ❌ W0 | ⬜ pending |
| 6-03-01 | 03 | 2 | DEPL-01 | — | Installer script exits 0 on valid inputs | manual | N/A — Docker environment required | N/A | ⬜ pending |
| 6-03-02 | 03 | 2 | DEPL-02 | — | docker-compose.yml has api + web + cloudflared services | automated | `grep -E 'api:|web:|cloudflared:' /opt/cronometrix/docker-compose.yml` | ❌ W0 | ⬜ pending |
| 6-03-03 | 03 | 2 | DEPL-03 | — | cloudflared service reads CLOUDFLARE_TUNNEL_TOKEN | automated | `grep 'CLOUDFLARE_TUNNEL_TOKEN' /opt/cronometrix/docker-compose.yml` | ❌ W0 | ⬜ pending |
| 6-03-04 | 03 | 2 | DEPL-04 | — | System operates offline after initial activation | manual | N/A — requires network isolation test | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `backend/tests/license_fingerprint_test.rs` — stubs for LIC-02 fingerprint determinism
- [ ] `backend/tests/license_cache_test.rs` — stubs for LIC-01, LIC-04 JWT caching
- [ ] `backend/tests/license_middleware_test.rs` — stubs for LIC-01, LIC-05 gate behavior

*Existing cargo nextest infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `curl \| bash` installer end-to-end | DEPL-01 | Requires a fresh Linux VM with Docker | Provision fresh VM, run installer script, verify all 3 services start and API responds |
| Offline operation after activation | DEPL-04 | Requires network isolation (iptables/firewall) | Activate license, disconnect internet, restart api service, verify API responds normally |
| Cloudflare tunnel routing | DEPL-03 | Requires live CF tunnel and DNS | After installer, verify `{slug}.cronometrix.com` resolves and proxies to API |
| Hardware mismatch rejection | LIC-05 | Requires cloning disk to different hardware | Activate on machine A, copy `.env` + `license.jwt` to machine B, verify 403 on all routes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
