//! License service: RS256 JWT verification, DO Functions activation call, and
//! cached-JWT load with anti-cloning fingerprint check.
//!
//! Crypto contract (per RESEARCH § Pattern 4 + D-01/D-02/D-07):
//! - The RS256 public key is embedded at compile time via include_str!. The
//!   private key lives ONLY on DO Functions; rotation requires a recompile.
//! - Algorithm is pinned to RS256 to defeat alg=HS256 confusion attacks.
//! - validate_exp = false (D-07 soft expiry): an expired-but-validly-signed
//!   token still verifies. Renewal is best-effort and never re-gates traffic.
//!
//! Anti-cloning (LIC-05):
//! - activate_license refuses to persist a JWT whose hardware_fingerprint
//!   claim does not match the local fingerprint, BEFORE writing to disk.
//! - load_and_validate_license re-checks at every startup.

use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use super::fingerprint;
use crate::errors::AppError;

/// JWT claims signed by DO Functions on activation. Mirrors RESEARCH § Pattern 4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub license_key: String,
    pub hardware_fingerprint: String,
    pub product: String,
    pub iat: i64,
    pub exp: i64,
}

/// Public key embedded at compile time (D-01, D-02). Recompile to rotate.
const LICENSE_PUBLIC_KEY_PEM: &str = include_str!("pubkey.pem");

static LICENSE_DECODING_KEY: OnceLock<DecodingKey> = OnceLock::new();

fn persist_license_token(jwt_path: &Path, token: &str) -> std::io::Result<()> {
    let parent = jwt_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = jwt_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "license path must name a file",
        )
    })?;
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);

    let result = (|| {
        let mut file = options.open(&tmp_path)?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
        #[cfg(unix)]
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
        drop(file);
        std::fs::rename(&tmp_path, jwt_path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

fn license_decoding_key() -> &'static DecodingKey {
    LICENSE_DECODING_KEY.get_or_init(|| {
        DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY_PEM.as_bytes())
            .expect("License public key is invalid PEM — recompile required")
    })
}

/// Verify a license JWT against the embedded RS256 public key.
/// D-07 soft expiry: validate_exp = false so an expired JWT still verifies.
/// Algorithm pinned to RS256 to defeat alg=HS256 confusion attacks.
///
/// This is the ONLY production entry point and it is not configurable at
/// runtime: the trusted key always comes from `license_decoding_key()`
/// (compile-time `include_str!`, see D-01/D-02 above). There is
/// deliberately no env var or other override here — see
/// `verify_license_jwt_with_key` for why.
pub fn verify_license_jwt(token: &str) -> Result<LicenseClaims, AppError> {
    verify_license_jwt_with_key(token, license_decoding_key())
}

/// Same verification logic as [`verify_license_jwt`], but the caller supplies
/// the `DecodingKey` instead of it being read from the embedded production
/// key (C-06). This exists ONLY so integration tests can sign JWTs with an
/// ephemeral, per-run keypair and verify them without needing a private key
/// that matches `pubkey.pem` — i.e. without committing a real private key to
/// the repo as a test fixture.
///
/// Not wired to any env var, CLI flag, or config value: nothing in the
/// production request path can reach this function with a key other than
/// the embedded one, because `verify_license_jwt` is the only production
/// caller and it always passes `license_decoding_key()`. Widening this to a
/// runtime-selectable key would trade a leaked-key problem for a
/// key-substitution problem — deliberately not done.
pub fn verify_license_jwt_with_key(
    token: &str,
    key: &DecodingKey,
) -> Result<LicenseClaims, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    let data = decode::<LicenseClaims>(token, key, &validation).map_err(|_| AppError::Unlicensed)?;
    Ok(data.claims)
}

