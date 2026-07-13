//! AES-256-GCM helpers for device credentials (D-01, D-02).
//!
//! Layout of the stored blob: `base64(nonce || ciphertext_with_tag)`.
//! - `nonce` is 12 bytes (96 bits), generated with `OsRng` per-encrypt.
//! - `ciphertext_with_tag` is AES-256-GCM output INCLUDING the authentication tag.
//! - The tag is verified in constant time by `aes_gcm` on `decrypt`.
//!
//! Security rules (enforced by the module shape):
//! 1. Plaintext device passwords leave this module only via the return value of
//!    `decrypt_password`. Callers MUST hold them on the stack as short-lived
//!    `String`s and never log them.
//! 2. The key is a `[u8; 32]` received by reference; the module does NOT clone
//!    or store it.
//! 3. Tampered ciphertext and wrong keys both yield non-panicking `Err`s.

// Full path `aes_gcm::Aes256Gcm` is referenced below; the `use` wildcards keep
// call-sites terse while the path is visible for grep-based audits.
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
type _Cipher = aes_gcm::Aes256Gcm;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;

/// Encrypt a device password.
///
/// Output: `base64(nonce || ciphertext_with_tag)`.
/// A fresh 12-byte nonce is drawn from `OsRng` for every call; re-encrypting
/// the same plaintext MUST yield a different output string.
pub fn encrypt_password(plaintext: &str, key_bytes: &[u8; 32]) -> Result<String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encrypt failed: {e}"))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(STANDARD.encode(&combined))
}

/// Decrypt a device password previously produced by [`encrypt_password`].
///
/// Returns `Err` (non-panicking) when:
/// - the base64 is malformed
/// - the blob is shorter than 12 bytes (nonce missing)
/// - the AES-GCM tag verification fails (wrong key or tampered ciphertext)
/// - the plaintext is not valid UTF-8
pub fn decrypt_password(encoded: &str, key_bytes: &[u8; 32]) -> Result<String> {
    let combined = STANDARD
        .decode(encoded.as_bytes())
        .context("device password ciphertext is not valid base64")?;

    anyhow::ensure!(
        combined.len() > 12,
        "device password ciphertext is too short to contain a nonce"
    );

    let (nonce_bytes, ciphertext) = combined.split_at(12);

    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("device password decrypt failed (key wrong or tampered)"))?;

    String::from_utf8(plaintext).context("decrypted device password is not valid UTF-8")
}

/// Load a 32-byte key from the named environment variable.
///
/// Used by `Config::from_env()` for `DEVICE_CREDS_KEY` (D-02). Returns `Err` if
/// the variable is missing, not valid base64, or does not decode to exactly 32 bytes.
pub fn load_key_from_env(var: &str) -> Result<[u8; 32]> {
    let raw =
        std::env::var(var).with_context(|| format!("{} environment variable is required", var))?;
    let decoded = STANDARD
        .decode(raw.as_bytes())
        .with_context(|| format!("{} must be valid base64", var))?;
    decoded.as_slice().try_into().map_err(|_| {
        anyhow::anyhow!(
            "{} must decode to exactly 32 bytes (got {} bytes)",
            var,
            decoded.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    /// A deterministic 32-byte key for tests only.
    /// b"01234567...abcdef" base64-encoded is kept in a named const so the
    /// shape is obvious and the test reader can decode it by hand if needed.
    const TEST_KEY_B64: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

    fn test_key() -> [u8; 32] {
        STANDARD
            .decode(TEST_KEY_B64)
            .expect("static test key is valid base64")
            .as_slice()
            .try_into()
            .expect("static test key is 32 bytes")
    }

    #[test]
    fn encrypt_then_decrypt() {
        let key = test_key();
        let ciphertext = encrypt_password("hunter2", &key).expect("encrypt ok");
        let plaintext = decrypt_password(&ciphertext, &key).expect("decrypt ok");
        assert_eq!(plaintext, "hunter2");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let ciphertext = encrypt_password("hunter2", &key).expect("encrypt ok");

        // Flip one byte of the raw blob, then re-encode base64. Flipping the
        // base64 string directly can yield the same bytes after decode.
        let mut raw = STANDARD
            .decode(ciphertext.as_bytes())
            .expect("ours is valid base64");
        assert!(
            raw.len() > 20,
            "fixture must have enough bytes for the target index"
        );
        raw[20] ^= 0x01;
        let tampered = STANDARD.encode(&raw);

        let err = decrypt_password(&tampered, &key);
        assert!(
            err.is_err(),
            "expected decrypt to fail after flipping ciphertext byte 20, got: {:?}",
            err
        );
    }

    #[test]
    fn wrong_key_fails() {
        let key_a = test_key();
        // Different key
        let mut key_b = test_key();
        key_b[0] ^= 0xff;

        let ciphertext = encrypt_password("hunter2", &key_a).expect("encrypt ok");
        let err = decrypt_password(&ciphertext, &key_b);
        assert!(
            err.is_err(),
            "expected decrypt to fail with different key, got Ok"
        );
    }

    #[test]
    fn nonce_is_random() {
        let key = test_key();
        let a = encrypt_password("hunter2", &key).expect("encrypt A ok");
        let b = encrypt_password("hunter2", &key).expect("encrypt B ok");
        assert_ne!(
            a, b,
            "two encrypts of the same plaintext must differ (random nonce)"
        );
    }
}
