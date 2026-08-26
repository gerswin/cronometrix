# Aiven License Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deploy Cronometrix's existing license authority on DigitalOcean Functions with verified-TLS Aiven PostgreSQL persistence, issue the NanoPi's hardware-bound license, and ship a matching ARM64 API.

**Architecture:** Keep license rules in the existing Functions and Rust license domain. Add one shared Node PostgreSQL adapter at the infrastructure boundary, provision an isolated database/schema/roles with an operator-only tool, and pass all credentials through `secretctl run`; only the RSA public key enters the repository.

**Tech Stack:** Node.js 22 DigitalOcean Functions, `pg` 8.x, Aiven PostgreSQL, Rust/jsonwebtoken RS256, Bash, `secretctl`, `doctl`, GitHub Actions, Docker Buildx.

**Spec:** `docs/superpowers/specs/2026-08-26-aiven-license-authority-design.md`

## Global Constraints

- Never set `CRONOMETRIX_E2E` or `CRONOMETRIX_LICENSE_BYPASS` in production.
- Never print or commit database URLs, passwords, CA contents, license values, private PEM material, JWTs, or installer secrets.
- Use `secretctl run` with output sanitization enabled; do not call `secretctl get` from agent-run code.
- Keep `avnadmin` out of Function runtime; Functions use `cronometrix_license_runtime` only.
- Verify Aiven TLS with its CA and `rejectUnauthorized: true`; no insecure fallback.
- Qualify every production query as `license_authority.licenses`.
- Preserve the atomic guarded bind that enforces LIC-05.
- The private key remains only in `secretctl` and DigitalOcean Functions; the API embeds only `backend/src/license/pubkey.pem`.
- All code changes use TDD and all release images remain digest-pinned for `linux/amd64,linux/arm64`.
- Do not deploy to the existing `tiquemax`, `somonexa`, or `tiquemax-pdf-fn` Functions namespaces.

---

### Task 1: Verified-TLS PostgreSQL adapter

**Files:**
- Create: `do-functions/packages/licenses/pg-store.js`
- Create: `do-functions/packages/licenses/pg-store.test.js`
- Modify: `do-functions/package.json`
- Modify: `do-functions/package-lock.json`

**Interfaces:**
- Produces: `normalizeDatabaseUrl(rawUrl: string): string`
- Produces: `decodeCa(base64: string): string`
- Produces: `buildPgConfig(env: object): object`
- Produces: `createPgStore(options?: { env?: object, Client?: class }): { lookup, bind, touch }`
- Produces: `LICENSES_TABLE = 'license_authority.licenses'`

- [ ] **Step 1: Add failing adapter contract tests**

Create `pg-store.test.js` with table-driven tests that assert URL normalization,
strict CA validation, verified TLS, timeouts, qualified parameterized queries,
guarded binding, and secret-free errors:

