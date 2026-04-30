# Evidence: LIC-05 Cross-Host Clone-Rejection — Deferral to First Production Install

- **Date captured:** 2026-04-29
- **Captured by:** Phase 11 planner
- **Command run / action:** No live test executed. This document records the formal deferral of the LIC-05 cross-host clone-rejection test to the first paying-customer production install per Phase 11 D-06 / D-07 / D-09.
- **Expected:** A live cross-host clone test would: (1) install Cronometrix on machine A, (2) activate license + capture hardware fingerprint, (3) copy the cached license JWT to machine B, (4) start Cronometrix on machine B, (5) confirm activation REJECTS with `AppError::Unlicensed` because the fingerprint computed on machine B does not match the JWT's bound fingerprint.
- **Actual:** Test not physically executed in Phase 11. The code path that LIC-05 protects is exercised by:
  - `backend/tests/license_*` (in-process synthetic mismatch tests — covered in 06-04 + 08-04B)
  - `backend/tests/license_bypass_safety.rs::bypass_without_e2e_aborts_with_code_2` (locks the fail-closed contract)

  Live cross-host hardware proof is deferred to first production deployment because: (a) it requires two distinct physical machines or two cloud VMs, (b) the customer's deploy environment IS the validation environment, (c) the fingerprint formula (CPU + MAC + disk serial — see `backend/src/license/fingerprint.rs`) is platform-specific and worth verifying on the actual customer host class rather than synthetic CI hardware.

- **Verdict:** DEFERRED — to first production install per Phase 11 D-06.

## Risk Acknowledgement

LIC-05 is the highest-risk deferral in this milestone. A license-binding failure at customer deploy would be visible (the activation literally would not succeed) but would also be embarrassing — it surfaces a v1.0 protection gap exactly when the system is supposed to be paid-for-and-operational. Phase 11 accepts this risk because:

1. The in-process tests cover the AppError::Unlicensed branch (synthetic mismatch).
2. `backend/tests/license_bypass_safety.rs` proves the misconfiguration abort contract (`CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true` exits 2). This eliminates the worst-case footgun of a deploy script silently disabling the gate.
3. First customer install is the appropriate moment for the live test — that's when real hardware enters the loop.
4. v1.1 backlog item `LIC-05-CROSS-HOST` (added to REQUIREMENTS.md by this plan) carries the formal commitment to convert this deferral into a CI-runnable test using two cloud VMs.

## Pre-Flight Checklist for First Production Install (mandatory)

The deploy engineer running the first paid customer install MUST run this checklist BEFORE handing the system over to the customer. Treat it as the live LIC-05 evidence run.

```
# On machine A (the customer's actual server):
1. Run installer: curl https://install.cronometrix.com/{client-slug} | bash
2. On the /setup/license screen, paste the customer-issued license key.
3. Confirm activation succeeds: GET /api/v1/license/status returns valid=true.
4. SSH in and snapshot the fingerprint:
   cat /var/lib/cronometrix/license.jwt > /tmp/license-machineA.jwt
   curl localhost/api/v1/license/fingerprint > /tmp/fingerprint-machineA.json
5. Save both files to a secure location (NOT in the customer's repo).

# On machine B (a separate physical machine — laptop, second VM, anything different):
6. Run installer same way: curl ... | bash
7. SCP the JWT from step 4 to machine B:
   scp /tmp/license-machineA.jwt machineB:/var/lib/cronometrix/license.jwt
8. Restart cronometrix-api service.
9. Curl /api/v1/license/status — MUST return 401 Unlicensed (or whatever AppError::Unlicensed maps to).
10. Curl /api/v1/license/fingerprint — fingerprint MUST differ from the value in fingerprint-machineA.json.
11. Cleanup: stop machineB cronometrix service, delete the relocated JWT.

PASS criteria: step 9 returns 401 Unlicensed AND step 10 shows a different fingerprint.
FAIL criteria: step 9 returns 200 OK (clone succeeded — LIC-05 is broken).
```

The deploy engineer should commit a screenshot or log of step 9's response to a (separate) field-test evidence dir on first install, and update v1.1 backlog item `LIC-05-CROSS-HOST` with the result.

- **Artifacts:** None — this is a markdown-only deferral evidence.
- **External refs:**
  - REQUIREMENTS.md `## v1.1 Backlog` row `LIC-05-CROSS-HOST` (added by Plan 11-03 per D-09)
  - `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` deferred-items table (cross-reference added by Plan 11-03 per D-08)
  - `backend/src/license/fingerprint.rs` (the fingerprint formula under live test)
  - `backend/tests/license_bypass_safety.rs::bypass_without_e2e_aborts_with_code_2` (related fail-closed contract test)
  - `backend/src/license/service.rs` (`verify_license_jwt`, `activate_license`, `load_and_validate_license` — the in-process functions that mismatch tests drive)
  - Phase 11 11-CONTEXT.md decisions D-06, D-07, D-09 (the binding deferral decisions)
