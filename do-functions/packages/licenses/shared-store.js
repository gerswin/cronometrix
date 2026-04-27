// Test-only in-memory license store. NOT used in production deployment.
//
// Activated when process.env.TEST_STORE is set; both activate/index.js and
// renew/index.js fall back to require('../shared-store') in test mode so the
// node:test suites can exercise the handlers without a Postgres dependency.
//
// Lookup contract (mirrored by the production pg-backed store):
//   undefined  -> row does NOT exist (license_key never seeded)            -> 404
//   null       -> row exists but no fingerprint bound yet                  -> proceed to bind
//   <string>   -> row exists, bound to that fingerprint                    -> compare
//
// Reset between tests via store.__reset(); seed rows via store.__seedRow().

const rows = new Map(); // license_key -> { fp: string|null, activated_at, last_renewed_at }

module.exports = {
    async lookup(licenseKey) {
        const row = rows.get(licenseKey);
        if (!row) return undefined; // not found
        return row.fp; // null when seeded but unbound, string when bound
    },
    async bind(licenseKey, fp, now) {
        const row = rows.get(licenseKey) || { fp: null, activated_at: null, last_renewed_at: null };
        if (row.activated_at == null) row.activated_at = now;
        row.fp = fp;
        rows.set(licenseKey, row);
    },
    async touch(licenseKey, now) {
        const row = rows.get(licenseKey);
        if (row) row.last_renewed_at = now;
    },

    // -------- Test helpers (NOT part of production contract) --------
    __reset() {
        rows.clear();
    },
    __seedRow(licenseKey, fp = null) {
        rows.set(licenseKey, {
            fp,
            activated_at: fp ? 1 : null,
            last_renewed_at: null,
        });
    },
};
