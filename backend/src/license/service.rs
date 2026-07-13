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

fn license_decoding_key() -> &'static DecodingKey {
    LICENSE_DECODING_KEY.get_or_init(|| {
        DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY_PEM.as_bytes())
            .expect("License public key is invalid PEM — recompile required")
    })
}

/// Verify a license JWT against the embedded RS256 public key.
/// D-07 soft expiry: validate_exp = false so an expired JWT still verifies.
/// Algorithm pinned to RS256 to defeat alg=HS256 confusion attacks.
pub fn verify_license_jwt(token: &str) -> Result<LicenseClaims, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    let data = decode::<LicenseClaims>(token, license_decoding_key(), &validation)
        .map_err(|_| AppError::Unlicensed)?;
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
pub async fn load_and_validate_license(jwt_path: &str) -> bool {
    let token = match std::fs::read_to_string(jwt_path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => return false, // first run — file does not exist yet
    };
    if token.is_empty() {
        return false;
    }
    let claims = match verify_license_jwt(&token) {
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
pub async fn activate_license(
    license_key: &str,
    do_functions_activate_url: &str,
    jwt_path: &str,
) -> Result<LicenseClaims, AppError> {
    if do_functions_activate_url.is_empty() {
        return Err(AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: "License server URL not configured".to_string(),
        });
    }
    let fp = fingerprint::collect_fingerprint().map_err(|e| AppError::Internal(e.into()))?;

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
    let claims = verify_license_jwt(token)?;
    if claims.hardware_fingerprint != fp {
        // LIC-05 — anti-cloning at activation time
        return Err(AppError::Forbidden);
    }

    // Atomically write to disk via temp + rename
    let tmp = format!("{}.tmp", jwt_path);
    std::fs::write(&tmp, token)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("write license tmp: {}", e)))?;
    std::fs::rename(&tmp, jwt_path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rename license file: {}", e)))?;

    Ok(claims)
}

/// Background task: at startup and every 24h, if the cached JWT is within 30 days
/// of expiry AND DO Functions is configured, attempt a silent renewal. Failures
/// are logged but never block the system (D-08, D-09: offline-first).
pub async fn renewal_task(
    license_jwt_path: String,
    do_functions_renew_url: String,
    license_valid: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cancel: tokio_util::sync::CancellationToken,
) {
    use std::sync::atomic::Ordering;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)) => {
                if !license_valid.load(Ordering::Relaxed) { continue; }
                if do_functions_renew_url.is_empty() { continue; }
                if let Err(e) = try_renew(&license_jwt_path, &do_functions_renew_url).await {
                    tracing::warn!("license renewal attempt failed: {}", e);
                }
            }
        }
    }
}

async fn try_renew(jwt_path: &str, renew_url: &str) -> Result<(), AppError> {
    let token = std::fs::read_to_string(jwt_path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("read license: {}", e)))?;
    let claims = verify_license_jwt(token.trim())?;

    // D-08: only renew if within 30 days of expiry
    let now = chrono::Utc::now().timestamp();
    let thirty_days = 30 * 24 * 60 * 60;
    if claims.exp - now > thirty_days {
        return Ok(()); // not yet within renewal window
    }

    let fp = fingerprint::collect_fingerprint().map_err(|e| AppError::Internal(e.into()))?;

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
    let new_claims = verify_license_jwt(new_token)?;
    if new_claims.hardware_fingerprint != fp {
        return Err(AppError::Forbidden);
    }

    let tmp = format!("{}.tmp", jwt_path);
    std::fs::write(&tmp, new_token)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("write tmp: {}", e)))?;
    std::fs::rename(&tmp, jwt_path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rename: {}", e)))?;
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
