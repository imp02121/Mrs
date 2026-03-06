//! API error types and response conversion.
//!
//! [`ApiError`] is the unified error type for all HTTP handlers. It implements
//! [`axum::response::IntoResponse`] to produce JSON error responses with
//! appropriate HTTP status codes.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Unified error type for API handlers.
///
/// Each variant maps to an HTTP status code and produces a JSON error body:
/// ```json
/// {"error": {"code": "BAD_REQUEST", "message": "...", "details": null}}
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 400 Bad Request — malformed input, missing fields, etc.
    #[error("{0}")]
    BadRequest(String),

    /// 404 Not Found — the requested resource does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// 422 Unprocessable Entity — input is syntactically valid but semantically wrong.
    #[error("validation error: {0}")]
    Validation(String),

    /// 500 Internal Server Error — unexpected failures.
    #[error("internal error: {0}")]
    Internal(String),

    /// Database layer error, mapped to 500 or 404 depending on variant.
    #[error("database error: {0}")]
    Database(#[from] crate::db::DbError),
}

impl ApiError {
    /// HTTP status code for this error variant.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Database(db_err) => match db_err {
                crate::db::DbError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        }
    }

    /// Machine-readable error code string.
    fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Database(db_err) => match db_err {
                crate::db::DbError::NotFound(_) => "NOT_FOUND",
                _ => "DATABASE_ERROR",
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code();
        let message = self.to_string();

        let body = serde_json::json!({
            "error": {
                "code": code,
                "message": message,
                "details": null,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_request_status() {
        let err = ApiError::BadRequest("missing field".into());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.code(), "BAD_REQUEST");
    }

    #[test]
    fn test_not_found_status() {
        let err = ApiError::NotFound("config id=123".into());
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[test]
    fn test_validation_status() {
        let err = ApiError::Validation("date range invalid".into());
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn test_internal_status() {
        let err = ApiError::Internal("unexpected".into());
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code(), "INTERNAL_ERROR");
    }

    #[test]
    fn test_database_not_found_maps_to_404() {
        let db_err = crate::db::DbError::NotFound("run id=x".into());
        let err = ApiError::Database(db_err);
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[test]
    fn test_database_other_maps_to_500() {
        let db_err = crate::db::DbError::Database(sqlx::Error::RowNotFound);
        let err = ApiError::Database(db_err);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code(), "DATABASE_ERROR");
    }

    #[test]
    fn test_display_messages() {
        assert_eq!(ApiError::BadRequest("bad".into()).to_string(), "bad");
        assert_eq!(
            ApiError::NotFound("item".into()).to_string(),
            "not found: item"
        );
        assert_eq!(
            ApiError::Validation("invalid".into()).to_string(),
            "validation error: invalid"
        );
        assert_eq!(
            ApiError::Internal("oops".into()).to_string(),
            "internal error: oops"
        );
    }

    async fn extract_error_body(err: ApiError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).expect("parse json");
        (status, body)
    }

    #[tokio::test]
    async fn test_error_json_body_format() {
        let (status, body) =
            extract_error_body(ApiError::BadRequest("missing field x".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_object());
        assert_eq!(body["error"]["code"], "BAD_REQUEST");
        assert_eq!(body["error"]["message"], "missing field x");
        assert!(body["error"]["details"].is_null());
    }

    #[tokio::test]
    async fn test_not_found_json_body() {
        let (status, body) = extract_error_body(ApiError::NotFound("user id=42".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "NOT_FOUND");
        assert_eq!(body["error"]["message"], "not found: user id=42");
    }

    #[tokio::test]
    async fn test_validation_json_body() {
        let (status, body) =
            extract_error_body(ApiError::Validation("end before start".into())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
        assert_eq!(
            body["error"]["message"],
            "validation error: end before start"
        );
    }

    #[tokio::test]
    async fn test_internal_json_body() {
        let (status, body) = extract_error_body(ApiError::Internal("disk full".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"]["code"], "INTERNAL_ERROR");
        assert_eq!(body["error"]["message"], "internal error: disk full");
    }

    #[test]
    fn test_from_db_error_not_found() {
        let db_err = crate::db::DbError::NotFound("config 123".into());
        let api_err: ApiError = db_err.into();
        assert!(matches!(api_err, ApiError::Database(_)));
        assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(api_err.code(), "NOT_FOUND");
    }

    #[test]
    fn test_from_db_error_database() {
        let db_err = crate::db::DbError::Database(sqlx::Error::RowNotFound);
        let api_err: ApiError = db_err.into();
        assert!(matches!(api_err, ApiError::Database(_)));
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.code(), "DATABASE_ERROR");
    }

    #[test]
    fn test_from_db_error_serialization() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad json").unwrap_err();
        let db_err = crate::db::DbError::Serialization(json_err);
        let api_err: ApiError = db_err.into();
        assert!(matches!(api_err, ApiError::Database(_)));
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.code(), "DATABASE_ERROR");
    }

    #[test]
    fn test_display_database_not_found() {
        let db_err = crate::db::DbError::NotFound("row".into());
        let api_err = ApiError::Database(db_err);
        let msg = api_err.to_string();
        assert!(msg.contains("database error"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_display_database_sqlx() {
        let db_err = crate::db::DbError::Database(sqlx::Error::RowNotFound);
        let api_err = ApiError::Database(db_err);
        let msg = api_err.to_string();
        assert!(msg.contains("database error"));
    }

    #[tokio::test]
    async fn test_database_not_found_json_body() {
        let db_err = crate::db::DbError::NotFound("record".into());
        let (status, body) = extract_error_body(ApiError::Database(db_err)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "NOT_FOUND");
        assert!(body["error"]["details"].is_null());
    }
}
