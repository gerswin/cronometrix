---
status: partial
phase: 06-licensing-deployment
source: [06-VERIFICATION.md]
started: 2026-04-28T01:15:00Z
updated: 2026-04-28T01:15:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. End-to-end installer smoke on a fresh Ubuntu 22.04 Linux VM with Docker
expected: `sudo bash deploy/install.sh` (with all CRONOMETRIX_* env vars set) brings up api+web+cloudflared, prints health-check OK, and exits 0 within ~3 minutes
result: [pending]

### 2. Cloudflare tunnel propagation: `https://{slug}.cronometrix.com` resolves to the web service
expected: After installer completes, `curl -sI https://{slug}.cronometrix.com/` returns 200 within ~60s of cloudflared startup; the page is the Cronometrix web UI served via the cloudflared tunnel
result: [pending]

### 3. Offline operation after activation (DEPL-04 manual smoke)
expected: After successful activation, run `docker compose stop cloudflared` then sever the host's WAN. `curl http://127.0.0.1:3001/api/v1/health` still returns 200; UI at http://127.0.0.1:3000 still renders and license-protected operations succeed (license JWT is cached on disk)
result: [pending]

### 4. Hardware fingerprint anti-cloning rejection (LIC-05 manual smoke)
expected: Activate license on machine A, then copy `/opt/cronometrix/data/license.jwt` + `.env` to machine B and restart api. license_valid stays false on B; all protected routes return 403 UNLICENSED on B
result: [pending]

### 5. Production RSA keypair rotation
expected: Operator-side: cargo build succeeds, deployed image verifies JWTs minted by prod DO Functions, test fixtures still align with whatever test keypair is used in CI (or test-keypair-dependent tests are skipped per Plan 04 README)
result: [pending]

### 6. Frontend visual fidelity: /setup/license page matches UI-SPEC
expected: Browser inspection confirms the rendered card layout matches the UI-SPEC mockup; error and success banners use the documented border-l-4 + lucide icons; submit button uses aria-disabled (NOT disabled attr) so screen readers announce state
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
