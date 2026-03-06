//! Error types for the data ingestion layer.

use std::path::PathBuf;

/// Errors that can occur during data storage and retrieval operations.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// An I/O error occurred while reading or writing files.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The filesystem path involved.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// A Parquet encoding or decoding error.
    #[error("parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    /// An Arrow conversion error.
    #[error("arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// A database error from SQLx.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A data validation error.
    #[error("validation error: {0}")]
    Validation(String),

    /// An API-level error (non-HTTP, e.g. upstream returned an error payload).
    #[error("API request failed: {0}")]
    Api(String),

    /// The upstream API returned HTTP 429 (rate limited).
    #[error("rate limited, retry after {retry_after_secs}s")]
    RateLimited {
        /// Suggested number of seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// No data was available for the requested instrument and date range.
    #[error("no data available for {instrument} from {start} to {end}")]
    NoData {
        /// The instrument that was requested.
        instrument: String,
        /// Start date of the request.
        start: String,
        /// End date of the request.
        end: String,
    },

    /// An HTTP transport error from reqwest.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl DataError {
    /// Create an I/O error associated with a specific filesystem path.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_data_error_io_display() {
        let err = DataError::io(
            "/tmp/test.parquet",
            io::Error::new(io::ErrorKind::NotFound, "file not found"),
        );
        let msg = err.to_string();
        assert!(msg.contains("/tmp/test.parquet"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_data_error_validation_display() {
        let err = DataError::Validation("high < low".into());
        assert_eq!(err.to_string(), "validation error: high < low");
    }

    #[test]
    fn test_data_error_api_display() {
        let err = DataError::Api("server error 500: internal".into());
        assert_eq!(
            err.to_string(),
            "API request failed: server error 500: internal"
        );
    }

    #[test]
    fn test_data_error_rate_limited_display() {
        let err = DataError::RateLimited {
            retry_after_secs: 60,
        };
        assert_eq!(err.to_string(), "rate limited, retry after 60s");
    }

    #[test]
    fn test_data_error_no_data_display() {
        let err = DataError::NoData {
            instrument: "DAX".into(),
            start: "2024-01-01".into(),
            end: "2024-01-31".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("DAX"));
        assert!(msg.contains("2024-01-01"));
        assert!(msg.contains("2024-01-31"));
    }

    #[test]
    fn test_data_error_io_helper() {
        let err = DataError::io(
            "some/path",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        match &err {
            DataError::Io { path, source } => {
                assert_eq!(path, &PathBuf::from("some/path"));
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            _ => panic!("expected Io variant"),
        }
    }
}
