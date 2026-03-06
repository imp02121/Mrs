//! Core Valkey (Redis-compatible) cache operations.
//!
//! Provides [`ValkeyCache`] for get/set/delete with JSON serialization,
//! plus domain-specific helpers for backtest results, signals, and progress.

use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::backtest::result::BacktestResult;
use crate::models::Instrument;

use super::cache_error::CacheError;

/// TTL for latest signal cache entries (24 hours).
const TTL_SIGNAL: u64 = 86400;

/// TTL for today's trades cache entries (24 hours).
const _TTL_TRADES_TODAY: u64 = 86400;

/// TTL for backtest progress entries (1 hour).
const TTL_PROGRESS: u64 = 3600;

/// TTL for backtest result entries (7 days).
const TTL_RESULT: u64 = 604800;

/// TTL for tracking/signal-state entries (24 hours).
const _TTL_TRACKING: u64 = 86400;

/// A Valkey (Redis-compatible) cache client backed by a connection manager.
///
/// All operations are async and use JSON serialization for values.
#[derive(Clone)]
pub struct ValkeyCache {
    conn: redis::aio::ConnectionManager,
}

impl ValkeyCache {
    /// Connect to a Valkey instance at the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Connection`] if the connection cannot be established.
    pub async fn new(url: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(url)?;
        let conn = redis::aio::ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    /// Store a JSON-serializable value with a TTL.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Serialization`] if the value cannot be serialized,
    /// or [`CacheError::Connection`] on Redis communication failure.
    pub async fn set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<(), CacheError> {
        let json = serde_json::to_string(value)?;
        let mut conn = self.conn.clone();
        conn.set_ex::<_, _, ()>(key, json, ttl_secs).await?;
        Ok(())
    }

    /// Retrieve a JSON value from the cache, deserializing into `T`.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Serialization`] if the stored value cannot be
    /// deserialized, or [`CacheError::Connection`] on Redis communication failure.
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
        let mut conn = self.conn.clone();
        let raw: Option<String> = conn.get(key).await?;
        match raw {
            Some(json) => {
                let value = serde_json::from_str(&json)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Delete a key from the cache.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Connection`] on Redis communication failure.
    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.conn.clone();
        conn.del::<_, ()>(key).await?;
        Ok(())
    }

    // -- Domain-specific helpers --

    /// Cache a backtest result with a 7-day TTL.
    ///
    /// Key pattern: `sr:backtest:{run_id}:result`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on serialization or connection failure.
    pub async fn set_backtest_result(
        &self,
        run_id: Uuid,
        result: &BacktestResult,
    ) -> Result<(), CacheError> {
        let key = format!("sr:backtest:{run_id}:result");
        self.set_json(&key, result, TTL_RESULT).await
    }

    /// Retrieve a cached backtest result.
    ///
    /// Key pattern: `sr:backtest:{run_id}:result`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on deserialization or connection failure.
    pub async fn get_backtest_result(
        &self,
        run_id: Uuid,
    ) -> Result<Option<BacktestResult>, CacheError> {
        let key = format!("sr:backtest:{run_id}:result");
        self.get_json(&key).await
    }

    /// Cache the latest signal for an instrument with a 24-hour TTL.
    ///
    /// Key pattern: `sr:signal:{ticker}:latest`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on serialization or connection failure.
    pub async fn set_signal(
        &self,
        instrument: Instrument,
        signal_json: &serde_json::Value,
    ) -> Result<(), CacheError> {
        let key = format!("sr:signal:{}:latest", instrument.ticker());
        self.set_json(&key, signal_json, TTL_SIGNAL).await
    }

    /// Retrieve the latest cached signal for an instrument.
    ///
    /// Key pattern: `sr:signal:{ticker}:latest`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on deserialization or connection failure.
    pub async fn get_latest_signal(
        &self,
        instrument: Instrument,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        let key = format!("sr:signal:{}:latest", instrument.ticker());
        self.get_json(&key).await
    }

    /// Cache backtest progress with a 1-hour TTL.
    ///
    /// Key pattern: `sr:backtest:{run_id}:progress`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on serialization or connection failure.
    pub async fn set_backtest_progress(
        &self,
        run_id: Uuid,
        progress: f64,
        status: &str,
    ) -> Result<(), CacheError> {
        let key = format!("sr:backtest:{run_id}:progress");
        let value = serde_json::json!({
            "progress": progress,
            "status": status,
        });
        self.set_json(&key, &value, TTL_PROGRESS).await
    }

    /// Retrieve cached backtest progress.
    ///
    /// Key pattern: `sr:backtest:{run_id}:progress`
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on deserialization or connection failure.
    pub async fn get_backtest_progress(
        &self,
        run_id: Uuid,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        let key = format!("sr:backtest:{run_id}:progress");
        self.get_json(&key).await
    }

    /// Build the cache key for a backtest result.
    #[must_use]
    pub fn backtest_result_key(run_id: Uuid) -> String {
        format!("sr:backtest:{run_id}:result")
    }

    /// Build the cache key for the latest signal.
    #[must_use]
    pub fn signal_key(instrument: Instrument) -> String {
        format!("sr:signal:{}:latest", instrument.ticker())
    }

    /// Build the cache key for backtest progress.
    #[must_use]
    pub fn progress_key(run_id: Uuid) -> String {
        format!("sr:backtest:{run_id}:progress")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_result_key_format() {
        let id = Uuid::nil();
        let key = ValkeyCache::backtest_result_key(id);
        assert_eq!(
            key,
            "sr:backtest:00000000-0000-0000-0000-000000000000:result"
        );
    }

    #[test]
    fn test_signal_key_format() {
        let key = ValkeyCache::signal_key(Instrument::Dax);
        assert_eq!(key, "sr:signal:DAX:latest");
    }

    #[test]
    fn test_signal_key_all_instruments() {
        assert_eq!(
            ValkeyCache::signal_key(Instrument::Dax),
            "sr:signal:DAX:latest"
        );
        assert_eq!(
            ValkeyCache::signal_key(Instrument::Ftse),
            "sr:signal:FTSE:latest"
        );
        assert_eq!(
            ValkeyCache::signal_key(Instrument::Nasdaq),
            "sr:signal:IXIC:latest"
        );
        assert_eq!(
            ValkeyCache::signal_key(Instrument::Dow),
            "sr:signal:DJI:latest"
        );
    }

    #[test]
    fn test_progress_key_format() {
        let id = Uuid::nil();
        let key = ValkeyCache::progress_key(id);
        assert_eq!(
            key,
            "sr:backtest:00000000-0000-0000-0000-000000000000:progress"
        );
    }

    #[test]
    fn test_ttl_constants() {
        assert_eq!(TTL_SIGNAL, 86400);
        assert_eq!(TTL_PROGRESS, 3600);
        assert_eq!(TTL_RESULT, 604800);
    }

    #[test]
    fn test_ttl_signal_is_24_hours() {
        assert_eq!(TTL_SIGNAL, 24 * 60 * 60);
    }

    #[test]
    fn test_ttl_progress_is_1_hour() {
        assert_eq!(TTL_PROGRESS, 60 * 60);
    }

    #[test]
    fn test_ttl_result_is_7_days() {
        assert_eq!(TTL_RESULT, 7 * 24 * 60 * 60);
    }

    #[test]
    fn test_backtest_result_key_with_max_uuid() {
        let id = Uuid::max();
        let key = ValkeyCache::backtest_result_key(id);
        assert_eq!(
            key,
            "sr:backtest:ffffffff-ffff-ffff-ffff-ffffffffffff:result"
        );
    }

    #[test]
    fn test_progress_key_with_max_uuid() {
        let id = Uuid::max();
        let key = ValkeyCache::progress_key(id);
        assert_eq!(
            key,
            "sr:backtest:ffffffff-ffff-ffff-ffff-ffffffffffff:progress"
        );
    }

    #[test]
    fn test_backtest_result_key_with_nil_uuid() {
        let id = Uuid::nil();
        let key = ValkeyCache::backtest_result_key(id);
        assert!(key.starts_with("sr:backtest:"));
        assert!(key.ends_with(":result"));
        assert!(key.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn test_progress_key_with_nil_uuid() {
        let id = Uuid::nil();
        let key = ValkeyCache::progress_key(id);
        assert!(key.starts_with("sr:backtest:"));
        assert!(key.ends_with(":progress"));
    }

    #[test]
    fn test_signal_key_contains_ticker() {
        for instrument in Instrument::ALL {
            let key = ValkeyCache::signal_key(instrument);
            assert!(key.contains(instrument.ticker()));
            assert!(key.starts_with("sr:signal:"));
            assert!(key.ends_with(":latest"));
        }
    }
}
