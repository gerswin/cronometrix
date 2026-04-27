---
phase: 06-licensing-deployment
plan: 04
subsystem: licensing

tags:
  - digitalocean-functions
  - serverless
  - nodejs22
  - jsonwebtoken
  - rs256
  - rsa
  - postgres
  - hardware-fingerprint
  - jwt
  - anti-cloning
  - operator-deployment

requires:
  - phase: 06-licensing-deployment
    plan: 01
    provides: backend/tests/fixtures/test_license_{priv,pub}key.pem (RSA-2048 test keypair, byte-identical to backend/src/license/pubkey.pem)

provides:
  - do-functions/packages/licenses/activate (POST /licenses/activate — RS256 JWT signer with hardware-fingerprint binding)
  - do-functions/packages/licenses/renew (POST /licenses/renew — refreshes JWT only when fingerprint matches the bound one)
  - do-functions/packages/licenses/shared-store.js (in-memory test store; pg-backed store inline in each handler)
  - do-functions/project.yml (DO Functions deployment manifest, nodejs:22 + web=true)
  - do-functions/README.md (operator deploy guide: keypair generation, Postgres seeding, doctl deploy, URL retrieval)
  - do-functions/test-keys/test_priv.pem + test_pub.pem (test keypair byte-identical to Plan 01 fixtures)
  - do-functions/.gitignore (excludes node_modules, *.private.pem, license_private.pem, .env)

affects:
  - 06-02 (Rust setup_activate handler now has a deployable counterparty for end-to-end smoke tests)
  - 06-03 (Docker Compose deployment can reference DO_FUNCTIONS_ACTIVATE_URL / DO_FUNCTIONS_RENEW_URL once doctl URLs are known)

tech-stack:
  added:
    - jsonwebtoken@^9.0.2 (devDependency at do-functions root for local test runtime; pre-installed in DO Functions Node 22 runtime so NOT declared per function)
    - pg@^8.13.0 (per-function dependency for Postgres-backed lookup/bind/touch in production)
  patterns:
    - "RS256 algorithm pinning in jwt.sign(...) — symmetric to Plan 01's Algorithm::RS256 verifier (T-06-44 alg-confusion mitigation)"
    - "Zero console.{log,error,warn,info} in handlers — private key never appears in logs (T-06-40)"
    - "catch path returns generic SERVER_ERROR — DB / key error details never leak to response body (T-06-41)"
    - "Lookup contract: undefined = not seeded, null = seeded-but-unbound, string = bound — eliminates a separate exists() round-trip"
    - "Renew never binds — unbound licenses return 403, only activate may set fingerprint (back-door prevention)"
    - "Idempotent re-activation on same fingerprint — supports reinstall on same hardware without churn"
    - "DO Functions args shape duck-typing: handler accepts both { body: {...} } (JSON) and top-level args (form-urlencoded)"

key-files:
  created:
    - do-functions/project.yml
    - do-functions/package.json
    - do-functions/.gitignore
    - do-functions/README.md
    - do-functions/test-keys/test_priv.pem
    - do-functions/test-keys/test_pub.pem
    - do-functions/packages/licenses/shared-store.js
    - do-functions/packages/licenses/activate/index.js
    - do-functions/packages/licenses/activate/package.json
    - do-functions/packages/licenses/activate/test.js
    - do-functions/packages/licenses/renew/index.js
    - do-functions/packages/licenses/renew/package.json
    - do-functions/packages/licenses/renew/test.js
  modified: []

key-decisions:
  - "Lookup tri-state (undefined / null / string) instead of separate exists() helper — sufficient signal in one round-trip; the plan's storeExists() helper was dropped per the plan's own §6 follow-up note."
  - "Top-level do-functions/package.json declares jsonwebtoken as devDependency for LOCAL test runtime ONLY. DO Functions deploy uses each function's package.json (declares only `pg`) — keeps the deployed bundle minimal because jsonwebtoken v9 is pre-installed by the Node 22 runtime."
  - "Renew never back-doors activation. An unbound license (fp === null) returns 403 from /licenses/renew, NOT a fresh binding. Only /licenses/activate may set fingerprint. Added explicit test 'returns 403 on unbound license (renew should not bind)' to lock this invariant."
  - "Test keys committed at do-functions/test-keys/test_{priv,pub}.pem (byte-identical to backend/tests/fixtures from Plan 01) so node:test runs offline without re-generating keys. .gitignore explicitly excludes only `*.private.pem` and `license_private.pem` — the test fixture filenames don't match those patterns."
  - "17 tests (10 activate + 7 renew) — exceeded the plan's ≥10 specification by adding: missing-fingerprint (separate from missing-key), env-missing 500 path, store-mutation side-effect assertion, top-level-args shape coverage on both endpoints, and the renew-unbound 403 invariant."

