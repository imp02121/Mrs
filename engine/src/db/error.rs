//! Error types for the database layer.

/// Errors that can occur during database operations.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A database query or connection error from SQLx.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// The requested record was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A serialization or deserialization error (e.g. JSONB round-trip).
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_display() {
        let err = DbError::NotFound("instrument id=99".into());
        assert_eq!(err.to_string(), "not found: instrument id=99");
    }

    #[test]
    fn test_serialization_display() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = DbError::Serialization(json_err);
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn test_database_display() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = DbError::Database(sqlx_err);
        assert!(err.to_string().contains("database error"));
    }

    #[test]
    fn test_from_sqlx_error() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err: DbError = sqlx_err.into();
        assert!(matches!(err, DbError::Database(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();
        let err: DbError = json_err.into();
        assert!(matches!(err, DbError::Serialization(_)));
    }

    #[test]
    fn test_not_found_debug() {
        let err = DbError::NotFound("missing row".into());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("missing row"));
    }

    #[test]
    fn test_database_debug() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = DbError::Database(sqlx_err);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Database"));
    }

    #[test]
    fn test_serialization_debug() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err = DbError::Serialization(json_err);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Serialization"));
    }

    #[test]
    fn test_not_found_with_empty_message() {
        let err = DbError::NotFound(String::new());
        assert_eq!(err.to_string(), "not found: ");
    }
}