// =============================================================================
// Phase 9 D-13: bypass-flag safety — evaluate_bypass pure function
//
// D-13 LOCKED decision: CRONOMETRIX_LICENSE_BYPASS is a test-only flag that
// MUST cause the binary to abort with exit code 2 if set without CRONOMETRIX_E2E.
// This pure function encodes that logic — no side effects, no env reads, no panics.
// Callers (main.rs) read the env vars and pass parsed booleans here.
// Locked by `backend/tests/license_bypass_safety.rs` AND by the unit tests below.
// =============================================================================

/// Decision returned by `evaluate_bypass`.
/// Callers (main.rs) branch on this enum BEFORE calling `load_and_validate_license`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BypassDecision {
    /// Both `CRONOMETRIX_E2E=true` AND `CRONOMETRIX_LICENSE_BYPASS=true` are set:
    /// skip fingerprint validation and mark the system as licensed. TEST/DEV only.
    AllowBypass,
    /// `CRONOMETRIX_LICENSE_BYPASS=true` without `CRONOMETRIX_E2E=true`:
    /// misconfiguration — abort startup with exit code 2.
    AbortMisconfigured,
    /// Neither bypass is set, or only E2E is set: proceed to normal
    /// `load_and_validate_license` path unchanged.
    NormalPath,
}

/// Pure logic — no side effects, no env reads, no I/O, no panics.
/// The caller (main.rs) passes the already-parsed boolean values.
///
/// Truth table (locked by `tests/license_bypass_safety.rs` and inline unit tests):
/// | e2e   | bypass | result              |
/// |-------|--------|---------------------|
/// | true  | true   | AllowBypass         |
/// | false | true   | AbortMisconfigured  |
/// | true  | false  | NormalPath          |
/// | false | false  | NormalPath          |
pub fn evaluate_bypass(e2e: bool, bypass: bool) -> BypassDecision {
    match (e2e, bypass) {
        (true, true) => BypassDecision::AllowBypass,
        (false, true) => BypassDecision::AbortMisconfigured,
        _ => BypassDecision::NormalPath,
    }
}

/// Load the cached JWT, verify signature, then re-compute the local fingerprint
/// and compare against the JWT claim. Returns true ONLY when all checks pass.
/// Returns false (without panic) on any error so the system can boot to /setup
/// for first-run activation.
///
/// Production entry point — always verifies against the embedded key. See
/// [`load_and_validate_license_with_key`] for the test-only variant.
pub async fn load_and_validate_license(jwt_path: &str) -> bool {
    load_and_validate_license_with_key(jwt_path, license_decoding_key()).await
}

/// Same as [`load_and_validate_license`], but the caller supplies the
/// `DecodingKey` (C-06 test seam — see [`verify_license_jwt_with_key`] for
/// the rationale). Not reachable from any production code path.
pub async fn load_and_validate_license_with_key(jwt_path: &str, key: &DecodingKey) -> bool {
    let token = match std::fs::read_to_string(jwt_path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => return false, // first run — file does not exist yet
    };
    if token.is_empty() {
        return false;
    }
    let claims = match verify_license_jwt_with_key(&token, key) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let current_fp = match fingerprint::collect_fingerprint() {
        Ok(fp) => fp,
        Err(e) => {
            tracing::warn!("fingerprint collection failed: {}", e);
            return false;
        }
    };
    if claims.hardware_fingerprint != current_fp {
        tracing::error!("license fingerprint mismatch — hardware may have changed");
        return false;
    }
    true
}

/// Call DO Functions to activate this installation. Persists JWT to disk on
/// success and verifies the returned JWT BEFORE persisting. Returns LicenseClaims
/// or AppError. Used by setup_activate handler in Plan 02.
///
/// Production entry point — always verifies against the embedded key. See
/// [`activate_license_with_key`] for the test-only variant.
pub async fn activate_license(
    license_key: &str,
    do_functions_activate_url: &str,
    jwt_path: &str,
) -> Result<LicenseClaims, AppError> {
    activate_license_with_key(
        license_key,
        do_functions_activate_url,
        jwt_path,
        license_decoding_key(),
    )
    .await
}

