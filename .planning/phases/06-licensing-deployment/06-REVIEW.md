---
phase: 06-licensing-deployment
reviewed: 2026-04-27T00:00:00Z
depth: standard
files_reviewed: 28
files_reviewed_list:
  - backend/Cargo.toml
  - backend/src/config.rs
  - backend/src/errors.rs
  - backend/src/lib.rs
  - backend/src/license/fingerprint.rs
  - backend/src/license/middleware.rs
  - backend/src/license/mod.rs
  - backend/src/license/service.rs
  - backend/src/main.rs
  - backend/src/setup/handlers.rs
  - backend/src/state.rs
  - backend/tests/license_tests.rs
  - deploy/.dockerignore
  - deploy/docker-compose.yml
  - deploy/Dockerfile.api
  - deploy/Dockerfile.web
  - deploy/install.sh
  - do-functions/packages/licenses/activate/index.js
  - do-functions/packages/licenses/renew/index.js
  - do-functions/packages/licenses/shared-store.js
  - do-functions/project.yml
  - do-functions/README.md
  - frontend/next.config.ts
  - frontend/src/app/login/page.tsx
  - frontend/src/app/setup/license/layout.tsx
  - frontend/src/app/setup/license/page.tsx
  - frontend/src/app/setup/page.tsx
  - frontend/src/lib/validations.ts
findings:
  critical: 1
  warning: 9
  info: 7
  total: 17
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-04-27
**Depth:** standard
**Files Reviewed:** 28
**Status:** issues_found

## Summary

Phase 6 delivers the licensing system (RS256-signed JWTs, anti-cloning hardware fingerprint, DO Functions activation/renewal) and the Docker Compose + Cloudflare-tunnel deployment package. The cryptographic core is correct: RS256 algorithm pinning (both Rust verifier and Node signer), fail-closed fingerprint check before JWT persistence, atomic temp+rename writes, and a license gate that runs before `require_auth` (verified by `route_layer` reverse-ordering tests in `license_tests.rs`).

The most serious issue is a **license-gate ordering gap on the Phase-1 cookie-auth refresh route**: while every protected resource router applies `require_license` correctly, the cookie-auth router (`/auth/refresh`, `/auth/logout`) only attaches `require_license` and never adds `require_auth`. This is consistent with the comment ("refresh/logout validate via refresh cookie, not Bearer") so it is intentional, BUT the gate-then-auth invariant the rest of the codebase relies on for "no auth-state leak on unlicensed boxes" is preserved here only because the auth check happens inside the handler. That is fine — **but the license gate inside `setup_activate` is bypassable through a fingerprint-collection race** (Critical-01 below). Several Warning-class issues exist around ordering atomicity, file permissions on the persisted JWT, and renewal-task race against fresh activation.

Findings are listed below with severity, file/line, issue, and concrete fix.

## Critical Issues

### CR-01: `setup_activate` idempotency check is racy — concurrent activations may both succeed and overwrite each other's JWT

**File:** `backend/src/setup/handlers.rs:155-172`
**Issue:** `setup_activate` reads `state.license_valid` (AtomicBool with `Relaxed` ordering) at line 157, then performs a network call to DO Functions, then writes the JWT to disk, then sets the flag. Between the load and the eventual store, **two concurrent POST requests both observe `license_valid=false`**, both call DO Functions, both receive a valid JWT (DO Functions activation is idempotent on same-fingerprint), and both race to write `license_jwt_path`. The second `std::fs::rename` could partially overwrite a JWT that a separate code path is concurrently reading via `load_and_validate_license` (file-read-trust gap during boot or test startup).

The token persisted is identical (same fingerprint binding), so this is not a security compromise of the binding — but it does:
1. Send two billable calls to DO Functions per concurrent attacker request.
2. Allow log-flood DoS via repeated `setup_activate` POSTs before the first completes.
3. Leak fingerprints over the network for any attacker who can reach `/setup/activate` (this endpoint is **public** and unauthenticated by design).

The route is exposed publicly (`public_routes` in `main.rs:140`) and pre-license, so an unauthenticated attacker who can reach the box (via `cloudflared` or local network) can trigger unlimited DO Functions calls until activation flips the flag.

