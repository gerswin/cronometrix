# Cronometrix License Server

DigitalOcean Functions implementation of the Cronometrix license activation
and renewal endpoints. Consumed by the Rust API's
`license::service::activate_license` and `license::service::renewal_task`
(see `backend/src/license/service.rs`).

## Architecture at a glance

| Endpoint                  | Method | Bound to               | Purpose                                              |
|---------------------------|--------|------------------------|------------------------------------------------------|
| `/licenses/activate`      | POST   | `packages/licenses/activate/index.js` | First-time activation; binds hardware fingerprint, signs RS256 JWT (`exp = +1y`) |
| `/licenses/renew`         | POST   | `packages/licenses/renew/index.js`    | Daily silent renewal; refreshes JWT only when fingerprint matches the bound one (anti-cloning) |

Both functions read the RSA private key from `process.env.LICENSE_PRIVATE_KEY`
and persist license records in Postgres via `process.env.DATABASE_URL`. The
private key NEVER appears in responses, logs, or repo files.

The Rust verifier (Plan 01) embeds the matching public key at compile time
and pins `Algorithm::RS256` — defense in depth against `alg=HS256` /
`alg=none` confusion attacks.

## One-time setup

1. **Install doctl** — <https://docs.digitalocean.com/reference/doctl/how-to/install/>
   then run `doctl auth init` and `doctl serverless install`.

2. **Generate the production RSA-2048 keypair** (once, kept in a vault):
   ```bash
   openssl genrsa -out license_private.pem 2048
   openssl rsa -in license_private.pem -pubout -out license_public.pem
   ```
   - Copy `license_public.pem` to `backend/src/license/pubkey.pem` and rebuild
     the API images. The public key must round-trip with the private key on
     every deploy — mismatched pairs cause every `verify_license_jwt` call to
     fail with `JwtInvalid`, which surfaces as `AppError::Unlicensed` (HTTP 403).
   - Store `license_private.pem` ONLY as a DO Functions env var. Do NOT commit.
     The repo's `.gitignore` excludes `*.private.pem` and `license_private.pem`.

3. **Provision a license database** (DO Managed Postgres recommended):
   ```sql
   CREATE TABLE licenses (
     license_key          TEXT PRIMARY KEY,
     hardware_fingerprint TEXT,
     activated_at         BIGINT,
     last_renewed_at      BIGINT
   );
   ```
   Pre-seed each customer's license key BEFORE shipping their installer:
   ```sql
   INSERT INTO licenses (license_key, hardware_fingerprint)
   VALUES ('XXXX-XXXX-XXXX-XXXX', NULL);
   ```
   The fingerprint stays NULL until the customer's first activation.

