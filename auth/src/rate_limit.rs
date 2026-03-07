//! In-memory sliding window rate limiter.
//!
//! Uses [`dashmap::DashMap`] for thread-safe concurrent access. Each rate limit
//! key tracks a vector of request timestamps, and old entries are cleaned up
//! lazily on each check.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::error::AuthError;

/// Default rate limit windows and thresholds (from design doc).
const REQUEST_OTP_PER_EMAIL_LIMIT: usize = 3;
const REQUEST_OTP_PER_EMAIL_WINDOW: Duration = Duration::from_secs(15 * 60);
const REQUEST_OTP_PER_IP_LIMIT: usize = 10;
const REQUEST_OTP_PER_IP_WINDOW: Duration = Duration::from_secs(15 * 60);
const REQUEST_OTP_GLOBAL_LIMIT: usize = 100;
const REQUEST_OTP_GLOBAL_WINDOW: Duration = Duration::from_secs(60);

const VERIFY_OTP_PER_EMAIL_LIMIT: usize = 5;
const VERIFY_OTP_PER_EMAIL_WINDOW: Duration = Duration::from_secs(15 * 60);
const VERIFY_OTP_PER_IP_LIMIT: usize = 20;
const VERIFY_OTP_PER_IP_WINDOW: Duration = Duration::from_secs(15 * 60);

/// Global key used for the global OTP request rate limit.
const GLOBAL_KEY: &str = "__global__";

