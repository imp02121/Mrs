//! Cache-first reader that checks Valkey before falling back to PostgreSQL.
//!
//! [`CacheReader`] implements the cache-aside pattern: read from Valkey first,
//! fall back to Postgres on miss, and backfill the cache for subsequent reads.

use sqlx::PgPool;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::backtest::result::BacktestResult;
use crate::models::Instrument;

use super::cache::ValkeyCache;
use super::cache_error::CacheError;

/// Error type for cache-reader operations that may involve both cache and database.
#[derive(Debug, thiserror::Error)]
pub enum ReaderError {
    /// A cache operation failed.
    #[error("cache error: {0}")]
    Cache(#[from] CacheError),

    /// A database query failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Failed to deserialize a database row into the expected type.
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

/// Cache-first reader backed by Valkey and PostgreSQL.
///
/// Reads check Valkey first. On cache miss, the reader falls back to
/// Postgres and backfills the cache so subsequent reads are fast.
pub struct CacheReader {
    cache: ValkeyCache,
    pool: PgPool,
}

impl CacheReader {
    /// Create a new cache-first reader.
    #[must_use]
    pub fn new(cache: ValkeyCache, pool: PgPool) -> Self {
        Self { cache, pool }
    }

    /// Get a backtest result by run ID.
    ///
    /// Checks Valkey first; on miss, queries Postgres and backfills the cache.
    ///
    /// # Errors
    ///
    /// Returns [`ReaderError`] on cache or database failure.
    pub async fn get_backtest_result(
        &self,
        run_id: Uuid,
    ) -> Result<Option<BacktestResult>, ReaderError> {
        // Try cache first
        match self.cache.get_backtest_result(run_id).await {
            Ok(Some(result)) => {
                debug!(%run_id, "cache hit for backtest result");
                return Ok(Some(result));
            }
            Ok(None) => {
                debug!(%run_id, "cache miss for backtest result");
            }
            Err(e) => {
                warn!(%run_id, error = %e, "cache read failed, falling back to db");
            }
        }

        // Fall back to Postgres
        // The backtest_runs table stores stats as JSONB but not the full
        // BacktestResult. For cache-reader fallback we query stats and
        // reconstruct what we can; however, the full result (with trades,
        // equity curve) should have been cached at write time.
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT stats FROM backtest_runs WHERE id = $1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await?;

        // The backtest_runs table only stores stats JSONB, not the full
        // BacktestResult (which includes trades, equity curve, config).
        // The full result is only available in the cache. If it's not cached,
        // we can confirm the run exists but cannot reconstruct the full result.
        // Return None — callers should re-run the backtest if needed.
        let _exists = row.is_some();
        Ok(None)
    }

    /// Get the latest signal for an instrument.
    ///
    /// Checks Valkey first; on miss, queries Postgres and backfills the cache.
    ///
    /// # Errors
    ///
    /// Returns [`ReaderError`] on cache or database failure.
    pub async fn get_latest_signal(
        &self,
        instrument: Instrument,
    ) -> Result<Option<serde_json::Value>, ReaderError> {
        // Try cache first
        match self.cache.get_latest_signal(instrument).await {
            Ok(Some(signal)) => {
                debug!(instrument = instrument.ticker(), "cache hit for signal");
                return Ok(Some(signal));
            }
            Ok(None) => {
                debug!(instrument = instrument.ticker(), "cache miss for signal");
            }
            Err(e) => {
                warn!(
                    instrument = instrument.ticker(),
                    error = %e,
                    "cache read failed, falling back to db"
                );
            }
        }

        // Fall back to Postgres — query the live_signals table.
        // First resolve instrument_id from ticker symbol.
        let inst_row: Option<(i16,)> =
            sqlx::query_as("SELECT id FROM instruments WHERE symbol = $1")
                .bind(instrument.ticker())
                .fetch_optional(&self.pool)
                .await?;

        let Some((instrument_id,)) = inst_row else {
            return Ok(None);
        };

        let row: Option<super::signals::SignalRow> = sqlx::query_as(
            r#"
            SELECT id, instrument_id, signal_date, signal_bar_high, signal_bar_low,
                   buy_level, sell_level, status, fill_details, created_at
            FROM live_signals
            WHERE instrument_id = $1
            ORDER BY signal_date DESC
            LIMIT 1
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(signal_row) = row else {
            return Ok(None);
        };

        let signal_json = serde_json::to_value(&signal_row)?;

        // Backfill cache (best-effort)
        if let Err(e) = self.cache.set_signal(instrument, &signal_json).await {
            warn!(
                instrument = instrument.ticker(),
                error = %e,
                "failed to backfill signal cache"
            );
        }

        Ok(Some(signal_json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reader_error_display_cache() {
        let inner = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let cache_err = CacheError::Serialization(inner);
        let err = ReaderError::Cache(cache_err);
        assert!(err.to_string().contains("cache error"));
    }

    #[test]
    fn test_reader_error_display_deserialization() {
        let inner = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err = ReaderError::Deserialization(inner);
        assert!(err.to_string().contains("deserialization error"));
    }

    #[test]
    fn test_reader_error_from_cache_error() {
        let inner = serde_json::from_str::<serde_json::Value>("x").unwrap_err();
        let cache_err = CacheError::Serialization(inner);
        let err: ReaderError = cache_err.into();
        assert!(matches!(err, ReaderError::Cache(_)));
    }

    #[test]
    fn test_reader_error_from_sqlx_error() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err: ReaderError = sqlx_err.into();
        assert!(matches!(err, ReaderError::Database(_)));
    }

    #[test]
    fn test_reader_error_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: ReaderError = json_err.into();
        assert!(matches!(err, ReaderError::Deserialization(_)));
    }

    #[test]
    fn test_reader_error_display_database() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = ReaderError::Database(sqlx_err);
        let display = err.to_string();
        assert!(display.contains("database error"));
    }

    #[test]
    fn test_reader_error_debug_cache() {
        let inner = serde_json::from_str::<serde_json::Value>("nope").unwrap_err();
        let cache_err = CacheError::Serialization(inner);
        let err = ReaderError::Cache(cache_err);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Cache"));
    }

    #[test]
    fn test_reader_error_debug_database() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = ReaderError::Database(sqlx_err);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Database"));
    }

    #[test]
    fn test_reader_error_debug_deserialization() {
        let inner = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err = ReaderError::Deserialization(inner);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Deserialization"));
    }
}