**Fix:** Replace the optimistic AtomicBool guard with a `tokio::sync::Mutex<()>` held across the entire DO Functions round-trip, or use `compare_exchange` to atomically claim the activation slot before the network call:

```rust
// In AppState: add an in-flight guard
pub activation_in_flight: Arc<tokio::sync::Mutex<()>>,

// In setup_activate, BEFORE the DO Functions call:
let _guard = state.activation_in_flight.try_lock()
    .map_err(|_| AppError::Conflict {
        code: "ACTIVATION_IN_PROGRESS",
        message: "Another activation attempt is in flight.".to_string(),
    })?;

// Then re-check license_valid under the lock:
if state.license_valid.load(Ordering::Acquire) {
    return Err(AppError::Conflict { code: "ALREADY_ACTIVATED", .. });
}
```

Additionally, add IP-based rate-limiting to `/setup/activate` (DO Functions has platform-level abuse limits per `do-functions/README.md`, but the local box does not — and pre-tunnel deploys may expose the port).

## Warnings

### WR-01: Persisted license JWT file inherits process umask — may be world-readable

**File:** `backend/src/license/service.rs:166-169` (and `:255-259` for renewal)
**Issue:** `std::fs::write` and `std::fs::rename` create the file with the calling process's default umask, typically `0644` inside Docker (root in container). Anyone reading the host's `./data/` volume mount (`docker-compose.yml:21`) can extract the JWT. While the JWT is not directly exploitable (fingerprint binding prevents replay on a different host), it leaks the license key in the `license_key` claim, the customer's hardware fingerprint hash, and the binding metadata.

**Fix:** Set explicit `0600` permissions after the rename. Use `std::os::unix::fs::PermissionsExt` (gated for Linux production target, since the only deploy target is Linux per `CLAUDE.md`):

```rust
std::fs::rename(&tmp, jwt_path)
    .map_err(|e| AppError::Internal(anyhow::anyhow!("rename license file: {}", e)))?;

#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(jwt_path, perms)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("set license perms: {}", e)))?;
}
```

Apply at both `service.rs:169` (activation) and `service.rs:259` (renewal). Also verify `deploy/install.sh:94-95` chowns/chmods the `data/` dir to a non-root container UID once the API container moves off `root` (see WR-08).

### WR-02: License JWT `iss`/`aud` claims not validated — JWT minted for another product would verify

**File:** `backend/src/license/service.rs:50-56`
**Issue:** `verify_license_jwt` pins `Algorithm::RS256` and disables `validate_exp`, but does NOT validate `iss` (issuer) or `aud` (audience). The same DO Functions private key signs whatever the operator wires up; if the operator ever reuses that key for a different product (or a different SaaS at the same DO account), a JWT minted for product B with a matching fingerprint would verify on a Cronometrix box. The `claims.product` field is set to `"cronometrix"` by the signer (`activate/index.js:76`) but the verifier never compares it.

**Fix:** Validate the `product` claim explicitly after decode, or add `iss`+`aud` enforcement via `Validation::set_audience` / `set_issuer`:

```rust
pub fn verify_license_jwt(token: &str) -> Result<LicenseClaims, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    let data = decode::<LicenseClaims>(token, license_decoding_key(), &validation)
        .map_err(|_| AppError::Unlicensed)?;
    if data.claims.product != "cronometrix" {
        return Err(AppError::Unlicensed);
    }
    Ok(data.claims)
}
```

This closes a key-reuse-across-products attack surface for free; the cost is one string compare per verify.

### WR-03: License renewal task races against fresh activation — `try_renew` may overwrite an in-flight `setup_activate` write

**File:** `backend/src/license/service.rs:177-261`
**Issue:** `renewal_task` runs every 24h and calls `try_renew`, which reads the cached JWT, calls DO Functions `/renew`, then atomically writes a new JWT. If `setup_activate` is in flight at the same moment (concurrent first-run + a re-activation flow), the renewal task could:
1. Read the OLD cached JWT (if any).
2. Decide it's not within 30 days of expiry → return Ok(()) (safe).

