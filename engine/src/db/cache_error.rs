//! Cache error types for Valkey operations.

/// Errors that can occur during cache operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// Failed to connect or communicate with Valkey.
    #[error("cache connection error: {0}")]
    Connection(#[from] redis::RedisError),

    /// Failed to serialize or deserialize a cached value.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_error_display_serialization() {
        let inner = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = CacheError::Serialization(inner);
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn test_cache_error_from_serde() {
        let inner = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err: CacheError = inner.into();
        assert!(matches!(err, CacheError::Serialization(_)));
    }

    #[test]
    fn test_cache_error_debug_serialization() {
        let inner = serde_json::from_str::<serde_json::Value>("nope").unwrap_err();
        let err = CacheError::Serialization(inner);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Serialization"));
    }

    #[test]
    fn test_cache_error_debug_connection() {
        // Construct a RedisError via an invalid URL parse
        let client_result = redis::Client::open("not_a_valid_url://bad");
        if let Err(redis_err) = client_result {
            let err = CacheError::Connection(redis_err);
            let debug_str = format!("{:?}", err);
            assert!(debug_str.contains("Connection"));
        }
    }
}