```js
const test = require('node:test');
const assert = require('node:assert/strict');
const {
  LICENSES_TABLE,
  normalizeDatabaseUrl,
  buildPgConfig,
  createPgStore,
} = require('./pg-store');

const CA = Buffer.from(
  '-----BEGIN CERTIFICATE-----\nZmFrZQ==\n-----END CERTIFICATE-----\n',
).toString('base64');

test('normalizes libpq ssl options before applying verified CA config', () => {
  const normalized = normalizeDatabaseUrl(
    'postgres://user:p%40ss@db.example:25362/licenses?sslmode=require&application_name=cronometrix',
  );
  const url = new URL(normalized);
  assert.equal(url.searchParams.has('sslmode'), false);
  assert.equal(url.searchParams.get('application_name'), 'cronometrix');
  assert.equal(url.password, 'p%40ss');
});

test('builds fail-closed TLS config with bounded timeouts', () => {
  const config = buildPgConfig({
    DATABASE_URL: 'postgres://user:pass@db.example:25362/licenses?sslmode=require',
    DATABASE_CA_CERT_BASE64: CA,
  });
  assert.deepEqual(config.ssl, {
    ca: Buffer.from(CA, 'base64').toString('utf8'),
    rejectUnauthorized: true,
  });
  assert.equal(config.connectionTimeoutMillis, 5000);
  assert.equal(config.query_timeout, 5000);
});

for (const env of [
  { DATABASE_CA_CERT_BASE64: CA },
  { DATABASE_URL: 'postgres://user:do-not-leak@db.example/licenses' },
  { DATABASE_URL: 'postgres://user:do-not-leak@db.example/licenses', DATABASE_CA_CERT_BASE64: '!' },
]) {
  test('rejects missing or invalid TLS configuration without leaking values', () => {
    assert.throws(() => buildPgConfig(env), (error) => {
      assert.equal(error.message.includes('do-not-leak'), false);
      return true;
    });
  });
}

test('uses qualified parameterized SQL and preserves guarded bind', async () => {
  const calls = [];
  class FakeClient {
    constructor(config) { calls.push(['config', config]); }
    async connect() { calls.push(['connect']); }
    async end() { calls.push(['end']); }
    async query(text, values) {
      calls.push(['query', text, values]);
      if (text.startsWith('SELECT')) return { rows: [{ hardware_fingerprint: null }] };
      return { rowCount: 1 };
    }
  }
  const store = createPgStore({
    env: {
      DATABASE_URL: 'postgres://user:pass@db.example/licenses',
      DATABASE_CA_CERT_BASE64: CA,
    },
    Client: FakeClient,
  });
  assert.equal(await store.lookup('KEY1-KEY2-KEY3-KEY4'), null);
  assert.equal(await store.bind('KEY1-KEY2-KEY3-KEY4', 'FP-A', 123), true);
  const sql = calls.filter(([kind]) => kind === 'query').map(([, text]) => text).join('\n');
  assert.match(sql, new RegExp(LICENSES_TABLE.replace('.', '\\.')));
  assert.match(sql, /hardware_fingerprint IS NULL OR hardware_fingerprint = \$1/);
  assert.equal(sql.includes('KEY1-KEY2-KEY3-KEY4'), false);
});
```

- [ ] **Step 2: Run the adapter test and verify RED**

Run: `cd do-functions && node --test packages/licenses/pg-store.test.js`

Expected: FAIL with `Cannot find module './pg-store'`.

- [ ] **Step 3: Implement the minimal shared adapter**

Implement strict validation and a `withClient` helper. Delete `sslmode`,
`sslcert`, `sslkey`, and `sslrootcert` from the parsed URL before passing the
programmatic TLS object. Use only the following parameterized statements:

```js
const LOOKUP_SQL = `SELECT hardware_fingerprint
  FROM license_authority.licenses WHERE license_key = $1`;
const BIND_SQL = `UPDATE license_authority.licenses
  SET hardware_fingerprint = $1,
      activated_at = COALESCE(activated_at, $2)
  WHERE license_key = $3
    AND (hardware_fingerprint IS NULL OR hardware_fingerprint = $1)`;
const TOUCH_SQL = `UPDATE license_authority.licenses
  SET last_renewed_at = $1 WHERE license_key = $2`;
```

`decodeCa` must reject malformed base64 and decoded text without both PEM
certificate delimiters. Errors name only the missing/invalid environment key.

- [ ] **Step 4: Add `pg` to the local tool/test package and run GREEN**

Run:

```bash
cd do-functions
npm install --save-dev pg@^8.13.0
node --test --test-name-pattern='pg|TLS|qualified|guarded' \
  packages/licenses/pg-store.test.js
```

Expected: adapter tests PASS with no warnings or secret-like output.

- [ ] **Step 5: Commit the adapter**

```bash
git add do-functions/package.json do-functions/package-lock.json \
  do-functions/packages/licenses/pg-store.js \
  do-functions/packages/licenses/pg-store.test.js
git commit -m "feat(licenses): add verified Aiven pg adapter"
```

