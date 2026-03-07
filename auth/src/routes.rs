//! Axum route handlers for the authentication service.
//!
//! Provides four endpoints:
//! - `POST /auth/request-otp` -- request an OTP code via email
//! - `POST /auth/verify-otp` -- verify an OTP and receive a JWT
//! - `GET /auth/me` -- get the authenticated user's info from the JWT
//! - `POST /auth/logout` -- no-op logout (stateless JWT)
//!
//! All endpoints produce JSON responses with consistent error formatting
//! via [`AuthError`].

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::email;
use crate::error::AuthError;
use crate::jwt;
use crate::otp;
use crate::rate_limit::RateLimiter;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,
    /// In-memory rate limiter.
    pub rate_limiter: Arc<RateLimiter>,
    /// JWT signing secret.
    pub jwt_secret: String,
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request body for `POST /auth/request-otp`.
#[derive(Debug, Deserialize)]
pub struct RequestOtpBody {
    /// The email address to send the OTP to.
    pub email: String,
}

/// Request body for `POST /auth/verify-otp`.
#[derive(Debug, Deserialize)]
pub struct VerifyOtpBody {
    /// The email address that received the OTP.
    pub email: String,
    /// The 6-digit OTP code.
    pub otp: String,
}

/// Successful response from `POST /auth/verify-otp`.
#[derive(Debug, Serialize)]
pub struct VerifyOtpResponse {
    /// The JWT access token.
    pub token: String,
    /// ISO 8601 UTC timestamp when the token expires.
    pub expires_at: String,
}

/// Successful response from `GET /auth/me`.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    /// The authenticated user's email address.
    pub email: String,
    /// The user's role ("viewer" or "admin").
    pub role: String,
}

/// Generic message response.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    /// Human-readable message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Builds the auth router with all endpoints.
///
/// Mounts the following routes:
/// - `POST /auth/request-otp`
/// - `POST /auth/verify-otp`
/// - `GET /auth/me`
/// - `POST /auth/logout`
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `rate_limiter` - Shared rate limiter instance
/// * `jwt_secret` - Secret used for signing/verifying JWTs
pub fn auth_routes(pool: PgPool, rate_limiter: Arc<RateLimiter>, jwt_secret: String) -> Router {
    let state = AppState {
        pool,
        rate_limiter,
        jwt_secret,
    };

    Router::new()
        .route("/auth/request-otp", post(request_otp))
        .route("/auth/verify-otp", post(verify_otp))
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Extracts the client IP from request headers.
///
/// Checks `X-Forwarded-For` first (for reverse proxies), then falls back
/// to "unknown" if no IP information is available.
fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// `POST /auth/request-otp`
///
/// Sends a one-time password to the provided email if it is in the allowed list.
/// Always returns 200 with a generic message to prevent email enumeration.
async fn request_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RequestOtpBody>,
) -> Result<Json<MessageResponse>, AuthError> {
    let email = body.email.trim().to_lowercase();

    if email.is_empty() {
        return Err(AuthError::BadRequest("email is required".to_owned()));
    }

    let ip = extract_client_ip(&headers);
    state.rate_limiter.check_request_otp(&email, &ip)?;

    let allowed = db::check_email_allowed(&state.pool, &email).await?;

    if let Some((role,)) = allowed {
        // Email is allowed -- generate and send OTP
        let otp_code = otp::generate_otp();
        let otp_hash = otp::hash_otp(&otp_code)?;
        let expires_at = Utc::now() + Duration::minutes(5);

        db::insert_otp(&state.pool, &email, &otp_hash, expires_at).await?;
        email::send_otp_email(&email, &otp_code).await?;

        tracing::info!(email = %email, role = %role, "OTP requested for allowed email");
    } else {
        tracing::info!(email = %email, "OTP requested for unknown email (ignored)");
    }

    // Always return the same response regardless of whether the email exists
    Ok(Json(MessageResponse {
        message: "If this email is registered, a code was sent.".to_owned(),
    }))
}

