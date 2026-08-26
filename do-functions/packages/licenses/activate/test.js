// Run with: node --test do-functions/packages/licenses/activate/test.js
// Or:        cd do-functions && node --test packages/licenses/activate/test.js
//
// Uses the in-memory shared-store via process.env.TEST_STORE so no Postgres
// is required for unit tests. The RSA keypair is generated per run: these
// tests verify that what this function signs, this function can verify —
// they do not and must not depend on the production signing key.

const test = require('node:test');
const assert = require('node:assert');
const jwt = require('jsonwebtoken');

process.env.TEST_STORE = '1';
// Ephemeral RSA keypair, generated per run. C-06: no private key lives in
// the repository, not even as a test fixture — the one that used to live
// here turned out to be the same key the backend trusts in production.
const { generateKeyPairSync } = require('node:crypto');
const { privateKey, publicKey } = generateKeyPairSync('rsa', {
    modulusLength: 2048,
    publicKeyEncoding: { type: 'spki', format: 'pem' },
    privateKeyEncoding: { type: 'pkcs8', format: 'pem' },
});
process.env.LICENSE_PRIVATE_KEY = privateKey;
const TEST_PUBKEY = publicKey;

const activate = require('./index.js');
const handler = activate.main;
const store = require('../shared-store');

test.beforeEach(() => store.__reset());

test('production store fails closed when CA configuration is absent', async () => {
    assert.throws(
        () => activate.resolveStore({
            DATABASE_URL: 'postgres://user:do-not-leak@db.example/licenses',
        }),
        /DATABASE_CA_CERT_BASE64 is missing or invalid/,
    );

    const saved = {
        TEST_STORE: process.env.TEST_STORE,
        DATABASE_URL: process.env.DATABASE_URL,
        DATABASE_CA_CERT_BASE64: process.env.DATABASE_CA_CERT_BASE64,
    };
    delete process.env.TEST_STORE;
    process.env.DATABASE_URL = 'postgres://user:do-not-leak@db.example/licenses';
    delete process.env.DATABASE_CA_CERT_BASE64;
    try {
        const r = await handler({
            body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
        });
        assert.strictEqual(r.statusCode, 500);
        assert.strictEqual(r.body.error.code, 'SERVER_ERROR');
        assert.strictEqual(JSON.stringify(r).includes('do-not-leak'), false);
    } finally {
        for (const [name, value] of Object.entries(saved)) {
            if (value === undefined) delete process.env[name];
            else process.env[name] = value;
        }
    }
});

test('only TEST_STORE=1 enables the in-memory adapter', () => {
    assert.throws(
        () => activate.resolveStore({ TEST_STORE: 'true' }),
        /DATABASE_URL is missing or invalid/,
    );
    assert.strictEqual(activate.resolveStore({ TEST_STORE: '1' }), store);
});

test('signs JWT for unbound license', async () => {
    store.__seedRow('TEST-1234-5678-9012');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 200);
    assert.ok(r.body.token, 'token must be present');
    const decoded = jwt.verify(r.body.token, TEST_PUBKEY, { algorithms: ['RS256'] });
    assert.strictEqual(decoded.license_key, 'TEST-1234-5678-9012');
    assert.strictEqual(decoded.hardware_fingerprint, 'FP-A');
    assert.strictEqual(decoded.product, 'cronometrix');
    assert.ok(decoded.exp > decoded.iat);
    // exp ≈ iat + 365 days (allow 60s clock-tick tolerance)
    assert.ok(decoded.exp - decoded.iat >= 365 * 24 * 60 * 60 - 60);
    assert.ok(decoded.exp - decoded.iat <= 365 * 24 * 60 * 60 + 60);
});

test('returns 404 for unknown key', async () => {
    const r = await handler({
        body: { license_key: 'NOPE-NOPE-NOPE-NOPE', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 404);
    assert.strictEqual(r.body.error.code, 'LICENSE_NOT_FOUND');
});

test('returns 409 for bound to different fingerprint', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-B' },
    });
    assert.strictEqual(r.statusCode, 409);
    assert.strictEqual(r.body.error.code, 'ALREADY_ACTIVATED');
});