---

### Task 2: Route activation and renewal through the adapter

**Files:**
- Modify: `do-functions/packages/licenses/activate/index.js`
- Modify: `do-functions/packages/licenses/activate/test.js`
- Modify: `do-functions/packages/licenses/renew/index.js`
- Modify: `do-functions/packages/licenses/renew/test.js`
- Modify: `do-functions/project.yml`
- Modify: `do-functions/README.md`

**Interfaces:**
- Consumes: `createPgStore()` and existing `shared-store` test contract.
- Produces: `resolveStore(env = process.env)` in each Function, returning the in-memory store only when `TEST_STORE === '1'` and otherwise `createPgStore({ env })`.

- [ ] **Step 1: Add failing integration assertions**

In both Function test files, clear `TEST_STORE` temporarily and assert that a
missing CA returns the existing generic `SERVER_ERROR`, then restore the test
environment. Also assert `project.yml` contains
`DATABASE_CA_CERT_BASE64: "${DATABASE_CA_CERT_BASE64}"` and
`SOURCE_SHA: "${SOURCE_SHA}"` in a Node static test.

```js
test('production store fails closed when CA configuration is absent', async () => {
  const saved = process.env.TEST_STORE;
  delete process.env.TEST_STORE;
  delete process.env.DATABASE_CA_CERT_BASE64;
  try {
    const r = await handler({
      body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.equal(r.statusCode, 500);
    assert.equal(r.body.error.code, 'SERVER_ERROR');
  } finally {
    process.env.TEST_STORE = saved;
  }
});
```

- [ ] **Step 2: Run Function tests and verify RED**

Run: `cd do-functions && npm test`

Expected: FAIL because production handlers still instantiate `pg.Client`
without requiring the CA and `project.yml` lacks the new variable.

- [ ] **Step 3: Replace duplicated stores with the shared adapter**

Delete the inline `Client` implementations from both handlers. Keep
`shared-store` for tests and call `createPgStore` for production. Do not change
response codes, JWT claims, RS256 pinning, or guarded-bind semantics.

Add to the package environment in `project.yml`:

```yaml
DATABASE_CA_CERT_BASE64: "${DATABASE_CA_CERT_BASE64}"
SOURCE_SHA: "${SOURCE_SHA}"
```

Update README deployment requirements, Aiven object names, TLS contract, and
the exact `secretctl run` flow without example secret values.

- [ ] **Step 4: Run full Function suite**

Run: `cd do-functions && npm test`

Expected: all activation, renewal, concurrency, TLS, and adapter tests PASS.

- [ ] **Step 5: Commit Function integration**

```bash
git add do-functions/packages/licenses/activate do-functions/packages/licenses/renew \
  do-functions/project.yml do-functions/README.md
git commit -m "refactor(licenses): use isolated verified pg store"
```

---

### Task 3: Idempotent Aiven provisioner and license issuer

**Files:**
- Create: `do-functions/tools/aiven-config.js`
- Create: `do-functions/tools/aiven-config.test.js`
- Create: `do-functions/tools/provision-aiven.js`
- Create: `do-functions/tools/provision-aiven.test.js`
- Create: `do-functions/tools/issue-license.js`
- Create: `do-functions/tools/issue-license.test.js`
- Modify: `do-functions/package.json`

**Interfaces:**
- Produces: `buildDatabaseUrl(adminUrl, { username, password, database }): string`
- Produces: `provision({ adminUrl, runtimePassword, caBase64, Client, dryRun }): Promise<ProvisionResult>`
- Produces: `issueLicense({ adminUrl, caBase64, licenseKey, Client }): Promise<'created'|'exists-unbound'|'exists-bound'>`
- CLI: `provision-aiven.js --print-runtime-url` writes the derived runtime URL only to stdout so the deploy script can capture it immediately; normal and dry-run modes never print it.
- CLI env: `CRONOMETRIX_AIVEN_ADMIN_URL`, `CRONOMETRIX_AIVEN_LICENSE_PASSWORD`, `CRONOMETRIX_AIVEN_CA_BASE64`, `CRONOMETRIX_LICENSE_NANOPI_192_168_1_239`.

