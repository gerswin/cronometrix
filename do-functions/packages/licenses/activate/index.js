// POST /licenses/activate
// Body: { license_key, hardware_fingerprint }
// Response 200: { token }                                  RS256 JWT, exp = iat + 1y
// Response 400: { error: { code: "BAD_REQUEST",         message } }
// Response 404: { error: { code: "LICENSE_NOT_FOUND",   message } }
// Response 409: { error: { code: "ALREADY_ACTIVATED",      message } }  lookup found a different fp
// Response 409: { error: { code: "LICENSE_ALREADY_BOUND",  message } }  bind() lost the race (C-07)
// Response 500: { error: { code: "CONFIG_ERROR" | "SERVER_ERROR", message } }
//
// Persistence: the shared pg adapter connects with an Aiven CA and uses:
//   license_authority.licenses(license_key TEXT PRIMARY KEY,
//            hardware_fingerprint TEXT,
//            activated_at BIGINT,
//            last_renewed_at BIGINT)
//
// For local tests, process.env.TEST_STORE = '1' swaps in an in-memory store
// from ../shared-store.js, eliminating the Postgres dependency for unit tests.
//
// Lookup contract:
//   undefined  -> license_key not seeded                            -> 404
//   null       -> seeded but no fingerprint bound yet               -> attempt bind
//   <string>   -> bound; compare to incoming hardware_fingerprint
//                   match    -> idempotent re-activation, attempt bind
//                   mismatch -> 409 ALREADY_ACTIVATED
//
// Bind contract (C-07): the lookup above is advisory only — it narrows the
// common case but does not gate the write. bind(licenseKey, fp, now) returns
// a boolean: true means the row is now bound to fp; false means another
// activation already bound it to a different fingerprint (race lost), and
// the handler must return 409 LICENSE_ALREADY_BOUND instead of signing a
// token. The actual guard lives in the UPDATE's WHERE clause (production) /
// the equivalent check in shared-store.js (tests) — see each for detail.

'use strict';

const jwt = require('jsonwebtoken');
const { createPrivateKey, createPublicKey } = require('node:crypto');
const { existsSync } = require('node:fs');
const { join } = require('node:path');

const ONE_YEAR_SECS = 365 * 24 * 60 * 60;

function resolveStore(env = process.env) {
    if (env.TEST_STORE === '1') {
        // Test mode — in-memory store; no DB connection.
        return require('../shared-store');
    }
    const runtimeAdapter = join(__dirname, 'pg-store.js');
    const adapter = existsSync(runtimeAdapter)
        ? require(runtimeAdapter)
        : require('../../../lib/pg-store');
    return adapter.createPgStore({ env });
}

function signJwt(licenseKey, hardwareFingerprint, signingKey) {
    const now = Math.floor(Date.now() / 1000);
    const payload = {
        license_key: licenseKey,
        hardware_fingerprint: hardwareFingerprint,
        fingerprint_version: 2,
        product: 'cronometrix',
        iat: now,
        exp: now + ONE_YEAR_SECS,
    };
    // Algorithm pinned to RS256 (D-01). The Rust verifier (Plan 01) also pins
    // RS256 — defense in depth against alg=HS256 / alg=none confusion attacks.
    return jwt.sign(payload, signingKey, { algorithm: 'RS256' });
}

function verifiesLegacyMigrationProof(token, licenseKey, existingFingerprint, verificationKey) {
    if (typeof token !== 'string' || token.trim() === '') return false;
    try {
        const claims = jwt.verify(token.trim(), verificationKey, { algorithms: ['RS256'] });
        return claims.product === 'cronometrix'
            && claims.license_key === licenseKey
            && claims.hardware_fingerprint === existingFingerprint
            && claims.fingerprint_version === undefined;
    } catch {
        return false;
    }
}

exports.main = async function main(args) {
    // DO Functions parses JSON request bodies under args.body sometimes and
    // top-level on args other times (form-urlencoded path). Accept both.
    const body = args && args.body && typeof args.body === 'object' ? args.body : args;
    const license_key = body && body.license_key;
    const hardware_fingerprint = body && body.hardware_fingerprint;
    const previous_token = body && body.previous_token;

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
        // Parse the signing key before touching persistence so deployment
        // probes cannot pass while the runtime key is malformed.
        const signingKey = createPrivateKey(privateKey);
        const verificationKey = createPublicKey(signingKey);
        const store = resolveStore();
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
            if (!previous_token) {
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

            // One-time V1 -> V2 migration. The caller must prove possession of
            // both the original license key and its still-valid, signed legacy
            // JWT. A V2 token can never authorize another move. rebind() guards
            // the old fingerprint in one UPDATE, closing migration races and
            // making the legacy proof non-replayable after the first success.
            if (!verifiesLegacyMigrationProof(
                previous_token,
                license_key,
                existingFp,
                verificationKey,
            )) {
                return {
                    statusCode: 403,
                    body: {
                        error: {
                            code: 'MIGRATION_PROOF_INVALID',
                            message: 'legacy license migration proof is invalid',
                        },
                    },
                };
            }
            const now = Math.floor(Date.now() / 1000);
            const rebound = await store.rebind(
                license_key,
                existingFp,
                hardware_fingerprint,
                now,
            );
            if (!rebound) {
                return {
                    statusCode: 409,
                    body: {
                        error: {
                            code: 'LICENSE_ALREADY_BOUND',
                            message: 'This license is already activated on different hardware.',
                        },
                    },
                };
            }
            const token = signJwt(license_key, hardware_fingerprint, signingKey);
            return { statusCode: 200, body: { token } };
        }

        // Bind (idempotent: existingFp === null OR === hardware_fingerprint).
        // The lookup above narrows the common case, but it does NOT decide
        // whether we bind — that decision belongs to bind()'s own guarded
        // WHERE clause (C-07). Two concurrent activations can both pass the
        // lookup check above (both see fp === null); only one of the bind()
        // calls below may actually win the race.
        const now = Math.floor(Date.now() / 1000);
        const bound = await store.bind(license_key, hardware_fingerprint, now);
        if (!bound) {
            // C-07: otra activación ganó la carrera y vinculó la licencia a otro
            // equipo. Firmar aquí entregaría un token válido para dos máquinas.
            return {
                statusCode: 409,
                body: {
                    error: {
                        code: 'LICENSE_ALREADY_BOUND',
                        message: 'This license is already activated on different hardware.',
                    },
                },
            };
        }

        const token = signJwt(license_key, hardware_fingerprint, signingKey);
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

exports.resolveStore = resolveStore;
exports.verifiesLegacyMigrationProof = verifiesLegacyMigrationProof;