/// `POST /auth/verify-otp`
///
/// Verifies a one-time password and issues a JWT on success.
async fn verify_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<VerifyOtpBody>,
) -> Result<Json<VerifyOtpResponse>, AuthError> {
    let email = body.email.trim().to_lowercase();
    let otp_code = body.otp.trim().to_owned();

    if email.is_empty() || otp_code.is_empty() {
        return Err(AuthError::BadRequest(
            "email and otp are required".to_owned(),
        ));
    }

    let ip = extract_client_ip(&headers);
    state.rate_limiter.check_verify_otp(&email, &ip)?;

    // Fetch active OTP record
    let record = db::get_active_otp(&state.pool, &email)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("no active OTP found".to_owned()))?;

    // Check if OTP has expired
    if record.expires_at < Utc::now() {
        return Err(AuthError::Unauthorized("OTP has expired".to_owned()));
    }

    // Check if max attempts exceeded
    if record.attempts >= record.max_attempts {
        db::invalidate_all_for_email(&state.pool, &email).await?;
        return Err(AuthError::Unauthorized(
            "too many failed attempts, please request a new code".to_owned(),
        ));
    }

    // Verify the OTP hash
    let is_valid = otp::verify_otp(&otp_code, &record.otp_hash)?;

    if !is_valid {
        db::increment_attempts(&state.pool, record.id).await?;

        // If this was the last allowed attempt, invalidate all OTPs
        if record.attempts + 1 >= record.max_attempts {
            db::invalidate_all_for_email(&state.pool, &email).await?;
        }

        return Err(AuthError::Unauthorized("invalid OTP code".to_owned()));
    }

    // OTP is valid -- consume it
    db::consume_otp(&state.pool, record.id).await?;

    // Look up the user's role
    let role = db::check_email_allowed(&state.pool, &email)
        .await?
        .map(|(r,)| r)
        .unwrap_or_else(|| "viewer".to_owned());

    // Create JWT
    let token = jwt::create_token(&email, &role, &state.jwt_secret)?;

    // Calculate expiry (24 hours from now)
    let expires_at = Utc::now() + Duration::hours(24);

    tracing::info!(email = %email, role = %role, "OTP verified, JWT issued");

    Ok(Json(VerifyOtpResponse {
        token,
        expires_at: expires_at.to_rfc3339(),
    }))
}

/// `GET /auth/me`
///
/// Returns the authenticated user's email and role from the JWT claims.
async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthError> {
    let claims = extract_claims(&headers, &state.jwt_secret)?;

    Ok(Json(MeResponse {
        email: claims.sub,
        role: claims.role,
    }))
}

/// `POST /auth/logout`
///
/// Stateless logout -- simply returns 204 No Content.
/// The client is responsible for removing the JWT from storage.
async fn logout() -> StatusCode {
    StatusCode::NO_CONTENT
}