- [ ] **Step 1: Write failing pure configuration tests**

```js
test('derives percent-encoded runtime URL without admin credentials', () => {
  const value = buildDatabaseUrl(
    'postgres://avnadmin:admin%40secret@db.example:25362/defaultdb?sslmode=require',
    { username: 'cronometrix_license_runtime', password: 'run:/@ secret', database: 'cronometrix_licenses' },
  );
  const parsed = new URL(value);
  assert.equal(parsed.username, 'cronometrix_license_runtime');
  assert.equal(decodeURIComponent(parsed.password), 'run:/@ secret');
  assert.equal(parsed.pathname, '/cronometrix_licenses');
  assert.equal(value.includes('admin%40secret'), false);
});
```

Add fake-client tests that demand this order: role existence checks, owner role,
runtime role creation with server-side `format('%L', $1)`, database existence
check/create, schema/table creation, revokes, grants, and privilege verification.
Assert a second run makes no create calls and does not alter the runtime password.

Add issuer tests for the exact regex `^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$`,
parameterized `INSERT ... ON CONFLICT DO NOTHING`, and refusal to reset a bound
row.

- [ ] **Step 2: Run tool tests and verify RED**

Run:

```bash
cd do-functions
node --test tools/aiven-config.test.js tools/provision-aiven.test.js tools/issue-license.test.js
```

Expected: FAIL because the tool modules do not exist.

- [ ] **Step 3: Implement pure URL construction and provisioner**

Use fixed object names only. Never interpolate a user-supplied identifier.
Generate the password-bearing `CREATE ROLE` statement server-side:

```sql
SELECT format(
  'CREATE ROLE cronometrix_license_runtime LOGIN PASSWORD %L',
  $1
) AS statement
```

Execute the returned statement only when the role is absent. Create the
database outside a transaction. In the target database create
`license_authority.licenses`, revoke PUBLIC privileges, grant runtime
`CONNECT`, schema `USAGE`, and table `SELECT, UPDATE`. Query
`has_database_privilege`, `has_schema_privilege`, and `has_table_privilege` to
fail if effective grants differ.

`--dry-run` must validate inputs and return the fixed object plan without
constructing any client.

- [ ] **Step 4: Implement safe issuer**

Insert with the admin connection:

```sql
INSERT INTO license_authority.licenses (license_key, hardware_fingerprint)
VALUES ($1, NULL)
ON CONFLICT (license_key) DO NOTHING
```

On conflict, query only `hardware_fingerprint`: return `exists-unbound` for
NULL, `exists-bound` otherwise, and exit non-zero for `exists-bound`. Never
print the license value or fingerprint.

- [ ] **Step 5: Run GREEN and expose npm commands**

Add scripts:

```json
{
  "test": "node --test packages/licenses/*.test.js tools/*.test.js",
  "provision:aiven": "node tools/provision-aiven.js",
  "issue:license": "node tools/issue-license.js"
}
```

Run: `cd do-functions && npm test`

Expected: all tests PASS.

- [ ] **Step 6: Commit provisioning tools**

```bash
git add do-functions/tools do-functions/package.json do-functions/package-lock.json
git commit -m "feat(licenses): provision isolated Aiven authority"
```

---

### Task 4: Secret-safe key preparation and trust verification

**Files:**
- Create: `scripts/prepare-license-secrets.sh`
- Create: `scripts/verify-license-keypair.sh`
- Create: `scripts/create-license-secret.sh`
- Create: `scripts/tests/license-secret-tools-test.sh`
- Modify at operator checkpoint: `backend/src/license/pubkey.pem`

