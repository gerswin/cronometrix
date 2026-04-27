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

use crate::errors::AppError;
use super::fingerprint;

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
    let fp = fingerprint::collect_fingerprint()
        .map_err(|e| AppError::Internal(e.into()))?;

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

    let body: serde_json::Value = resp.json().await
        .map_err(|_| AppError::BadGateway {
            code: "ACTIVATION_UNREACHABLE",
            message: "License server returned malformed body".to_string(),
        })?;
    let token = body.get("token")
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