But more concerning: if `setup_activate` re-binds with a NEW `license_key` (operator entered a different key), the renewal task's stale `claims.license_key` value gets sent to `/renew`, which now binds against the new fingerprint binding. There's also no mutex between renewal write and activation write — both call `std::fs::rename` on `jwt_path` and the OS guarantees rename atomicity, but the `.tmp` filename is deterministic (`format!("{}.tmp", jwt_path)`), so two concurrent writers race on the temp file too.

**Fix:** Use a unique temp file name per write (PID + random suffix) and serialize all writes to `license_jwt_path` through the same `tokio::sync::Mutex<()>` proposed in CR-01:

```rust
let tmp = format!("{}.tmp.{}", jwt_path, uuid::Uuid::new_v4());
std::fs::write(&tmp, token)
    .map_err(|e| AppError::Internal(anyhow::anyhow!("write license tmp: {}", e)))?;
std::fs::rename(&tmp, jwt_path)
    .map_err(|e| {
        let _ = std::fs::remove_file(&tmp); // cleanup on rename failure
        AppError::Internal(anyhow::anyhow!("rename license file: {}", e))
    })?;
```

Apply at both `service.rs:165-169` and `service.rs:255-259`.

### WR-04: License gate flag uses `Relaxed` ordering — a freshly-activated install may still 403 protected requests for a window

**File:** `backend/src/license/middleware.rs:22`, `backend/src/setup/handlers.rs:175`, `backend/src/main.rs:74`
**Issue:** `license_valid.load(Ordering::Relaxed)` in the middleware can observe a stale `false` value AFTER `setup_activate` has stored `true` with `Relaxed` ordering. On x86 this happens to be safe by accident (TSO), but on ARM (typical for cloud VPS) the load may briefly observe `false` for several milliseconds after activation. Any request that arrives in that window gets 403 UNLICENSED even though activation succeeded.

The store at line 174 uses `Relaxed` and the load at middleware uses `Relaxed`. There is no `Acquire`/`Release` pairing.

**Fix:** Use `Release` on the store and `Acquire` on the load to publish the activation transition:

```rust
// middleware.rs
if !state.license_valid.load(Ordering::Acquire) {
    return Err(AppError::Unlicensed);
}

// setup_activate after the JWT write succeeds:
state.license_valid.store(true, Ordering::Release);

// main.rs at boot:
license_valid.store(true, Ordering::Release);
```

The same change is needed for the read in `setup_status` (`handlers.rs:34`) for consistency, though that is read-only and a stale read just shows `licensed:false` for one extra frontend poll.

### WR-05: `install.sh` writes Cloudflare tunnel token + DO Functions URLs in `.env` without verifying the file is on a local filesystem

**File:** `deploy/install.sh:115-132`
**Issue:** The installer sets `umask 077` and writes `.env` via heredoc. If `INSTALL_DIR` is on an NFS or shared mount (operator override via `CRONOMETRIX_INSTALL_DIR`), umask may be ignored by the remote filesystem, resulting in a world-readable `.env` containing `JWT_SECRET`, `DEVICE_CREDS_KEY`, `CLOUDFLARE_TUNNEL_TOKEN`. The installer never validates `INSTALL_DIR` is local.

Also: at line 132 the installer calls `chmod 600 "$ENV_FILE"`, which is correct; but it relies on the heredoc having been written with umask 077 first — there is a brief window where the file exists with a more permissive mode if the umask was already permissive. Most filesystems honor umask; remote/network mounts often do not.

**Fix:** Add an explicit local-fs check:

```bash
# Before writing $ENV_FILE
fs_type="$(stat -f -c %T "$INSTALL_DIR" 2>/dev/null || stat -f -c %T "$(dirname "$INSTALL_DIR")")"
case "$fs_type" in
    nfs|cifs|smbfs|fuse*) err "INSTALL_DIR ($INSTALL_DIR) is on $fs_type — use a local filesystem to protect secrets" ;;
esac
```

Also create `.env` empty with `install -m 0600` first, then append, to avoid the umask-window race:

```bash
install -m 0600 /dev/null "$ENV_FILE"
cat >> "$ENV_FILE" <<EOF
JWT_SECRET=${JWT_SECRET}
...
EOF
```

### WR-06: Frontend `setup_activate` toUpperCase + Zod regex case-insensitive flag inconsistency

