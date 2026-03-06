//! PostgreSQL storage for candle data.
//!
//! Provides bulk upsert and query operations against the `candles` table.
//! Upserts use `INSERT ... ON CONFLICT (instrument_id, timestamp) DO UPDATE`
//! to ensure idempotent writes. Large batches are split into groups of
//! 1000-row batches to stay within Postgres parameter limits.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Candle, DateRange, Instrument};

use super::error::DataError;

/// Maximum number of rows per INSERT batch.
///
/// Each candle uses 7 bind parameters. Postgres has a 65535 parameter limit,
/// so 1000 rows * 7 = 7000 parameters is well within bounds.
const BATCH_SIZE: usize = 1000;

/// PostgreSQL-backed store for OHLCV candle data.
///
/// Uses SQLx with the Postgres driver. All queries are runtime-checked
/// (not compile-time) because the schema may not be available at build time.
pub struct PostgresStore {
    /// Connection pool.
    pool: PgPool,
}

impl PostgresStore {
    /// Create a new store backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Bulk upsert candles into the `candles` table.
    ///
    /// Candles are inserted in batches of 1000. On conflict
    /// (same `instrument_id` + `timestamp`), the existing row is updated
    /// with the new OHLCV values.
    ///
    /// Returns the total number of rows affected (inserted or updated).
    ///
    /// # Errors
    ///
    /// Returns [`DataError::Database`] on any SQL failure.
    pub async fn upsert_candles(&self, candles: &[Candle]) -> Result<usize, DataError> {
        if candles.is_empty() {
            return Ok(0);
        }

        let mut total_affected = 0usize;

        for chunk in candles.chunks(BATCH_SIZE) {
            let affected = self.upsert_batch(chunk).await?;
            total_affected += affected;
        }

        Ok(total_affected)
    }

    /// Query candles for an instrument within a date range, ordered by timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`DataError::Database`] on any SQL failure.
    pub async fn get_candles(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<Vec<Candle>, DataError> {
        let instrument_id = instrument.ticker();
        let start_ts = range
            .start
            .and_hms_opt(0, 0, 0)
            .map(|ndt| ndt.and_utc())
            .unwrap_or_else(|| DateTime::<Utc>::MIN_UTC);
        let end_ts = range
            .end
            .succ_opt()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|ndt| ndt.and_utc())
            .unwrap_or_else(|| DateTime::<Utc>::MAX_UTC);

        let rows = sqlx::query_as::<_, CandleRow>(
            r#"
            SELECT instrument_id, timestamp, open, high, low, close, volume
            FROM candles
            WHERE instrument_id = $1
              AND timestamp >= $2
              AND timestamp < $3
            ORDER BY timestamp ASC
            "#,
        )
        .bind(instrument_id)
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| row.into_candle())
            .collect::<Result<Vec<_>, _>>()
    }

    /// Get the latest timestamp stored for a given instrument.
    ///
    /// Returns `None` if no candles exist for the instrument.
    ///
    /// # Errors
    ///
    /// Returns [`DataError::Database`] on any SQL failure.
    pub async fn latest_timestamp(
        &self,
        instrument: Instrument,
    ) -> Result<Option<DateTime<Utc>>, DataError> {
        let row: Option<(DateTime<Utc>,)> = sqlx::query_as(
            r#"
            SELECT MAX(timestamp) as max_ts
            FROM candles
            WHERE instrument_id = $1
            "#,
        )
        .bind(instrument.ticker())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(ts,)| ts))
    }

    /// Insert a single batch using a dynamically built INSERT statement.
    async fn upsert_batch(&self, batch: &[Candle]) -> Result<usize, DataError> {
        // Build a dynamic INSERT with multiple VALUE rows.
        // INSERT INTO candles (instrument_id, timestamp, open, high, low, close, volume)
        // VALUES ($1,$2,$3,$4,$5,$6,$7), ($8,...), ...
        // ON CONFLICT (instrument_id, timestamp) DO UPDATE SET ...

        let mut sql = String::from(
            "INSERT INTO candles (instrument_id, timestamp, open, high, low, close, volume) VALUES ",
        );

        let params_per_row = 7;
        for (i, _) in batch.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            let base = i * params_per_row + 1;
            sql.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base,
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6,
            ));
        }

        sql.push_str(
            " ON CONFLICT (instrument_id, timestamp) DO UPDATE SET \
             open = EXCLUDED.open, \
             high = EXCLUDED.high, \
             low = EXCLUDED.low, \
             close = EXCLUDED.close, \
             volume = EXCLUDED.volume",
        );

        let mut query = sqlx::query(&sql);
        for candle in batch {
            query = query
                .bind(candle.instrument.ticker())
                .bind(candle.timestamp)
                .bind(candle.open)
                .bind(candle.high)
                .bind(candle.low)
                .bind(candle.close)
                .bind(candle.volume);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected() as usize)
    }
}

/// Internal row type for mapping SQL query results.
#[derive(Debug, sqlx::FromRow)]
struct CandleRow {
    instrument_id: String,
    timestamp: DateTime<Utc>,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: i64,
}

impl CandleRow {
    /// Convert this row into a domain [`Candle`].
    fn into_candle(self) -> Result<Candle, DataError> {
        let instrument: Instrument =
            self.instrument_id
                .parse()
                .map_err(|e: crate::models::ParseInstrumentError| {
                    DataError::Validation(e.to_string())
                })?;

        Ok(Candle {
            instrument,
            timestamp: self.timestamp,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_size_within_postgres_limits() {
        // 1000 rows * 7 params = 7000, well under 65535 limit.
        const { assert!(BATCH_SIZE * 7 < 65535) };
    }

    #[test]
    fn test_candle_row_into_candle_valid() {
        let row = CandleRow {
            instrument_id: "DAX".to_owned(),
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            open: Decimal::new(1600050, 2),
            high: Decimal::new(1605000, 2),
            low: Decimal::new(1598025, 2),
            close: Decimal::new(1603075, 2),
            volume: 12345,
        };

        let candle = row.into_candle().unwrap();
        assert_eq!(candle.instrument, Instrument::Dax);
        assert_eq!(candle.volume, 12345);
    }

    #[test]
    fn test_candle_row_into_candle_invalid_instrument() {
        let row = CandleRow {
            instrument_id: "UNKNOWN".to_owned(),
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            open: Decimal::ZERO,
            high: Decimal::ZERO,
            low: Decimal::ZERO,
            close: Decimal::ZERO,
            volume: 0,
        };

        let result = row.into_candle();
        assert!(result.is_err());
    }
}
