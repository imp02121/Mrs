//! Data fetch orchestration: ties data providers to storage backends.
//!
//! The [`DataFetcher`] coordinates fetching candle data from a
//! [`DataProvider`](super::provider::DataProvider), deduplicating it, and
//! writing it to both the local Parquet store and PostgreSQL.

use chrono::Utc;
use tracing::{info, warn};

use crate::models::{Candle, DateRange, Instrument};

use super::error::DataError;
use super::parquet_store::ParquetStore;
use super::postgres_store::PostgresStore;
use super::provider::DataProvider;

/// Orchestrates data fetching from a provider and storage to Parquet + Postgres.
///
/// The generic parameter `P` is the data provider implementation (e.g. Twelve Data).
pub struct DataFetcher<P> {
    /// The data provider to fetch candles from.
    provider: P,
    /// Local Parquet file store.
    parquet: ParquetStore,
    /// PostgreSQL store.
    postgres: PostgresStore,
}

impl<P: DataProvider> DataFetcher<P> {
    /// Create a new fetcher with the given provider and storage backends.
    pub fn new(provider: P, parquet: ParquetStore, postgres: PostgresStore) -> Self {
        Self {
            provider,
            parquet,
            postgres,
        }
    }

    /// Perform a full historical backfill for an instrument over the given date range.
    ///
    /// Fetches all candles from the provider, deduplicates by timestamp,
    /// writes to Parquet files, and upserts into PostgreSQL.
    ///
    /// # Errors
    ///
    /// Returns [`DataError`] if the provider fetch, Parquet write, or
    /// Postgres upsert fails.
    pub async fn backfill(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<usize, DataError> {
        info!(
            instrument = %instrument,
            start = %range.start,
            end = %range.end,
            "starting backfill"
        );

        let candles = self.provider.fetch_candles(instrument, range).await?;
        info!(
            instrument = %instrument,
            raw_count = candles.len(),
            "fetched candles from provider"
        );

        if candles.is_empty() {
            warn!(instrument = %instrument, "provider returned no candles");
            return Ok(0);
        }

        let deduped = deduplicate(candles);
        info!(
            instrument = %instrument,
            deduped_count = deduped.len(),
            "deduplicated candles"
        );

        let parquet_count = self.parquet.write_candles(&deduped)?;
        info!(
            instrument = %instrument,
            written = parquet_count,
            "wrote candles to parquet"
        );

        let pg_count = self.postgres.upsert_candles(&deduped).await?;
        info!(
            instrument = %instrument,
            upserted = pg_count,
            "upserted candles to postgres"
        );

        Ok(deduped.len())
    }

    /// Perform an incremental fetch, retrieving only candles newer than the
    /// latest stored timestamp.
    ///
    /// Checks PostgreSQL for the most recent candle timestamp for the
    /// instrument, then fetches from that point to today.
    ///
    /// # Errors
    ///
    /// Returns [`DataError`] if the provider fetch, Parquet write, or
    /// Postgres upsert fails.
    pub async fn incremental(&self, instrument: Instrument) -> Result<usize, DataError> {
        let latest = self.postgres.latest_timestamp(instrument).await?;

        let start = match latest {
            Some(ts) => ts.date_naive(),
            None => {
                warn!(
                    instrument = %instrument,
                    "no existing data found, falling back to 2 years ago"
                );
                Utc::now().date_naive() - chrono::Duration::days(730)
            }
        };

        let end = Utc::now().date_naive();

        if start >= end {
            info!(instrument = %instrument, "already up to date");
            return Ok(0);
        }

        let range = DateRange::new(start, end).map_err(|e| DataError::Validation(e.to_string()))?;

        info!(
            instrument = %instrument,
            start = %range.start,
            end = %range.end,
            "starting incremental fetch"
        );

        let candles = self.provider.fetch_candles(instrument, range).await?;
        info!(
            instrument = %instrument,
            raw_count = candles.len(),
            "fetched candles from provider"
        );

        if candles.is_empty() {
            info!(instrument = %instrument, "no new candles from provider");
            return Ok(0);
        }

        // Filter out candles we already have (at or before latest timestamp).
        let new_candles: Vec<Candle> = if let Some(ts) = latest {
            candles.into_iter().filter(|c| c.timestamp > ts).collect()
        } else {
            candles
        };

        if new_candles.is_empty() {
            info!(instrument = %instrument, "no new candles after filtering");
            return Ok(0);
        }

        let deduped = deduplicate(new_candles);
        info!(
            instrument = %instrument,
            new_count = deduped.len(),
            "new candles after deduplication"
        );

        let parquet_count = self.parquet.write_candles(&deduped)?;
        info!(
            instrument = %instrument,
            written = parquet_count,
            "wrote candles to parquet"
        );

        let pg_count = self.postgres.upsert_candles(&deduped).await?;
        info!(
            instrument = %instrument,
            upserted = pg_count,
            "upserted candles to postgres"
        );

        Ok(deduped.len())
    }
}

/// Deduplicate candles by timestamp, keeping the last occurrence.
///
/// Sorts by timestamp and removes consecutive duplicates (same timestamp).
fn deduplicate(mut candles: Vec<Candle>) -> Vec<Candle> {
    candles.sort_by_key(|c| c.timestamp);
    candles.dedup_by_key(|c| c.timestamp);
    candles
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_candle(instrument: Instrument, ts_secs: i64) -> Candle {
        Candle {
            instrument,
            timestamp: DateTime::from_timestamp(ts_secs, 0).unwrap(),
            open: dec!(100.0),
            high: dec!(105.0),
            low: dec!(95.0),
            close: dec!(102.0),
            volume: 1000,
        }
    }

    use chrono::DateTime;

    #[test]
    fn test_deduplicate_removes_duplicates() {
        let candles = vec![
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 2000),
            make_candle(Instrument::Dax, 1000), // duplicate
            make_candle(Instrument::Dax, 3000),
        ];

        let result = deduplicate(candles);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].timestamp.timestamp(), 1000);
        assert_eq!(result[1].timestamp.timestamp(), 2000);
        assert_eq!(result[2].timestamp.timestamp(), 3000);
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let candles = vec![
            make_candle(Instrument::Dax, 3000),
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 2000),
        ];

        let result = deduplicate(candles);
        assert_eq!(result.len(), 3);
        // Should be sorted by timestamp.
        assert!(result[0].timestamp < result[1].timestamp);
        assert!(result[1].timestamp < result[2].timestamp);
    }

    #[test]
    fn test_deduplicate_empty() {
        let result = deduplicate(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deduplicate_single() {
        let candles = vec![make_candle(Instrument::Ftse, 5000)];
        let result = deduplicate(candles);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_deduplicate_all_same_timestamp() {
        let candles = vec![
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 1000),
        ];
        let result = deduplicate(candles);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].timestamp.timestamp(), 1000);
    }

    #[test]
    fn test_deduplicate_already_sorted_unique() {
        let candles = vec![
            make_candle(Instrument::Nasdaq, 1000),
            make_candle(Instrument::Nasdaq, 2000),
            make_candle(Instrument::Nasdaq, 3000),
        ];
        let result = deduplicate(candles);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_deduplicate_large_batch() {
        let candles: Vec<Candle> = (0..1000)
            .map(|i| make_candle(Instrument::Dow, i * 900))
            .collect();
        let result = deduplicate(candles);
        assert_eq!(result.len(), 1000);
        // Verify ascending order.
        for pair in result.windows(2) {
            assert!(pair[0].timestamp < pair[1].timestamp);
        }
    }

    #[test]
    fn test_deduplicate_with_interleaved_duplicates() {
        let candles = vec![
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 2000),
            make_candle(Instrument::Dax, 1000),
            make_candle(Instrument::Dax, 3000),
            make_candle(Instrument::Dax, 2000),
        ];
        let result = deduplicate(candles);
        assert_eq!(result.len(), 3);
    }
}