**File:** `frontend/src/app/setup/license/page.tsx:91-93` + `frontend/src/lib/validations.ts:81-89`
**Issue:** The Zod schema (`validations.ts:86-87`) uses regex flag `/i` (case-insensitive), so the client accepts `abcd-efgh-ijkl-mnop`. The backend validator (`setup/handlers.rs:146`) uses `c.is_ascii_alphanumeric()` which is also case-insensitive. The frontend then uppercases the key before POSTing (line 92). But:

1. If the user types `abcd-EFGH-ijkl-MNOP`, Zod accepts it, then the frontend uppercases the whole string for the network call — **the backend now sees an uppercased key while the user typed mixed-case**, which is fine because `setup_activate` uses `body.license_key` raw against DO Functions.
2. **The DO Functions store does an EXACT string comparison** on the license_key in Postgres (`activate/index.js:46`) — if the operator seeded the DB row with `xxxx-xxxx-xxxx-xxxx` (lowercase) but the frontend forces uppercase, lookup returns `undefined` → 404 LICENSE_NOT_FOUND even though the key is correct.

**Fix:** Decide on a canonical case at the seed stage and document it. If keys are always seeded uppercase (`XXXX-XXXX-XXXX-XXXX` format implies it), uppercase normalization should happen on the **backend** in `setup_activate` BEFORE the DO Functions call, so other clients (curl, install.sh) get the same treatment:

```rust
// setup/handlers.rs around line 167
let normalized_key = body.license_key.trim().to_uppercase();
let _claims = crate::license::service::activate_license(
    &normalized_key,
    &state.config.do_functions_activate_url,
    &state.config.license_jwt_path,
).await?;
```

Also drop the `/i` flag from the Zod regex if uppercase is canonical, OR keep the lowercase tolerance and document that DO Functions DB seed must be lowercase.

### WR-07: Installer's license activation call leaks the license key into shell history and `ps aux`

**File:** `deploy/install.sh:172-177`
**Issue:** `curl ... -d "{\"license_key\":\"${LICENSE_KEY}\"}"` passes the license key as a command-line argument. On Linux, `ps aux` exposes process arguments to all local users (and Docker container processes if the runtime is shared). Even if the installer is run as root, any user who can run `ps` during the ~1-second curl window sees the active license key.

**Fix:** Pipe the JSON body into curl via stdin with `--data @-`:

```bash
ACTIVATE_RESP="$(
    printf '{"license_key":"%s"}' "${LICENSE_KEY}" |
    curl -fsS -X POST http://127.0.0.1:3001/api/v1/setup/activate \
        -H "Content-Type: application/json" \
        --data-binary @-
)" || err "license activation failed — see api logs ('docker compose logs api'); your hardware may already be bound to a different license"
```

Same applies to the admin password at line 191-194.

### WR-08: API container runs as root — license JWT file and DB owned by root in mounted volume

**File:** `deploy/Dockerfile.api:1-44`
**Issue:** The Dockerfile never adds a `USER` directive, so the runtime stage runs as root. The volume-mounted `./data` directory on the host (`docker-compose.yml:21`) gets root-owned files, including `license.jwt` and `cronometrix.db`. If the container is ever escaped (CVE in glibc, Tokio, libsql, etc.), the attacker has root on the host volume and can pivot to host-level secrets.

**Fix:** Add a non-root user in the runner stage:

```dockerfile
FROM debian:bookworm-slim AS runner
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r cronometrix && useradd -r -g cronometrix -u 10001 cronometrix

WORKDIR /opt/cronometrix
COPY --from=builder /app/target/release/cronometrix-api /usr/local/bin/cronometrix-api
RUN mkdir -p /opt/cronometrix/data && chown -R cronometrix:cronometrix /opt/cronometrix
USER cronometrix:cronometrix
```

The installer then needs to chown the host's `./data` directory to UID 10001 before `docker compose up`. Same change for `Dockerfile.web` — Next.js standalone server has no need for root.

### WR-09: `setup_status` exposes `licensed` boolean publicly — fingerprints unlicensed boxes pre-cloudflared

