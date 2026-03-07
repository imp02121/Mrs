//! JWT token creation and validation.
//!
//! Issues HS256 JWTs with 24-hour expiry containing the user's email and role.
//! Tokens are validated by both the auth service and the engine API middleware.

use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// JWT expiry duration in seconds (24 hours).
const TOKEN_EXPIRY_SECS: i64 = 24 * 60 * 60;

/// JWT claims payload.
///
/// Embedded in every issued token. The `sub` field contains the user's email,
/// and `role` indicates their access level ("viewer" or "admin").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Subject -- the authenticated user's email address.
    pub sub: String,
    /// User role ("viewer" or "admin").
    pub role: String,
    /// Expiration time (UTC unix timestamp).
    pub exp: usize,
    /// Issued at time (UTC unix timestamp).
    pub iat: usize,
}

/// Creates a signed JWT for the given user.
///
/// The token is signed with HS256 using the provided secret and expires
/// 24 hours after issuance.
///
/// # Arguments
///
/// * `email` - The user's email address (becomes the `sub` claim)
/// * `role` - The user's role ("viewer" or "admin")
/// * `secret` - The HS256 signing secret (must be at least 32 bytes)
///
/// # Errors
///
/// Returns [`AuthError::Internal`] if token encoding fails.
pub fn create_token(email: &str, role: &str, secret: &str) -> Result<String, AuthError> {
    let now = Utc::now();
    let exp = (now.timestamp() + TOKEN_EXPIRY_SECS) as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: email.to_owned(),
        role: role.to_owned(),
        exp,
        iat,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AuthError::Internal(format!("failed to create JWT: {e}")))
}