/// Same as [`activate_license`], but the caller supplies the `DecodingKey`
/// used to verify the JWT DO Functions returns (C-06 test seam — see
/// [`verify_license_jwt_with_key`] for the rationale). Not reachable from any
/// production code path.
pub async fn activate_license_with_key(
    license_key: &str,
    do_functions_activate_url: &str,
    jwt_path: &str,
    key: &DecodingKey,
) -> Result<LicenseClaims, AppError> {
    if do_functions_activate_url.is_empty() {
        return Err(AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: "License server URL not configured".to_string(),
        });
    }
    let fp = fingerprint::collect_fingerprint().map_err(AppError::Internal)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("build reqwest client: {}", e)))?;

    let resp = client
        .post(do_functions_activate_url)
        .json(&serde_json::json!({
            "license_key": license_key,
            "hardware_fingerprint": fp,
        }))
        .send()
        .await
        .map_err(|_| AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: "Could not reach license server".to_string(),
        })?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::NotFound {
            code: "LICENSE_NOT_FOUND",
            message: "License key not found".to_string(),
        });
    }
    if status == reqwest::StatusCode::CONFLICT {
        return Err(AppError::Conflict {
            code: "ALREADY_ACTIVATED",
            message: "This license is already active on another installation".to_string(),
        });
    }
    if !status.is_success() {
        return Err(AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: format!("License server returned {}", status.as_u16()),
        });
    }

    let body: serde_json::Value = resp.json().await.map_err(|_| AppError::BadGateway {
        code: "ACTIVATION_UNREACHABLE",
        message: "License server returned malformed body".to_string(),
    })?;
    let token = body
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or(AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: "License server response missing token".to_string(),
        })?;

    // VERIFY before persisting. This blocks server-side fingerprint forgery:
    // the server cannot return a JWT we accept unless its claims include OUR fp.
    let claims = verify_license_jwt_with_key(token, key)?;
    if claims.hardware_fingerprint != fp {
        // LIC-05 — anti-cloning at activation time
        return Err(AppError::Forbidden);
    }

    persist_license_token(Path::new(jwt_path), token)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("persist license: {}", e)))?;

    Ok(claims)
}

/// Background task: at startup and every 24h, if the cached JWT is within 30 days
/// of expiry AND DO Functions is configured, attempt a silent renewal. Failures
/// are logged but never block the system (D-08, D-09: offline-first).
///
/// Production entry point — always verifies against the embedded key. See
/// [`renewal_task_with_key`] for the test-only variant.
pub async fn renewal_task(
    license_jwt_path: String,
    do_functions_renew_url: String,
    license_valid: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cancel: tokio_util::sync::CancellationToken,
) {
    renewal_task_with_key(
        license_jwt_path,
        do_functions_renew_url,
        license_valid,
        cancel,
        license_decoding_key(),
    )
    .await
}

/// Same as [`renewal_task`], but the caller supplies the `DecodingKey` used
/// for every renewal cycle's verification (C-06 test seam — see
/// [`verify_license_jwt_with_key`] for the rationale). Not reachable from any
/// production code path.
pub async fn renewal_task_with_key(
    license_jwt_path: String,
    do_functions_renew_url: String,
    license_valid: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cancel: tokio_util::sync::CancellationToken,
    key: &DecodingKey,
) {
    use std::sync::atomic::Ordering;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)) => {
                if !license_valid.load(Ordering::Relaxed) { continue; }
                if do_functions_renew_url.is_empty() { continue; }
                if let Err(e) = try_renew_with_key(&license_jwt_path, &do_functions_renew_url, key).await {
                    tracing::warn!("license renewal attempt failed: {}", e);
                }
            }
        }
    }
}

