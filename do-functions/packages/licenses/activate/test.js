// Run with: node --test do-functions/packages/licenses/activate/test.js
// Or:        cd do-functions && node --test packages/licenses/activate/test.js
//
// Uses the in-memory shared-store via process.env.TEST_STORE so no Postgres
// is required for unit tests. The RSA test keypair is copied byte-for-byte
// from backend/tests/fixtures/ in Plan 01, so JWTs signed here verify with
// the same public key the Rust backend uses (end-to-end determinism).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const jwt = require('jsonwebtoken');

process.env.TEST_STORE = '1';
// __dirname = do-functions/packages/licenses/activate
// 3 levels up = do-functions/, then into test-keys/
process.env.LICENSE_PRIVATE_KEY = fs.readFileSync(
    path.join(__dirname, '../../../test-keys/test_priv.pem'),
    'utf8',
);
const TEST_PUBKEY = fs.readFileSync(
    path.join(__dirname, '../../../test-keys/test_pub.pem'),
    'utf8',
);

const handler = require('./index.js').main;
const store = require('../shared-store');

test.beforeEach(() => store.__reset());

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

test('binds fingerprint on first activation (state mutation)', async () => {
    store.__seedRow('TEST-1234-5678-9012');
    assert.strictEqual(await store.lookup('TEST-1234-5678-9012'), null);
    await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(await store.lookup('TEST-1234-5678-9012'), 'FP-A');
});
