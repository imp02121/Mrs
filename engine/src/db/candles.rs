//! Candle database queries.
//!
//! Provides bulk upsert and query operations for OHLCV candle data.
//! Large batches are split into 1000-row chunks to stay within
//! Postgres parameter limits.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::error::DbError;

/// Maximum rows per INSERT batch (7 params each = 7000, well under 65535).
const BATCH_SIZE: usize = 1000;

/// A row from the `candles` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct CandleRow {
    /// Foreign key to instruments.
    pub instrument_id: i16,
    /// Candle timestamp in UTC.
    pub timestamp: DateTime<Utc>,
    /// Opening price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Closing price.
    pub close: Decimal,
    /// Trade volume.
    pub volume: i64,
}

/// Bulk upsert candle rows into the `candles` table.
///
/// Splits into batches of 1000. On conflict (instrument_id, timestamp),
/// existing rows are updated with the new OHLCV values.
///
/// Returns the total number of rows affected.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn upsert_candles(pool: &PgPool, candles: &[CandleRow]) -> Result<usize, DbError> {
    if candles.is_empty() {
        return Ok(0);
    }

    let mut total_affected = 0usize;

    for chunk in candles.chunks(BATCH_SIZE) {
        let affected = upsert_batch(pool, chunk).await?;
        total_affected += affected;
    }

    Ok(total_affected)
}

/// Query candles for an instrument within a time range.
///
/// Returns rows ordered by timestamp ascending.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn get_candles(
    pool: &PgPool,
    instrument_id: i16,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<CandleRow>, DbError> {
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
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get the most recent candle timestamp for an instrument.
///
/// Returns `None` if no candles exist.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn latest_timestamp(
    pool: &PgPool,
    instrument_id: i16,
) -> Result<Option<DateTime<Utc>>, DbError> {
    let row: Option<(DateTime<Utc>,)> =
        sqlx::query_as("SELECT MAX(timestamp) FROM candles WHERE instrument_id = $1")
            .bind(instrument_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|(ts,)| ts))
}

/// Count total candles for an instrument.
///
/// # Errors
///
/// Returns [`DbError::Database`] on SQL failure.
pub async fn count_candles(pool: &PgPool, instrument_id: i16) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM candles WHERE instrument_id = $1")
        .bind(instrument_id)
        .fetch_one(pool)
        .await?;

    Ok(row.0)
}

/// Insert a single batch using a dynamically built INSERT statement.
async fn upsert_batch(pool: &PgPool, batch: &[CandleRow]) -> Result<usize, DbError> {
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
            .bind(candle.instrument_id)
            .bind(candle.timestamp)
            .bind(candle.open)
            .bind(candle.high)
            .bind(candle.low)
            .bind(candle.close)
            .bind(candle.volume);
    }

    let result = query.execute(pool).await?;
    Ok(result.rows_affected() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_size_within_postgres_limits() {
        const { assert!(BATCH_SIZE * 7 < 65535) };
    }

    #[test]
    fn test_candle_row_construction() {
        let row = CandleRow {
            instrument_id: 1,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("valid ts"),
            open: Decimal::new(1600050, 2),
            high: Decimal::new(1605000, 2),
            low: Decimal::new(1598025, 2),
            close: Decimal::new(1603075, 2),
            volume: 12345,
        };
        assert_eq!(row.instrument_id, 1);
        assert_eq!(row.volume, 12345);
    }

    #[test]
    fn test_candle_row_serde_roundtrip() {
        let row = CandleRow {
            instrument_id: 1,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("valid ts"),
            open: Decimal::new(1600050, 2),
            high: Decimal::new(1605000, 2),
            low: Decimal::new(1598025, 2),
            close: Decimal::new(1603075, 2),
            volume: 100,
        };
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: CandleRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.instrument_id, row.instrument_id);
        assert_eq!(parsed.volume, row.volume);
    }

    #[test]
    fn test_candle_row_with_zero_volume() {
        let row = CandleRow {
            instrument_id: 1,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("valid ts"),
            open: Decimal::new(1600000, 2),
            high: Decimal::new(1600000, 2),
            low: Decimal::new(1600000, 2),
            close: Decimal::new(1600000, 2),
            volume: 0,
        };
        assert_eq!(row.volume, 0);
        let json = serde_json::to_string(&row).expect("serialize");
        let parsed: CandleRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.volume, 0);
    }

    #[test]
    fn test_candle_row_with_max_decimal() {
        use rust_decimal_macros::dec;
        let row = CandleRow {
            instrument_id: 1,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("valid ts"),
            open: dec!(9999999999.99),
            high: dec!(9999999999.99),
            low: dec!(0.01),
            close: dec!(5000000000.00),
            volume: i64::MAX,
        };
        assert_eq!(row.open, dec!(9999999999.99));
        assert_eq!(row.low, dec!(0.01));
        assert_eq!(row.volume, i64::MAX);
    }

    #[test]
    fn test_candle_row_clone() {
        let row = CandleRow {
            instrument_id: 2,
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("valid ts"),
            open: Decimal::new(1600050, 2),
            high: Decimal::new(1605000, 2),
            low: Decimal::new(1598025, 2),
            close: Decimal::new(1603075, 2),
            volume: 500,
        };
        let cloned = row.clone();
        assert_eq!(cloned.instrument_id, row.instrument_id);
        assert_eq!(cloned.timestamp, row.timestamp);
        assert_eq!(cloned.volume, row.volume);
    }

    #[test]
    fn test_batch_size_within_postgres_limits_13_params() {
        // Trades use 13 params per row; verify that also fits.
        const { assert!(BATCH_SIZE * 13 < 65535) };
    }
}
