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
and persist license records in Aiven PostgreSQL through the shared
`pg-store.js` infrastructure adapter. `DATABASE_URL` uses a restricted runtime
role and `DATABASE_CA_CERT_BASE64` supplies the Aiven CA; TLS certificate
verification is mandatory. The private key and database credentials NEVER
appear in responses, logs, or repo files.

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

3. **Provision the isolated Aiven database and runtime role** with the
   repository tooling documented below. The resulting table is:
   ```sql
   CREATE SCHEMA license_authority;
   CREATE TABLE license_authority.licenses (
     license_key          TEXT PRIMARY KEY,
     hardware_fingerprint TEXT,
     activated_at         BIGINT,
     last_renewed_at      BIGINT
   );
   ```
   Pre-seed each customer's license key BEFORE shipping their installer:
   ```sql
   INSERT INTO license_authority.licenses (license_key, hardware_fingerprint)
   VALUES ('XXXX-XXXX-XXXX-XXXX', NULL);
   ```
   The fingerprint stays NULL until the customer's first activation.

4. **Inject deploy-time env vars through `secretctl run`** (never paste them
   into shell history or commit them):
   ```bash
   secretctl run \
     -k cronometrix-license-private-key-pem \
     -k cronometrix-license-runtime-url \
     -k cronometrix-aiven-ca-base64 \
     -- bash scripts/deploy-license-functions.sh
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
fixture (no Postgres needed) and an ephemeral RSA keypair generated at test
load time (C-06: no private key lives in the repo, not even as a test
fixture — see `docs/runbooks/rotacion-clave-licencia.md`).

```bash
# 1. Install local-only test runtime (jsonwebtoken at the do-functions root).
cd do-functions
npm ci

# 2. Each test file generates its own RSA keypair at module load
#    (node:crypto generateKeyPairSync) and signs/verifies against it —
#    these tests only prove that what the handler signs, it can verify.
#    They do not and must not depend on the production signing key.

# 3. Run the full suite.
npm test
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
- **Database credentials never logged.** Same hygiene: `DATABASE_URL` and the
  CA are validated inside `resolveStore()`, the pg client closes the connection
  in `finally`, and any pg error is collapsed to `SERVER_ERROR` (T-06-41
  mitigation).
- **Single npm dep: `pg`.** Declared in each function's `package.json`.
  `jsonwebtoken` v9 is pre-installed in the DO Functions Node 22 runtime —
  it is referenced via a normal `require('jsonwebtoken')` but is NOT in
  the function `package.json`.
- **Top-level `do-functions/package.json` is local-test only.** It pulls
  `jsonwebtoken` as a devDependency so `node --test` can verify signed
  JWTs offline. DO Functions does NOT use it during deployment — it
  builds each function package independently.

## Test keys (C-06)

`do-functions/test-keys/` was removed. It used to hold a committed RSA
keypair (`test_priv.pem` / `test_pub.pem`) that turned out to be
byte-identical to `backend/src/license/pubkey.pem` — the public key the
Rust binary embeds and trusts in production. Anyone with a clone of this
repository could mint license JWTs the product would accept.

`packages/licenses/activate/test.js` and `packages/licenses/renew/test.js`
now each generate their own ephemeral RSA-2048 keypair at module load
(`node:crypto.generateKeyPairSync`) and verify only against that keypair.
This deliberately breaks the old three-way alignment with
`backend/src/license/pubkey.pem` — these unit tests were never meant to
prove production trust; that only requires proving a handler verifies what
it signs.

Production key rotation (including why `backend/tests/fixtures/` still
needs separate attention) is documented in
`docs/runbooks/rotacion-clave-licencia.md`.

## Why pg over alternatives

- Aiven is the selected managed PostgreSQL service. The runtime role is scoped
  to `license_authority.licenses`; the Aiven administrator credential is used
  only by the provisioning tool and never deployed to Functions.
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
