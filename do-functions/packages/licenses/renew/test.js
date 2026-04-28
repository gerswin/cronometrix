// Run with: node --test do-functions/packages/licenses/renew/test.js
//
// Renewal-side defense in depth: even with a valid JWT in hand, if the
// requesting fingerprint no longer matches the bound fingerprint we refuse
// to issue a fresh token. Mirrors Plan 01's Rust LIC-05 anti-cloning check.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const jwt = require('jsonwebtoken');

process.env.TEST_STORE = '1';
// __dirname = do-functions/packages/licenses/renew
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

test('signs new jwt for matched fingerprint', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 200);
    assert.ok(r.body.token);
    const decoded = jwt.verify(r.body.token, TEST_PUBKEY, { algorithms: ['RS256'] });
    assert.strictEqual(decoded.license_key, 'TEST-1234-5678-9012');
    assert.strictEqual(decoded.hardware_fingerprint, 'FP-A');
    assert.strictEqual(decoded.product, 'cronometrix');
    assert.ok(decoded.exp - decoded.iat >= 365 * 24 * 60 * 60 - 60);
});

test('returns 403 on fingerprint mismatch (anti-cloning)', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-B' },
    });
    assert.strictEqual(r.statusCode, 403);
    assert.strictEqual(r.body.error.code, 'HARDWARE_MISMATCH');
});

test('returns 404 for unknown key', async () => {
    const r = await handler({
        body: { license_key: 'GHOST', hardware_fingerprint: 'X' },
    });
    assert.strictEqual(r.statusCode, 404);
    assert.strictEqual(r.body.error.code, 'LICENSE_NOT_FOUND');
});

test('returns 400 on missing fields', async () => {
    const r = await handler({ body: {} });
    assert.strictEqual(r.statusCode, 400);
    assert.strictEqual(r.body.error.code, 'BAD_REQUEST');
});

test('uses RS256 algorithm in JWT header', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 200);
    const [headerB64] = r.body.token.split('.');
    const header = JSON.parse(Buffer.from(headerB64, 'base64url').toString());
    assert.strictEqual(header.alg, 'RS256');
});

test('returns 403 on unbound license (renew should not bind)', async () => {
    // Edge case: license seeded but never activated. Renew must NOT
    // act as a back-door activation — it requires existing fingerprint.
    store.__seedRow('TEST-1234-5678-9012'); // fp=null
    const r = await handler({
        body: { license_key: 'TEST-1234-5678-9012', hardware_fingerprint: 'FP-A' },
    });
    assert.strictEqual(r.statusCode, 403);
    assert.strictEqual(r.body.error.code, 'HARDWARE_MISMATCH');
});

test('handles top-level args (no body wrapper)', async () => {
    store.__seedRow('TEST-1234-5678-9012', 'FP-A');
    const r = await handler({
        license_key: 'TEST-1234-5678-9012',
        hardware_fingerprint: 'FP-A',
    });
    assert.strictEqual(r.statusCode, 200);
});
