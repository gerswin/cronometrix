// POST /licenses/renew
// Body: { license_key, hardware_fingerprint }
// Response 200: { token }                                  fresh RS256 JWT, exp = now + 1y
// Response 400: { error: { code: "BAD_REQUEST",         message } }
// Response 403: { error: { code: "HARDWARE_MISMATCH",   message } }   anti-cloning
// Response 404: { error: { code: "LICENSE_NOT_FOUND",   message } }
// Response 500: { error: { code: "CONFIG_ERROR" | "SERVER_ERROR", message } }
//
// Renewal is more strict than activation: it ALWAYS requires an existing
// fingerprint binding. An unbound license (fp === null) returns 403, NOT a
// new binding — renew is never a back-door activation path. The Rust client
// (Plan 01) calls renew silently every 24h, only after activate has bound.

'use strict';

const jwt = require('jsonwebtoken');

const ONE_YEAR_SECS = 365 * 24 * 60 * 60;

function getStore() {
    if (process.env.TEST_STORE) {
        return require('../shared-store');
    }
    const { Client } = require('pg');
    return {
        async lookup(licenseKey) {
            const client = new Client({ connectionString: process.env.DATABASE_URL });
            await client.connect();
            try {
                const r = await client.query(
                    'SELECT hardware_fingerprint FROM licenses WHERE license_key = $1',
                    [licenseKey],
                );
                if (r.rows.length === 0) return undefined; // not found
                return r.rows[0].hardware_fingerprint || null;
            } finally {
                await client.end();
            }
        },
        async touch(licenseKey, now) {
            const client = new Client({ connectionString: process.env.DATABASE_URL });
            await client.connect();
            try {
                await client.query(
                    'UPDATE licenses SET last_renewed_at = $1 WHERE license_key = $2',
                    [now, licenseKey],
                );
            } finally {
                await client.end();
            }
        },
    };
}

exports.main = async function main(args) {
    const body = args && args.body && typeof args.body === 'object' ? args.body : args;
    const license_key = body && body.license_key;
    const hardware_fingerprint = body && body.hardware_fingerprint;

    if (!license_key || !hardware_fingerprint) {
        return {
            statusCode: 400,
            body: {
                error: {
                    code: 'BAD_REQUEST',
                    message: 'license_key and hardware_fingerprint required',
                },
            },
        };
    }

    const privateKey = process.env.LICENSE_PRIVATE_KEY;
    if (!privateKey) {
        return {
            statusCode: 500,
            body: {
                error: {
                    code: 'CONFIG_ERROR',
                    message: 'license server misconfigured',
                },
            },
        };
    }

    try {
        const store = getStore();
        const existingFp = await store.lookup(license_key);

        if (existingFp === undefined) {
            return {
                statusCode: 404,
                body: {
                    error: {
                        code: 'LICENSE_NOT_FOUND',
                        message: 'license key not found',
                    },
                },
            };
        }

        // Anti-cloning defense in depth:
        //   - Unbound license (fp === null) -> 403, NOT a back-door activation
        //   - Mismatched fingerprint        -> 403, mirrors Rust LIC-05 startup check
        if (existingFp !== hardware_fingerprint) {
            return {
                statusCode: 403,
                body: {
                    error: {
                        code: 'HARDWARE_MISMATCH',
                        message: 'license bound to different hardware',
                    },
                },
            };
        }

        const now = Math.floor(Date.now() / 1000);
        await store.touch(license_key, now);

        const payload = {
            license_key,
            hardware_fingerprint,
            product: 'cronometrix',
            iat: now,
            exp: now + ONE_YEAR_SECS,
        };
        // RS256 pinned — same algorithm/key path as activate (D-01).
        const token = jwt.sign(payload, privateKey, { algorithm: 'RS256' });
        return { statusCode: 200, body: { token } };
    } catch (e) {
        // Generic SERVER_ERROR: never leak DB / key details.
        return {
            statusCode: 500,
            body: {
                error: {
                    code: 'SERVER_ERROR',
                    message: 'license renewal failed',
                },
            },
        };
    }
};