4. **Set deploy-time env vars** (operator's shell, never the repo):
   ```bash
   export LICENSE_PRIVATE_KEY="$(cat license_private.pem)"
   export DATABASE_URL="postgres://user:pass@host:5432/licenses?sslmode=require"
   ```

5. **Deploy**:
   ```bash
   cd do-functions
   doctl serverless deploy . --remote-build
   ```
   `--remote-build` makes DO install `pg` (declared in
   `packages/licenses/{activate,renew}/package.json`) inside the runtime
   sandbox. `jsonwebtoken` v9 is pre-installed by the Node 22 DO runtime —
   we do NOT vendor it.

6. **Get the function URLs**:
   ```bash
   doctl serverless functions get licenses/activate --url
   doctl serverless functions get licenses/renew    --url
   ```
   Set these as `DO_FUNCTIONS_ACTIVATE_URL` and `DO_FUNCTIONS_RENEW_URL`
   in each client server's `/opt/cronometrix/.env` after install.

## Local testing

The unit tests run entirely offline using the in-memory `shared-store.js`
fixture (no Postgres needed) and the test RSA keypair under
`do-functions/test-keys/`.

```bash
# 1. Install local-only test runtime (jsonwebtoken at the do-functions root).
cd do-functions
npm install --silent

# 2. Test keys are committed at do-functions/test-keys/. They are byte-
#    identical to backend/tests/fixtures/test_license_{priv,pub}key.pem
#    (Plan 01 fixtures) so the JWTs the DO Functions sign verify against
#    the same public key the Rust backend embeds.
ls test-keys/

# 3. Run the full suite (17 tests across both handlers).
node --test packages/licenses/activate/test.js
node --test packages/licenses/renew/test.js
```

The tests cover:
- 200 happy paths for activate (unbound + idempotent re-bind) and renew
- 400 on missing `license_key` / `hardware_fingerprint`
- 403 on renew with mismatched OR unbound fingerprint (anti-cloning)
- 404 on unknown license key (both endpoints)
- 409 on activate with already-bound-to-different-fingerprint
- 500 on missing `LICENSE_PRIVATE_KEY` env var
- RS256 algorithm pinning verified by inspecting the JWT header

## Architecture notes

- **RS256 algorithm pinning.** Hardcoded in both
  `packages/licenses/activate/index.js` and
  `packages/licenses/renew/index.js`. The Rust verifier hardcodes
  `Algorithm::RS256` symmetrically. Swapping algorithms requires changes in
  both places (T-06-44 alg-confusion mitigation).
- **License records seeded by operator.** Activation only binds the
  fingerprint; it never creates new license_key rows. Customer license keys
  must exist in the `licenses` table before the customer attempts their
  first activation.
- **Renewal never back-doors activation.** If `hardware_fingerprint` is NULL
  in the row (license seeded but never activated), `/licenses/renew` returns
  403 — only `/licenses/activate` may bind a fingerprint. This pairs with
  Plan 01's Rust LIC-05 startup fingerprint check for two-tier defense
  against stolen-JWT replay (T-06-42).
- **Private key never logged, never in responses.** The handlers contain no
  `console.log` / `console.error` statements. The catch path returns a
  generic `SERVER_ERROR` body — no exception message, no stack trace
  (T-06-40 mitigation).
- **Database credentials never logged.** Same hygiene: `DATABASE_URL` is
  read once inside `getStore()`, the pg client closes the connection in
  `finally`, and any pg error is collapsed to `SERVER_ERROR` (T-06-41
  mitigation).
- **Single npm dep: `pg`.** Declared in each function's `package.json`.
  `jsonwebtoken` v9 is pre-installed in the DO Functions Node 22 runtime —
  it is referenced via a normal `require('jsonwebtoken')` but is NOT in
  the function `package.json`.
- **Top-level `do-functions/package.json` is local-test only.** It pulls
  `jsonwebtoken` as a devDependency so `node --test` can verify signed
  JWTs offline. DO Functions does NOT use it during deployment — it
  builds each function package independently.

## Test key alignment with Plan 01

`do-functions/test-keys/test_priv.pem` and `test_pub.pem` are byte-identical
copies of `backend/tests/fixtures/test_license_privkey.pem` and
`test_license_pubkey.pem`, which are themselves byte-identical to
`backend/src/license/pubkey.pem` (the public key embedded in the Rust
binary). This three-way alignment lets:

1. The DO Functions sign a test JWT with the test private key.
2. A Rust integration test verify that JWT against the embedded public key.
3. The same Rust test embed-and-verify cycle work without network access.

When operators rotate keys for production:
- Replace `backend/src/license/pubkey.pem` with the production public key.
- Replace `do-functions/test-keys/test_priv.pem` and `test_pub.pem` with a
  new test-only pair (do NOT use the production keys for tests), OR remove
  the integration tests that depend on test sign-and-verify (typical for
  production CI).
- Recompile the Rust API and redeploy DO Functions with the new private
  key in `LICENSE_PRIVATE_KEY`.

## Why pg over alternatives

- DO Managed Postgres is the path of least resistance for DO Functions
  deployments — same dashboard, same billing, same network.
- `pg` (node-postgres) is the de-facto Node Postgres driver: stable since
  2010, no compiled deps, ships pure-JS.
- Alternatives considered:
  - **DO App Platform Database (KV)** — stronger lock-in, slower cold
    starts, no SQL.
  - **Supabase** — extra account / billing dependency the operator does
    not need.
  - **In-Functions SQLite** — DO Functions filesystem is ephemeral; would
    lose all bindings on every redeploy.

## Deferred (out of scope for v1)

- **Rate limiting per source IP.** DO Functions has platform-level abuse
  limits (T-06-43 accept). Application-level fail2ban-style rate limiting
  is deferred until v1 telemetry shows abuse.
- **Signed Cloudflare Tunnel telemetry** — out of scope for the licensing
  server; lives in the install bash + cloudflared service.
- **Multi-region deployment.** Single region is fine for the license traffic
  volume (one POST per client per 24h). Add a second region if SLOs demand.
- **Audit log of binding decisions.** `activated_at` + `last_renewed_at`
  columns provide coarse audit; full audit (who, when, IP) is deferred per
  CONTEXT.md "license analytics out of scope".
