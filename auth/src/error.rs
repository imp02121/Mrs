//! Authentication error types.
//!
//! Defines [`AuthError`] with variants for all failure modes in the auth service.
//! Implements [`axum::response::IntoResponse`] to produce consistent JSON error responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Error type for authentication operations.
///
/// Each variant maps to an HTTP status code and produces a JSON response
/// with the format `{"error": {"code": "...", "message": "..."}}`.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// 400 Bad Request -- malformed input or missing fields.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// 401 Unauthorized -- invalid credentials, expired token, etc.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// 429 Too Many Requests -- rate limit exceeded.
    #[error("rate limited: {0}")]
    RateLimited(String),

    /// 500 Internal Server Error -- unexpected internal failure.
    #[error("internal error: {0}")]
    Internal(String),

    /// 500 Internal Server Error -- database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl AuthError {
    /// Returns the HTTP status code for this error variant.
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the error code string for the JSON response.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Database(_) => "DATABASE_ERROR",
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code();
        let message = self.to_string();

        tracing::error!(%status, %code, %message, "auth error");

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn test_bad_request_status_code() {
        let err = AuthError::BadRequest("missing email".into());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.code(), "BAD_REQUEST");
    }

    #[test]
    fn test_unauthorized_status_code() {
        let err = AuthError::Unauthorized("invalid token".into());
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(err.code(), "UNAUTHORIZED");
    }

    #[test]
    fn test_rate_limited_status_code() {
        let err = AuthError::RateLimited("too many requests".into());
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err.code(), "RATE_LIMITED");
    }

    #[test]
    fn test_internal_status_code() {
        let err = AuthError::Internal("something broke".into());
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code(), "INTERNAL_ERROR");
    }

    #[test]
    fn test_database_error_status_code() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = AuthError::Database(sqlx_err);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code(), "DATABASE_ERROR");
    }

    #[test]
    fn test_error_display() {
        let err = AuthError::BadRequest("missing email field".into());
        assert_eq!(err.to_string(), "bad request: missing email field");
    }

    #[tokio::test]
    async fn test_into_response_json_format() {
        let err = AuthError::Unauthorized("expired token".into());
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
        assert_eq!(json["error"]["message"], "unauthorized: expired token");
    }

    #[test]
    fn test_database_error_from_sqlx() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err: AuthError = sqlx_err.into();
        assert!(matches!(err, AuthError::Database(_)));
    }

    #[test]
    fn test_rate_limited_maps_to_429() {
        let err = AuthError::RateLimited("slow down".into());
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err.status_code().as_u16(), 429);
    }

    #[tokio::test]
    async fn test_into_response_json_format_all_variants() {
        let variants: Vec<AuthError> = vec![
            AuthError::BadRequest("bad input".into()),
            AuthError::Unauthorized("no access".into()),
            AuthError::RateLimited("too fast".into()),
            AuthError::Internal("server broke".into()),
            AuthError::Database(sqlx::Error::RowNotFound),
        ];

        let expected_statuses = vec![
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::INTERNAL_SERVER_ERROR,
        ];

        let expected_codes = vec![
            "BAD_REQUEST",
            "UNAUTHORIZED",
            "RATE_LIMITED",
            "INTERNAL_ERROR",
            "DATABASE_ERROR",
        ];

        for ((err, expected_status), expected_code) in variants
            .into_iter()
            .zip(expected_statuses.iter())
            .zip(expected_codes.iter())
        {
            let response = err.into_response();
            assert_eq!(response.status(), *expected_status);

            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            // Verify JSON structure
            assert!(json.get("error").is_some(), "Must have 'error' key");
            assert_eq!(json["error"]["code"], *expected_code);
            assert!(
                json["error"]["message"].is_string(),
                "message must be a string"
            );
        }
    }

    #[test]
    fn test_display_all_variants() {
        let bad = AuthError::BadRequest("missing field".into());
        assert_eq!(bad.to_string(), "bad request: missing field");

        let unauth = AuthError::Unauthorized("expired".into());
        assert_eq!(unauth.to_string(), "unauthorized: expired");

        let limited = AuthError::RateLimited("calm down".into());
        assert_eq!(limited.to_string(), "rate limited: calm down");

        let internal = AuthError::Internal("oops".into());
        assert_eq!(internal.to_string(), "internal error: oops");

        let db = AuthError::Database(sqlx::Error::RowNotFound);
        let display = db.to_string();
        assert!(
            display.starts_with("database error:"),
            "Database error display: {display}"
        );
    }

    #[test]
    fn test_from_sqlx_error_conversion() {
        // Test multiple sqlx error variants
        let row_err = sqlx::Error::RowNotFound;
        let auth_err: AuthError = row_err.into();
        assert!(matches!(auth_err, AuthError::Database(_)));
        assert_eq!(auth_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(auth_err.code(), "DATABASE_ERROR");

        let col_err = sqlx::Error::ColumnNotFound("id".to_owned());
        let auth_err: AuthError = col_err.into();
        assert!(matches!(auth_err, AuthError::Database(_)));
    }

    #[test]
    fn test_error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        // AuthError should be Send + Sync for use across async boundaries
        assert_send::<AuthError>();
        assert_sync::<AuthError>();
    }
}
