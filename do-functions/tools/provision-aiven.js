#!/usr/bin/env node
'use strict';

const { buildPgConfig } = require('../lib/pg-store');
const { buildDatabaseUrl, retargetDatabaseUrl } = require('./aiven-config');

const DATABASE_NAME = 'cronometrix_licenses';
const OWNER_ROLE = 'cronometrix_license_owner';
const RUNTIME_ROLE = 'cronometrix_license_runtime';
const SCHEMA_NAME = 'license_authority';
const TABLE_NAME = 'licenses';

function plan(dryRun) {
  return {
    database: DATABASE_NAME,
    ownerRole: OWNER_ROLE,
    runtimeRole: RUNTIME_ROLE,
    schema: SCHEMA_NAME,
    table: TABLE_NAME,
    dryRun,
  };
}

async function withClient(Client, config, operation) {
  const client = new Client(config);
  try {
    await client.connect();
    return await operation(client);
  } finally {
    await client.end();
  }
}

async function roleExists(client, role) {
  const result = await client.query(
    'SELECT 1 AS exists FROM pg_roles WHERE rolname = $1',
    [role],
  );
  return result.rows.length > 0;
}

async function provision({
  adminUrl,
  runtimePassword,
  caBase64,
  Client,
  dryRun = false,
}) {
  if (typeof runtimePassword !== 'string' || runtimePassword.length < 12) {
    throw new Error('CRONOMETRIX_AIVEN_LICENSE_PASSWORD is missing or invalid');
  }

  const adminConfig = buildPgConfig({
    DATABASE_URL: adminUrl,
    DATABASE_CA_CERT_BASE64: caBase64,
  });
  const runtimeUrl = buildDatabaseUrl(adminUrl, {
    username: RUNTIME_ROLE,
    password: runtimePassword,
    database: DATABASE_NAME,
  });
  const result = { ...plan(dryRun), runtimeUrl };
  if (dryRun) return plan(true);

  const PgClient = Client || require('pg').Client;
  await withClient(PgClient, adminConfig, async (client) => {
    if (!await roleExists(client, OWNER_ROLE)) {
      await client.query(`CREATE ROLE ${OWNER_ROLE} NOLOGIN`);
    }

    const runtimeRoleExists = await roleExists(client, RUNTIME_ROLE);
    const runtimeRoleStatement = runtimeRoleExists
      ? `ALTER ROLE ${RUNTIME_ROLE} PASSWORD %L`
      : `CREATE ROLE ${RUNTIME_ROLE} LOGIN PASSWORD %L`;
    const formatted = await client.query(
      `SELECT format('${runtimeRoleStatement}', $1::text) AS statement`,
      [runtimePassword],
    );
    await client.query(formatted.rows[0].statement);

    const database = await client.query(
      'SELECT 1 AS exists FROM pg_database WHERE datname = $1',
      [DATABASE_NAME],
    );
    if (database.rows.length === 0) {
      await client.query(`CREATE DATABASE ${DATABASE_NAME} OWNER ${OWNER_ROLE}`);
    }
  });

  const targetConfig = buildPgConfig({
    DATABASE_URL: retargetDatabaseUrl(adminUrl, DATABASE_NAME),
    DATABASE_CA_CERT_BASE64: caBase64,
  });
  await withClient(PgClient, targetConfig, async (client) => {
    await client.query(
      `CREATE SCHEMA IF NOT EXISTS ${SCHEMA_NAME} AUTHORIZATION ${OWNER_ROLE}`,
    );
    await client.query(`SET ROLE ${OWNER_ROLE}`);
    try {
      await client.query(`CREATE TABLE IF NOT EXISTS ${SCHEMA_NAME}.${TABLE_NAME} (
        license_key TEXT PRIMARY KEY,
        hardware_fingerprint TEXT,
        activated_at BIGINT,
        last_renewed_at BIGINT
      )`);
    } finally {
      await client.query('RESET ROLE');
    }

    await client.query(`REVOKE ALL ON DATABASE ${DATABASE_NAME} FROM PUBLIC`);
    await client.query(`GRANT CONNECT ON DATABASE ${DATABASE_NAME} TO ${RUNTIME_ROLE}`);
    await client.query(`REVOKE ALL ON SCHEMA ${SCHEMA_NAME} FROM PUBLIC`);
    await client.query(`REVOKE ALL ON TABLE ${SCHEMA_NAME}.${TABLE_NAME} FROM PUBLIC`);
    await client.query(`GRANT USAGE ON SCHEMA ${SCHEMA_NAME} TO ${RUNTIME_ROLE}`);
    await client.query(
      `GRANT SELECT, UPDATE ON TABLE ${SCHEMA_NAME}.${TABLE_NAME} TO ${RUNTIME_ROLE}`,
    );

    const privileges = await client.query(
      `SELECT
        has_database_privilege($1, $2, 'CONNECT') AS can_connect,
        has_schema_privilege($1, $3, 'USAGE') AS can_use_schema,
        has_table_privilege($1, $4, 'SELECT') AS can_select,
        has_table_privilege($1, $4, 'UPDATE') AS can_update`,
      [RUNTIME_ROLE, DATABASE_NAME, SCHEMA_NAME, `${SCHEMA_NAME}.${TABLE_NAME}`],
    );
    const effective = privileges.rows[0] || {};
    if (!effective.can_connect || !effective.can_use_schema
        || !effective.can_select || !effective.can_update) {
      throw new Error('runtime database privileges could not be verified');
    }
  });

  return result;
}

async function cli() {
  const args = new Set(process.argv.slice(2));
  const printRuntimeUrl = args.has('--print-runtime-url');
  const dryRun = args.has('--dry-run');
  try {
    const result = await provision({
      adminUrl: process.env.CRONOMETRIX_AIVEN_ADMIN_URL,
      runtimePassword: process.env.CRONOMETRIX_AIVEN_LICENSE_PASSWORD,
      caBase64: process.env.CRONOMETRIX_AIVEN_CA_BASE64,
      dryRun,
    });
    if (printRuntimeUrl) {
      if (dryRun) throw new Error('cannot print a runtime URL during dry-run');
      process.stdout.write(`${result.runtimeUrl}\n`);
    } else {
      process.stdout.write(dryRun
        ? 'Aiven license authority plan validated\n'
        : 'Aiven license authority provisioned\n');
    }
  } catch {
    process.stderr.write('Aiven license authority provisioning failed\n');
    process.exitCode = 1;
  }
}

if (require.main === module) cli();

module.exports = {
  DATABASE_NAME,
  OWNER_ROLE,
  RUNTIME_ROLE,
  provision,
};
