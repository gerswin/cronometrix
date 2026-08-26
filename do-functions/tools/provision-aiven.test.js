'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  DATABASE_NAME,
  OWNER_ROLE,
  RUNTIME_ROLE,
  provision,
} = require('./provision-aiven');

const CA_PEM = '-----BEGIN CERTIFICATE-----\nZmFrZQ==\n-----END CERTIFICATE-----\n';
const CA_BASE64 = Buffer.from(CA_PEM).toString('base64');
const ADMIN_URL = 'postgres://avnadmin:admin-secret@db.example:25362/defaultdb?sslmode=require';

function fakeClient({ existing = false, privilegeRow } = {}) {
  const calls = [];
  class FakeClient {
    constructor(config) {
      this.config = config;
      calls.push(['construct', new URL(config.connectionString).pathname]);
    }
    async connect() { calls.push(['connect']); }
    async end() { calls.push(['end']); }
    async query(statement, values = []) {
      const compact = statement.replace(/\s+/g, ' ').trim();
      calls.push(['query', compact, values]);
      if (compact.includes('FROM pg_roles')) {
        return { rows: existing ? [{ exists: 1 }] : [] };
      }
      if (compact.startsWith('SELECT format(')) {
        return { rows: [{ statement: `CREATE ROLE ${RUNTIME_ROLE} LOGIN PASSWORD 'server-escaped'` }] };
      }
      if (compact.includes('FROM pg_database')) {
        return { rows: existing ? [{ exists: 1 }] : [] };
      }
      if (compact.includes('has_database_privilege')) {
        return {
          rows: [privilegeRow || {
            can_connect: true,
            can_use_schema: true,
            can_select: true,
            can_update: true,
          }],
        };
      }
      return { rows: [], rowCount: 1 };
    }
  }
  return { FakeClient, calls };
}

test('creates roles, database, schema and least-privilege grants in order', async () => {
  const { FakeClient, calls } = fakeClient();
  const result = await provision({
    adminUrl: ADMIN_URL,
    runtimePassword: 'runtime-secret',
    caBase64: CA_BASE64,
    Client: FakeClient,
  });

  assert.equal(result.database, DATABASE_NAME);
  assert.equal(result.runtimeRole, RUNTIME_ROLE);
  const sql = calls.filter(([kind]) => kind === 'query').map(([, statement]) => statement);
  const at = (fragment) => sql.findIndex((statement) => statement.includes(fragment));
  const runtimeCreate = sql.findIndex((statement) => (
    statement.startsWith(`CREATE ROLE ${RUNTIME_ROLE} LOGIN PASSWORD`)
  ));
  assert.ok(at(`CREATE ROLE ${OWNER_ROLE} NOLOGIN`) > at('FROM pg_roles'));
  assert.ok(at('SELECT format(') < runtimeCreate);
  assert.ok(at(`CREATE DATABASE ${DATABASE_NAME} OWNER ${OWNER_ROLE}`) > at('FROM pg_database'));
  assert.ok(at(`CREATE SCHEMA IF NOT EXISTS license_authority AUTHORIZATION ${OWNER_ROLE}`) > at(`CREATE DATABASE ${DATABASE_NAME}`));
  assert.ok(at('REVOKE ALL ON DATABASE') < at('GRANT CONNECT ON DATABASE'));
  assert.ok(at('GRANT SELECT, UPDATE ON TABLE license_authority.licenses') < at('has_database_privilege'));

  const formatCall = calls
    .filter(([kind]) => kind === 'query')
    .find(([, statement]) => statement.startsWith('SELECT format('));
  assert.deepEqual(formatCall[2], ['runtime-secret']);
  assert.equal(sql.join('\n').includes('runtime-secret'), false);
});

test('second run is idempotent and never alters the runtime password', async () => {
  const { FakeClient, calls } = fakeClient({ existing: true });
  await provision({
    adminUrl: ADMIN_URL,
    runtimePassword: 'new-password-must-not-apply',
    caBase64: CA_BASE64,
    Client: FakeClient,
  });

  const sql = calls.filter(([kind]) => kind === 'query').map(([, statement]) => statement).join('\n');
  assert.doesNotMatch(sql, /CREATE ROLE|ALTER ROLE|CREATE DATABASE|SELECT format/);
  assert.equal(sql.includes('new-password-must-not-apply'), false);
});

test('dry-run validates inputs without constructing a database client', async () => {
  class ForbiddenClient {
    constructor() { throw new Error('client must not be constructed'); }
  }
  const result = await provision({
    adminUrl: ADMIN_URL,
    runtimePassword: 'runtime-secret',
    caBase64: CA_BASE64,
    Client: ForbiddenClient,
    dryRun: true,
  });
  assert.deepEqual(result, {
    database: DATABASE_NAME,
    ownerRole: OWNER_ROLE,
    runtimeRole: RUNTIME_ROLE,
    schema: 'license_authority',
    table: 'licenses',
    dryRun: true,
  });
});

test('fails closed when effective runtime privileges are incomplete', async () => {
  const { FakeClient } = fakeClient({
    existing: true,
    privilegeRow: {
      can_connect: true,
      can_use_schema: true,
      can_select: true,
      can_update: false,
    },
  });
  await assert.rejects(
    provision({
      adminUrl: ADMIN_URL,
      runtimePassword: 'runtime-secret',
      caBase64: CA_BASE64,
      Client: FakeClient,
    }),
    /runtime database privileges could not be verified/,
  );
});