/// Thread-safe in-memory rate limiter using sliding windows.
///
/// Tracks request timestamps per key in a [`DashMap`]. Old entries beyond
/// the window are pruned lazily on each check. Designed for single-instance
/// deployments; for multi-instance, use Redis-based rate limiting instead.
pub struct RateLimiter {
    /// Maps rate limit keys to their request timestamps.
    buckets: Arc<DashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    /// Creates a new, empty rate limiter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// Checks rate limits for OTP request endpoints.
    ///
    /// Enforces three limits:
    /// - Per-email: 3 requests per 15 minutes
    /// - Per-IP: 10 requests per 15 minutes
    /// - Global: 100 requests per 1 minute
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::RateLimited`] if any limit is exceeded.
    pub fn check_request_otp(&self, email: &str, ip: &str) -> Result<(), AuthError> {
        let email_key = format!("req_otp:email:{email}");
        self.check(
            &email_key,
            REQUEST_OTP_PER_EMAIL_LIMIT,
            REQUEST_OTP_PER_EMAIL_WINDOW,
        )?;

        let ip_key = format!("req_otp:ip:{ip}");
        self.check(&ip_key, REQUEST_OTP_PER_IP_LIMIT, REQUEST_OTP_PER_IP_WINDOW)?;

        let global_key = format!("req_otp:global:{GLOBAL_KEY}");
        self.check(
            &global_key,
            REQUEST_OTP_GLOBAL_LIMIT,
            REQUEST_OTP_GLOBAL_WINDOW,
        )?;

        Ok(())
    }

    /// Checks rate limits for OTP verification endpoints.
    ///
    /// Enforces two limits:
    /// - Per-email: 5 attempts per 15 minutes
    /// - Per-IP: 20 attempts per 15 minutes
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::RateLimited`] if any limit is exceeded.
    pub fn check_verify_otp(&self, email: &str, ip: &str) -> Result<(), AuthError> {
        let email_key = format!("verify_otp:email:{email}");
        self.check(
            &email_key,
            VERIFY_OTP_PER_EMAIL_LIMIT,
            VERIFY_OTP_PER_EMAIL_WINDOW,
        )?;

        let ip_key = format!("verify_otp:ip:{ip}");
        self.check(&ip_key, VERIFY_OTP_PER_IP_LIMIT, VERIFY_OTP_PER_IP_WINDOW)?;

        Ok(())
    }

    /// Sliding window rate limit check for a single key.
    ///
    /// Prunes entries older than `window`, then checks if the count
    /// (after adding the current request) would exceed `limit`.
    fn check(&self, key: &str, limit: usize, window: Duration) -> Result<(), AuthError> {
        let now = Instant::now();
        let cutoff = now - window;

        let mut entry = self.buckets.entry(key.to_owned()).or_default();
        let timestamps = entry.value_mut();

        // Prune old entries
        timestamps.retain(|t| *t > cutoff);

        if timestamps.len() >= limit {
            return Err(AuthError::RateLimited(
                "Too many requests. Try again later.".to_owned(),
            ));
        }

        timestamps.push(now);
        Ok(())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rate_limiter() {
        let limiter = RateLimiter::new();
        assert!(limiter.buckets.is_empty());
    }

    #[test]
    fn test_default_rate_limiter() {
        let limiter = RateLimiter::default();
        assert!(limiter.buckets.is_empty());
    }

    #[test]
    fn test_check_within_limit() {
        let limiter = RateLimiter::new();
        let result = limiter.check("test_key", 3, Duration::from_secs(60));
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_at_limit() {
        let limiter = RateLimiter::new();
        for _ in 0..3 {
            limiter
                .check("test_key", 3, Duration::from_secs(60))
                .unwrap();
        }
        let result = limiter.check("test_key", 3, Duration::from_secs(60));
        assert!(result.is_err());
        match result {
            Err(AuthError::RateLimited(_)) => {}
            other => panic!("Expected RateLimited, got: {other:?}"),
        }
    }

    #[test]
    fn test_check_different_keys_independent() {
        let limiter = RateLimiter::new();
        for _ in 0..3 {
            limiter.check("key_a", 3, Duration::from_secs(60)).unwrap();
        }
        // key_a is exhausted
        assert!(limiter.check("key_a", 3, Duration::from_secs(60)).is_err());
        // key_b should still work
        assert!(limiter.check("key_b", 3, Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn test_check_expired_entries_pruned() {
        let limiter = RateLimiter::new();
        // Insert entries with a very short window
        for _ in 0..5 {
            limiter
                .check("prune_key", 5, Duration::from_secs(60))
                .unwrap();
        }
        // At limit with 60s window
        assert!(
            limiter
                .check("prune_key", 5, Duration::from_secs(60))
                .is_err()
        );

        // With a zero-length window, all entries are "expired"
        // (cutoff = now, entries are at now or slightly before)
        // Since entries were just added, they might or might not pass with Duration::ZERO.
        // Use a known-past approach instead: insert manually.
        {
            let mut entry = limiter.buckets.entry("manual_key".to_owned()).or_default();
            let past = Instant::now() - Duration::from_secs(120);
            for _ in 0..5 {
                entry.value_mut().push(past);
            }
        }
        // These should all be pruned with a 60s window
        assert!(
            limiter
                .check("manual_key", 5, Duration::from_secs(60))
                .is_ok()
        );
    }

    #[test]
    fn test_check_request_otp_within_limits() {
        let limiter = RateLimiter::new();
        let result = limiter.check_request_otp("user@example.com", "192.168.1.1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_request_otp_email_limit() {
        let limiter = RateLimiter::new();
        for _ in 0..3 {
            limiter
                .check_request_otp("limited@example.com", "10.0.0.1")
                .unwrap();
        }
        let result = limiter.check_request_otp("limited@example.com", "10.0.0.2");
        assert!(
            result.is_err(),
            "4th request from same email must be rate limited"
        );
    }

    #[test]
    fn test_check_request_otp_ip_limit() {
        let limiter = RateLimiter::new();
        for i in 0..10 {
            let email = format!("user{i}@example.com");
            limiter.check_request_otp(&email, "1.2.3.4").unwrap();
        }
        let result = limiter.check_request_otp("user10@example.com", "1.2.3.4");
        assert!(
            result.is_err(),
            "11th request from same IP must be rate limited"
        );
    }

    #[test]
    fn test_check_verify_otp_within_limits() {
        let limiter = RateLimiter::new();
        let result = limiter.check_verify_otp("user@example.com", "192.168.1.1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_verify_otp_email_limit() {
        let limiter = RateLimiter::new();
        for _ in 0..5 {
            limiter
                .check_verify_otp("limited@example.com", "10.0.0.1")
                .unwrap();
        }
        let result = limiter.check_verify_otp("limited@example.com", "10.0.0.2");
        assert!(
            result.is_err(),
            "6th verify from same email must be rate limited"
        );
    }

    #[test]
    fn test_check_verify_otp_ip_limit() {
        let limiter = RateLimiter::new();
        for i in 0..20 {
            let email = format!("user{i}@example.com");
            limiter.check_verify_otp(&email, "5.6.7.8").unwrap();
        }
        let result = limiter.check_verify_otp("user20@example.com", "5.6.7.8");
        assert!(
            result.is_err(),
            "21st verify from same IP must be rate limited"
        );
    }

    #[test]
    fn test_request_and_verify_limits_independent() {
        let limiter = RateLimiter::new();
        // Exhaust request OTP email limit
        for _ in 0..3 {
            limiter
                .check_request_otp("user@example.com", "10.0.0.1")
                .unwrap();
        }
        assert!(
            limiter
                .check_request_otp("user@example.com", "10.0.0.1")
                .is_err()
        );

        // Verify OTP should still work for the same email (different bucket keys)
        assert!(
            limiter
                .check_verify_otp("user@example.com", "10.0.0.1")
                .is_ok()
        );
    }

    #[test]
    fn test_global_rate_limit() {
        let limiter = RateLimiter::new();
        for i in 0..100 {
            let email = format!("user{i}@example.com");
            let ip = format!("10.0.{}.{}", i / 256, i % 256);
            limiter.check_request_otp(&email, &ip).unwrap();
        }
        // 101st request should hit global limit
        let result = limiter.check_request_otp("extra@example.com", "10.1.0.1");
        assert!(result.is_err(), "101st global request must be rate limited");
    }

    #[test]
    fn test_exactly_at_limit_boundary() {
        let limiter = RateLimiter::new();
        let limit = 5;
        let window = Duration::from_secs(60);

        // Exactly N requests should succeed
        for i in 0..limit {
            let result = limiter.check("boundary_key", limit, window);
            assert!(
                result.is_ok(),
                "Request {i} should succeed (under limit {limit})"
            );
        }
        // The N+1th should fail
        let result = limiter.check("boundary_key", limit, window);
        assert!(result.is_err(), "Request at position {limit} must fail");
    }

    #[test]
    fn test_email_and_ip_scopes_independent() {
        let limiter = RateLimiter::new();
        // Exhaust email limit for request_otp
        for _ in 0..3 {
            limiter
                .check_request_otp("scope@example.com", "10.0.0.1")
                .unwrap();
        }
        // Email scope exhausted
        assert!(
            limiter
                .check_request_otp("scope@example.com", "10.0.0.99")
                .is_err(),
            "Email scope should be exhausted"
        );

        // But a different email from the same IP should still work
        assert!(
            limiter
                .check_request_otp("other@example.com", "10.0.0.1")
                .is_ok(),
            "Different email should be independent"
        );
    }

    #[test]
    fn test_window_expiry_allows_new_requests() {
        let limiter = RateLimiter::new();
        let key = "expiry_test";

        // Manually insert entries in the past (beyond the window)
        {
            let mut entry = limiter.buckets.entry(key.to_owned()).or_default();
            let past = Instant::now() - Duration::from_secs(200);
            for _ in 0..10 {
                entry.value_mut().push(past);
            }
        }

        // With a 60s window, all past entries should be pruned
        let result = limiter.check(key, 5, Duration::from_secs(60));
        assert!(
            result.is_ok(),
            "After window expiry, requests should succeed again"
        );
    }

    #[test]
    fn test_concurrent_access_no_panic() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(RateLimiter::new());
        let mut handles = vec![];

        for t in 0..10 {
            let limiter_clone = Arc::clone(&limiter);
            handles.push(thread::spawn(move || {
                for i in 0..50 {
                    let key = format!("concurrent_key_{t}");
                    // Ignore result -- we just want to ensure no panics
                    let _ = limiter_clone.check(&key, 100, Duration::from_secs(60));
                    let _ = limiter_clone
                        .check_request_otp(&format!("user{i}@t{t}.com"), &format!("10.{t}.0.{i}"));
                    let _ = limiter_clone
                        .check_verify_otp(&format!("user{i}@t{t}.com"), &format!("10.{t}.0.{i}"));
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread must not panic");
        }
    }

    #[test]
    fn test_global_limit_independent_of_per_key() {
        let limiter = RateLimiter::new();

        // Per-email limit is 3 for request_otp, global is 100.
        // Use 3 requests from one email (hitting email limit), then use different emails.
        // The global counter should have all of them.
        for _ in 0..3 {
            limiter
                .check_request_otp("first@example.com", "10.0.0.1")
                .unwrap();
        }
        // first@example.com is email-limited now
        assert!(
            limiter
                .check_request_otp("first@example.com", "10.0.0.99")
                .is_err()
        );

        // But global counter only has 3 so far, should allow others
        assert!(
            limiter
                .check_request_otp("second@example.com", "10.0.0.2")
                .is_ok(),
            "Global limit not hit yet"
        );
    }

    #[test]
    fn test_verify_otp_has_different_limits_than_request_otp() {
        let limiter = RateLimiter::new();
        let email = "limits@example.com";
        let ip = "10.0.0.1";

        // request_otp email limit is 3, verify_otp email limit is 5
        // Exhaust request_otp limit
        for _ in 0..3 {
            limiter.check_request_otp(email, ip).unwrap();
        }
        assert!(limiter.check_request_otp(email, ip).is_err());

        // verify_otp should still allow 5 requests (separate key namespace)
        for _ in 0..5 {
            limiter.check_verify_otp(email, ip).unwrap();
        }
        assert!(
            limiter.check_verify_otp(email, ip).is_err(),
            "verify_otp email limit should be 5"
        );
    }

    #[test]
    fn test_rate_limited_error_variant() {
        let limiter = RateLimiter::new();
        for _ in 0..3 {
            limiter
                .check("err_key", 3, Duration::from_secs(60))
                .unwrap();
        }
        let result = limiter.check("err_key", 3, Duration::from_secs(60));
        match result {
            Err(AuthError::RateLimited(msg)) => {
                assert!(
                    msg.contains("Too many requests"),
                    "Error message should say 'Too many requests', got: {msg}"
                );
            }
            other => panic!("Expected RateLimited, got: {other:?}"),
        }
    }
}
