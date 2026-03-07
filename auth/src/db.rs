//! Database query functions for the auth service.
//!
//! Provides operations against the `allowed_emails` and `otp_requests` tables.
//! All queries use runtime-checked SQLx (not compile-time macros) to avoid
//! requiring a live database connection at build time.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AuthError;

/// A row from the `otp_requests` table representing an active OTP.
#[derive(Debug, Clone)]
pub struct OtpRecord {
    /// Unique identifier for this OTP request.
    pub id: Uuid,
    /// Email address the OTP was sent to.
    pub email: String,
    /// Argon2id hash of the OTP code.
    pub otp_hash: String,
    /// Number of verification attempts made so far.
    pub attempts: i16,
    /// Maximum allowed verification attempts.
    pub max_attempts: i16,
    /// When this OTP expires.
    pub expires_at: DateTime<Utc>,
}

/// Checks if an email address is in the allowed list.
///
/// Returns the role (e.g., "viewer", "admin") if the email is allowed,
/// or `None` if the email is not in the allowed list.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the query fails.
pub async fn check_email_allowed(
    pool: &PgPool,
    email: &str,
) -> Result<Option<(String,)>, AuthError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT role FROM allowed_emails WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Inserts a new OTP request into the database.
///
/// Stores the Argon2id hash of the OTP (never the plaintext) along with
/// the expiration time. Returns the UUID of the newly created record.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the insert fails.
pub async fn insert_otp(
    pool: &PgPool,
    email: &str,
    otp_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, AuthError> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO otp_requests (email, otp_hash, expires_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(email)
    .bind(otp_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Fetches the most recent active (unconsumed, unexpired) OTP for an email.
///
/// Returns `None` if there is no active OTP for the given email address.
/// An OTP is considered active if it has not been consumed and has not expired.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the query fails.
pub async fn get_active_otp(pool: &PgPool, email: &str) -> Result<Option<OtpRecord>, AuthError> {
    let row = sqlx::query_as::<_, (Uuid, String, String, i16, i16, DateTime<Utc>)>(
        "SELECT id, email, otp_hash, attempts, max_attempts, expires_at \
         FROM otp_requests \
         WHERE email = $1 AND consumed = FALSE AND expires_at > NOW() \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, email, otp_hash, attempts, max_attempts, expires_at)| OtpRecord {
            id,
            email,
            otp_hash,
            attempts,
            max_attempts,
            expires_at,
        },
    ))
}

/// Increments the attempt counter for an OTP request.
///
/// Called each time a user submits an incorrect OTP code.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the update fails.
pub async fn increment_attempts(pool: &PgPool, otp_id: Uuid) -> Result<(), AuthError> {
    sqlx::query("UPDATE otp_requests SET attempts = attempts + 1 WHERE id = $1")
        .bind(otp_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Marks an OTP as consumed (used successfully).
///
/// A consumed OTP cannot be used again, preventing replay attacks.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the update fails.
pub async fn consume_otp(pool: &PgPool, otp_id: Uuid) -> Result<(), AuthError> {
    sqlx::query("UPDATE otp_requests SET consumed = TRUE WHERE id = $1")
        .bind(otp_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Invalidates all active OTPs for an email address.
///
/// Used when rate limits are exceeded to force the user to request a new OTP.
/// Marks all unconsumed, unexpired OTPs for the email as consumed.
///
/// # Errors
///
/// Returns [`AuthError::Database`] if the update fails.
pub async fn invalidate_all_for_email(pool: &PgPool, email: &str) -> Result<(), AuthError> {
    sqlx::query(
        "UPDATE otp_requests SET consumed = TRUE \
         WHERE email = $1 AND consumed = FALSE AND expires_at > NOW()",
    )
    .bind(email)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otp_record_debug() {
        let record = OtpRecord {
            id: Uuid::nil(),
            email: "test@example.com".to_owned(),
            otp_hash: "$argon2id$...".to_owned(),
            attempts: 0,
            max_attempts: 3,
            expires_at: Utc::now(),
        };
        let debug = format!("{record:?}");
        assert!(debug.contains("test@example.com"));
    }

    #[test]
    fn test_otp_record_clone() {
        let record = OtpRecord {
            id: Uuid::new_v4(),
            email: "test@example.com".to_owned(),
            otp_hash: "hash".to_owned(),
            attempts: 1,
            max_attempts: 3,
            expires_at: Utc::now(),
        };
        let cloned = record.clone();
        assert_eq!(cloned.id, record.id);
        assert_eq!(cloned.email, record.email);
        assert_eq!(cloned.attempts, record.attempts);
    }

    // Note: Integration tests for DB functions require a live PostgreSQL instance.
    // They are located in tests/db_integration.rs and run with --features integration.
    // The unit tests here verify the data structures compile and derive correctly.

    #[test]
    fn test_otp_record_fields() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let record = OtpRecord {
            id,
            email: "alice@example.com".to_owned(),
            otp_hash: "hash_value".to_owned(),
            attempts: 2,
            max_attempts: 3,
            expires_at: now,
        };

        assert_eq!(record.id, id);
        assert_eq!(record.email, "alice@example.com");
        assert_eq!(record.otp_hash, "hash_value");
        assert_eq!(record.attempts, 2);
        assert_eq!(record.max_attempts, 3);
        assert_eq!(record.expires_at, now);
    }

    #[test]
    fn test_otp_record_default_values() {
        let record = OtpRecord {
            id: Uuid::nil(),
            email: String::new(),
            otp_hash: String::new(),
            attempts: 0,
            max_attempts: 0,
            expires_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        };
        assert_eq!(record.id, Uuid::nil());
        assert!(record.email.is_empty());
        assert_eq!(record.attempts, 0);
    }

    #[test]
    fn test_otp_record_clone_is_independent() {
        let record = OtpRecord {
            id: Uuid::new_v4(),
            email: "orig@example.com".to_owned(),
            otp_hash: "hash".to_owned(),
            attempts: 1,
            max_attempts: 3,
            expires_at: Utc::now(),
        };
        let mut cloned = record.clone();
        cloned.attempts = 5;
        cloned.email = "changed@example.com".to_owned();

        // Original should be unchanged
        assert_eq!(record.attempts, 1);
        assert_eq!(record.email, "orig@example.com");
        assert_eq!(cloned.attempts, 5);
        assert_eq!(cloned.email, "changed@example.com");
    }

    #[test]
    fn test_otp_record_debug_contains_all_fields() {
        let record = OtpRecord {
            id: Uuid::nil(),
            email: "debug@example.com".to_owned(),
            otp_hash: "$argon2id$hash".to_owned(),
            attempts: 2,
            max_attempts: 5,
            expires_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        let debug = format!("{record:?}");
        assert!(debug.contains("debug@example.com"));
        assert!(debug.contains("argon2id"));
        assert!(debug.contains("attempts"));
        assert!(debug.contains("max_attempts"));
    }
}
