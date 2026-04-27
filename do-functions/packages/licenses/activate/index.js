// POST /licenses/activate
// Body: { license_key, hardware_fingerprint }
// Response 200: { token }                                  RS256 JWT, exp = iat + 1y
// Response 400: { error: { code: "BAD_REQUEST",         message } }
// Response 404: { error: { code: "LICENSE_NOT_FOUND",   message } }
// Response 409: { error: { code: "ALREADY_ACTIVATED",   message } }
// Response 500: { error: { code: "CONFIG_ERROR" | "SERVER_ERROR", message } }
//
// Persistence: process.env.DATABASE_URL points to Postgres with table:
//   licenses(license_key TEXT PRIMARY KEY,
//            hardware_fingerprint TEXT,
//            activated_at BIGINT,
//            last_renewed_at BIGINT)
//
// For local tests, process.env.TEST_STORE = '1' swaps in an in-memory store
// from ../shared-store.js, eliminating the Postgres dependency for unit tests.
//
// Lookup contract:
//   undefined  -> license_key not seeded                            -> 404
//   null       -> seeded but no fingerprint bound yet               -> bind, return JWT
//   <string>   -> bound; compare to incoming hardware_fingerprint
//                   match    -> idempotent re-activation, return JWT
//                   mismatch -> 409 ALREADY_ACTIVATED

'use strict';

const jwt = require('jsonwebtoken');

const ONE_YEAR_SECS = 365 * 24 * 60 * 60;

function getStore() {
    if (process.env.TEST_STORE) {
        // Test mode — in-memory store; no DB connection.
        return require('../shared-store');
    }
    // Production mode — pg client lazily-loaded so test runs don't need it
    // installed at the function-package level until DO Functions does its
    // remote build.
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
        async bind(licenseKey, fingerprint, now) {
            const client = new Client({ connectionString: process.env.DATABASE_URL });
            await client.connect();
            try {
                await client.query(
                    'UPDATE licenses SET hardware_fingerprint = $1, activated_at = COALESCE(activated_at, $2) WHERE license_key = $3',
                    [fingerprint, now, licenseKey],
                );
            } finally {
                await client.end();
            }
        },
    };
}

function signJwt(licenseKey, hardwareFingerprint, privateKey) {
    const now = Math.floor(Date.now() / 1000);
    const payload = {
        license_key: licenseKey,
        hardware_fingerprint: hardwareFingerprint,
        product: 'cronometrix',
        iat: now,
        exp: now + ONE_YEAR_SECS,
    };
    // Algorithm pinned to RS256 (D-01). The Rust verifier (Plan 01) also pins
    // RS256 — defense in depth against alg=HS256 / alg=none confusion attacks.
    return jwt.sign(payload, privateKey, { algorithm: 'RS256' });
}

exports.main = async function main(args) {
    // DO Functions parses JSON request bodies under args.body sometimes and
    // top-level on args other times (form-urlencoded path). Accept both.
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

        // existingFp is now either null (unbound) or a fingerprint string.
        if (existingFp !== null && existingFp !== hardware_fingerprint) {
            return {
                statusCode: 409,
                body: {
                    error: {
                        code: 'ALREADY_ACTIVATED',
                        message: 'license already bound to different hardware',
                    },
                },
            };
        }

        // Bind (idempotent: existingFp === null OR === hardware_fingerprint)
        const now = Math.floor(Date.now() / 1000);
        await store.bind(license_key, hardware_fingerprint, now);

        const token = signJwt(license_key, hardware_fingerprint, privateKey);
        return { statusCode: 200, body: { token } };
    } catch (e) {
        // Never leak DB error details / stack traces / private key material.
        // The catch path is the last line of defense for T-06-40 (private key
        // disclosure) and T-06-41 (DB credential disclosure).
        return {
            statusCode: 500,
            body: {
                error: {
                    code: 'SERVER_ERROR',
                    message: 'license activation failed',
                },
            },
        };
    }
};