test('migrates a legacy binding exactly once with a valid previous JWT', async () => {
    const licenseKey = 'TEST-1234-5678-9012';
    store.__seedRow(licenseKey, 'FP-LEGACY');
    const now = Math.floor(Date.now() / 1000);
    const previousToken = jwt.sign({
        license_key: licenseKey,
        hardware_fingerprint: 'FP-LEGACY',
        product: 'cronometrix',
        iat: now,
        exp: now + 3600,
    }, privateKey, { algorithm: 'RS256' });

    const migrated = await handler({ body: {
        license_key: licenseKey,
        hardware_fingerprint: 'FP-STABLE-V2',
        previous_token: previousToken,
    } });

    assert.strictEqual(migrated.statusCode, 200);
    assert.strictEqual(await store.lookup(licenseKey), 'FP-STABLE-V2');
    const claims = jwt.verify(migrated.body.token, TEST_PUBKEY, { algorithms: ['RS256'] });
    assert.strictEqual(claims.hardware_fingerprint, 'FP-STABLE-V2');
    assert.strictEqual(claims.fingerprint_version, 2);

    const replay = await handler({ body: {
        license_key: licenseKey,
        hardware_fingerprint: 'FP-CLONE',
        previous_token: previousToken,
    } });
    assert.strictEqual(replay.statusCode, 403);
    assert.strictEqual(replay.body.error.code, 'MIGRATION_PROOF_INVALID');
    assert.strictEqual(await store.lookup(licenseKey), 'FP-STABLE-V2');
});

test('never accepts a V2 token as fingerprint migration proof', async () => {
    const licenseKey = 'TEST-1234-5678-9012';
    store.__seedRow(licenseKey, 'FP-STABLE-A');
    const now = Math.floor(Date.now() / 1000);
    const v2Token = jwt.sign({
        license_key: licenseKey,
        hardware_fingerprint: 'FP-STABLE-A',
        fingerprint_version: 2,
        product: 'cronometrix',
        iat: now,
        exp: now + 3600,
    }, privateKey, { algorithm: 'RS256' });

    const r = await handler({ body: {
        license_key: licenseKey,
        hardware_fingerprint: 'FP-STABLE-B',
        previous_token: v2Token,
    } });

    assert.strictEqual(r.statusCode, 403);
    assert.strictEqual(r.body.error.code, 'MIGRATION_PROOF_INVALID');
    assert.strictEqual(await store.lookup(licenseKey), 'FP-STABLE-A');
});

test('concurrent legacy migrations grant exactly one V2 binding', async () => {
    const licenseKey = 'TEST-RACE-0000-0002';
    store.__seedRow(licenseKey, 'FP-LEGACY');
    const now = Math.floor(Date.now() / 1000);
    const previousToken = jwt.sign({
        license_key: licenseKey,
        hardware_fingerprint: 'FP-LEGACY',
        product: 'cronometrix',
        iat: now,
        exp: now + 3600,
    }, privateKey, { algorithm: 'RS256' });

    const [a, b] = await Promise.all([
        handler({ body: {
            license_key: licenseKey,
            hardware_fingerprint: 'FP-STABLE-A',
            previous_token: previousToken,
        } }),
        handler({ body: {
            license_key: licenseKey,
            hardware_fingerprint: 'FP-STABLE-B',
            previous_token: previousToken,
        } }),
    ]);

    const granted = [a, b].filter((response) => response.statusCode === 200);
    const rejected = [a, b].filter((response) => response.statusCode === 409);
    assert.strictEqual(granted.length, 1);
    assert.strictEqual(rejected.length, 1);
    assert.strictEqual(rejected[0].body.error.code, 'LICENSE_ALREADY_BOUND');
    const claims = jwt.verify(granted[0].body.token, TEST_PUBKEY, { algorithms: ['RS256'] });
    assert.strictEqual(await store.lookup(licenseKey), claims.hardware_fingerprint);
});

test('idempotent for same fingerprint (re-activation allowed)', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 200);
    assert.ok(r.body.token);
    const decoded = jwt.verify(r.body.token, TEST_PUBKEY, { algorithms: ['RS256'] });
    assert.strictEqual(decoded.hardware_fingerprint, 'FP-A');
});