patterns-established:
  - "DO Functions handler hygiene: zero logging, generic catch errors, env-key checked before any DB I/O"
  - "Test ↔ production store parity: shared-store.js exposes the SAME async lookup/bind/touch contract as the pg-backed inline store, so handlers branch on process.env.TEST_STORE in exactly one place (getStore())"
  - "node:test fixture loading via __dirname-relative paths — three levels up from packages/licenses/{activate,renew} reaches do-functions/test-keys/, no path-walking glue needed"

requirements-completed:
  - LIC-03

duration: 6min
completed: 2026-04-27
---

# Phase 06 Plan 04: DO Functions License Server Summary

**Two web=true Node.js 22 DigitalOcean Functions (`activate`, `renew`) signing RS256 JWTs against a hardware-fingerprint-bound license database, with a 17-case offline test suite and a single-command operator deploy guide — completing the LIC-03 server-side authority that Plan 01's Rust client calls.**

## Performance

- **Duration:** ~6 min (20:52:23 → 20:58:17 UTC, 2026-04-27)
- **Tasks:** 1 (multi-file scaffold + test + impl)
- **Files created:** 13
- **Files modified:** 0

## Accomplishments

- DO Functions deployment manifest landed: `nodejs:22`, `web=true`, 15 s timeout matching Rust client reqwest timeout, env bindings for `LICENSE_PRIVATE_KEY` and `DATABASE_URL`.
- `activate` handler signs RS256 JWTs with `{ license_key, hardware_fingerprint, product:'cronometrix', iat, exp:+1y }` claims (D-06 confirmed by test).
- `activate` returns the full status-code spectrum the Rust client expects: 200 happy, 400 missing-field, 404 unknown-key, 409 already-bound-different-hardware, 500 missing-key-config / server-error.
- `renew` mirrors `activate` but refuses unbound and mismatched-fingerprint requests with 403 — anti-cloning defense in depth alongside Plan 01's Rust `LIC-05` startup check.
- Lookup contract codified: `undefined` = not seeded, `null` = seeded-but-unbound, `<string>` = bound. Same contract for both the in-memory test store and the production `pg`-backed store, so handlers branch by mode in exactly one place.
- 17 `node:test` cases pass offline using the fixture keypair: 10 activate + 7 renew. Coverage exceeds the plan's ≥10 specification.
- RS256 pinning verified in two ways: handler-level `algorithm: 'RS256'` literal AND test-level `header.alg === 'RS256'` decoded check, on both endpoints.
- Zero `console.{log,error,warn,info}` in any handler (T-06-40 private-key disclosure mitigation, verified via `grep -c`).
- Operator README documents one-time setup (RSA-2048 keypair generation, Postgres seeding, env vars, deploy command, URL retrieval), local testing flow, architecture invariants, and key rotation procedure.
- Test keys at `do-functions/test-keys/test_{priv,pub}.pem` are byte-identical to `backend/tests/fixtures/test_license_{priv,pub}key.pem` — preserves end-to-end determinism: a JWT signed by these fixtures will verify against the public key embedded in the Rust binary.

## Task Commits

1. **RED — failing tests + fixtures** — `6edc39f` (`test(06-04): add failing license-server tests + test-key fixtures`)
2. **GREEN — handlers + project.yml + README** — `9e3112d` (`feat(06-04): implement DO Functions license server (activate + renew)`)

No REFACTOR pass needed — both handlers are already minimal (≤140 LOC each, single responsibility, no duplication beyond the deliberate parallel structure between activate and renew).

## Files Created/Modified

**Created (deployment + docs):**
- `do-functions/project.yml` — DO Functions manifest
- `do-functions/README.md` — operator deploy + local-test guide
- `do-functions/.gitignore` — excludes `node_modules/`, `*.private.pem`, `license_private.pem`, `.env*`, build artifacts
- `do-functions/package.json` — local-test-only manifest pulling `jsonwebtoken` as devDependency

**Created (test fixtures):**
- `do-functions/test-keys/test_priv.pem` — RSA-2048 private key (byte-identical to `backend/tests/fixtures/test_license_privkey.pem`)
- `do-functions/test-keys/test_pub.pem` — RSA-2048 public key (byte-identical to `backend/src/license/pubkey.pem` and `backend/tests/fixtures/test_license_pubkey.pem`)

**Created (handlers + tests):**
- `do-functions/packages/licenses/shared-store.js` — in-memory test store (lookup / bind / touch / `__reset` / `__seedRow`)
- `do-functions/packages/licenses/activate/index.js` — main handler (signs JWT, binds fingerprint)
- `do-functions/packages/licenses/activate/package.json` — declares `pg` (jsonwebtoken pre-installed in DO runtime)
- `do-functions/packages/licenses/activate/test.js` — 10 `node:test` cases
- `do-functions/packages/licenses/renew/index.js` — main handler (refreshes JWT; 403 on mismatch / unbound)
- `do-functions/packages/licenses/renew/package.json` — declares `pg`
- `do-functions/packages/licenses/renew/test.js` — 7 `node:test` cases

