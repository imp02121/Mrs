//! Shared application state for Axum handlers.
//!
//! [`AppState`] holds the database pool and optional cache connection,
//! passed to every handler via Axum's state extractor.

use sqlx::PgPool;

use crate::db::ValkeyCache;

/// Shared state accessible by all API handlers.
///
/// Cloned into each handler via `State<AppState>`. The `PgPool` and
/// `ValkeyCache` (connection manager) are both cheap to clone.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub db_pool: PgPool,
    /// Optional Valkey (Redis-compatible) cache connection.
    pub cache: Option<ValkeyCache>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }

    #[test]
    fn test_app_state_has_db_pool_field() {
        // Verify the struct has the expected field types by checking compilation.
        fn assert_fields(state: &AppState) {
            let _pool: &PgPool = &state.db_pool;
            let _cache: &Option<ValkeyCache> = &state.cache;
        }
        // This test succeeds if it compiles — field accessibility confirmed.
        let _ = assert_fields as fn(&AppState);
    }

    #[test]
    fn test_app_state_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AppState>();
    }
}