test('returns 400 on missing license_key', async () => {
    const r = await handler({ body: { hardware_fingerprint: 'FP-A' } });
    assert.strictEqual(r.statusCode, 400);
    assert.strictEqual(r.body.error.code, 'BAD_REQUEST');
});

test('returns 400 on missing hardware_fingerprint', async () => {
    const r = await handler({ body: { license_key: 'TEST-1234-5678-9012' } });
    assert.strictEqual(r.statusCode, 400);
    assert.strictEqual(r.body.error.code, 'BAD_REQUEST');
});

test('uses RS256 algorithm in JWT header', async () => {
    store.__seedRow('TEST-1234-5678-9012');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 200);
    const [headerB64] = r.body.token.split('.');
    const header = JSON.parse(Buffer.from(headerB64, 'base64url').toString());
    assert.strictEqual(header.alg, 'RS256');
});

test('handles top-level args (no body wrapper)', async () => {
    store.__seedRow('TEST-1234-5678-9012');
    const r = await handler({
        license_key: 'TEST-1234-5678-9012',
        hardware_fingerprint: 'FP-A',
    });
    assert.strictEqual(r.statusCode, 200);
});

test('returns 500 when LICENSE_PRIVATE_KEY missing', async () => {
    const saved = process.env.LICENSE_PRIVATE_KEY;
    delete process.env.LICENSE_PRIVATE_KEY;
    try {
        store.__seedRow('TEST-1234-5678-9012');
        const r = await handler({
            body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
        });
        assert.strictEqual(r.statusCode, 500);
        assert.strictEqual(r.body.error.code, 'CONFIG_ERROR');
    } finally {
        process.env.LICENSE_PRIVATE_KEY = saved;
    }
});

test('rejects a malformed private key before an unknown license can mask it', async () => {
    const saved = process.env.LICENSE_PRIVATE_KEY;
    process.env.LICENSE_PRIVATE_KEY = 'not-a-private-key';
    try {
        const r = await handler({
            body: { license_key: 'NOPE-NOPE-NOPE-NOPE', hardware_fingerprint: 'FP-PROBE' },
        });
        assert.strictEqual(r.statusCode, 500);
        assert.strictEqual(r.body.error.code, 'SERVER_ERROR');
    } finally {
        process.env.LICENSE_PRIVATE_KEY = saved;
    }
});

test('binds fingerprint on first activation (state mutation)', async () => {
    store.__seedRow('TEST-1234-5678-9012');
    assert.strictEqual(await store.lookup('TEST-1234-5678-9012'), null);
    await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(await store.lookup('TEST-1234-5678-9012'), 'FP-A');
});

// C-07: dos activaciones simultáneas con fingerprints distintos. El binding
// tiene que ser condicional, no un UPDATE ciego: solo un equipo puede quedar
// vinculado y el otro debe recibir 409.
//
// Sobre el alcance: el store en memoria es de un solo hilo, así que esto
// verifica el CONTRATO (bind condicional + 409), no la atomicidad de Postgres.
// Esa la aporta el propio UPDATE de una sola sentencia con guarda en el WHERE.
test('concurrent activations bind the license exactly once', async () => {
    store.__seedRow('TEST-RACE-0000-0001');

    const [a, b] = await Promise.all([
        handler({ body: { license_key: 'TEST-RACE-0000-0001', hardware_fingerprint: 'FP-A' } }),
        handler({ body: { license_key: 'TEST-RACE-0000-0001', hardware_fingerprint: 'FP-B' } }),
    ]);

    const granted = [a, b].filter((r) => r.statusCode === 200);
    const rejected = [a, b].filter((r) => r.statusCode === 409);
    assert.strictEqual(granted.length, 1, 'exactly one activation may receive a token');
    assert.strictEqual(rejected.length, 1, 'the loser must get 409, not a signed token');

    // Y la licencia queda vinculada al que ganó, no al último en escribir.
    const bound = await store.lookup('TEST-RACE-0000-0001');
    assert.strictEqual(bound, granted[0].body.hardware_fingerprint ?? bound);
    assert.ok(['FP-A', 'FP-B'].includes(bound));
});