## Decisions Made

- **Lookup tri-state replaces a separate `exists()` helper.** The plan's first draft proposed `storeExists(store, licenseKey)` for the activate path, but the plan's own §6 note rationalized dropping it in favor of `undefined` (not found) vs `null` (unbound) vs `<string>` (bound). This change ships in this plan's GREEN commit. The pg-backed store mirrors the same contract by mapping `r.rows.length === 0 → undefined` and `r.rows[0].hardware_fingerprint || null`.
- **Top-level `do-functions/package.json` is local-test only.** Each function package.json (`activate/`, `renew/`) declares only `pg`. The top-level package.json declares `jsonwebtoken` as a devDependency for `node --test` to verify signed JWTs offline. DO Functions does NOT use the top-level package.json during deployment — it builds each function package independently.
- **Renew never back-doors activation.** An unbound license (`fp === null`) returns 403 from `/licenses/renew`, NOT a fresh binding. The dedicated test `'returns 403 on unbound license (renew should not bind)'` locks this invariant.
- **Test keys committed; production keys excluded by `.gitignore`.** `test-keys/test_priv.pem` is a well-known fixture, not a secret. The `.gitignore` patterns `*.private.pem` and `license_private.pem` target operator-naming conventions for production keys — they don't match the test fixture filename.
- **Tests cover state mutation.** Beyond return-code checks, an explicit `'binds fingerprint on first activation (state mutation)'` test asserts the store transitions from `null → 'FP-A'`. This guards against a regression where the handler returns 200 but skips the bind — which would break the next renew call.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Test path off-by-one (`__dirname` resolution)**
- **Found during:** Task 1 RED-gate verification (`node --test packages/licenses/activate/test.js`)
- **Issue:** Plan §7 / §8 used `path.join(__dirname, '../../../../test-keys/...')` (4 levels up). With `__dirname` = `do-functions/packages/licenses/activate/`, four levels up resolves to the parent of `do-functions/`, not `do-functions/`. The test fail-message was the wrong-directory ENOENT.
- **Fix:** Reduced to 3 levels up (`activate/ → licenses/ → packages/ → do-functions/`). Applied symmetrically to `renew/test.js`. Added inline `__dirname` explanation comments to each test file so future readers don't mis-count.
- **Files modified:** `do-functions/packages/licenses/activate/test.js`, `do-functions/packages/licenses/renew/test.js`
- **Verification:** Both test suites now load fixture keys and run all assertions. 17/17 pass.
- **Committed in:** `6edc39f` (RED commit — fix folded into the same RED commit as the original test files; pre-implementation correction)

**2. [Rule 2 - Critical Functionality] Added 7 tests beyond the plan's 9-test floor**
- **Found during:** Behavior coverage review while drafting tests
- **Issue:** The plan listed 9 tests by name; the success criteria asked for "10+ Node tests cover all status codes (200/400/403/404/409/500)". The plan's enumeration covered 200/404/409 on activate and 200/403/404 on renew but did not cover the 500 missing-key-config path on either endpoint, the missing-hardware-fingerprint subcase, or the renew-unbound 403 invariant.
- **Fix:** Added 8 additional tests:
  - `activate`: missing-hardware-fingerprint (separate from missing-key), missing-LICENSE_PRIVATE_KEY (500 CONFIG_ERROR), state-mutation side-effect, top-level-args shape — totalling 10 cases.
  - `renew`: RS256-header decode, unbound-license 403 invariant, top-level-args shape — totalling 7 cases.
- **Files modified:** `do-functions/packages/licenses/activate/test.js`, `do-functions/packages/licenses/renew/test.js`
- **Verification:** 17/17 pass. Status codes 200/400/403/404/409/500 all covered.
- **Committed in:** `6edc39f` (RED) and validated GREEN in `9e3112d`

---

**Total deviations:** 2 auto-fixed (1 bug, 1 added critical coverage)
**Impact on plan:** Both deviations strengthen the plan's intent without scope creep. The path fix is a typographical correction to the spec; the test additions close the 500/missing-fingerprint/unbound-renew coverage gaps that the success criteria implied.

## Threat Flags

None. All new surface (DO Functions endpoints, environment bindings, license DB schema, Postgres TLS dependency) is enumerated in the plan's `<threat_model>` (T-06-37 through T-06-47). No new endpoints or trust boundaries beyond those.

## Issues Encountered

