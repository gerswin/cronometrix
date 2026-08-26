'use strict';

const LICENSES_TABLE = 'license_authority.licenses';
const SSL_QUERY_OPTIONS = ['sslmode', 'sslcert', 'sslkey', 'sslrootcert'];

const LOOKUP_SQL = `SELECT hardware_fingerprint
  FROM ${LICENSES_TABLE} WHERE license_key = $1`;
const BIND_SQL = `UPDATE ${LICENSES_TABLE}
  SET hardware_fingerprint = $1,
      activated_at = COALESCE(activated_at, $2)
  WHERE license_key = $3
    AND (hardware_fingerprint IS NULL OR hardware_fingerprint = $1)`;
const TOUCH_SQL = `UPDATE ${LICENSES_TABLE}
  SET last_renewed_at = $1 WHERE license_key = $2`;

function invalidEnvironment(name) {
  return new Error(`${name} is missing or invalid`);
}

function normalizeDatabaseUrl(rawUrl) {
  if (typeof rawUrl !== 'string' || rawUrl.trim() === '') {
    throw invalidEnvironment('DATABASE_URL');
  }

  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    throw invalidEnvironment('DATABASE_URL');
  }

  if (!['postgres:', 'postgresql:'].includes(url.protocol) || !url.hostname) {
    throw invalidEnvironment('DATABASE_URL');
  }

  for (const option of SSL_QUERY_OPTIONS) {
    url.searchParams.delete(option);
  }
  return url.toString();
}

function decodeCa(encoded) {
  if (typeof encoded !== 'string') {
    throw invalidEnvironment('DATABASE_CA_CERT_BASE64');
  }

  const compact = encoded.trim();
  if (
    compact === ''
    || compact.length % 4 !== 0
    || !/^[A-Za-z0-9+/]+={0,2}$/.test(compact)
  ) {
    throw invalidEnvironment('DATABASE_CA_CERT_BASE64');
  }

  const decoded = Buffer.from(compact, 'base64');
  if (decoded.toString('base64') !== compact) {
    throw invalidEnvironment('DATABASE_CA_CERT_BASE64');
  }

  const pem = decoded.toString('utf8');
  if (
    !pem.includes('-----BEGIN CERTIFICATE-----')
    || !pem.includes('-----END CERTIFICATE-----')
  ) {
    throw invalidEnvironment('DATABASE_CA_CERT_BASE64');
  }
  return pem;
}

function buildPgConfig(env = process.env) {
  return {
    connectionString: normalizeDatabaseUrl(env.DATABASE_URL),
    ssl: {
      ca: decodeCa(env.DATABASE_CA_CERT_BASE64),
      rejectUnauthorized: true,
    },
    connectionTimeoutMillis: 5000,
    query_timeout: 5000,
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

function createPgStore({ env = process.env, Client } = {}) {
  const PgClient = Client || require('pg').Client;
  const config = buildPgConfig(env);

  return {
    async lookup(licenseKey) {
      return withClient(PgClient, config, async (client) => {
        const result = await client.query(LOOKUP_SQL, [licenseKey]);
        if (result.rows.length === 0) return undefined;
        return result.rows[0].hardware_fingerprint;
      });
    },

    async bind(licenseKey, fingerprint, activatedAt) {
      return withClient(PgClient, config, async (client) => {
        const result = await client.query(BIND_SQL, [
          fingerprint,
          activatedAt,
          licenseKey,
        ]);
        return result.rowCount === 1;
      });
    },

    async touch(licenseKey, renewedAt) {
      return withClient(PgClient, config, async (client) => {
        await client.query(TOUCH_SQL, [renewedAt, licenseKey]);
      });
    },
  };
}

module.exports = {
  LICENSES_TABLE,
  normalizeDatabaseUrl,
  decodeCa,
  buildPgConfig,
  createPgStore,
};
