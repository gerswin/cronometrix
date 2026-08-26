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
// Bind contract (C-07, mirrored by the production pg-backed store):
//   bind(licenseKey, fp, now) -> Promise<boolean>
//   true  -> the row is now bound to fp (it was unbound, or already bound to fp)
//   false -> the row is bound to a DIFFERENT fingerprint already; no write
//            happened. Caller must NOT sign a token in this case — another
//            activation won the race.
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
        // Mismo contrato que el UPDATE con guarda: solo vincula si está libre o
        // ya es el mismo equipo. Devuelve si la vinculación quedó hecha.
        if (row.fp != null && row.fp !== fp) return false;
        if (row.activated_at == null) row.activated_at = now;
        row.fp = fp;
        rows.set(licenseKey, row);
        return true;
    },
    async rebind(licenseKey, previousFp, fp, now) {
        const row = rows.get(licenseKey);
        if (!row || row.fp !== previousFp) return false;
        row.fp = fp;
        row.last_renewed_at = now;
        return true;
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
