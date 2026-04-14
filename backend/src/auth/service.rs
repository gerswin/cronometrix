use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sha2::{Digest, Sha256};

use crate::errors::AppError;

use super::models::{Claims, Role};

/// Hash a plaintext password using Argon2id (via password-auth crate).
/// Per D-08: min 8 chars enforced at the API layer, not here.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    Ok(password_auth::generate_hash(password))
}

/// Verify a plaintext password against an Argon2id hash.
/// Returns Unauthorized on mismatch — timing-safe comparison per T-01-05.
pub fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    password_auth::verify_password(password, hash).map_err(|_| AppError::Unauthorized)
}

/// Issue a short-lived access JWT (20 minute expiry). token_type = "access".
pub fn issue_access_token(user_id: &str, role: &Role, secret: &[u8]) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.clone(),
        exp: now + 20 * 60, // 20 minutes
        iat: now,
        token_type: "access".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to issue access token: {}", e)))
}

/// Issue a long-lived refresh JWT (7 day expiry). token_type = "refresh".
/// The caller stores SHA-256(token) in the DB, not the raw token.
pub fn issue_refresh_token(user_id: &str, role: &Role, secret: &[u8]) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.clone(),
        exp: now + 7 * 24 * 60 * 60, // 7 days
        iat: now,
        token_type: "refresh".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to issue refresh token: {}", e)))
}

/// Verify and decode an access token. Rejects expired tokens and wrong token_type.
pub fn verify_access_token(token: &str, secret: &[u8]) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized)?;

    if token_data.claims.token_type != "access" {
        return Err(AppError::Unauthorized);
    }

    Ok(token_data.claims)
}

/// Verify and decode a refresh token. Rejects expired tokens and wrong token_type.
pub fn verify_refresh_token(token: &str, secret: &[u8]) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized)?;

    if token_data.claims.token_type != "refresh" {
        return Err(AppError::Unauthorized);
    }

    Ok(token_data.claims)
}

/// SHA-256 hash of a refresh token string for safe storage in the DB.
/// Per T-01-10: store hash not raw token to prevent theft from DB dump.
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
