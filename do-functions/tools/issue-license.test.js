'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { issueLicense } = require('./issue-license');

const CA_BASE64 = Buffer.from(
  '-----BEGIN CERTIFICATE-----\nZmFrZQ==\n-----END CERTIFICATE-----\n',
).toString('base64');
const ADMIN_URL = 'postgres://avnadmin:admin-secret@db.example:25362/defaultdb?sslmode=require';
const LICENSE_KEY = 'ABCD-EF12-3456-WXYZ';

function issuerClient({ inserted, fingerprint = null }) {
  const calls = [];
  class FakeClient {
    constructor(config) { calls.push(['config', config]); }
    async connect() { calls.push(['connect']); }
    async end() { calls.push(['end']); }
    async query(statement, values) {
      const compact = statement.replace(/\s+/g, ' ').trim();
      calls.push(['query', compact, values]);
      if (compact.startsWith('INSERT')) return { rowCount: inserted ? 1 : 0 };
      return { rows: [{ hardware_fingerprint: fingerprint }] };
    }
  }
  return { FakeClient, calls };
}

for (const invalid of ['', 'abcd-EF12-3456-WXYZ', 'ABCDE-F123-4567-WXYZ', 'ABCD-EF12-3456']) {
  test(`rejects malformed license key: ${invalid || '<empty>'}`, async () => {
    class ForbiddenClient { constructor() { throw new Error('must not connect'); } }
    await assert.rejects(
      issueLicense({
        adminUrl: ADMIN_URL,
        caBase64: CA_BASE64,
        licenseKey: invalid,
        Client: ForbiddenClient,
      }),
      /license key format is invalid/,
    );
  });
}

test('creates an unbound license with a parameterized idempotent insert', async () => {
  const { FakeClient, calls } = issuerClient({ inserted: true });
  assert.equal(await issueLicense({
    adminUrl: ADMIN_URL,
    caBase64: CA_BASE64,
    licenseKey: LICENSE_KEY,
    Client: FakeClient,
  }), 'created');

  const insert = calls
    .filter(([kind]) => kind === 'query')
    .find(([, statement]) => statement.startsWith('INSERT'));
  assert.match(insert[1], /INSERT INTO license_authority\.licenses/);
  assert.match(insert[1], /ON CONFLICT \(license_key\) DO NOTHING/);
  assert.deepEqual(insert[2], [LICENSE_KEY]);
  assert.equal(insert[1].includes(LICENSE_KEY), false);
});

test('reports an existing unbound license without changing it', async () => {
  const { FakeClient } = issuerClient({ inserted: false, fingerprint: null });
  assert.equal(await issueLicense({
    adminUrl: ADMIN_URL,
    caBase64: CA_BASE64,
    licenseKey: LICENSE_KEY,
    Client: FakeClient,
  }), 'exists-unbound');
});

test('refuses to reset a bound license', async () => {
  const { FakeClient, calls } = issuerClient({ inserted: false, fingerprint: 'FP-SECRET' });
  assert.equal(await issueLicense({
    adminUrl: ADMIN_URL,
    caBase64: CA_BASE64,
    licenseKey: LICENSE_KEY,
    Client: FakeClient,
  }), 'exists-bound');
  const sql = calls.filter(([kind]) => kind === 'query').map(([, statement]) => statement).join('\n');
  assert.doesNotMatch(sql, /SET hardware_fingerprint|DELETE/);
  assert.equal(sql.includes('FP-SECRET'), false);
});