/// Extracts and validates JWT claims from the Authorization header.
///
/// Expects the header format `Bearer <token>`.
///
/// # Errors
///
/// Returns [`AuthError::Unauthorized`] if the header is missing, malformed,
/// or the token is invalid/expired.
fn extract_claims(headers: &HeaderMap, jwt_secret: &str) -> Result<jwt::Claims, AuthError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::Unauthorized("missing authorization header".to_owned()))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        AuthError::Unauthorized("authorization header must start with 'Bearer '".to_owned())
    })?;

    jwt::validate_token(token, jwt_secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use jsonwebtoken::{EncodingKey, Header as JwtHeader};

    #[test]
    fn test_extract_client_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.1"));
        assert_eq!(extract_client_ip(&headers), "203.0.113.1");
    }

    #[test]
    fn test_extract_client_ip_multiple_ips() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.1, 198.51.100.1, 192.0.2.1"),
        );
        assert_eq!(extract_client_ip(&headers), "203.0.113.1");
    }

    #[test]
    fn test_extract_client_ip_missing_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_client_ip(&headers), "unknown");
    }

    #[test]
    fn test_extract_claims_missing_header() {
        let headers = HeaderMap::new();
        let result = extract_claims(&headers, "secret");
        assert!(result.is_err());
        match result {
            Err(AuthError::Unauthorized(msg)) => {
                assert!(msg.contains("missing"), "Error: {msg}");
            }
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    #[test]
    fn test_extract_claims_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        let result = extract_claims(&headers, "secret");
        assert!(result.is_err());
        match result {
            Err(AuthError::Unauthorized(msg)) => {
                assert!(msg.contains("Bearer"), "Error: {msg}");
            }
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    #[test]
    fn test_extract_claims_valid_token() {
        let secret = "test-secret-that-is-at-least-32-bytes-long";
        let token = jwt::create_token("alice@example.com", "admin", secret).unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );

        let claims = extract_claims(&headers, secret).unwrap();
        assert_eq!(claims.sub, "alice@example.com");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_extract_claims_invalid_token() {
        let secret = "test-secret-that-is-at-least-32-bytes-long";
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer invalid.token.here"),
        );

        let result = extract_claims(&headers, secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_request_otp_body_deserialize() {
        let json = r#"{"email": "test@example.com"}"#;
        let body: RequestOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "test@example.com");
    }

    #[test]
    fn test_verify_otp_body_deserialize() {
        let json = r#"{"email": "test@example.com", "otp": "123456"}"#;
        let body: VerifyOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "test@example.com");
        assert_eq!(body.otp, "123456");
    }

    #[test]
    fn test_verify_otp_response_serialize() {
        let response = VerifyOtpResponse {
            token: "jwt.token.here".to_owned(),
            expires_at: "2026-03-08T12:00:00Z".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["token"], "jwt.token.here");
        assert_eq!(json["expires_at"], "2026-03-08T12:00:00Z");
    }

    #[test]
    fn test_me_response_serialize() {
        let response = MeResponse {
            email: "user@example.com".to_owned(),
            role: "viewer".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["email"], "user@example.com");
        assert_eq!(json["role"], "viewer");
    }

    #[test]
    fn test_message_response_serialize() {
        let response = MessageResponse {
            message: "test message".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["message"], "test message");
    }

    #[test]
    fn test_request_otp_body_deserialize_various() {
        // Normal email
        let json = r#"{"email": "roundtrip@example.com"}"#;
        let body: RequestOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "roundtrip@example.com");

        // Email with extra whitespace (deserialized as-is; trimming happens in handler)
        let json = r#"{"email": "  spaces@example.com  "}"#;
        let body: RequestOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "  spaces@example.com  ");

        // Extra fields are ignored
        let json = r#"{"email": "extra@example.com", "extra": true}"#;
        let body: RequestOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "extra@example.com");
    }

    #[test]
    fn test_verify_otp_body_deserialize_various() {
        // With leading zero OTP
        let json = r#"{"email": "test@example.com", "otp": "007392"}"#;
        let body: VerifyOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.email, "test@example.com");
        assert_eq!(body.otp, "007392");

        // Extra fields ignored
        let json = r#"{"email": "test@example.com", "otp": "123456", "extra": 42}"#;
        let body: VerifyOtpBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.otp, "123456");
    }

    #[test]
    fn test_verify_otp_response_has_all_fields() {
        let response = VerifyOtpResponse {
            token: "header.payload.sig".to_owned(),
            expires_at: "2026-03-08T00:00:00+00:00".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();

        // Must have exactly these fields
        assert!(json.get("token").is_some(), "Must have 'token' field");
        assert!(
            json.get("expires_at").is_some(),
            "Must have 'expires_at' field"
        );
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "VerifyOtpResponse must have exactly 2 fields");
    }

    #[test]
    fn test_me_response_has_all_fields() {
        let response = MeResponse {
            email: "me@example.com".to_owned(),
            role: "admin".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["email"], "me@example.com");
        assert_eq!(json["role"], "admin");
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "MeResponse must have exactly 2 fields");
    }

    #[test]
    fn test_message_response_shape() {
        let response = MessageResponse {
            message: "If this email is registered, a code was sent.".to_owned(),
        };
        let json = serde_json::to_value(&response).unwrap();

        assert!(json.get("message").is_some());
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 1, "MessageResponse must have exactly 1 field");
        assert_eq!(
            json["message"],
            "If this email is registered, a code was sent."
        );
    }

    #[test]
    fn test_request_otp_body_missing_email_fails() {
        let json = r#"{}"#;
        let result: Result<RequestOtpBody, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "Missing email field must fail deserialization"
        );
    }

    #[test]
    fn test_verify_otp_body_missing_fields_fails() {
        // Missing otp
        let json = r#"{"email": "test@example.com"}"#;
        let result: Result<VerifyOtpBody, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Missing otp field must fail");

        // Missing email
        let json = r#"{"otp": "123456"}"#;
        let result: Result<VerifyOtpBody, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Missing email field must fail");
    }

    #[test]
    fn test_extract_client_ip_whitespace_trimmed() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("  203.0.113.1  , 198.51.100.1"),
        );
        assert_eq!(extract_client_ip(&headers), "203.0.113.1");
    }

    #[test]
    fn test_extract_claims_bearer_with_expired_token() {
        let secret = "test-secret-that-is-at-least-32-bytes-long";

        // Create an expired token manually
        let claims = jwt::Claims {
            sub: "expired@example.com".to_owned(),
            role: "viewer".to_owned(),
            exp: 0,
            iat: 0,
        };
        let token = jsonwebtoken::encode(
            &JwtHeader::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );

        let result = extract_claims(&headers, secret);
        assert!(result.is_err(), "Expired token must fail");
        match result {
            Err(AuthError::Unauthorized(_)) => {}
            other => panic!("Expected Unauthorized, got: {other:?}"),
        }
    }

    // Note: Full handler tests (request_otp, verify_otp, me, logout) require
    // a database connection and are covered in integration tests. The unit tests
    // above verify the helper functions and serialization logic.
}