**Interfaces:**
- `prepare-license-secrets.sh --ca-file PATH --public-key-out PATH`
- `verify-license-keypair.sh PUBLIC_KEY_PATH`, consuming `CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM`.
- `create-license-secret.sh SECRETCTL_KEY`, storing a cryptographically random uppercase key without printing it.
- Stores: `cronometrix-aiven-license-password`, `cronometrix-aiven-ca-base64`, `cronometrix-license-private-key-pem`.

- [ ] **Step 1: Write failing shell contract tests with fake binaries**

The test creates fake `secretctl` and `openssl` executables in a temporary PATH.
The fake vault consumes stdin, records only key names and SHA-256 hashes, and
fails if secret material appears on stdout/stderr. Assert:

```bash
grep -Fxq 'cronometrix-aiven-license-password' "${RECORDED_KEYS}"
grep -Fxq 'cronometrix-aiven-ca-base64' "${RECORDED_KEYS}"
grep -Fxq 'cronometrix-license-private-key-pem' "${RECORDED_KEYS}"
test -s "${PUBLIC_KEY_OUT}"
! grep -R -- 'PRIVATE KEY' "${CAPTURED_OUTPUT}"
```

Also generate an ephemeral real RSA key and assert
`verify-license-keypair.sh` passes for the matching public key and fails for a
different public key without printing PEM or JWT content.

Invoke `create-license-secret.sh cronometrix-license-test`, make the fake
`secretctl` validate stdin against `^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$`, and assert
the script prints only `license secret stored: cronometrix-license-test`.

- [ ] **Step 2: Run shell tests and verify RED**

Run: `bash scripts/tests/license-secret-tools-test.sh`

Expected: FAIL because the three scripts are missing.

- [ ] **Step 3: Implement one-shot preparation**

Use `umask 077`, `mktemp -d`, and a trap that removes the exact temporary
directory. Generate a 40-character runtime password and RSA-2048 PKCS#8 private
key; pipe values directly to `secretctl set`; copy only the SPKI public PEM to
the requested repository path with mode `0644`. Refuse to overwrite an
existing public key unless `--rotate` is explicitly supplied.

- [ ] **Step 4: Implement keypair verification**

Derive the public key from the injected private key through stdin into a
temporary `0600` file and `cmp` it with the normalized repository public key.
Print only `license keypair verified` on success.

- [ ] **Step 5: Implement secret-only license generation**

Validate the destination vault key against
`^cronometrix-license-[a-z0-9-]+$`. Use Node `crypto.randomInt(36)` to select 16
characters independently from `ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789`, format
them as four groups, and pipe directly to `secretctl set "$1"`. Do not assign
the license to a shell argument, environment variable, file, or output stream.
After `secretctl` succeeds, print only the destination vault key name.

- [ ] **Step 6: Run GREEN and static secret scan**

Run:

```bash
bash scripts/tests/license-secret-tools-test.sh
if git grep -nE -- '-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY-----' -- . ':!*.md'; then exit 1; fi
```

Expected: PASS and `No private keys` behavior.

- [ ] **Step 7: Commit safe tooling before generating production material**

```bash
git add scripts/prepare-license-secrets.sh scripts/verify-license-keypair.sh \
  scripts/create-license-secret.sh scripts/tests/license-secret-tools-test.sh
git commit -m "feat(licenses): add secret-safe key bootstrap"
```

- [ ] **Step 8: Operator checkpoint — generate production key and CA secrets**

The operator runs in their terminal, never through agent stdin:

```bash
bash scripts/prepare-license-secrets.sh \
  --ca-file "$HOME/Downloads/ca.pem" \
  --public-key-out backend/src/license/pubkey.pem \
  --rotate
secretctl run -k cronometrix-license-private-key-pem -- \
  bash scripts/verify-license-keypair.sh backend/src/license/pubkey.pem
```

Expected: `license keypair verified`. No secret is pasted into chat.

- [ ] **Step 9: Validate and commit only the public key**