**File:** `backend/src/setup/handlers.rs:32-35`
**Issue:** Returning `licensed: false` to an unauthenticated caller tells an attacker the box is in "first-run, unactivated" state. Combined with the now-public `/setup/activate` endpoint, this is a fingerprintable signal that a Cronometrix install is reachable but unhardened. While not directly exploitable (DO Functions enforces the license-key-fingerprint binding), it accelerates targeted scanning: an attacker with a stolen license key can now find unactivated boxes via Cloudflare's certificate-transparency log and try to activate against that box's fingerprint.

This is also the single endpoint that distinguishes "fresh box" from "already-set-up box" externally.

**Fix:** This is a design tradeoff — the frontend needs `licensed`/`initialized` to drive routing. Two mitigations:
1. Move `/setup/status` behind a localhost-only or LAN-only filter (the install flow is local anyway). Add a `tower::filter` that rejects requests whose `X-Forwarded-For` indicates a public-internet origin.
2. OR collapse the response to just `phase: "license"|"setup"|"ready"` so external callers can't distinguish "unlicensed" from "uninitialized" — the frontend doesn't need separate booleans.

Lower priority because cloudflared is the primary entry point and CF Access can gate the route, but worth tracking.

## Info

### IN-01: Fingerprint disk-serial fallback silently accepts shared VPS templates as "same machine"

**File:** `backend/src/license/fingerprint.rs:62-79`
**Issue:** The disk-serial reader returns empty string when `/sys/block/.../device/serial` is empty (typical for VPS). On VPS templates where every clone has the same MAC pool and CPU model, the fingerprint **collapses to SHA256(cpu_model + mac + "")**, which is non-unique across cloned VMs. The doc-comment acknowledges this is acceptable per D-05, but the only fingerprint variance left is the MAC address — and Hetzner/DO/Vultr often assign sequentially-numbered MAC blocks.

**Fix:** Documented as accepted risk in fingerprint.rs:8-11 and D-05. No code change recommended; just confirm a VPS-specific test fixture validates fingerprint stability across reboots in production. Consider adding `/etc/machine-id` (Linux systemd) as a fourth input when available — it's randomly generated at first boot and survives reboot:

```rust
let machine_id = std::fs::read_to_string("/etc/machine-id")
    .map(|s| s.trim().to_string())
    .unwrap_or_default();
hasher.update(machine_id.as_bytes());
```

### IN-02: `LICENSE_PUBLIC_KEY_PEM` panics at runtime if PEM is malformed — should panic at compile time

**File:** `backend/src/license/service.rs:36-44`
**Issue:** `include_str!` succeeds at compile time for any text content; the PEM parse happens lazily on first request via `OnceLock`. An operator who replaces `pubkey.pem` with garbage gets a successful build but a runtime `expect` panic on the first license check. This is fail-closed (no licenses verify), but the panic only fires when the first request hits `verify_license_jwt`, not at startup.

**Fix:** Eagerly initialize at startup (in `main.rs`) so a bad PEM crashes during boot, not on the first request:

```rust
// main.rs after Config::from_env
let _ = license::service::license_decoding_key(); // force PEM parse
```

This requires making `license_decoding_key` `pub`. Alternatively, add a startup probe that calls `verify_license_jwt` against a known-bad token and asserts it returns `Unlicensed` (not panic).

### IN-03: `try_renew` collapses all renewal failures to a single warning log line — no observability for sustained outages

**File:** `backend/src/license/service.rs:185-194`
**Issue:** When DO Functions renewal fails, the renewal task logs `tracing::warn!("license renewal attempt failed: {}", e)` and continues. There is no metric, no escalation, no health-check signal. A customer's renewal can fail for 11 months silently before the JWT actually expires (D-07 soft expiry means even an expired JWT keeps the system running, but the operator has zero visibility).

**Fix:** Expose a renewal-status field in `/health` or a dedicated `/admin/license` endpoint:

```rust
// In AppState
pub last_renewal_attempt: Arc<RwLock<Option<RenewalResult>>>,

pub struct RenewalResult {
    pub at: chrono::DateTime<chrono::Utc>,
    pub status: Result<(), String>,
    pub days_to_expiry: i64,
}
```

Then surface the last 5 renewal attempts in `/health` or a Prometheus-style metric.

### IN-04: `Cargo.toml` pins `aes-gcm = "0.10.3"` but `argon2` and `password-auth` versions are loosely specified