/// Validates a JWT and returns the decoded claims.
///
/// Verifies the HS256 signature, checks expiration, and extracts claims.
///
/// # Arguments
///
/// * `token` - The JWT string to validate
/// * `secret` - The HS256 signing secret used when the token was created
///
/// # Errors
///
/// Returns [`AuthError::Unauthorized`] if the token is invalid, expired,
/// or the signature does not match.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| AuthError::Unauthorized(format!("invalid token: {e}")))?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-that-is-at-least-32-bytes-long";

    #[test]
    fn test_create_token_returns_jwt_string() {
        let token = create_token("user@example.com", "viewer", TEST_SECRET).unwrap();
        assert!(!token.is_empty(), "Token must not be empty");
        // JWT has 3 dot-separated parts
        assert_eq!(
            token.split('.').count(),
            3,
            "JWT must have 3 parts separated by dots"
        );
    }

    #[test]
    fn test_create_and_validate_roundtrip() {
        let token = create_token("alice@example.com", "admin", TEST_SECRET).unwrap();
        let claims = validate_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, "alice@example.com");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_validate_token_wrong_secret() {
        let token = create_token("user@example.com", "viewer", TEST_SECRET).unwrap();
        let result = validate_token(&token, "wrong-secret-that-is-also-long-enough");

        assert!(result.is_err(), "Wrong secret must fail validation");
        match result {
            Err(AuthError::Unauthorized(_)) => {}
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    #[test]
    fn test_validate_token_malformed() {
        let result = validate_token("not.a.jwt", TEST_SECRET);
        assert!(result.is_err(), "Malformed token must fail validation");
    }

    #[test]
    fn test_validate_token_empty() {
        let result = validate_token("", TEST_SECRET);
        assert!(result.is_err(), "Empty token must fail validation");
    }

    #[test]
    fn test_claims_exp_is_24h_from_iat() {
        let token = create_token("user@example.com", "viewer", TEST_SECRET).unwrap();
        let claims = validate_token(&token, TEST_SECRET).unwrap();

        let diff = claims.exp - claims.iat;
        assert_eq!(diff, 86400, "Token expiry must be 24 hours (86400 seconds)");
    }

    #[test]
    fn test_claims_iat_is_recent() {
        let token = create_token("user@example.com", "viewer", TEST_SECRET).unwrap();
        let claims = validate_token(&token, TEST_SECRET).unwrap();

        let now = Utc::now().timestamp() as usize;
        // iat should be within 5 seconds of now
        assert!(
            now.abs_diff(claims.iat) < 5,
            "iat should be approximately now"
        );
    }

    #[test]
    fn test_different_users_different_tokens() {
        let token1 = create_token("alice@example.com", "admin", TEST_SECRET).unwrap();
        let token2 = create_token("bob@example.com", "viewer", TEST_SECRET).unwrap();
        assert_ne!(token1, token2, "Different users must get different tokens");
    }

    #[test]
    fn test_validate_expired_token() {
        // Manually create a token that expired in the past
        let claims = Claims {
            sub: "user@example.com".to_owned(),
            role: "viewer".to_owned(),
            exp: 0, // epoch = expired
            iat: 0,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = validate_token(&token, TEST_SECRET);
        assert!(result.is_err(), "Expired token must fail validation");
    }

    #[test]
    fn test_token_with_empty_secret_works() {
        // HS256 allows any-length secret; empty secret should still produce a valid token
        let result = create_token("user@example.com", "viewer", "");
        assert!(result.is_ok(), "Empty secret should still create a token");

        let token = result.unwrap();
        let claims = validate_token(&token, "").unwrap();
        assert_eq!(claims.sub, "user@example.com");
    }

    #[test]
    fn test_claims_roundtrip_preserves_all_fields() {
        let token = create_token("roundtrip@example.com", "admin", TEST_SECRET).unwrap();
        let claims = validate_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, "roundtrip@example.com", "sub must be preserved");
        assert_eq!(claims.role, "admin", "role must be preserved");
        assert!(claims.exp > 0, "exp must be set");
        assert!(claims.iat > 0, "iat must be set");
        assert_eq!(
            claims.exp - claims.iat,
            86400,
            "exp - iat must be exactly 24h"
        );
    }

    #[test]
    fn test_expired_token_returns_unauthorized() {
        let claims = Claims {
            sub: "expired@example.com".to_owned(),
            role: "viewer".to_owned(),
            exp: 1, // very far in the past
            iat: 0,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = validate_token(&token, TEST_SECRET);
        match result {
            Err(AuthError::Unauthorized(msg)) => {
                assert!(msg.contains("invalid token"), "Error message: {msg}");
            }
            other => panic!("Expected Unauthorized for expired token, got: {other:?}"),
        }
    }

    #[test]
    fn test_tampered_payload_rejected() {
        let token = create_token("user@example.com", "viewer", TEST_SECRET).unwrap();

        // JWT has 3 parts: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);

        // Tamper with the payload (flip a character)
        let mut tampered_payload = parts[1].to_owned();
        if tampered_payload.ends_with('A') {
            tampered_payload.push('B');
        } else {
            tampered_payload.push('A');
        }

        let tampered_token = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);
        let result = validate_token(&tampered_token, TEST_SECRET);
        assert!(result.is_err(), "Tampered token must fail validation");
    }

    #[test]
    fn test_very_long_email_and_role() {
        let long_email = format!("{}@example.com", "a".repeat(500));
        let long_role = "x".repeat(500);

        let token = create_token(&long_email, &long_role, TEST_SECRET).unwrap();
        let claims = validate_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, long_email);
        assert_eq!(claims.role, long_role);
    }

    #[test]
    fn test_validate_garbage_string_returns_unauthorized() {
        let garbage_inputs = [
            "completely-random-garbage",
            "abc.def.ghi",
            "🔥.💧.🌊",
            "",
            "   ",
            "eyJ.eyJ.sig.extra",
        ];

        for garbage in &garbage_inputs {
            let result = validate_token(garbage, TEST_SECRET);
            match result {
                Err(AuthError::Unauthorized(_)) => {}
                other => {
                    panic!("Expected Unauthorized for garbage input '{garbage}', got: {other:?}")
                }
            }
        }
    }

    #[test]
    fn test_claims_serde_roundtrip() {
        let claims = Claims {
            sub: "serde@example.com".to_owned(),
            role: "admin".to_owned(),
            exp: 1_700_000_000,
            iat: 1_699_913_600,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let deserialized: Claims = serde_json::from_str(&json).unwrap();

        assert_eq!(claims, deserialized, "Claims must survive serde roundtrip");
    }

    #[test]
    fn test_validate_token_with_wrong_secret_returns_unauthorized() {
        let token = create_token(
            "user@example.com",
            "viewer",
            "secret-one-is-long-enough-32b",
        )
        .unwrap();
        let result = validate_token(&token, "secret-two-is-long-enough-32b");
        match result {
            Err(AuthError::Unauthorized(_)) => {}
            other => panic!("Expected Unauthorized with wrong secret, got: {other:?}"),
        }
    }
}