Run:

```bash
openssl pkey -pubin -in backend/src/license/pubkey.pem -text -noout | grep -F 'Public-Key: (2048 bit)'
git diff --check
git add backend/src/license/pubkey.pem
git commit -m "chore(licenses): trust production signing key"
```

---

### Task 5: Deployment orchestration and live probes

**Files:**
- Create: `scripts/deploy-license-authority.sh`
- Create: `scripts/tests/deploy-license-authority-test.sh`
- Modify: `do-functions/README.md`

**Interfaces:**
- Consumes injected variables from Tasks 3-4.
- Produces non-secret stdout lines `ACTIVATE_URL=https://...` and `RENEW_URL=https://...` only after successful probes.
- Uses namespace label `cronometrix`, region `nyc1`.
- Refuses to deploy unless tracked files are clean and `HEAD == origin/main`; exports that exact commit as `SOURCE_SHA`.

- [ ] **Step 1: Write failing orchestration test with fake `doctl` and `npm`**

The fake commands record arguments and return a namespace list that contains
other labels. Assert the script selects or creates exactly `cronometrix`, runs
`npm run provision:aiven`, deploys `do-functions` with `--remote-build`, fetches
both URLs, and never connects to a partial label match.

Add failure cases for absent injected env variables, malformed HTTPS Function
URLs, a dirty/stale source checkout, and a 200 response to an invalid-body
probe (expected 400).

- [ ] **Step 2: Run orchestration test and verify RED**

Run: `bash scripts/tests/deploy-license-authority-test.sh`

Expected: FAIL because the deploy script is missing.

- [ ] **Step 3: Implement deployment script**

Requirements:

```bash
set -euo pipefail
umask 077
git fetch origin main
test -z "$(git status --porcelain --untracked-files=no)"
SOURCE_SHA="$(git rev-parse HEAD)"
test "${SOURCE_SHA}" = "$(git rev-parse origin/main)"
export SOURCE_SHA
: "${CRONOMETRIX_AIVEN_ADMIN_URL:?required}"
: "${CRONOMETRIX_AIVEN_LICENSE_PASSWORD:?required}"
: "${CRONOMETRIX_AIVEN_CA_BASE64:?required}"
: "${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM:?required}"
export DATABASE_URL="$(node do-functions/tools/provision-aiven.js --print-runtime-url)"
export DATABASE_CA_CERT_BASE64="${CRONOMETRIX_AIVEN_CA_BASE64}"
export LICENSE_PRIVATE_KEY="${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM}"
```

The `--print-runtime-url` stdout must appear only inside command substitution;
the test fails if it reaches captured terminal output. Keep `set -x` disabled
and unset `DATABASE_URL` immediately after `doctl serverless deploy` returns.

Do not use `set -x`, echo env, or write an env file. Match namespace label
exactly from JSON/text output; create with
`doctl serverless namespaces create --label cronometrix --region nyc1` only
when absent, then `doctl serverless connect cronometrix`. Deploy with
`doctl serverless deploy do-functions --remote-build`.

Fetch URLs with `doctl serverless functions get licenses/activate --url` and
renew equivalent. POST `{}` and require HTTP 400 plus `BAD_REQUEST`. Unset all
secret-bearing variables before printing URLs.

- [ ] **Step 4: Run GREEN**

Run: `bash scripts/tests/deploy-license-authority-test.sh`

Expected: PASS with no captured secret values.

- [ ] **Step 5: Document exact secretctl execution**

Document:

```bash
secretctl run \
  -k cronometrix-aiven-admin-url \
  -k cronometrix-aiven-license-password \
  -k cronometrix-aiven-ca-base64 \
  -k cronometrix-license-private-key-pem \
  --timeout=20m -- bash scripts/deploy-license-authority.sh
```

- [ ] **Step 6: Commit deployment orchestration**