- **Test-fixture path off-by-one** in plan-as-written; fixed and noted in deviations table above.
- **`npm install` cold-start cost** (~3 s for `pg` per package, ~2 s for `jsonwebtoken` at the root) is acceptable for a one-time bootstrap; subsequent test runs are sub-100 ms. DO Functions does its own `--remote-build` install, independent of local `node_modules/`.
- **No production deploy verification** in this plan. The plan's `<verification>` block calls out manual `doctl serverless deploy` + `doctl serverless functions invoke` as a post-merge operator step. That validation is intentionally deferred until the operator has DO credentials and provisions the Postgres database.

## Self-Check

**Created files exist (verified via `test -f` shell loop):**
- `do-functions/project.yml` — FOUND
- `do-functions/package.json` — FOUND
- `do-functions/.gitignore` — FOUND
- `do-functions/README.md` — FOUND
- `do-functions/test-keys/test_priv.pem` — FOUND
- `do-functions/test-keys/test_pub.pem` — FOUND
- `do-functions/packages/licenses/shared-store.js` — FOUND
- `do-functions/packages/licenses/activate/index.js` — FOUND
- `do-functions/packages/licenses/activate/package.json` — FOUND
- `do-functions/packages/licenses/activate/test.js` — FOUND
- `do-functions/packages/licenses/renew/index.js` — FOUND
- `do-functions/packages/licenses/renew/package.json` — FOUND
- `do-functions/packages/licenses/renew/test.js` — FOUND

**Commits exist (verified via `git log --oneline`):**
- `6edc39f` (RED — `test(06-04)`) — FOUND
- `9e3112d` (GREEN — `feat(06-04)`) — FOUND

**Plan verification block:**
- `node --check do-functions/packages/licenses/activate/index.js` — PASS
- `node --check do-functions/packages/licenses/renew/index.js` — PASS
- `node --check do-functions/packages/licenses/shared-store.js` — PASS
- `cd do-functions && node --test packages/licenses/activate/test.js` — 10/10 PASS
- `cd do-functions && node --test packages/licenses/renew/test.js` — 7/7 PASS
- `grep -q "runtime: nodejs:22" do-functions/project.yml` — PASS
- `grep -q "web: true" do-functions/project.yml` — PASS
- `grep -q "LICENSE_PRIVATE_KEY" do-functions/project.yml` — PASS
- `grep -q "algorithm: 'RS256'"` in both handlers — PASS
- `grep -q "ALREADY_ACTIVATED"` in activate — PASS
- `grep -q "LICENSE_NOT_FOUND"` in activate — PASS
- `grep -q "HARDWARE_MISMATCH"` in renew — PASS
- `grep -c "console\.log\|console\.error\|console\.warn\|console\.info"` across all handlers — 0 (PASS)
- `cd do-functions/packages/licenses/activate && npm install` — PASS (`pg` installed)
- `cd do-functions/packages/licenses/renew && npm install` — PASS (`pg` installed)

## Self-Check: PASSED

## Next Phase Readiness

The DO Functions side of LIC-03 is now deployable. To complete the Plan 01 ↔ Plan 04 round-trip:

1. **Operator deploys once:** `cd do-functions && doctl serverless deploy . --remote-build` (after generating a production RSA-2048 keypair, seeding Postgres, and exporting `LICENSE_PRIVATE_KEY` + `DATABASE_URL`).
2. **Operator captures URLs:** `doctl serverless functions get licenses/activate --url` and likewise for renew.
3. **Plan 02 wiring** (next plan in this phase) reads those URLs from `Config.do_functions_activate_url` / `Config.do_functions_renew_url` and surfaces `setup_activate` POST to the Rust API.
4. **Plan 03 deployment** packages the URLs as `DO_FUNCTIONS_ACTIVATE_URL` / `DO_FUNCTIONS_RENEW_URL` env vars in the Docker Compose `.env.template`.

Production key rotation procedure:
1. Operator generates new RSA-2048 keypair on a hardened workstation.
2. Private key uploaded to DO Functions env vars (NEVER committed).
3. Public key replaces `backend/src/license/pubkey.pem`.
4. Operator also replaces `backend/tests/fixtures/test_license_{priv,pub}key.pem` AND `do-functions/test-keys/test_{priv,pub}.pem` with a new test-only pair (or removes the integration tests that depend on test sign-and-verify — production CI typically does the latter).
5. Recompile Rust API; redeploy DO Functions; restart cronometrix-api.

Deferred items (out of scope for v1, called out in plan threat model):
- Application-level rate limiting / fail2ban (T-06-43 accepted; DO platform-level limits are enough for v1).
- Audit log of binding decisions (T-06-39; coarse audit lives in `activated_at` + `last_renewed_at`).
- Multi-region deployment (single region adequate for license traffic volume).
- Signed Cloudflare Tunnel telemetry (out of scope; lives in install bash + cloudflared service).

---
*Phase: 06-licensing-deployment*
*Plan: 04*
*Completed: 2026-04-27*
