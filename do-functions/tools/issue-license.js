#!/usr/bin/env node
'use strict';

const { buildPgConfig } = require('../lib/pg-store');
const { retargetDatabaseUrl } = require('./aiven-config');
const { DATABASE_NAME } = require('./provision-aiven');

const LICENSE_PATTERN = /^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$/;

async function issueLicense({ adminUrl, caBase64, licenseKey, Client }) {
  if (typeof licenseKey !== 'string' || !LICENSE_PATTERN.test(licenseKey)) {
    throw new Error('license key format is invalid');
  }

  const config = buildPgConfig({
    DATABASE_URL: retargetDatabaseUrl(adminUrl, DATABASE_NAME),
    DATABASE_CA_CERT_BASE64: caBase64,
  });
  const PgClient = Client || require('pg').Client;
  const client = new PgClient(config);
  try {
    await client.connect();
    const inserted = await client.query(
      `INSERT INTO license_authority.licenses (license_key, hardware_fingerprint)
       VALUES ($1, NULL)
       ON CONFLICT (license_key) DO NOTHING`,
      [licenseKey],
    );
    if (inserted.rowCount === 1) return 'created';

    const existing = await client.query(
      `SELECT hardware_fingerprint
       FROM license_authority.licenses
       WHERE license_key = $1`,
      [licenseKey],
    );
    if (existing.rows.length === 0) {
      throw new Error('license state could not be verified');
    }
    return existing.rows[0].hardware_fingerprint === null
      ? 'exists-unbound'
      : 'exists-bound';
  } finally {
    await client.end();
  }
}

async function cli() {
  try {
    const result = await issueLicense({
      adminUrl: process.env.CRONOMETRIX_AIVEN_ADMIN_URL,
      caBase64: process.env.CRONOMETRIX_AIVEN_CA_BASE64,
      licenseKey: process.env.CRONOMETRIX_LICENSE_NANOPI_192_168_1_239,
    });
    if (result === 'exists-bound') {
      process.stderr.write('license issuance refused: existing license is already bound\n');
      process.exitCode = 1;
      return;
    }
    process.stdout.write(`license issuance status: ${result}\n`);
  } catch {
    process.stderr.write('license issuance failed\n');
    process.exitCode = 1;
  }
}

if (require.main === module) cli();

module.exports = { LICENSE_PATTERN, issueLicense };