/// Attempt one best-effort renewal cycle for a cached license.
///
/// This is public so operators and integration tests can exercise the exact
/// renewal transaction independently from the 24-hour scheduler. Callers that
/// want the scheduler should use [`renewal_task`].
///
/// Production entry point — always verifies against the embedded key. See
/// [`try_renew_with_key`] for the test-only variant.
pub async fn try_renew(jwt_path: &str, renew_url: &str) -> Result<(), AppError> {
    try_renew_with_key(jwt_path, renew_url, license_decoding_key()).await
}

/// Same as [`try_renew`], but the caller supplies the `DecodingKey` used to
/// verify both the existing cached JWT and the freshly renewed one (C-06 test
/// seam — see [`verify_license_jwt_with_key`] for the rationale). Not
/// reachable from any production code path.
pub async fn try_renew_with_key(
    jwt_path: &str,
    renew_url: &str,
    key: &DecodingKey,
) -> Result<(), AppError> {
    let token = std::fs::read_to_string(jwt_path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("read license: {}", e)))?;
    let claims = verify_license_jwt_with_key(token.trim(), key)?;

    // D-08: only renew if within 30 days of expiry
    let now = chrono::Utc::now().timestamp();
    let thirty_days = 30 * 24 * 60 * 60;
    if claims.exp - now > thirty_days {
        return Ok(()); // not yet within renewal window
    }

    let fp = fingerprint::collect_fingerprint().map_err(AppError::Internal)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("build client: {}", e)))?;

    let resp = client
        .post(renew_url)
        .json(&serde_json::json!({
            "license_key": claims.license_key,
            "hardware_fingerprint": fp,
        }))
        .send()
        .await
        .map_err(|_| AppError::BadGateway {
            code: "RENEWAL_UNREACHABLE",
            message: "renew endpoint unreachable".to_string(),
        })?;

    if !resp.status().is_success() {
        return Err(AppError::BadGateway {
            code: "RENEWAL_FAILED",
            message: format!("renew returned {}", resp.status()),
        });
    }

    let body: serde_json::Value = resp.json().await.map_err(|_| AppError::BadGateway {
        code: "RENEWAL_FAILED",
        message: "malformed body".into(),
    })?;
    let new_token = body
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or(AppError::BadGateway {
            code: "RENEWAL_FAILED",
            message: "missing token".into(),
        })?;

    // Verify new token before persisting
    let new_claims = verify_license_jwt_with_key(new_token, key)?;
    if new_claims.hardware_fingerprint != fp {
        return Err(AppError::Forbidden);
    }

    persist_license_token(Path::new(jwt_path), new_token)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("persist renewed license: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod evaluate_bypass_tests {
    use super::{evaluate_bypass, BypassDecision};

    #[test]
    fn both_flags_set_allows_bypass() {
        assert_eq!(evaluate_bypass(true, true), BypassDecision::AllowBypass);
    }

    #[test]
    fn bypass_without_e2e_aborts_misconfigured() {
        assert_eq!(
            evaluate_bypass(false, true),
            BypassDecision::AbortMisconfigured
        );
    }

    #[test]
    fn e2e_without_bypass_normal_path() {
        assert_eq!(evaluate_bypass(true, false), BypassDecision::NormalPath);
    }

    #[test]
    fn neither_flag_normal_path() {
        assert_eq!(evaluate_bypass(false, false), BypassDecision::NormalPath);
    }
}

#[cfg(all(test, unix))]
mod persist_license_tests {
    use std::os::unix::fs::PermissionsExt;

    use super::persist_license_token;

    fn assert_owner_only(path: &std::path::Path) {
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn persist_license_token_creates_owner_only_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("license.jwt");

        persist_license_token(&path, "signed-token").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "signed-token");
        assert_owner_only(&path);
    }

    #[test]
    fn persist_license_token_replaces_permissive_file_with_owner_only_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("license.jwt");
        std::fs::write(&path, "old-token").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        persist_license_token(&path, "renewed-token").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "renewed-token");
        assert_owner_only(&path);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
}
