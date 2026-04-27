# Phase 6: Licensing & Deployment - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-27
**Phase:** 06-licensing-deployment
**Areas discussed:** License JWT signing, Fingerprint resilience, License renewal model, Installer interactivity

---

## License JWT Signing

| Option | Description | Selected |
|--------|-------------|----------|
| RS256 asymmetric | Private key on DO Functions only, public key hardcoded in binary. Full offline validation. | ✓ |
| HS256 shared secret | Simpler, same as auth JWTs. Secret must exist in binary env — extractable. | |
| Ed25519 asymmetric | Faster/smaller than RS256, same offline benefit. Less common in JWT libs. | |

**User's choice:** RS256 asymmetric (recommended)

**Follow-up — public key storage:**

| Option | Description | Selected |
|--------|-------------|----------|
| Hardcoded in source | String constant in Rust source. No env var. Recompile to rotate. | ✓ |
| Env var at runtime | LICENSE_PUBLIC_KEY in Docker Compose .env. Flexible but extra config. | |

**User's choice:** Hardcoded in source (recommended)

---

## Fingerprint Resilience

| Option | Description | Selected |
|--------|-------------|----------|
| CPU + primary MAC + disk serial | 3 components, all from /proc and /sys, no root required. Standard approach. | ✓ |
| MAC address only | Simplest. Fragile — changes on NIC swap or virtual adapters. | |
| CPU only | Stable but less unique, especially on VMs with shared CPU info. | |

**User's choice:** CPU + primary MAC + disk serial (recommended)

**Follow-up — matching strictness:**

| Option | Description | Selected |
|--------|-------------|----------|
| Strict exact match | All 3 must match. Simplest. Reactivation on any hardware change. | ✓ |
| 2-of-3 match | Tolerates one component change. More complex logic. | |
| Hash with salt | Fingerprint tied to install event. Any swap = new fingerprint. | |

**User's choice:** Strict exact match (recommended)

---

## License Renewal Model

| Option | Description | Selected |
|--------|-------------|----------|
| Annual expiry + online renewal | JWT exp = 1 year. Startup renewal check within 30 days. Offline grace via cached JWT. | ✓ |
| Perpetual (no expiry) | Activate once. No recurring revenue. | |
| 30-day check-in | Monthly phone-home. Aggressive, breaks offline easily. | |

**User's choice:** Annual expiry + online renewal

**User's notes:** "1 annual expiry of updates and support"

**Follow-up — post-expiry behavior:**

| Option | Description | Selected |
|--------|-------------|----------|
| System keeps running, no updates | Software works indefinitely. Expiry = end of support+updates entitlement only. | ✓ |
| System blocks on expired license | JWT exp = hard API gate. Forces renewal for continued operation. | |

**User's choice:** System keeps running, no updates

---

## Installer Interactivity

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal prompts | 3 prompts: license key, client slug, admin password. Rest auto-generated. | ✓ |
| Fully interactive | All config options prompted. More control, longer install. | |
| Non-interactive (pre-created .env) | Zero prompts, reads existing .env. Too technical for typical clients. | |

**User's choice:** Minimal prompts (recommended)

**Follow-up — Cloudflare tunnel mechanism:**

| Option | Description | Selected |
|--------|-------------|----------|
| cloudflared with pre-created tunnel token | Operator creates tunnel in CF dashboard, provides token to installer. Simple, one manual step per client. | ✓ |
| CF Zero Trust API (fully automated) | Script creates tunnel programmatically. Zero manual steps but adds CF API token complexity. | |
| Manual post-install | Skip in installer, admin configures cloudflared manually. Not one-command. | |

**User's choice:** cloudflared with pre-created tunnel token

---

## Claude's Discretion

- Fingerprint collection details (cpuinfo parsing, NIC selection when multiple, disk selection when multiple)
- Docker image tagging and registry
- Installer error messages and rollback
- DO Functions implementation language

## Deferred Ideas

- CF Zero Trust API automated tunnel creation — v1 pre-created token is sufficient
- License usage analytics / telemetry — out of scope
- Grace period for hardware change reactivation — support process, not system feature
- Multi-client license management portal — each install is independent by design
