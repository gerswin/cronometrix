'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  LICENSES_TABLE,
  normalizeDatabaseUrl,
  decodeCa,
  buildPgConfig,
  createPgStore,
} = require('./pg-store');

const CA_PEM = [
  '-----BEGIN CERTIFICATE-----',
  'ZmFrZQ==',
  '-----END CERTIFICATE-----',
  '',
].join('\n');
const CA_BASE64 = Buffer.from(CA_PEM).toString('base64');

test('normalizes libpq SSL options before applying verified CA config', () => {
  const normalized = normalizeDatabaseUrl(
    'postgres://user:p%40ss@db.example:25362/licenses?sslmode=require&sslcert=x&sslkey=y&sslrootcert=z&application_name=cronometrix',
  );
  const url = new URL(normalized);

  for (const option of ['sslmode', 'sslcert', 'sslkey', 'sslrootcert']) {
    assert.equal(url.searchParams.has(option), false);
  }
  assert.equal(url.searchParams.get('application_name'), 'cronometrix');
  assert.equal(url.password, 'p%40ss');
});

for (const rawUrl of ['', 'https://db.example/licenses', 'not-a-url']) {
  test(`rejects invalid PostgreSQL URL: ${rawUrl || '<empty>'}`, () => {
    assert.throws(
      () => normalizeDatabaseUrl(rawUrl),
      /DATABASE_URL is missing or invalid/,
    );
  });
}

test('decodes an Aiven CA certificate from strict base64', () => {
  assert.equal(decodeCa(CA_BASE64), CA_PEM);
});

for (const encoded of ['', '!', Buffer.from('not a certificate').toString('base64')]) {
  test('rejects missing, malformed, or non-certificate CA values', () => {
    assert.throws(
      () => decodeCa(encoded),
      /DATABASE_CA_CERT_BASE64 is missing or invalid/,
    );
  });
}

test('builds fail-closed TLS config with bounded timeouts', () => {
  const config = buildPgConfig({
    DATABASE_URL: 'postgres://user:pass@db.example:25362/licenses?sslmode=require',
    DATABASE_CA_CERT_BASE64: CA_BASE64,
  });

  assert.equal(new URL(config.connectionString).searchParams.has('sslmode'), false);
  assert.deepEqual(config.ssl, {
    ca: CA_PEM,
    rejectUnauthorized: true,
  });
  assert.equal(config.connectionTimeoutMillis, 5000);
  assert.equal(config.query_timeout, 5000);
});

for (const env of [
  { DATABASE_CA_CERT_BASE64: CA_BASE64 },
  { DATABASE_URL: 'postgres://user:do-not-leak@db.example/licenses' },
  {
    DATABASE_URL: 'postgres://user:do-not-leak@db.example/licenses',
    DATABASE_CA_CERT_BASE64: '!',
  },
]) {
  test('configuration errors never expose credential values', () => {
    assert.throws(() => buildPgConfig(env), (error) => {
      assert.equal(error.message.includes('do-not-leak'), false);
      return true;
    });
  });
}

test('uses qualified parameterized SQL and preserves guarded binding', async () => {
  const calls = [];
  class FakeClient {
    constructor(config) {
      calls.push(['config', config]);
    }

    async connect() {
      calls.push(['connect']);
    }

    async end() {
      calls.push(['end']);
    }

    async query(statement, values) {
      calls.push(['query', statement, values]);
      if (statement.startsWith('SELECT')) {
        return { rows: [{ hardware_fingerprint: null }] };
      }
      return { rowCount: 1 };
    }
  }

  const store = createPgStore({
    env: {
      DATABASE_URL: 'postgres://user:pass@db.example/licenses',
      DATABASE_CA_CERT_BASE64: CA_BASE64,
    },
    Client: FakeClient,
  });

  assert.equal(await store.lookup('KEY1-KEY2-KEY3-KEY4'), null);
  assert.equal(await store.bind('KEY1-KEY2-KEY3-KEY4', 'FP-A', 123), true);
  await store.touch('KEY1-KEY2-KEY3-KEY4', 456);

  const queries = calls.filter(([kind]) => kind === 'query');
  const sql = queries.map(([, statement]) => statement).join('\n');
  assert.match(sql, new RegExp(LICENSES_TABLE.replace('.', '\\.')));
  assert.match(sql, /hardware_fingerprint IS NULL OR hardware_fingerprint = \$1/);
  assert.equal(sql.includes('KEY1-KEY2-KEY3-KEY4'), false);
  assert.deepEqual(queries.map(([, , values]) => values), [
    ['KEY1-KEY2-KEY3-KEY4'],
    ['FP-A', 123, 'KEY1-KEY2-KEY3-KEY4'],
    [456, 'KEY1-KEY2-KEY3-KEY4'],
  ]);
  assert.equal(calls.filter(([kind]) => kind === 'connect').length, 3);
  assert.equal(calls.filter(([kind]) => kind === 'end').length, 3);
});

test('returns undefined for unknown licenses and false for rejected binds', async () => {
  class EmptyClient {
    async connect() {}
    async end() {}
    async query(statement) {
      return statement.startsWith('SELECT') ? { rows: [] } : { rowCount: 0 };
    }
  }

  const store = createPgStore({
    env: {
      DATABASE_URL: 'postgres://user:pass@db.example/licenses',
      DATABASE_CA_CERT_BASE64: CA_BASE64,
    },
    Client: EmptyClient,
  });

  assert.equal(await store.lookup('UNKNOWN'), undefined);
  assert.equal(await store.bind('UNKNOWN', 'FP-A', 123), false);
});

test('always closes the PostgreSQL client when a query fails', async () => {
  let ended = false;
  class FailingClient {
    async connect() {}
    async end() { ended = true; }
    async query() { throw new Error('query failed'); }
  }

  const store = createPgStore({
    env: {
      DATABASE_URL: 'postgres://user:pass@db.example/licenses',
      DATABASE_CA_CERT_BASE64: CA_BASE64,
    },
    Client: FailingClient,
  });

  await assert.rejects(store.lookup('KEY'), /query failed/);
  assert.equal(ended, true);
});
