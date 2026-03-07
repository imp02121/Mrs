//! OTP generation, hashing, and verification.
//!
//! Uses cryptographically secure random number generation for OTP creation
//! and Argon2id for hashing stored OTPs.

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::Rng;
use rand::rngs::OsRng;

use crate::error::AuthError;

/// Generates a cryptographically secure 6-digit OTP.
///
/// Returns a zero-padded string (e.g., "004821", "847291").
/// Uses `OsRng` via `rand::thread_rng()` for cryptographic security.
#[must_use]
pub fn generate_otp() -> String {
    let code: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{code:06}")
}

/// Hashes an OTP using Argon2id with a random salt.
///
/// The resulting hash string contains the algorithm parameters, salt, and hash,
/// following the PHC string format. This is safe to store in the database.
///
/// # Errors
///
/// Returns [`AuthError::Internal`] if the hashing operation fails.
pub fn hash_otp(otp: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(otp.as_bytes(), &salt)
        .map_err(|e| AuthError::Internal(format!("failed to hash OTP: {e}")))?;
    Ok(hash.to_string())
}

/// Verifies a plaintext OTP against an Argon2id hash.
///
/// Returns `true` if the OTP matches the stored hash, `false` otherwise.
///
/// # Errors
///
/// Returns [`AuthError::Internal`] if the hash string is malformed
/// and cannot be parsed.
pub fn verify_otp(otp: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AuthError::Internal(format!("failed to parse OTP hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(otp.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_otp_length() {
        let otp = generate_otp();
        assert_eq!(otp.len(), 6, "OTP must be exactly 6 characters");
    }

    #[test]
    fn test_generate_otp_is_numeric() {
        let otp = generate_otp();
        assert!(
            otp.chars().all(|c| c.is_ascii_digit()),
            "OTP must contain only digits, got: {otp}"
        );
    }

    #[test]
    fn test_generate_otp_zero_padded() {
        // Generate many OTPs and verify they are always 6 chars.
        // Statistically, some should start with 0.
        for _ in 0..100 {
            let otp = generate_otp();
            assert_eq!(otp.len(), 6);
        }
    }

    #[test]
    fn test_generate_otp_randomness() {
        let otp1 = generate_otp();
        let otp2 = generate_otp();
        // Extremely unlikely (1 in 1M) that two consecutive OTPs are the same
        // This test just verifies we're not returning a constant
        // We accept the tiny chance of false failure
        let otp3 = generate_otp();
        assert!(
            otp1 != otp2 || otp1 != otp3,
            "OTPs should not all be identical"
        );
    }

    #[test]
    fn test_hash_otp_produces_argon2_hash() {
        let hash = hash_otp("123456").unwrap();
        assert!(
            hash.starts_with("$argon2"),
            "Hash must be in Argon2 PHC format, got: {hash}"
        );
    }

    #[test]
    fn test_hash_otp_different_salts() {
        let hash1 = hash_otp("123456").unwrap();
        let hash2 = hash_otp("123456").unwrap();
        assert_ne!(
            hash1, hash2,
            "Same OTP hashed twice must produce different hashes (different salts)"
        );
    }

    #[test]
    fn test_verify_otp_correct() {
        let otp = "847291";
        let hash = hash_otp(otp).unwrap();
        assert!(verify_otp(otp, &hash).unwrap(), "Correct OTP must verify");
    }

    #[test]
    fn test_verify_otp_incorrect() {
        let hash = hash_otp("123456").unwrap();
        assert!(
            !verify_otp("654321", &hash).unwrap(),
            "Wrong OTP must not verify"
        );
    }

    #[test]
    fn test_verify_otp_invalid_hash() {
        let result = verify_otp("123456", "not-a-valid-hash");
        assert!(result.is_err(), "Invalid hash format must return error");
    }

    #[test]
    fn test_hash_and_verify_roundtrip() {
        let otp = generate_otp();
        let hash = hash_otp(&otp).unwrap();
        assert!(
            verify_otp(&otp, &hash).unwrap(),
            "Generated OTP must verify against its own hash"
        );
    }

    #[test]
    fn test_generate_otp_is_exactly_6_chars() {
        // Generate many OTPs and ensure every single one is exactly 6 chars
        for _ in 0..200 {
            let otp = generate_otp();
            assert_eq!(
                otp.len(),
                6,
                "Every OTP must be exactly 6 characters, got: {otp}"
            );
        }
    }

    #[test]
    fn test_generate_otp_preserves_leading_zeros() {
        // Generate enough OTPs that statistically some should start with '0'.
        // With 1000 samples, ~100 should start with '0' (10% chance each).
        let mut found_leading_zero = false;
        for _ in 0..1000 {
            let otp = generate_otp();
            assert_eq!(otp.len(), 6, "OTP length must always be 6");
            if otp.starts_with('0') {
                found_leading_zero = true;
                // Verify leading zero is preserved as a character, not stripped
                assert_eq!(otp.len(), 6, "Leading zero OTP must still be 6 chars");
                break;
            }
        }
        assert!(
            found_leading_zero,
            "After 1000 OTPs, at least one should start with '0'"
        );
    }

    #[test]
    fn test_hash_otp_produces_different_hashes_for_same_input() {
        // Argon2 with random salts must produce different hashes for identical input
        let hashes: Vec<String> = (0..5).map(|_| hash_otp("999999").unwrap()).collect();
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(
                    hashes[i], hashes[j],
                    "Same OTP must produce different hashes due to unique salts"
                );
            }
        }
    }

    #[test]
    fn test_verify_otp_empty_string_returns_false() {
        let hash = hash_otp("123456").unwrap();
        assert!(
            !verify_otp("", &hash).unwrap(),
            "Empty string must not verify against any hash"
        );
    }

    #[test]
    fn test_verify_otp_partial_otp_returns_false() {
        let hash = hash_otp("123456").unwrap();
        // Partial OTPs must not verify
        assert!(
            !verify_otp("123", &hash).unwrap(),
            "3-digit partial must fail"
        );
        assert!(
            !verify_otp("12345", &hash).unwrap(),
            "5-digit partial must fail"
        );
        assert!(
            !verify_otp("1234567", &hash).unwrap(),
            "7-digit string must fail"
        );
    }

    #[test]
    fn test_hash_otp_non_empty() {
        let hash = hash_otp("000000").unwrap();
        assert!(!hash.is_empty(), "Hash must not be empty");
        assert!(
            hash.len() > 20,
            "Argon2 hash should be substantial in length"
        );
    }

    #[test]
    fn test_verify_otp_case_sensitivity() {
        // OTPs are numeric, but if someone passes letters they should fail
        let hash = hash_otp("123456").unwrap();
        assert!(
            !verify_otp("abcdef", &hash).unwrap(),
            "Alphabetic string must not match numeric OTP"
        );
    }
}