```bash
git add scripts/deploy-license-authority.sh \
  scripts/tests/deploy-license-authority-test.sh do-functions/README.md
git commit -m "feat(licenses): orchestrate authority deployment"
```

---

### Task 6: CI and release gates for the license authority

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/tests/release-workflow-test.py`
- Create: `scripts/tests/license-ci-workflow-test.py`
- Modify: `Makefile`

**Interfaces:**
- Produces required check name `License Functions`.
- Produces Make target `test-license-authority`.
- Release promotion requires `License Functions` alongside existing gates.

- [ ] **Step 1: Write failing workflow contract tests**

Assert CI contains a least-privilege `license-functions` job with Node from
`.nvmrc`, `npm ci`, `npm test`, and all three shell contract tests. Extend the
release test required-check tuple with `License Functions`.

- [ ] **Step 2: Run workflow tests and verify RED**

Run:

```bash
python3 scripts/tests/license-ci-workflow-test.py
python3 scripts/tests/release-workflow-test.py
```

Expected: FAIL because the CI job/check and release requirement are absent.

- [ ] **Step 3: Add the CI job and local aggregate target**

The job must use `permissions: contents: read`, cache
`do-functions/package-lock.json`, run `npm ci && npm test`, then:

```bash
bash scripts/tests/license-secret-tools-test.sh
bash scripts/tests/deploy-license-authority-test.sh
```

Add `test-license-authority` with the identical local commands. Add
`License Functions` to release promotion required checks.

- [ ] **Step 4: Run GREEN and config validation**

Run:

```bash
make test-license-authority
python3 scripts/tests/license-ci-workflow-test.py
python3 scripts/tests/release-workflow-test.py
make test-ci-config
```

Expected: all PASS.

- [ ] **Step 5: Commit CI gates**

```bash
git add .github/workflows/ci.yml .github/workflows/release.yml Makefile \
  scripts/tests/license-ci-workflow-test.py scripts/tests/release-workflow-test.py
git commit -m "ci: gate Aiven license authority"
```

---

### Task 7: Full verification, PR, release, authority deployment, and NanoPi install

**Files:**
- Modify through generated public material: `backend/src/license/pubkey.pem`
- Update after live validation: `do-functions/README.md` if commands differ from actual provider output.

**Interfaces:**
- Consumes all tested scripts and secret names from Tasks 1-6.
- Produces merged `main`, successful multi-arch release, live Function URLs, one seeded license, and healthy NanoPi services.

- [ ] **Step 1: Run local verification before PR**

Run:

```bash
make test-license-authority
cd do-functions && npm test
cd ../backend && cargo nextest run --all-features \
  --test license_tests --test license_service_extra_test
cd .. && make container-smoke
bash deploy/tests/install-static-test.sh
git diff --check
git status --short
```

Expected: all tests PASS; only planned files changed; no generated private or
environment files present.

- [ ] **Step 2: Run secret scan**

Run:

```bash
if git grep -nE -- '-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY-----' -- . ':!*.md'; then exit 1; fi
git grep -nE 'postgres(ql)?://[^[:space:]]+:[^[:space:]]+@' -- . ':!*.md' && exit 1 || true
```

Expected: no committed private key or credential-bearing PostgreSQL URL.

- [ ] **Step 3: Push PR and wait for all gates**

```bash
git push -u origin feat/aiven-license-authority
gh pr create --base main --head feat/aiven-license-authority \
  --title "feat: deploy Aiven-backed license authority" \
  --body-file /tmp/cronometrix-license-pr-body.md
gh pr checks --watch
```

Required green checks: Backend Coverage, Frontend Coverage, E2E Tests,
Container Smoke, Secret Scan, and License Functions. Fix root causes and repeat
until green.

- [ ] **Step 4: Merge and verify `main`**

```bash
gh pr merge --squash --delete-branch
git fetch origin main
git rev-parse origin/main
gh run list --branch main --limit 5
```

Expected: PR merged, the merge commit is on `origin/main`, and main CI succeeds.

- [ ] **Step 5: Require the new check on `main` without removing existing gates**

Read the current protection first and require the exact expected set:

```bash
gh api repos/gerswin/cronometrix/branches/main/protection/required_status_checks \
  > /tmp/cronometrix-required-checks.json
