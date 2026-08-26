// Run with: node --test do-functions/packages/licenses/renew/test.js
//
// Renewal-side defense in depth: even with a valid JWT in hand, if the
// requesting fingerprint no longer matches the bound fingerprint we refuse
// to issue a fresh token. Mirrors Plan 01's Rust LIC-05 anti-cloning check.

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

const renew = require('./index.js');
const handler = renew.main;
const store = require('../shared-store');

test.beforeEach(() => store.__reset());

test('production store fails closed when CA configuration is absent', async () => {
    assert.throws(
        () => renew.resolveStore({
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
        () => renew.resolveStore({ TEST_STORE: 'true' }),
        /DATABASE_URL is missing or invalid/,
    );
    assert.strictEqual(renew.resolveStore({ TEST_STORE: '1' }), store);
});

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
