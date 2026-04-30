# Plan 11-03 Summary — LIC-05 + Hikvision Deferral Evidence

**Status:** Complete
**Commit:** `6a02166 — docs(11-03): LIC-05 + Hikvision deferral evidence + v1.1 Backlog + 06-VERIFICATION xref`
**Date:** 2026-04-29

## What shipped

Pure markdown plan, fully autonomous, no operator action required. 5 file modifications in one atomic commit:

| Path | Change | Lines |
|------|--------|-------|
| `evidence/06-lic-05-deferral/README.md` | NEW — risk-accept doc + 11-step pre-flight checklist | 60 |
| `evidence/07-hikvision-deferral/README.md` | NEW — risk-accept doc + 6-step field test checklist | 43 |
| `evidence/00-overview.md` | Rows 06 + 07 verdicts: `pending` → `DEFERRED` | 4 (delta) |
| `.planning/REQUIREMENTS.md` | v1.1 Backlog table: added `LIC-05-CROSS-HOST` row at line 234; footer "Last updated" stamp at line 244 reflects Plan 11-03 | 3 (delta) |
| `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` | Cross-reference paragraph added at line 61 (Shape B — paragraph after deferred-items table) linking to the Phase 11 LIC-05 evidence | 2 (delta) |

## Decisions honored

| Decision | What it required | Where it landed |
|----------|------------------|-----------------|
| **D-06** | LIC-05 cross-host deferred to first prod install | `evidence/06-lic-05-deferral/README.md` Verdict: DEFERRED |
| **D-07** | Pre-flight checklist for deploy engineer at first install | 11-step checklist with PASS/FAIL criteria in LIC-05 README |
| **D-08** | 06-VERIFICATION.md cross-references Phase 11 evidence | Paragraph appended after deferred-items table (Shape B per plan); link uses relative path `../11-v1-0-live-environment-validation/evidence/06-lic-05-deferral/README.md` |
| **D-09** | REQUIREMENTS.md v1.1 Backlog gets `LIC-05-CROSS-HOST` row | Inserted IMMEDIATELY AFTER `DEPL-03-AUTO`; backlog table now has 2 rows |
| **D-10** | Real Hikvision live test deferred (mock_hikvision sufficient) | `evidence/07-hikvision-deferral/README.md` Verdict: DEFERRED |
| **D-13** | DEFERRED is acceptable verdict for these two requirements | Both READMEs use Verdict: DEFERRED; overview rows 06 + 07 transitioned `pending → DEFERRED` |
| **D-14** | Atomic commit per evidence item | All 5 files in single commit `6a02166` |
| **D-15** | No production code changes | Confirmed: 0 files under `backend/src/`, `frontend/src/`, `.github/workflows/` modified |

## LIC-05 pre-flight checklist (verbatim copy for deploy-prep reuse)

The deploy engineer running the first paid customer install MUST run this checklist BEFORE handing the system over to the customer.

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

## Bidirectional traceability sealed

```
06-VERIFICATION.md deferred-items table
  ↓ (Phase 11 deferral cross-reference paragraph at line 61)
Phase 11 / 06-lic-05-deferral / README.md (Verdict: DEFERRED)
  ↓ (External refs section)
REQUIREMENTS.md v1.1 Backlog (LIC-05-CROSS-HOST row at line 234)
  ↓ (Notes column references back to evidence file)
Phase 11 / 06-lic-05-deferral / README.md
```

Any audit trail walks the chain in either direction without losing context.

## Audit-gate impact

| Phase 11 requirement | Pre-11-03 | Post-11-03 |
|----------------------|-----------|------------|
| VALIDATE-LIC-05-CLONE | unaccounted-for | DEFERRED with risk-accept doc + pre-flight checklist (gate-pass per D-13) |
| VALIDATE-HIKVISION-LIVE | unaccounted-for | DEFERRED with risk-accept doc + field test checklist (gate-pass per D-13) |

Both deferred-but-evidenced requirements now satisfy the Phase 11 D-13 gate. Remaining 4 active items (VALIDATE-CI-GREEN, VALIDATE-CI-RED, VALIDATE-BRANCH-PROTECTION, VALIDATE-INSTALLER-SMOKE) still need operator-driven evidence runs.

## Next-step handoff

Plan 11-03 is independent of the operator-driven plans (11-01, 11-02, 11-04, 11-05). They can run in any order once the operator has the D-11 prerequisites ready (GitHub Actions session, Ubuntu 22.04 VM, Cloudflare tunnel TOKEN, DO Functions URL). Recommended execution order when operator is ready:

1. 11-01 Tasks 2 + 3 (push branch → live CI green run + local make e2e)
2. 11-02 (deliberately-red regression PR)
3. 11-04 (branch protection toggle in GitHub UI)
4. 11-05 (fresh-VM installer smoke — independent track, can run in parallel with 11-01/02 if VM ready)

After all 4 active items show `Verdict: PASS`, Phase 11 verification can run and milestone v1.0 unlocks for archive.