**File:** `backend/Cargo.toml:7,21`
**Issue:** `password-auth = "1"` resolves to whatever `1.x` is current at lockfile-update time. While Cargo's lockfile pins the actual version, the `Cargo.toml` declaration is loose enough that future `cargo update` could pull in a major-version patch with breaking semantics for password verification. CLAUDE.md mandates `argon2` from RustCrypto for password hashing — `password-auth` is a wrapper around `argon2` and `bcrypt`; pin it precisely.

**Fix:** Pin to the patch version:

```toml
password-auth = "=1.0.5"  # or whatever Cargo.lock currently resolves to
```

Or switch directly to `argon2 = "0.5"` per CLAUDE.md.

### IN-05: `do-functions/packages/licenses/activate/index.js` swallows all DB errors as `SERVER_ERROR`

**File:** `do-functions/packages/licenses/activate/index.js:151-163`
**Issue:** The catch block returns 500 SERVER_ERROR for every exception path: PG connection refused, PG auth error, PG row-level lock conflict, `jsonwebtoken.sign` failure, even programmer errors like `TypeError: Cannot read property 'rows' of undefined`. The Rust client maps any non-2xx, non-404, non-409 to `BadGateway { code: "ACTIVATION_UNREACHABLE" }` — operators see "ACTIVATION_UNREACHABLE" for every infra issue, with no way to distinguish DB outage from code bug.

**Fix:** Differentiate at minimum DB-unreachable from programming-error:

```js
} catch (e) {
    // Postgres-specific connection errors get a distinct code so the operator
    // can page DB infra rather than hunting application bugs.
    if (e.code === 'ECONNREFUSED' || e.code === 'ETIMEDOUT' ||
        e.code === '57P03' /* pg cannot_connect_now */) {
        return {
            statusCode: 503,
            body: { error: { code: 'DB_UNAVAILABLE', message: 'license server temporarily unavailable' } },
        };
    }
    return {
        statusCode: 500,
        body: { error: { code: 'SERVER_ERROR', message: 'license activation failed' } },
    };
}
```

Still don't include `e.message` (avoids leaking PG connection strings or stack traces), but a coarse status code helps observability.

### IN-06: Login page `safeRedirect` does not handle URL-encoded protocol-relative paths

**File:** `frontend/src/app/login/page.tsx:36-43`
**Issue:** `safeRedirect` rejects raw `//evil.com` and `\\` prefixes, but does not rejecturl-encoded variants like `%2F%2Fevil.com` or unicode-fullwidth slashes (`／／evil.com`). Modern browsers normalize most of these before `useSearchParams` returns them, but defense in depth would explicitly decode-and-revalidate:

```ts
function safeRedirect(raw: string | null): string {
  if (!raw) return "/"
  let decoded = raw
  try { decoded = decodeURIComponent(raw) } catch { return "/" }
  if (!decoded.startsWith("/")) return "/"
  if (decoded.startsWith("//")) return "/"
  if (decoded.match(/^\/[\\\/]/)) return "/"  // /\foo, //foo
  return decoded
}
```

Browser-level normalization makes this a hardening item, not an active vuln.

### IN-07: `validate_exp = false` is correct per D-07 but should explicitly call `Validation::insecure_disable_signature_validation` is NOT used (good) — confirm via test

**File:** `backend/src/license/service.rs:51-52` + `backend/tests/license_tests.rs:111-118`
**Issue:** D-07 soft-expiry semantics depend on `validate_exp = false` being the ONLY relaxation. The test `test_verify_rejects_invalid_signature` covers signature-tamper resistance, and `test_verify_rejects_hs256_token` covers algorithm-confusion. Good — but no explicit test asserts `validate_nbf` (not-before) is enforced. If a future maintainer adds `nbf` to `LicenseClaims` and a refactor sets `validate_nbf = false`, the change goes unnoticed.

**Fix:** Add a test that signs a JWT with `nbf` 1 hour in the future and asserts verify fails. Defense-in-depth:

```rust
#[test]
fn test_verify_rejects_not_yet_valid_nbf_when_present() {
    // optional — only matters if LicenseClaims ever grows nbf
}
```

Lower priority since `nbf` isn't in the current claim shape.

---

_Reviewed: 2026-04-27_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