jq -e '.strict == true' /tmp/cronometrix-required-checks.json
jq -e '.contexts | sort == (["Backend Coverage", "Container Smoke", "E2E Tests", "Frontend Coverage", "Secret Scan"] | sort)' \
  /tmp/cronometrix-required-checks.json
gh api --method POST \
  repos/gerswin/cronometrix/branches/main/protection/required_status_checks/contexts \
  -f contexts[]='License Functions'
gh api repos/gerswin/cronometrix/branches/main/protection/required_status_checks \
  --jq '.contexts | sort | join("\n")'
```

Expected: the original five contexts plus `License Functions`, with strict
status checking still enabled.

- [ ] **Step 6: Publish and verify the immutable multi-arch release**

Push `codex/release-build-<MAIN_SHA>`, wait for Release, download the private
bundle, verify outer and inner SHA-256 checksums, assert `SOURCE_SHA=<MAIN_SHA>`,
and inspect API/web/gateway/cloudflared manifests for `linux/arm64` and
`linux/amd64` exactly as locked by the release workflow.

- [ ] **Step 7: Operator checkpoint — deploy authority through secretctl**

Run in the operator terminal:

```bash
secretctl run \
  -k cronometrix-aiven-admin-url \
  -k cronometrix-aiven-license-password \
  -k cronometrix-aiven-ca-base64 \
  -k cronometrix-license-private-key-pem \
  --timeout=20m -- bash scripts/deploy-license-authority.sh
```

Expected: provision/grant probes PASS; both HTTPS URLs are printed; invalid
requests return 400.

- [ ] **Step 8: Generate, store, and issue the NanoPi license**

The operator generates the formatted value without showing it to the agent:

```bash
bash scripts/create-license-secret.sh cronometrix-license-nanopi-192-168-1-239
secretctl run \
  -k cronometrix-aiven-admin-url \
  -k cronometrix-aiven-ca-base64 \
  -k cronometrix-license-nanopi-192-168-1-239 \
  --timeout=5m -- npm --prefix do-functions run issue:license
```

Expected: `license created` or `license already exists and is unbound`; never
`exists-bound` on first install.

- [ ] **Step 9: Install the verified release on the NanoPi**

Copy the new verified bundle to a new `/home/pi/cronometrix-release-<SHA>`
directory and verify `SHA256SUMS` remotely. The operator runs the installer in
their own terminal, retrieving the license with `secretctl get` locally and
entering secrets only into the SSH TTY. Set the printed activation/renewal URLs;
do not set test flags.

- [ ] **Step 10: Verify production health and persistence**

From the agent, with no secret output:

```bash
ssh pi@192.168.1.239 'cd /opt/cronometrix && docker compose ps'
ssh pi@192.168.1.239 'curl -fsS http://127.0.0.1:8080/gateway-health'
ssh pi@192.168.1.239 'curl -fsS http://127.0.0.1:8080/api/v1/health'
ssh pi@192.168.1.239 'curl -fsS http://127.0.0.1:8080/api/v1/setup/status | jq -e ".licensed == true"'
ssh pi@192.168.1.239 'test "$(stat -c %a /opt/cronometrix/data/license.jwt)" = 600'
```

Restart Docker Compose, repeat health/status checks, inspect redacted logs, and
verify the Cloudflare hostname. Ship is complete only when `main`, release,
Functions, license activation, restart persistence, and tunnel are all green.

- [ ] **Step 11: Record live validation**

Update the README with only non-secret namespace, Function route names, release
SHA, and validation timestamps. Commit via a follow-up PR if documentation
changed; never record the license, URLs containing auth, database host, or
fingerprint.
